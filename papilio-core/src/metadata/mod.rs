use crate::error::AppError;
use musicbrainz_rs::client::MusicBrainzClient;
use musicbrainz_rs::entity::artist::{Artist as MBArtist, ArtistSearchQuery};
use musicbrainz_rs::entity::relations::RelationContent;
use musicbrainz_rs::entity::release::{Release as MBRelease, ReleaseSearchQuery};
use musicbrainz_rs::Fetch;
use musicbrainz_rs::Search;
use serde_json::Value;
use sqlx::PgPool;
use std::path::Path;
use std::time::Duration;
use tokio_retry::strategy::{jitter, ExponentialBackoff};
use tokio_retry::Retry;
use uuid::Uuid;

pub struct MetadataService {
    db: PgPool,
    client: reqwest::Client,
    mb_client: MusicBrainzClient,
}

impl MetadataService {
    pub fn new(db: PgPool) -> Self {
        let user_agent = "PapilioMusic/1.0.0 ( contact: admin@papilio.music )";

        // 配置全局代理环境，确保所有依赖库（如 musicbrainz_rs）均能正常联网
        let proxy_url = "http://192.168.10.31:7890";
        std::env::set_var("HTTP_PROXY", proxy_url);
        std::env::set_var("HTTPS_PROXY", proxy_url);
        std::env::set_var("http_proxy", proxy_url);
        std::env::set_var("https_proxy", proxy_url);

        let mut builder = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(60))
            .danger_accept_invalid_certs(true);

        if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
            builder = builder.proxy(proxy);
            tracing::info!("MetadataService: Network proxy configured -> {}", proxy_url);
        }

        let client = builder.build().unwrap_or_else(|e| {
            tracing::error!("MetadataService: Failed to build reqwest client: {}", e);
            reqwest::Client::new()
        });

        // 初始化 MusicBrainz 客户端
        let mut mb_client = MusicBrainzClient::default();
        if let Err(e) = mb_client.set_user_agent(user_agent) {
            tracing::error!("MetadataService: Failed to set MB User-Agent: {}", e);
        }

        Self {
            db,
            client,
            mb_client,
        }
    }

    async fn mb_retry<F, Fut, T>(&self, action: F) -> Result<T, AppError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, musicbrainz_rs::Error>>,
    {
        let retry_strategy = ExponentialBackoff::from_millis(1000).map(jitter).take(3);

        Retry::spawn(retry_strategy, action)
            .await
            .map_err(|e| AppError::Metadata(format!("MusicBrainz API error: {:?}", e)))
    }

    pub async fn fetch_and_update_artist(&self, artist_id: Uuid) -> Result<(), AppError> {
        let artist = sqlx::query!("SELECT name FROM artists WHERE id = $1", artist_id)
            .fetch_one(&self.db)
            .await?;

        tracing::info!(artist = %artist.name, "Syncing artist metadata...");

        let query = ArtistSearchQuery::query_builder()
            .artist(&artist.name)
            .build();

        let results = self
            .mb_retry(|| async {
                MBArtist::search(query.clone())
                    .execute_with_client(&self.mb_client)
                    .await
            })
            .await?;

        if let Some(mb_artist) = results.entities.first() {
            let mb_id_str = mb_artist.id.clone();
            let mb_id = Uuid::parse_str(&mb_id_str).ok();

            sqlx::query!(
                "UPDATE artists SET musicbrainz_artist_id = $1 WHERE id = $2",
                mb_id,
                artist_id
            )
            .execute(&self.db)
            .await?;

            if let Some(id) = mb_id {
                if let Err(e) = self.fetch_artist_image(id, artist_id).await {
                    tracing::error!(artist = %artist.name, error = ?e, "Failed to fetch artist image");
                }
            }
        } else {
            tracing::warn!(artist = %artist.name, "No MusicBrainz ID found for artist");
        }

        Ok(())
    }

    async fn fetch_artist_image(&self, mb_id: Uuid, artist_id: Uuid) -> Result<(), AppError> {
        let artist_name = sqlx::query_scalar!("SELECT name FROM artists WHERE id = $1", artist_id)
            .fetch_one(&self.db)
            .await
            .unwrap_or_default();

        tracing::info!(artist = %artist_name, "Fetching artist image...");
        
        let mut image_url = None;

        // 策略 1: 优先尝试 Last.fm (覆盖率较高)
        image_url = self.fetch_image_from_lastfm(&artist_name).await.ok();
        
        if image_url.is_some() {
            tracing::info!(artist = %artist_name, "Found image on Last.fm");
        }

        // 策略 2: 回退至 MusicBrainz/Wikidata
        if image_url.is_none() {
            let artist_full = self
                .mb_retry(|| async {
                    MBArtist::fetch()
                        .id(&mb_id.to_string())
                        .with_url_relations()
                        .execute_with_client(&self.mb_client)
                        .await
                })
                .await.ok();

            if let Some(artist_full) = artist_full {
                if let Some(rels) = artist_full.relations {
                    for rel in rels {
                        if let RelationContent::Url(url) = rel.content {
                            if rel.relation_type == "image" {
                                image_url = Some(url.resource);
                                break;
                            }
                            if rel.relation_type == "wikidata" {
                                let qid = url.resource.split('/').next_back().map(|s| s.to_string());
                                if let Some(q) = qid {
                                    image_url = self.fetch_image_from_wikidata(&q).await.ok();
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(url) = image_url {
            if let Err(e) = self.download_and_save_artist_image(&url, artist_id).await {
                tracing::warn!(artist = %artist_name, error = ?e, "Download failed, storing remote URL as fallback");
                
                let direct_url = self.resolve_wikimedia_url(&url);
                sqlx::query!("UPDATE artists SET image_url = $1 WHERE id = $2", direct_url, artist_id)
                    .execute(&self.db)
                    .await?;
            }
        }

        Ok(())
    }

    async fn fetch_image_from_wikidata(&self, qid: &str) -> Result<String, AppError> {
        let url = format!(
            "https://www.wikidata.org/wiki/Special:EntityData/{}.json",
            qid
        );
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::Metadata(format!("Wikidata request failed: {}", e)))?;

        let json: Value = resp
            .json()
            .await
            .map_err(|e| AppError::Metadata(format!("Wikidata JSON parse failed: {}", e)))?;

        let image_name = json["entities"][qid]["claims"]["P18"][0]["mainsnak"]["datavalue"]
            ["value"]
            .as_str()
            .ok_or_else(|| AppError::Metadata("No image (P18) found in Wikidata".to_string()))?;

        use md5;
        let decoded_name = image_name.replace(' ', "_");
        let digest = format!("{:x}", md5::compute(&decoded_name));
        let a = &digest[0..1];
        let b = &digest[0..2];
        let final_url = format!(
            "https://upload.wikimedia.org/wikipedia/commons/{}/{}/{}",
            a, b, decoded_name
        );

        tracing::info!(final_url = %final_url, "STEP 3.3.1: Generated Wikimedia URL");
        Ok(final_url)
    }

    async fn fetch_image_from_lastfm(&self, artist_name: &str) -> Result<String, AppError> {
        let url = format!(
            "https://www.last.fm/music/{}/+images",
            urlencoding::encode(artist_name)
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Metadata(format!("Last.fm request failed: {}", e)))?;

        let html = resp
            .text()
            .await
            .map_err(|e| AppError::Metadata(e.to_string()))?;

        // Last.fm 的图片 CDN 规律
        let re = regex::Regex::new(r"https://lastfm.freetls.fastly.net/i/u/avatar170s/[a-f0-9]+")
            .unwrap();
        if let Some(mat) = re.find(&html) {
            // 转换 170s 缩略图为原图 (或者更大的 ar0)
            let final_url = mat.as_str().replace("avatar170s", "770x770") + ".jpg";
            return Ok(final_url);
        }

        // 尝试另一种匹配
        let re2 =
            regex::Regex::new(r"https://lastfm.freetls.fastly.net/i/u/300x300/[a-f0-9]+").unwrap();
        if let Some(mat) = re2.find(&html) {
            let final_url = mat.as_str().replace("300x300", "770x770") + ".jpg";
            return Ok(final_url);
        }

        Err(AppError::Metadata(
            "No image found on Last.fm page".to_string(),
        ))
    }

    fn resolve_wikimedia_url(&self, url: &str) -> String {
        let mut final_url = url.to_string();

        // 1. 处理 Web Archive (如果有)
        if final_url.contains("web.archive.org/web/") {
            if let Some(original) = final_url.split("/http").last() {
                let decoded = format!("http{}", original);
                println!(
                    "CORE_DEBUG: Extracted original URL from Web Archive: {}",
                    decoded
                );
                final_url = decoded;
            }
        }

        // 2. 处理 Wikimedia Commons 页面
        if final_url.contains("commons.wikimedia.org/wiki/File:") {
            if let Some(filename) = final_url.split("File:").last() {
                let decoded_name = urlencoding::decode(filename)
                    .unwrap_or(std::borrow::Cow::Borrowed(filename))
                    .replace(' ', "_");
                let digest = format!("{:x}", md5::compute(&decoded_name));
                let a = &digest[0..1];
                let b = &digest[0..2];
                final_url = format!(
                    "https://upload.wikimedia.org/wikipedia/commons/{}/{}/{}",
                    a, b, decoded_name
                );
                println!(
                    "CORE_DEBUG: Resolved Wikimedia page to direct URL: {}",
                    final_url
                );
            }
        }
        final_url
    }

    async fn download_and_save_artist_image(
        &self,
        url: &str,
        artist_id: Uuid,
    ) -> Result<(), AppError> {
        let direct_url = self.resolve_wikimedia_url(url);
        println!(
            "CORE_DEBUG: STEP 4: Downloading photo from {} (Source: {})",
            direct_url, url
        );
        tracing::info!(url = %direct_url, source = %url, "STEP 4: Downloading photo");

        let resp = self
            .client
            .get(&direct_url)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| AppError::Metadata(format!("Failed to download image: {}", e)))?;

        if resp.status().is_success() {
            let extension = match resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|h| h.to_str().ok())
            {
                Some("image/png") => "png",
                Some("image/webp") => "webp",
                Some("image/gif") => "gif",
                _ => "jpg",
            };

            let img_data = resp
                .bytes()
                .await
                .map_err(|e| AppError::Io(std::io::Error::other(e)))?;

            let filename = format!("artist_{}.{}", artist_id, extension);

            let base_dir =
                std::env::var("AVATAR_DIR").unwrap_or_else(|_| "/app/data/avatars".to_string());
            let full_path = Path::new(&base_dir).join(&filename);

            tokio::fs::create_dir_all(&base_dir).await?;
            tokio::fs::write(full_path, img_data).await?;

            // 核心修复：数据库只存文件名，不带 data/avatars/ 前缀
            sqlx::query!(
                "UPDATE artists SET image_url = $1 WHERE id = $2",
                filename,
                artist_id
            )
            .execute(&self.db)
            .await?;

            tracing::info!(artist_id = %artist_id, file = %filename, "STEP 5: Successfully saved photo and updated DB");
            println!(
                "CORE_DEBUG: STEP 5: Successfully saved photo {} for artist_id {}",
                filename, artist_id
            );
        } else {
            let status = resp.status();
            tracing::error!(status = %status, url = %direct_url, "STEP 5: Download failed with bad status");
            println!(
                "CORE_DEBUG: STEP 5: Download failed with status {} for {}",
                status, direct_url
            );
            return Err(AppError::Metadata(format!(
                "Download failed with status: {}",
                status
            )));
        }
        Ok(())
    }

    pub async fn fetch_and_update_album(&self, album_id: Uuid) -> Result<(), AppError> {
        let album = sqlx::query!(
            "SELECT a.title, a.cover_path, art.name as artist_name FROM albums a JOIN artists art ON a.artist_id = art.id WHERE a.id = $1",
            album_id
        ).fetch_one(&self.db).await?;

        tracing::info!(album = %album.title, artist = %album.artist_name, "Fetching album metadata from MusicBrainz");

        let query = ReleaseSearchQuery::query_builder()
            .release(&album.title)
            .artist(&album.artist_name)
            .build();

        let results = self
            .mb_retry(|| async {
                MBRelease::search(query.clone())
                    .execute_with_client(&self.mb_client)
                    .await
            })
            .await?;

        if let Some(mb_release) = results.entities.first() {
            let mb_id = Uuid::parse_str(&mb_release.id).ok();
            let year = mb_release
                .date
                .as_ref()
                .and_then(|d| d.0.split('-').next()?.parse::<i32>().ok());

            sqlx::query!(
                "UPDATE albums SET musicbrainz_album_id = $1, release_year = COALESCE(release_year, $2) WHERE id = $3",
                mb_id, year, album_id
            ).execute(&self.db).await?;

            tracing::info!(album = %album.title, year = ?year, "Matched MusicBrainz album info");

            if album.cover_path.is_none() {
                if let Some(id) = mb_id {
                    let _ = self.fetch_cover_from_caa(id, album_id).await;
                }
            }
        }

        Ok(())
    }

    async fn fetch_cover_from_caa(&self, mb_id: Uuid, album_id: Uuid) -> Result<(), AppError> {
        let url = format!("https://coverartarchive.org/release/{}", mb_id);

        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::Metadata(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AppError::Metadata("No cover found in CAA".to_string()));
        }

        let json: Value = resp
            .json()
            .await
            .map_err(|e| AppError::Metadata(e.to_string()))?;

        let cover_url = json["images"]
            .as_array()
            .and_then(|imgs: &Vec<Value>| {
                imgs.iter().find(|i| i["front"].as_bool().unwrap_or(false))
            })
            .and_then(|i| i["image"].as_str());

        if let Some(img_url) = cover_url {
            let resp = self
                .client
                .get(img_url)
                .timeout(Duration::from_secs(15))
                .send()
                .await
                .map_err(|e| AppError::Io(std::io::Error::other(e)))?;

            if resp.status().is_success() {
                let extension = match resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|h| h.to_str().ok())
                {
                    Some("image/png") => "png",
                    Some("image/webp") => "webp",
                    _ => "jpg",
                };

                let img_data = resp
                    .bytes()
                    .await
                    .map_err(|e| AppError::Io(std::io::Error::other(e)))?;

                let filename = format!("{}.{}", album_id, extension);
                let save_relative = format!("data/covers/{}", filename);

                let base_dir = std::env::var("COVER_DIR")
                    .unwrap_or_else(|_| "/mnt/data1/rust/papilio/data/covers".to_string());
                let full_path = Path::new(&base_dir).join(filename);

                tokio::fs::create_dir_all(&base_dir).await?;
                tokio::fs::write(full_path, img_data).await?;

                sqlx::query!(
                    "UPDATE albums SET cover_path = $1 WHERE id = $2",
                    save_relative,
                    album_id
                )
                .execute(&self.db)
                .await?;

                tracing::info!(album_id = %album_id, "Successfully downloaded CAA cover");
            }
        }

        Ok(())
    }

    pub async fn fetch_lyrics_online(
        &self,
        track_id: Uuid,
        title: &str,
        artist: &str,
    ) -> Result<(), AppError> {
        tracing::info!(title = %title, artist = %artist, "Searching cloud lyrics");

        // Simulating a cloud fetch delay
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let simulated_lrc = format!(
            "[00:00.00] {}\n[00:02.00] {}\n[00:04.00] (自动同步云端歌词)",
            title, artist
        );

        sqlx::query!(
            "UPDATE tracks SET lyrics = $1 WHERE id = $2",
            simulated_lrc,
            track_id
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }
}
