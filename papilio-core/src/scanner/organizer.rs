use crate::error::AppError;
use lofty::{prelude::*, probe::Probe};
use sqlx::PgPool;
use std::path::{Path, PathBuf};
use tokio::fs;
use uuid::Uuid;
use walkdir::WalkDir;
use super::SCAN_LOCK;

pub struct Organizer {
    db: PgPool,
    music_root: PathBuf,
}

impl Organizer {
    pub fn new(db: PgPool, music_root: PathBuf) -> Self {
        Self { db, music_root }
    }

    pub async fn organize(&self) -> Result<(), AppError> {
        let _lock = SCAN_LOCK.try_lock().map_err(|_| {
            AppError::BadRequest("A scan or reorganization is already in progress".to_string())
        })?;

        tracing::info!("Starting library reorganization...");

        // 1. 递归扫描曲库
        let entries: Vec<_> = WalkDir::new(&self.music_root)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file() && self.is_audio_file(e.path()))
            .collect();

        let total = entries.len() as i32;
        sqlx::query("UPDATE scan_status SET is_scanning = TRUE, current_count = 0, total_count = $1 WHERE id = 1")
            .bind(total)
            .execute(&self.db).await?;

        let mut current = 0;
        for entry in entries {
            if let Err(e) = self.process_organize_file(entry.path()).await {
                tracing::error!(
                    "Failed to organize file {}: {:?}",
                    entry.path().display(),
                    e
                );
            }
            current += 1;
            // 逐个更新进度，确保安卓端能即时看到进度条
            let _ = sqlx::query(
                "UPDATE scan_status SET current_count = $1, is_scanning = TRUE WHERE id = 1",
            )
            .bind(current)
            .execute(&self.db)
            .await;
        }

        // 2. 整理关联图片资产 (歌手/专辑图片)
        self.organize_assets().await?;

        // 3. 特殊处理：清理根目录下的孤立 LRC 文件 (模糊匹配抓回)
        self.cleanup_root_lrc_files().await?;

        sqlx::query(
            "UPDATE scan_status SET is_scanning = FALSE, last_scan_at = NOW() WHERE id = 1",
        )
        .execute(&self.db)
        .await?;

        tracing::info!("Library reorganization completed.");
        Ok(())
    }

    async fn cleanup_root_lrc_files(&self) -> Result<(), AppError> {
        tracing::info!("!!! ULTIMATE CLEANUP: Aggressive root scan...");
        let mut entries = fs::read_dir(&self.music_root).await?;

        let mut lrc_files = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("lrc") {
                lrc_files.push(path);
            }
        }

        for (i, path) in lrc_files.into_iter().enumerate() {
            let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("unknown.lrc");
            let file_stem = path.file_stem().and_then(|f| f.to_str()).unwrap_or("unknown");

            // 改进匹配：取第一个横杠或下划线前的部分作为关键词
            let keyword = file_stem
                .split(&['-', '_', ' '][..])
                .next()
                .unwrap_or(file_stem)
                .trim();

            if keyword.len() < 2 {
                continue;
            }

            let track = sqlx::query!(
                "SELECT path, title FROM tracks WHERE title ILIKE $1 OR path ILIKE $1 LIMIT 1",
                format!("%{}%", keyword)
            )
            .fetch_optional(&self.db)
            .await?;

            if let Some(t) = track {
                let audio_path = Path::new(&t.path);
                if let Some(dest_dir) = audio_path.parent() {
                    let dest_lrc = dest_dir.join(file_name);
                    tracing::info!(
                        ">>> CATCH SUCCESS: '{}' -> {}",
                        file_name,
                        dest_dir.display()
                    );
                    fs::create_dir_all(dest_dir).await?;
                    let _ = self.robust_move(&path, &dest_lrc).await;
                }
            }

            // 更新进度 (假设这是第二阶段)
            let _ = sqlx::query("UPDATE scan_status SET current_count = $1 WHERE id = 1")
                .bind(i as i32)
                .execute(&self.db)
                .await;
        }
        Ok(())
    }

    async fn process_organize_file(&self, path: &Path) -> Result<(), AppError> {
        let tagged_file = Probe::open(path)
            .map_err(|e| AppError::Metadata(format!("Failed to open {}: {}", path.display(), e)))?
            .read()
            .map_err(|e| {
                AppError::Metadata(format!(
                    "Failed to read tags from {}: {}",
                    path.display(),
                    e
                ))
            })?;

        let mut artist = None;
        let mut album = None;
        let mut title = None;

        for tag in tagged_file.tags() {
            if artist.is_none() {
                artist = tag.artist().map(|s| s.to_string());
            }
            if album.is_none() {
                album = tag.album().map(|s| s.to_string());
            }
            if title.is_none() {
                title = tag.title().map(|s| s.to_string());
            }
        }

        let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("flac");

        let dest_path = if let (Some(art), Some(alb), Some(tit)) = (artist, album, title) {
            let safe_art = sanitize_filename::sanitize(&art);
            let safe_alb = sanitize_filename::sanitize(&alb);
            let safe_tit = sanitize_filename::sanitize(&tit);
            self.music_root
                .join(safe_art)
                .join(safe_alb)
                .join(format!("{}.{}", safe_tit, extension))
        } else {
            let unsorted_dir = self.music_root.join("Unsorted");
            if !unsorted_dir.exists() {
                fs::create_dir_all(&unsorted_dir).await?;
            }
            unsorted_dir.join(path.file_name().ok_or_else(|| AppError::Internal("Invalid filename".into()))?)
        };

        if path == dest_path {
            self.move_associated_files(path, &dest_path).await?;
            return Ok(());
        }

        if dest_path.exists() {
            return Ok(());
        }

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // 使用鲁棒的移动逻辑
        self.robust_move(path, &dest_path).await?;
        self.move_associated_files(path, &dest_path).await?;

        let old_path_str = path.to_str().ok_or_else(|| AppError::Internal("Invalid path encoding".into()))?;
        let new_path_str = dest_path.to_str().ok_or_else(|| AppError::Internal("Invalid path encoding".into()))?;
        sqlx::query!(
            "UPDATE tracks SET path = $1, updated_at = NOW() WHERE path = $2",
            new_path_str,
            old_path_str
        )
        .execute(&self.db)
        .await?;

        Ok(())
    }

    async fn robust_move(&self, src: &Path, dest: &Path) -> Result<(), AppError> {
        if let Err(e) = fs::rename(src, dest).await {
            // 错误码 18 (EXDEV) 表示跨设备链接，即不同磁盘
            tracing::warn!(
                "Rename failed ({:?}), trying copy+delete for {}",
                e,
                src.display()
            );
            fs::copy(src, dest).await?;
            fs::remove_file(src).await?;
        }
        Ok(())
    }

    async fn move_associated_files(
        &self,
        src_audio: &Path,
        dest_audio: &Path,
    ) -> Result<(), AppError> {
        let lrc_src_current = src_audio.with_extension("lrc");
        let lrc_dest = dest_audio.with_extension("lrc");

        if !lrc_dest.exists() && lrc_src_current.exists() {
            let _ = self.robust_move(&lrc_src_current, &lrc_dest).await;
        }
        Ok(())
    }

    async fn organize_assets(&self) -> Result<(), AppError> {
        let music_root = &self.music_root;
        let internal_avatar_dir =
            std::env::var("AVATAR_DIR").unwrap_or_else(|_| "data/avatars".to_string());
        let internal_cover_dir =
            std::env::var("COVER_DIR").unwrap_or_else(|_| "data/covers".to_string());

        tracing::info!("!!! ASSET RECOVERY: Scanning internal folders...");

        // 1. 物理扫描内部头像目录
        if let Ok(mut entries) = fs::read_dir(&internal_avatar_dir).await {
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() {
                    let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or_default();
                    let artist_id_str = file_name
                        .replace("artist_", "")
                        .replace(".jpg", "")
                        .replace(".png", "");

                    if let Ok(artist_id) = Uuid::parse_str(&artist_id_str) {
                        if let Some(a) =
                            sqlx::query!("SELECT name FROM artists WHERE id = $1", artist_id)
                                .fetch_optional(&self.db)
                                .await?
                        {
                            let safe_art = sanitize_filename::sanitize(&a.name);
                            let target_dir = music_root.join(safe_art);
                            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("jpg");
                            let dest_path = target_dir.join(format!("folder.{}", ext));

                            fs::create_dir_all(&target_dir).await?;
                            let rel_path = dest_path
                                .strip_prefix(music_root)
                                .map_err(|_| AppError::Internal("Path strip error".into()))?
                                .to_str()
                                .ok_or_else(|| AppError::Internal("Invalid path encoding".into()))?
                                .to_string();

                            if !dest_path.exists() {
                                tracing::info!(
                                    "Recovering artist avatar: {} -> {}",
                                    file_name,
                                    dest_path.display()
                                );
                                self.robust_move(&path, &dest_path).await?;
                            } else {
                                let _ = fs::remove_file(&path).await;
                            }
                            sqlx::query!(
                                "UPDATE artists SET image_url = $1 WHERE id = $2",
                                rel_path,
                                artist_id
                            )
                            .execute(&self.db)
                            .await?;
                        }
                    }
                }
            }
        }

        // 2. 物理扫描内部封面目录
        if let Ok(mut entries) = fs::read_dir(&internal_cover_dir).await {
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() {
                    let file_stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or_default();
                    if let Ok(album_id) = Uuid::parse_str(file_stem) {
                        if let Some(alb) = sqlx::query!(
                            "SELECT a.title, ar.name as artist_name FROM albums a JOIN artists ar ON a.artist_id = ar.id WHERE a.id = $1",
                            album_id
                        ).fetch_optional(&self.db).await? {
                            let safe_art = sanitize_filename::sanitize(&alb.artist_name);
                            let safe_alb = sanitize_filename::sanitize(&alb.title);
                            let target_dir = music_root.join(safe_art).join(safe_alb);
                            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("jpg");
                            let dest_path = target_dir.join(format!("cover.{}", ext));

                            fs::create_dir_all(&target_dir).await?;
                            let rel_path = dest_path.strip_prefix(music_root).map_err(|_| AppError::Internal("Path strip error".into()))?.to_str().ok_or_else(|| AppError::Internal("Invalid path encoding".into()))?.to_string();

                            if !dest_path.exists() {
                                tracing::info!("Recovering album cover: {} -> {}", file_stem, dest_path.display());
                                self.robust_move(&path, &dest_path).await?;
                            } else {
                                let _ = fs::remove_file(&path).await;
                            }
                            sqlx::query!("UPDATE albums SET cover_path = $1 WHERE id = $2", rel_path, album_id).execute(&self.db).await?;
                        }
                    }
                }
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
}
