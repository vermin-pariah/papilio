use crate::error::AppError;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use lofty::{prelude::*, probe::Probe, tag::Accessor};
use sqlx::{PgPool, Row};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Semaphore;
use uuid::Uuid;
use walkdir::WalkDir;

use dashmap::DashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use tokio::fs as tfs;
use once_cell::sync::Lazy;
use tokio::sync::Mutex;
use sanitize_filename;

pub mod organizer;

static SCAN_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

pub struct Scanner {
    db: PgPool,
    concurrency_limit: Arc<Semaphore>,
    progress_counter: Arc<AtomicI32>,
    artist_cache: Arc<DashMap<String, Uuid>>,
    album_cache: Arc<DashMap<(String, Uuid), Uuid>>,
}

impl Scanner {
    pub fn new(db: PgPool) -> Self {
        let limit = std::env::var("SCAN_CONCURRENCY")
            .unwrap_or_else(|_| "8".to_string())
            .parse()
            .unwrap_or(8);
        Self {
            db,
            concurrency_limit: Arc::new(Semaphore::new(limit)),
            progress_counter: Arc::new(AtomicI32::new(0)),
            artist_cache: Arc::new(DashMap::new()),
            album_cache: Arc::new(DashMap::new()),
        }
    }

    pub fn is_scanning(&self) -> bool {
        SCAN_LOCK.try_lock().is_err()
    }

    pub async fn scan_directory(&self, path: &str) -> Result<(), AppError> {
        let _lock = SCAN_LOCK.try_lock().map_err(|_| {
            AppError::BadRequest("A scan is already in progress".to_string())
        })?;

        let scan_path = Path::new(path);
        if !scan_path.exists() || !scan_path.is_dir() {
            tracing::error!("Invalid scan path: {}", path);
            return Err(AppError::BadRequest(format!(
                "Scan path does not exist or is not a directory: {}",
                path
            )));
        }

        tracing::info!("Starting scan of directory: {}", path);
        self.progress_counter.store(0, Ordering::SeqCst);

        let entries: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file() && self.is_audio_file(e.path()))
            .collect();

        let total = entries.len() as i32;
        tracing::info!("Found {} audio files to process", total);

        sqlx::query("UPDATE scan_status SET is_scanning = TRUE, current_count = 0, total_count = $1 WHERE id = 1")
            .bind(total)
            .execute(&self.db).await?;

        let mut futures = FuturesUnordered::new();
        let mut failure_count = 0;
        const MAX_FAILURES: i32 = 10;

        for entry in entries {
            if failure_count >= MAX_FAILURES {
                tracing::error!(
                    "Failure threshold reached ({}). Aborting scan.",
                    MAX_FAILURES
                );
                break;
            }

            let permit = self
                .concurrency_limit
                .clone()
                .acquire_owned()
                .await
                .map_err(|e| AppError::Internal(format!("Semaphore error: {}", e)))?;

            let scanner_ref = self.clone_for_spawn();
            let file_path = entry.path().to_path_buf();
            let counter = self.progress_counter.clone();

            futures.push(tokio::spawn(async move {
                let _permit = permit;
                let path_display = file_path.display().to_string();
                tracing::debug!("Dispatching scan task for: {}", path_display);

                let res = scanner_ref.process_file(&file_path).await;

                match &res {
                    Ok(_) => {
                        counter.fetch_add(1, Ordering::SeqCst);
                        tracing::debug!("Scan task completed successfully: {}", path_display);
                    }
                    Err(e) => {
                        tracing::error!("Scan task failed for {}: {}", path_display, e);
                    }
                }
                res
            }));

            if futures.len() >= 10 {
                if let Some(res) = futures.next().await {
                    if self.handle_task_result(res).is_err() {
                        failure_count += 1;
                    }
                }
                self.update_scan_progress_inc().await?;
            }
        }

        while let Some(res) = futures.next().await {
            if self.handle_task_result(res).is_err() {
                // failure_count += 1; // Don't care about trailing failures for threshold, but could log
            }
        }

        // 最终强制校准一次
        self.update_scan_progress_final().await?;
        self.cleanup_orphan_tracks().await?;

        sqlx::query(
            "UPDATE scan_status SET is_scanning = FALSE, last_scan_at = NOW() WHERE id = 1",
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    fn handle_task_result(
        &self,
        res: Result<Result<(), AppError>, tokio::task::JoinError>,
    ) -> Result<(), AppError> {
        match res {
            Ok(Err(e)) => {
                tracing::error!("Scan task logic failed: {}", e);
                Err(e)
            }
            Err(e) => {
                tracing::error!("Scan task panicked: {}", e);
                Err(AppError::Internal(format!("Task panicked: {}", e)))
            }
            Ok(Ok(_)) => Ok(()),
        }
    }

    fn clone_for_spawn(&self) -> Arc<Self> {
        Arc::new(Self {
            db: self.db.clone(),
            concurrency_limit: self.concurrency_limit.clone(),
            progress_counter: self.progress_counter.clone(),
            artist_cache: self.artist_cache.clone(),
            album_cache: self.album_cache.clone(),
        })
    }

    async fn update_scan_progress_inc(&self) -> Result<(), AppError> {
        let current = self.progress_counter.load(Ordering::SeqCst);
        // 每完成 5 个任务才更新一次数据库状态，减少 IO 压力
        if current % 5 != 0 {
            return Ok(());
        }
        sqlx::query("UPDATE scan_status SET current_count = $1 WHERE id = 1")
            .bind(current)
            .execute(&self.db)
            .await
            .map(|_| ())
            .map_err(AppError::Database)
    }

    async fn update_scan_progress_final(&self) -> Result<(), AppError> {
        sqlx::query(
            "UPDATE scan_status SET current_count = (SELECT COUNT(*) FROM tracks) WHERE id = 1",
        )
        .execute(&self.db)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
    }

    async fn cleanup_orphan_tracks(&self) -> Result<(), AppError> {
        tracing::info!("Cleaning up orphan tracks...");
        let rows = sqlx::query("SELECT id, path FROM tracks")
            .fetch_all(&self.db)
            .await?;

        for row in rows {
            let id: Uuid = row.get("id");
            let path: String = row.get("path");
            if !Path::new(&path).exists() {
                tracing::warn!("Removing orphan track from DB: {}", path);
                sqlx::query("DELETE FROM tracks WHERE id = $1")
                    .bind(id)
                    .execute(&self.db)
                    .await?;
            }
        }
        Ok(())
    }

    fn is_audio_file(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|s| s.to_str())
            .map(|s| {
                matches!(
                    s.to_lowercase().as_str(),
                    "flac" | "mp3" | "m4a" | "ogg" | "wav"
                )
            })
            .unwrap_or(false)
    }

    #[tracing::instrument(skip(self, path), fields(path = %path.display()))]
    async fn process_file(&self, path: &Path) -> Result<(), AppError> {
        let path_str = path
            .to_str()
            .ok_or_else(|| AppError::Internal("Invalid path encoding".to_string()))?;
        tracing::debug!("Processing metadata for file");

        let tagged_file = Probe::open(path)
            .map_err(|e| AppError::Metadata(format!("Failed to open {}: {}", path_str, e)))?
            .read()
            .map_err(|e| {
                AppError::Metadata(format!("Failed to read tags from {}: {}", path_str, e))
            })?;

        let properties = tagged_file.properties();
        let duration = properties.duration().as_secs() as i32;
        let bitrate = properties.audio_bitrate();

        let mut title_opt = None;
        let mut artist_opt = None;
        let mut album_opt = None;
        let mut track_num = None;
        let mut year = None;

        // 优先级 1: 遍历所有可用的 Tag (ID3v2, Vorbis, etc.) 以获取基础元数据
        for tag in tagged_file.tags() {
            if title_opt.is_none() {
                title_opt = tag.title().map(|s| s.to_string());
            }
            if artist_opt.is_none() {
                artist_opt = tag.artist().map(|s| s.to_string());
            }
            if album_opt.is_none() {
                album_opt = tag.album().map(|s| s.to_string());
            }
            if track_num.is_none() {
                track_num = tag.track();
            }
            if year.is_none() {
                year = tag.year();
            }
        }

        let final_title = title_opt.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });
        let final_artist = artist_opt.unwrap_or_else(|| "Unknown Artist".to_string());
        let final_album = album_opt.unwrap_or_else(|| "Unknown Album".to_string());

        tracing::debug!(title = %final_title, artist = %final_artist, album = %final_album, "Extracted basic metadata");

        // 策略 A: 外部 .lrc 文件
        if let Some(lrc_path) = self.find_lrc_file(path).await {
            if let Ok(bytes) = tokio::fs::read(&lrc_path).await {
                // 尝试多种编码探测：UTF-8 -> GBK -> Big5
                let (content, encoding_used, has_errors) = encoding_rs::UTF_8.decode(&bytes);
                
                let final_content = if has_errors {
                    let (gbk_content, _, gbk_errors) = encoding_rs::GBK.decode(&bytes);
                    if gbk_errors {
                        let (big5_content, _, _) = encoding_rs::BIG5.decode(&bytes);
                        big5_content.to_string()
                    } else {
                        gbk_content.to_string()
                    }
                } else {
                    content.to_string()
                };

                lyrics = Some(final_content.replace('\0', ""));
                lyrics_source = "file";
                tracing::info!(
                    "Loaded LRC file: {} (Encoding: {})",
                    lrc_path.display(),
                    encoding_used.name()
                );
            }
        }

        // 策略 B: 提取内嵌歌词 (USLT/LYRICS 标签)
        if lyrics.is_none() {
            for tag in tagged_file.tags() {
                if let Some(content) = tag.get_string(&lofty::tag::ItemKey::Lyrics) {
                    lyrics = Some(content.to_string());
                    lyrics_source = "embedded";
                    break;
                }
            }
        }

        let artist_id = self.get_or_create_artist(&final_artist).await?;
        let album_id = self
            .get_or_create_album(&final_album, artist_id, year)
            .await?;

        // 封面提取：遍历所有标签尝试提取
        let mut cover_extracted = false;
        
        // 检查数据库中是否已存在封面记录
        let existing_cover: Option<String> = sqlx::query_scalar("SELECT cover_path FROM albums WHERE id = $1")
            .bind(album_id)
            .fetch_one(&self.db).await.unwrap_or(None);

        if existing_cover.is_some() {
            cover_extracted = true;
        }

        if !cover_extracted {
            if let Some(tag) = tagged_file.primary_tag() {
                if let Some(pic) = tag.pictures().first() {
                    let _ = self.save_cover(pic, album_id).await;
                    cover_extracted = true;
                }
            }
        }

        if !cover_extracted {
            for tag in tagged_file.tags() {
                if let Some(pic) = tag.pictures().first() {
                    let _ = self.save_cover(pic, album_id).await;
                    cover_extracted = true;
                    break;
                }
            }
        }

        // 策略 C: 外部封面探测 (cover.jpg, folder.jpg 等)
        if !cover_extracted {
            if let Some(ext_cover_path) = self.find_external_cover(path).await {
                let _ = self.save_external_cover(&ext_cover_path, album_id).await;
            }
        }

        // 策略 D: 关联歌手头像 (探测歌手目录下的 folder.jpg)
        let _ = self.link_existing_artist_image(path, artist_id).await;

        tracing::debug!(track = %final_title, "Inserting track into database...");

        let row = sqlx::query!(
            r#"
            INSERT INTO tracks (
                title, album_id, artist_id, duration, path, bitrate, format, size, track_number,
                lyrics, lyrics_source, sync_status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::lyrics_source_type, $12::lyric_sync_status)
            ON CONFLICT (path) DO UPDATE SET
                title = EXCLUDED.title,
                duration = EXCLUDED.duration,
                bitrate = EXCLUDED.bitrate,
                track_number = EXCLUDED.track_number,
                lyrics = EXCLUDED.lyrics,
                lyrics_source = EXCLUDED.lyrics_source,
                sync_status = CASE
                    WHEN tracks.lyrics <> EXCLUDED.lyrics THEN 'pending'::lyric_sync_status
                    ELSE tracks.sync_status
                END,
                updated_at = NOW()
            RETURNING id
            "#,
            final_title, album_id, artist_id, duration, path_str,
            bitrate.map(|b| b as i32), path.extension().and_then(|s| s.to_str()),
            path.metadata().map(|m| m.len() as i64).ok(),
            track_num.map(|n| n as i32),
            lyrics,
            lyrics_source as &str,
            sync_status as &str
        )
        .fetch_one(&self.db).await
        .map_err(|e| {
            tracing::error!("Database error for '{}': {}", final_title, e);
            AppError::Database(e)
        })?;

        let track_id = row.id;
        tracing::debug!(id = %track_id, "Track inserted/updated successfully");

        Ok(())
    }

    async fn save_cover(
        &self,
        pic: &lofty::picture::Picture,
        album_id: Uuid,
    ) -> Result<(), AppError> {
        // 获取专辑和歌手信息以确定路径
        let album_info = sqlx::query!(
            "SELECT a.title, ar.name as artist_name FROM albums a JOIN artists ar ON a.artist_id = ar.id WHERE a.id = $1",
            album_id
        ).fetch_one(&self.db).await.map_err(AppError::Database)?;

        let extension = match pic.mime_type() {
            Some(lofty::picture::MimeType::Jpeg) => "jpg",
            Some(lofty::picture::MimeType::Png) => "png",
            _ => "jpg",
        };

        // 直接存入曲库目录
        let music_root = std::env::var("MUSIC_DIR").unwrap_or_else(|_| "data/music".to_string());
        let safe_art = album_info
            .artist_name
            .chars()
            .map(|c| if "/\\<>:\"|?*".contains(c) { '_' } else { c })
            .collect::<String>();
        let safe_alb = album_info
            .title
            .chars()
            .map(|c| if "/\\<>:\"|?*".contains(c) { '_' } else { c })
            .collect::<String>();

        let target_dir = Path::new(&music_root).join(safe_art).join(safe_alb);
        let full_save_path = target_dir.join(format!("cover.{}", extension));

        // 数据库存储相对路径
        let rel_path = full_save_path
            .strip_prefix(&music_root)
            .unwrap_or(&full_save_path)
            .to_str()
            .unwrap()
            .to_string();

        // 如果文件已存在且不为空，更新数据库并跳过写入以节省 IO
        if full_save_path.exists()
            && fs::metadata(&full_save_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0)
                > 0
        {
            sqlx::query("UPDATE albums SET cover_path = $1 WHERE id = $2")
                .bind(rel_path)
                .bind(album_id)
                .execute(&self.db)
                .await?;
            return Ok(());
        }

        tracing::info!(album = %album_info.title, "Saving cover directly to library: {}", full_save_path.display());
        tokio::fs::write(&full_save_path, pic.data()).await?;

        sqlx::query("UPDATE albums SET cover_path = $1 WHERE id = $2")
            .bind(rel_path)
            .bind(album_id)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    async fn get_or_create_artist(&self, name: &str) -> Result<Uuid, AppError> {
        if let Some(id) = self.artist_cache.get(name) {
            return Ok(*id);
        }

        let res = sqlx::query!(
            "INSERT INTO artists (name) VALUES ($1) ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
            name
        )
        .fetch_one(&self.db).await
        .map_err(|e| {
            tracing::error!("Database error for artist '{}': {}", name, e);
            AppError::Database(e)
        })?;

        self.artist_cache.insert(name.to_string(), res.id);
        Ok(res.id)
    }

    async fn get_or_create_album(
        &self,
        title: &str,
        artist_id: Uuid,
        year: Option<u32>,
    ) -> Result<Uuid, AppError> {
        let cache_key = (title.to_string(), artist_id);
        if let Some(id) = self.album_cache.get(&cache_key) {
            return Ok(*id);
        }

        let res = sqlx::query!(
            "INSERT INTO albums (title, artist_id, release_year)
             VALUES ($1, $2, $3)
             ON CONFLICT (title, artist_id) DO UPDATE SET release_year = COALESCE(albums.release_year, EXCLUDED.release_year)
             RETURNING id",
            title, artist_id, year.map(|y| y as i32)
        )
        .fetch_one(&self.db).await
        .map_err(|e| {
            tracing::error!("Database error for album '{}': {}", title, e);
            AppError::Database(e)
        })?;

        self.album_cache.insert(cache_key, res.id);
        Ok(res.id)
    }

    async fn link_existing_artist_image(&self, audio_path: &Path, artist_id: Uuid) -> Result<(), AppError> {
        // 如果该歌手已经有图片，跳过
        let current_image: Option<String> = sqlx::query_scalar("SELECT image_url FROM artists WHERE id = $1")
            .bind(artist_id)
            .fetch_one(&self.db).await.unwrap_or(None);
        
        if current_image.is_some() && !current_image.unwrap_or_default().is_empty() {
            return Ok(());
        }

        let music_root = std::env::var("MUSIC_DIR").unwrap_or_else(|_| "/music".to_string());
        let mut current_dir = audio_path.parent();

        // 最多向上找两级 (专辑目录 -> 歌手目录)
        for _ in 0..2 {
            if let Some(dir) = current_dir {
                // 常见的歌手头像文件名
                let patterns = ["folder.jpg", "folder.png", "artist.jpg", "artist.png", "logo.jpg", "logo.png"];
                for pattern in patterns {
                    let candidate = dir.join(pattern);
                    if candidate.exists() && candidate.is_file() {
                        let rel_path = candidate
                            .strip_prefix(&music_root)
                            .unwrap_or(&candidate)
                            .to_str()
                            .unwrap_or_default()
                            .to_string();
                        
                        let clean_path = if rel_path.starts_with('/') { &rel_path[1..] } else { &rel_path };

                        if !clean_path.is_empty() {
                            tracing::info!("Found existing artist image at: {}", clean_path);
                            sqlx::query!("UPDATE artists SET image_url = $1 WHERE id = $2", clean_path, artist_id)
                                .execute(&self.db)
                                .await?;
                            return Ok(());
                        }
                    }
                }
                current_dir = dir.parent();
            } else {
                break;
            }
        }

        Ok(())
    }

    async fn find_external_cover(&self, audio_path: &Path) -> Option<PathBuf> {
        let parent = audio_path.parent()?;
        tracing::info!("DEBUG_SCAN: Searching for external cover in {}", parent.display());
        let mut entries = tfs::read_dir(parent).await.ok()?;

        let mut candidates = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().and_then(|f| f.to_str()).unwrap_or_default().to_lowercase();
                tracing::info!("DEBUG_SCAN: Checking file: {}", name);
                if name.ends_with(".jpg") || name.ends_with(".jpeg") || name.ends_with(".png") || name.ends_with(".webp") {
                    tracing::info!("DEBUG_SCAN: Potential candidate found: {}", name);
                    candidates.push(path);
                }
            }
        }

        // 优先级：精确匹配
        let priority = ["cover.", "folder.", "front.", "album.", "art."];
        for p in priority {
            if let Some(found) = candidates.iter().find(|c| {
                let fname = c.file_name()
                    .and_then(|f| f.to_str())
                    .map(|s| s.to_lowercase())
                    .unwrap_or_default();
                fname.starts_with(p)
            }) {
                tracing::info!("DEBUG_SCAN: Priority match success: {}", p);
                return Some(found.clone());
            }
        }

        // 模糊匹配：跟歌曲同名
        if let Some(stem) = audio_path.file_stem().and_then(|s| s.to_str()) {
            let stem_lower = stem.to_lowercase();
            if let Some(found) = candidates.iter().find(|c| {
                c.file_stem()
                    .and_then(|f| f.to_str())
                    .map(|s| s.to_lowercase() == stem_lower)
                    .unwrap_or(false)
            }) {
                return Some(found.clone());
            }
        }

        // 如果该目录下只有一个图片，默认为封面
        if candidates.len() == 1 {
            return Some(candidates[0].clone());
        }

        None
    }

    async fn save_external_cover(&self, cover_path: &Path, album_id: Uuid) -> Result<(), AppError> {
        let album_info = sqlx::query!(
            "SELECT a.title, ar.name as artist_name FROM albums a JOIN artists ar ON a.artist_id = ar.id WHERE a.id = $1",
            album_id
        ).fetch_one(&self.db).await.map_err(AppError::Database)?;

        let music_root = std::env::var("MUSIC_DIR").unwrap_or_else(|_| "/music".to_string());
        let safe_art = sanitize_filename::sanitize(&album_info.artist_name);
        let safe_alb = sanitize_filename::sanitize(&album_info.title);

        let target_dir = Path::new(&music_root).join(safe_art).join(safe_alb);
        let extension = cover_path.extension().and_then(|s| s.to_str()).unwrap_or("jpg");
        let full_save_path = target_dir.join(format!("cover.{}", extension));

        let mut final_path = cover_path.to_path_buf();

        // 尝试规范化拷贝
        if cover_path != full_save_path && !full_save_path.exists() {
            if !target_dir.exists() {
                let _ = tokio::fs::create_dir_all(&target_dir).await;
            }
            if let Err(e) = tokio::fs::copy(cover_path, &full_save_path).await {
                tracing::warn!("Failed to copy cover to structured path (possibly Read-Only FS): {}. Using original path.", e);
            } else {
                final_path = full_save_path;
            }
        } else if full_save_path.exists() {
            final_path = full_save_path;
        }

        let mut rel_path = final_path
            .strip_prefix(&music_root)
            .unwrap_or(&final_path)
            .to_str()
            .ok_or_else(|| AppError::Internal("Invalid path encoding".into()))?
            .to_string();

        if rel_path.starts_with('/') {
            rel_path = rel_path[1..].to_string();
        }

        tracing::info!("Linking album {} to cover: {}", album_id, rel_path);

        sqlx::query("UPDATE albums SET cover_path = $1 WHERE id = $2")
            .bind(rel_path)
            .bind(album_id)
            .execute(&self.db)
            .await?;

        Ok(())
    }

    async fn find_lrc_file(&self, audio_path: &Path) -> Option<PathBuf> {
        // 1. 同目录下同名文件 (最快路径)
        let same_dir_lrc = audio_path.with_extension("lrc");
        if same_dir_lrc.exists() {
            return Some(same_dir_lrc);
        }

        // 2. 同目录下模糊匹配 (如果只有一个 lrc 文件)
        if let Some(parent) = audio_path.parent() {
            if let Ok(mut entries) = tfs::read_dir(parent).await {
                let mut lrcs = Vec::new();
                while let Ok(Some(entry)) = entries.next_entry().await {
                    let path = entry.path();
                    if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("lrc") {
                        lrcs.push(path);
                    }
                }
                if lrcs.len() == 1 {
                    return Some(lrcs[0].clone());
                }
            }
        }

        None
    }

    pub async fn process_track_by_id(&self, track_id: Uuid) -> Result<(), AppError> {
        let row = sqlx::query("SELECT path FROM tracks WHERE id = $1")
            .bind(track_id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("Track {} not found", track_id)))?;

        let path_str: String = row.get("path");
        self.process_file(Path::new(&path_str)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    #[tokio::test]
    async fn test_is_audio_file() {
        let db = PgPool::connect_lazy("postgres://localhost/dummy").unwrap();
        let scanner = Scanner::new(db);
        
        assert!(scanner.is_audio_file(Path::new("test.mp3")));
        assert!(scanner.is_audio_file(Path::new("test.FLAC")));
        assert!(scanner.is_audio_file(Path::new("test.m4a")));
        assert!(!scanner.is_audio_file(Path::new("test.txt")));
        assert!(!scanner.is_audio_file(Path::new("test.exe")));
    }

    #[tokio::test]
    async fn test_scan_lock() {
        let db = PgPool::connect_lazy("postgres://localhost/dummy").unwrap();
        let scanner = Scanner::new(db);
        
        // 获取锁
        let lock = SCAN_LOCK.lock().await;
        
        // 尝试触发扫描应该失败
        assert!(scanner.is_scanning());
        let result = scanner.scan_directory("/tmp").await;
        assert!(matches!(result, Err(AppError::BadRequest(_))));
        
        drop(lock);
        assert!(!scanner.is_scanning());
    }
}
