use crate::{ApiError, AppState};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use papilio_core::models::music::{Album, Artist, Track, UpdateLyricOffset};
use papilio_core::{error::AppError, scanner::Scanner};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub album_id: Option<Uuid>,
    pub artist_id: Option<Uuid>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Deserialize)]
pub struct StreamQuery {
    pub bitrate: Option<String>,
    pub start_time: Option<f64>,
}

#[derive(Serialize)]
pub struct TrackWithFavorite {
    #[serde(flatten)]
    pub track: Track,
    pub is_favorite: bool,
}

#[derive(Serialize)]
pub struct ScanStatusResponse {
    pub is_scanning: bool,
    pub current_count: i32,
    pub total_count: i32,
}

#[derive(Deserialize)]
pub struct UpdatePlaybackRequest {
    pub track_id: Uuid,
    pub position_ms: i32,
}

#[derive(Serialize)]
pub struct PlaybackStateResponse {
    pub track_id: Uuid,
    pub position_ms: i32,
    pub updated_at: DateTime<Utc>,
}

pub async fn get_scan_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let _user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let row =
        sqlx::query("SELECT is_scanning, current_count, total_count FROM scan_status WHERE id = 1")
            .fetch_optional(&state.db)
            .await?;

    match row {
        Some(r) => Ok(Json(ScanStatusResponse {
            is_scanning: r.get("is_scanning"),
            current_count: r.get("current_count"),
            total_count: r.get("total_count"),
        })),
        None => Ok(Json(ScanStatusResponse {
            is_scanning: false,
            current_count: 0,
            total_count: 0,
        })),
    }
}

pub async fn trigger_scan(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let user_is_admin = sqlx::query("SELECT is_admin FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?
        .get::<bool, _>("is_admin");

    if !user_is_admin {
        return Err(ApiError(AppError::Auth(
            "Requires administrator privileges".to_string(),
        )));
    }

    // --- 核心改进：预检扫描锁 ---
    use papilio_core::scanner::Scanner;
    // 由于 Scanner::scan_directory 内部有 try_lock，
    // 我们可以在这里尝试获取锁来决定是否允许触发。
    // 注意：这里需要一个 Dummy 调用或者暴露锁状态。
    // 为了不破坏封装，我们直接调用一个检查方法。
    
    let scanner = Scanner::new(state.db.clone());
    if scanner.is_scanning() {
        return Err(ApiError(AppError::BadRequest(
            "A scan is already in progress".to_string(),
        )));
    }

    let scan_path = std::env::var("MUSIC_DIR").unwrap_or_else(|_| "data/music".to_string());

    // 尝试在主线程触发一次，如果已经被锁，scan_directory 会返回 BadRequest
    // 但因为它是异步的且在后台运行，我们需要一个非阻塞的检查方式。
    // 修改：我们在 Scanner 中增加 is_locked 方法。

    tokio::spawn(async move {
        if let Err(e) = scanner.scan_directory(&scan_path).await {
            tracing::error!("Scan task failed: {:?}", e);
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({"message": "Scan started in background"})),
    ))
}

pub async fn update_playback_state(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<UpdatePlaybackRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    sqlx::query(
        "INSERT INTO user_playback_state (user_id, track_id, position_ms, updated_at)
         VALUES ($1, $2, $3, NOW())
         ON CONFLICT (user_id) DO UPDATE SET track_id = EXCLUDED.track_id, position_ms = EXCLUDED.position_ms, updated_at = NOW()"
    )
    .bind(user_id)
    .bind(payload.track_id)
    .bind(payload.position_ms)
    .execute(&state.db).await?;

    Ok(StatusCode::OK)
}

pub async fn get_playback_state(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let row = sqlx::query(
        "SELECT track_id, position_ms, updated_at FROM user_playback_state WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => Ok(Json(Some(PlaybackStateResponse {
            track_id: r.get("track_id"),
            position_ms: r.get("position_ms"),
            updated_at: r.get("updated_at"),
        }))),
        None => Ok(Json(None::<PlaybackStateResponse>)),
    }
}

#[derive(Serialize)]
pub struct GlobalSearchResponse {
    pub artists: Vec<Artist>,
    pub albums: Vec<Album>,
    pub tracks: Vec<TrackWithFavorite>,
}

pub async fn global_search(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state).await;
    let q_str = params.q.clone().unwrap_or_default();
    let q = format!("%{}%", q_str);

    // Search Artists
    let artists = sqlx::query_as!(
        Artist,
        "SELECT * FROM artists WHERE name ILIKE $1 ORDER BY name LIMIT 5",
        q
    )
    .fetch_all(&state.db)
    .await?;

    // Search Albums
    let albums = sqlx::query_as!(
        Album,
        "SELECT * FROM albums WHERE title ILIKE $1 ORDER BY release_year DESC LIMIT 5",
        q
    )
    .fetch_all(&state.db)
    .await?;

    // Search Tracks
    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.title, t.album_id, t.artist_id,
               t.duration, t.track_number,
               t.disc_number, t.path, t.bitrate,
               t.format, t.size, t.bpm,
               t.musicbrainz_track_id,
               t.lyrics,
               t.created_at, t.updated_at,
               (f.user_id IS NOT NULL) as "is_favorite!",
               a.name as "artist_name?",
               a.image_url as "artist_image_url?",
               al.title as "album_title?",
               COALESCE(m.lyric_offset_ms, 0) as "lyric_offset_ms!"
        FROM tracks t
        LEFT JOIN user_favorites f ON t.id = f.track_id AND f.user_id = $2
        LEFT JOIN artists a ON t.artist_id = a.id
        LEFT JOIN albums al ON t.album_id = al.id
        LEFT JOIN user_track_metadata m ON t.id = m.track_id AND m.user_id = $2
        WHERE t.title ILIKE $1
        ORDER BY t.title
        LIMIT 20
        "#,
        q,
        user_id
    )
    .fetch_all(&state.db)
    .await?;

    let tracks = rows
        .into_iter()
        .map(|row| TrackWithFavorite {
            track: Track {
                id: row.id,
                title: row.title,
                album_id: row.album_id,
                artist_id: row.artist_id,
                artist_name: row.artist_name,
                album_title: row.album_title,
                artist_image_url: row.artist_image_url,
                duration: row.duration,
                track_number: row.track_number,
                disc_number: row.disc_number.unwrap_or(1),
                path: row.path,
                bitrate: row.bitrate,
                format: row.format,
                size: row.size,
                bpm: row.bpm,
                musicbrainz_track_id: row.musicbrainz_track_id,
                lyrics: row.lyrics,
                lyric_offset_ms: row.lyric_offset_ms,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            is_favorite: row.is_favorite,
        })
        .collect::<Vec<_>>();

    Ok(Json(GlobalSearchResponse {
        artists,
        albums,
        tracks,
    }))
}

pub async fn list_artists(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let q = format!("%{}%", params.q.unwrap_or_default());
    let artists = sqlx::query_as!(
        Artist,
        "SELECT * FROM artists WHERE name ILIKE $1 ORDER BY name",
        q
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(artists))
}

pub async fn list_albums(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    println!("DEBUG: list_albums called");
    let q = format!("%{}%", params.q.unwrap_or_default());
    let albums = sqlx::query_as!(Album,
        "SELECT * FROM albums WHERE (title ILIKE $1 OR $1 = '%%') AND ($2::uuid IS NULL OR artist_id = $2) ORDER BY release_year DESC",
        q, params.artist_id
    ).fetch_all(&state.db).await?;
    Ok(Json(albums))
}

pub async fn get_track(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state).await;

    // 安全审计修正：取消强制非空标志，处理匿名访问
    let row = sqlx::query!(
        r#"
        SELECT t.id, t.title, t.album_id, t.artist_id,
               t.duration, t.track_number,
               t.disc_number, t.path, t.bitrate,
               t.format, t.size, t.bpm,
               t.musicbrainz_track_id,
               t.lyrics,
               t.created_at, t.updated_at,
               (f.user_id IS NOT NULL) as is_favorite,
               a.name as "artist_name?",
               a.image_url as "artist_image_url?",
               al.title as "album_title?",
               COALESCE(m.lyric_offset_ms, 0) as lyric_offset_ms
        FROM tracks t
        LEFT JOIN user_favorites f ON t.id = f.track_id AND f.user_id = $2
        LEFT JOIN artists a ON t.artist_id = a.id
        LEFT JOIN albums al ON t.album_id = al.id
        LEFT JOIN user_track_metadata m ON t.id = m.track_id AND m.user_id = $2
        WHERE t.id = $1
        "#,
        id,
        user_id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError(AppError::NotFound("Track not found".to_string())))?;

    let track = Track {
        id: row.id,
        title: row.title,
        album_id: row.album_id,
        artist_id: row.artist_id,
        artist_name: row.artist_name,
        album_title: row.album_title,
        artist_image_url: row.artist_image_url,
        duration: row.duration,
        track_number: row.track_number,
        disc_number: row.disc_number.unwrap_or(1),
        path: row.path,
        bitrate: row.bitrate,
        format: row.format,
        size: row.size,
        bpm: row.bpm,
        musicbrainz_track_id: row.musicbrainz_track_id,
        lyrics: row.lyrics,
        lyric_offset_ms: row.lyric_offset_ms.unwrap_or(0),
        created_at: row.created_at,
        updated_at: row.updated_at,
    };

    Ok(Json(TrackWithFavorite {
        track,
        is_favorite: row.is_favorite.unwrap_or(false),
    }))
}

pub async fn list_tracks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<SearchQuery>,
) -> Result<impl IntoResponse, ApiError> {
    println!("DEBUG: list_tracks called");
    let user_id = crate::get_user_id(&headers, &state).await;
    let q = format!("%{}%", params.q.unwrap_or_default());

    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.title, t.album_id, t.artist_id,
               t.duration, t.track_number,
               t.disc_number, t.path, t.bitrate,
               t.format, t.size, t.bpm,
               t.musicbrainz_track_id,
               t.lyrics,
               t.created_at, t.updated_at,
               (f.user_id IS NOT NULL) as is_favorite,
               a.name as "artist_name?",
               a.image_url as "artist_image_url?",
               al.title as "album_title?",
               COALESCE(m.lyric_offset_ms, 0) as lyric_offset_ms
        FROM tracks t
        LEFT JOIN user_favorites f ON t.id = f.track_id AND f.user_id = $2
        LEFT JOIN artists a ON t.artist_id = a.id
        LEFT JOIN albums al ON t.album_id = al.id
        LEFT JOIN user_track_metadata m ON t.id = m.track_id AND m.user_id = $2
        WHERE (t.title ILIKE $1 OR $1 = '%%')
          AND ($3::uuid IS NULL OR t.album_id = $3)
          AND ($4::uuid IS NULL OR t.artist_id = $4)
        ORDER BY t.album_id, t.track_number, t.title
        LIMIT $5 OFFSET $6
        "#,
        q,
        user_id,
        params.album_id,
        params.artist_id,
        params.limit.unwrap_or(50),
        params.offset.unwrap_or(0)
    )
    .fetch_all(&state.db)
    .await?;

    let tracks = rows
        .into_iter()
        .map(|row| TrackWithFavorite {
            track: Track {
                id: row.id,
                title: row.title,
                album_id: row.album_id,
                artist_id: row.artist_id,
                artist_name: row.artist_name,
                album_title: row.album_title,
                artist_image_url: row.artist_image_url,
                duration: row.duration,
                track_number: row.track_number,
                disc_number: row.disc_number.unwrap_or(1),
                path: row.path,
                bitrate: row.bitrate,
                format: row.format,
                size: row.size,
                bpm: row.bpm,
                musicbrainz_track_id: row.musicbrainz_track_id,
                lyrics: row.lyrics,
                lyric_offset_ms: row.lyric_offset_ms.unwrap_or(0),
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            is_favorite: row.is_favorite.unwrap_or(false),
        })
        .collect::<Vec<_>>();

    Ok(Json(tracks))
}

pub async fn stream_track(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Query(params): Query<StreamQuery>,
) -> Result<impl IntoResponse, ApiError> {
    tracing::debug!(
        "Streaming track request: {} (Bitrate: {:?}, Start: {:?})",
        id,
        params.bitrate,
        params.start_time
    );

    let track = sqlx::query!("SELECT path FROM tracks WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| {
            tracing::error!("Track not found in DB: {}", id);
            ApiError(AppError::NotFound("Track not found".to_string()))
        })?;

    let path = std::path::Path::new(&track.path);
    if !path.exists() {
        tracing::error!("File missing on disk: {:?}", path);
        return Err(ApiError(AppError::NotFound(
            "File missing on disk".to_string(),
        )));
    }

    // 处理转码流
    if let Some(br) = params.bitrate {
        let path_str = path
            .to_str()
            .ok_or_else(|| ApiError(AppError::Internal("Invalid path encoding".to_string())))?;
        let mut args = Vec::new();

        // 必须通过 String 保持生命周期
        let ss_val;

        // 如果有起始时间，让 FFmpeg 直接跳过
        if let Some(start) = params.start_time {
            ss_val = start.to_string();
            args.extend(["-ss", &ss_val]);
        }

        args.extend([
            "-i", path_str, "-map", "0:a:0", "-b:a", &br, "-f", "mp3", "pipe:1",
        ]);

        let mut child = Command::new("ffmpeg")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| ApiError(AppError::Internal(format!("FFmpeg failed: {}", e))))?;

        let stdout = child.stdout.take().ok_or_else(|| {
            ApiError(AppError::Internal(
                "Failed to capture FFmpeg stdout".to_string(),
            ))
        })?;
        let stream = ReaderStream::new(stdout);

        let response = Response::builder()
            .header(header::CONTENT_TYPE, "audio/mpeg")
            .body(Body::from_stream(stream))
            .map_err(|e| ApiError(AppError::Internal(e.to_string())))?;
        return Ok(response);
    }

    // 核心改进：处理原始文件的 Range Request (206 Partial Content)
    let file = tokio::fs::File::open(path).await?;
    let metadata = file.metadata().await?;
    let file_size = metadata.len();
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    let range_header = headers.get(header::RANGE).and_then(|h| h.to_str().ok());

    if let Some(range) = range_header {
        if let Some(captures) = regex::Regex::new(r"bytes=(\d+)-(\d+)?")
            .unwrap()
            .captures(range)
        {
            let start = captures
                .get(1)
                .map(|m| m.as_str().parse::<u64>().unwrap_or(0))
                .unwrap_or(0);
            let end = captures
                .get(2)
                .map(|m| m.as_str().parse::<u64>().unwrap_or(file_size - 1))
                .unwrap_or(file_size - 1);

            if start < file_size {
                let end = end.min(file_size - 1);
                let content_length = end - start + 1;

                use std::io::{Seek, SeekFrom};
                let mut std_file = file.into_std().await;
                std_file.seek(SeekFrom::Start(start))?;

                let file = tokio::fs::File::from_std(std_file);
                let stream = ReaderStream::new(file).take(content_length as usize);

                let response = Response::builder()
                    .status(StatusCode::PARTIAL_CONTENT)
                    .header(header::CONTENT_TYPE, mime.as_ref())
                    .header(
                        header::CONTENT_RANGE,
                        format!("bytes {}-{}/{}", start, end, file_size),
                    )
                    .header(header::ACCEPT_RANGES, "bytes")
                    .header(header::CONTENT_LENGTH, content_length)
                    .body(Body::from_stream(stream))
                    .map_err(|e| ApiError(AppError::Internal(e.to_string())))?;
                return Ok(response);
            }
        }
    }

    // 默认全量响应
    let stream = ReaderStream::new(file);
    let response = Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CONTENT_LENGTH, file_size)
        .header(header::ACCEPT_RANGES, "bytes")
        .body(Body::from_stream(stream))
        .map_err(|e| ApiError(AppError::Internal(e.to_string())))?;
    Ok(response)
}

pub async fn get_lyrics(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let track = sqlx::query!(
        "SELECT id, title, artist_id, lyrics FROM tracks WHERE id = $1",
        id
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| ApiError(AppError::NotFound("Track not found".to_string())))?;

    if let Some(lrc) = track.lyrics {
        return Ok(lrc.into_response());
    }

    let artist = sqlx::query!("SELECT name FROM artists WHERE id = $1", track.artist_id)
        .fetch_optional(&state.db)
        .await?
        .map(|a| a.name)
        .unwrap_or_else(|| "Unknown".to_string());

    let db_clone = state.db.clone();
    let track_id = track.id;
    let title = track.title.clone();
    tokio::spawn(async move {
        let service = papilio_core::metadata::MetadataService::new(db_clone);
        let _ = service.fetch_lyrics_online(track_id, &title, &artist).await;
    });
    Err(ApiError(AppError::NotFound(
        "Lyrics fetching...".to_string(),
    )))
}

pub async fn toggle_favorite(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let exists = sqlx::query!(
        "SELECT 1 as x FROM user_favorites WHERE user_id = $1 AND track_id = $2",
        user_id,
        track_id
    )
    .fetch_optional(&state.db)
    .await?;

    if exists.is_some() {
        sqlx::query!(
            "DELETE FROM user_favorites WHERE user_id = $1 AND track_id = $2",
            user_id,
            track_id
        )
        .execute(&state.db)
        .await?;
        Ok(Json(json!({"is_favorite": false})))
    } else {
        sqlx::query!(
            "INSERT INTO user_favorites (user_id, track_id) VALUES ($1, $2)",
            user_id,
            track_id
        )
        .execute(&state.db)
        .await?;
        Ok(Json(json!({"is_favorite": true})))
    }
}

pub async fn list_favorites(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.title, t.album_id, t.artist_id,
               t.duration, t.track_number,
               t.disc_number, t.path, t.bitrate,
               t.format, t.size, t.bpm,
               t.musicbrainz_track_id,
               t.lyrics,
               t.created_at, t.updated_at,
               TRUE as "is_favorite!",
               a.name as "artist_name?",
               a.image_url as "artist_image_url?",
               al.title as "album_title?",
               COALESCE(m.lyric_offset_ms, 0) as "lyric_offset_ms!"
        FROM tracks t
        JOIN user_favorites f ON t.id = f.track_id
        LEFT JOIN artists a ON t.artist_id = a.id
        LEFT JOIN albums al ON t.album_id = al.id
        LEFT JOIN user_track_metadata m ON t.id = m.track_id AND m.user_id = $1
        WHERE f.user_id = $1
        ORDER BY f.created_at DESC
        "#,
        user_id
    )
    .fetch_all(&state.db)
    .await?;

    let tracks = rows
        .into_iter()
        .map(|row| TrackWithFavorite {
            track: Track {
                id: row.id,
                title: row.title,
                album_id: row.album_id,
                artist_id: row.artist_id,
                artist_name: row.artist_name,
                album_title: row.album_title,
                artist_image_url: row.artist_image_url,
                duration: row.duration,
                track_number: row.track_number,
                disc_number: row.disc_number.unwrap_or(1),
                path: row.path,
                bitrate: row.bitrate,
                format: row.format,
                size: row.size,
                bpm: row.bpm,
                musicbrainz_track_id: row.musicbrainz_track_id,
                lyrics: row.lyrics,
                lyric_offset_ms: row.lyric_offset_ms,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            is_favorite: row.is_favorite,
        })
        .collect::<Vec<_>>();
    Ok(Json(tracks))
}

pub async fn record_play(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;
    sqlx::query!(
        "INSERT INTO play_history (user_id, track_id) VALUES ($1, $2)",
        user_id,
        track_id
    )
    .execute(&state.db)
    .await?;
    Ok(StatusCode::OK)
}

pub async fn list_history(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let rows = sqlx::query!(
        r#"
        SELECT t.id, t.title, t.album_id, t.artist_id,
               t.duration, t.track_number,
               t.disc_number, t.path, t.bitrate,
               t.format, t.size, t.bpm,
               t.musicbrainz_track_id,
               t.lyrics,
               t.created_at, t.updated_at,
               (f.user_id IS NOT NULL) as "is_favorite!",
               a.name as "artist_name?",
               a.image_url as "artist_image_url?",
               al.title as "album_title?",
               COALESCE(m.lyric_offset_ms, 0) as "lyric_offset_ms!"
        FROM tracks t
        JOIN (SELECT track_id, MAX(played_at) as last_p FROM play_history WHERE user_id = $1 GROUP BY track_id) h ON t.id = h.track_id
        LEFT JOIN user_favorites f ON t.id = f.track_id AND f.user_id = $1
        LEFT JOIN artists a ON t.artist_id = a.id
        LEFT JOIN albums al ON t.album_id = al.id
        LEFT JOIN user_track_metadata m ON t.id = m.track_id AND m.user_id = $1
        ORDER BY h.last_p DESC LIMIT 50
        "#,
        user_id
    ).fetch_all(&state.db).await?;

    let tracks = rows
        .into_iter()
        .map(|row| TrackWithFavorite {
            track: Track {
                id: row.id,
                title: row.title,
                album_id: row.album_id,
                artist_id: row.artist_id,
                artist_name: row.artist_name,
                album_title: row.album_title,
                artist_image_url: row.artist_image_url,
                duration: row.duration,
                track_number: row.track_number,
                disc_number: row.disc_number.unwrap_or(1),
                path: row.path,
                bitrate: row.bitrate,
                format: row.format,
                size: row.size,
                bpm: row.bpm,
                musicbrainz_track_id: row.musicbrainz_track_id,
                lyrics: row.lyrics,
                lyric_offset_ms: row.lyric_offset_ms,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            is_favorite: row.is_favorite,
        })
        .collect::<Vec<_>>();
    Ok(Json(tracks))
}

pub async fn get_cover(
    State(state): State<Arc<AppState>>,
    Path(album_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let album = sqlx::query!("SELECT cover_path FROM albums WHERE id = $1", album_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError(AppError::NotFound("Album not found".to_string())))?;

    let internal_base_dir =
        std::env::var("COVER_DIR").unwrap_or_else(|_| "data/covers".to_string());
    let music_root = std::env::var("MUSIC_DIR").unwrap_or_else(|_| "data/music".to_string());

    let mut target_path = None;
    if let Some(ref path_str) = album.cover_path {
        // 逻辑：如果路径是绝对路径或包含 /music/，尝试直接打开
        // 如果路径是相对的且不包含 data/covers，尝试在 MUSIC_DIR 下找
        let p = std::path::Path::new(path_str);

        if p.is_absolute() && p.exists() {
            target_path = Some(p.to_path_buf());
        } else if !path_str.contains("data/covers") {
            // 它是整理后的相对路径，如 "Artist/Album/cover.jpg"
            let full = std::path::Path::new(&music_root).join(path_str);
            if full.exists() {
                target_path = Some(full);
            }
        } else {
            // 它是旧的内部路径，如 "data/covers/uuid.jpg"
            let filename = p.file_name().and_then(|f| f.to_str());
            if let Some(f) = filename {
                let full = std::path::Path::new(&internal_base_dir).join(f);
                if full.exists() {
                    target_path = Some(full);
                }
            }
        }
    }

    // 回退逻辑：按 ID 匹配
    if target_path.is_none() {
        let fallback_jpg =
            std::path::Path::new(&internal_base_dir).join(format!("{}.jpg", album_id));
        let fallback_png =
            std::path::Path::new(&internal_base_dir).join(format!("{}.png", album_id));
        if fallback_jpg.exists() {
            target_path = Some(fallback_jpg);
        } else if fallback_png.exists() {
            target_path = Some(fallback_png);
        }
    }

    let full_path = target_path.ok_or_else(|| {
        tracing::error!(
            "Cover not found for album {}. Checked music_root: {}",
            album_id,
            music_root
        );
        ApiError(AppError::NotFound("Cover file missing on disk".to_string()))
    })?;

    let mime = mime_guess::from_path(&full_path).first_or_octet_stream();
    let file = tokio::fs::File::open(&full_path).await?;
    let stream = ReaderStream::new(file);

    Response::builder()
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from_stream(stream))
        .map_err(|e| ApiError(AppError::Internal(e.to_string())))
}

pub async fn get_lyric_offset(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let row = sqlx::query!(
        "SELECT lyric_offset_ms FROM user_track_metadata WHERE user_id = $1 AND track_id = $2",
        user_id,
        track_id
    )
    .fetch_optional(&state.db)
    .await?;

    let offset = row.map(|r| r.lyric_offset_ms).unwrap_or(0);

    Ok(Json(json!({
        "track_id": track_id,
        "offset_ms": offset
    })))
}

pub async fn update_lyric_offset(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
    Json(payload): Json<UpdateLyricOffset>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    sqlx::query!(
        "INSERT INTO user_track_metadata (user_id, track_id, lyric_offset_ms, updated_at)
         VALUES ($1, $2, $3, NOW())
         ON CONFLICT (user_id, track_id) DO UPDATE SET lyric_offset_ms = EXCLUDED.lyric_offset_ms, updated_at = NOW()",
        user_id, track_id, payload.offset_ms
    )
    .execute(&state.db).await?;

    Ok(Json(json!({"status": "success"})))
}

pub async fn rescan_track_metadata(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let user_is_admin = sqlx::query("SELECT is_admin FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?
        .get::<bool, _>("is_admin");

    if !user_is_admin {
        return Err(ApiError(AppError::BadRequest(
            "Requires administrator privileges".to_string(),
        )));
    }

    let scanner = Scanner::new(state.db.clone());
    scanner.process_track_by_id(track_id).await?;

    Ok(Json(
        json!({"status": "success", "message": "Track metadata rescanned"}),
    ))
}

pub async fn sync_artist_metadata(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(artist_id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let user_is_admin = sqlx::query("SELECT is_admin FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?
        .get::<bool, _>("is_admin");

    if !user_is_admin {
        return Err(ApiError(AppError::BadRequest(
            "Requires administrator privileges".to_string(),
        )));
    }

    let db_clone = state.db.clone();
    tokio::spawn(async move {
        let service = papilio_core::metadata::MetadataService::new(db_clone);
        if let Err(e) = service.fetch_and_update_artist(artist_id).await {
            tracing::error!("Failed to sync artist {}: {:?}", artist_id, e);
        }
    });

    Ok(Json(
        json!({"status": "success", "message": "Artist metadata sync started"}),
    ))
}
