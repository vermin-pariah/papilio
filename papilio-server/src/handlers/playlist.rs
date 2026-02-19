use crate::handlers::music::TrackWithFavorite;
use crate::{get_user_id, ApiError, AppState};
use axum::{
    extract::{Json, Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use papilio_core::error::AppError;
use papilio_core::models::music::{CreatePlaylist, Playlist, Track};
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct PlaylistWithFavoriteTracks {
    #[serde(flatten)]
    pub playlist: Playlist,
    pub tracks: Vec<TrackWithFavorite>,
}

pub async fn create_playlist(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CreatePlaylist>,
) -> Result<impl IntoResponse, ApiError> {
    let name = payload.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(ApiError(AppError::BadRequest(
            "Playlist name must be between 1 and 100 characters".to_string(),
        )));
    }

    let user_id = get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let playlist = sqlx::query_as!(
        Playlist,
        r#"INSERT INTO playlists (user_id, name, description, is_public) VALUES ($1, $2, $3, $4)
           RETURNING id, user_id, name, description, is_public as "is_public!", created_at, updated_at"#,
        user_id,
        name,
        payload.description,
        payload.is_public.unwrap_or(false)
    )
    .fetch_one(&state.db).await?;

    Ok((StatusCode::CREATED, Json(playlist)))
}

pub async fn list_my_playlists(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    println!("DEBUG: list_my_playlists called");
    let user_id = get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let playlists = sqlx::query_as!(
        Playlist,
        r#"SELECT id, user_id, name, description, is_public as "is_public!", created_at, updated_at
           FROM playlists WHERE user_id = $1 ORDER BY updated_at DESC"#,
        user_id
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(playlists))
}

pub async fn add_track(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((id, track_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let p = sqlx::query!("SELECT user_id FROM playlists WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError(AppError::NotFound("Playlist not found".to_string())))?;

    if p.user_id != user_id {
        return Err(ApiError(AppError::Auth("Forbidden: Not your playlist".to_string())));
    }

    let pos = sqlx::query!(r#"SELECT COALESCE(MAX(position), 0) as "max_pos!" FROM playlist_tracks WHERE playlist_id = $1"#, id)
        .fetch_one(&state.db).await?
        .max_pos;

    sqlx::query!(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
        id, track_id, pos + 1
    ).execute(&state.db).await?;

    Ok(Json(json!({"status": "success"})))
}

pub async fn reorder_tracks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(playlist_id): Path<Uuid>,
    Json(track_ids): Json<Vec<Uuid>>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let p = sqlx::query!("SELECT user_id FROM playlists WHERE id = $1", playlist_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError(AppError::NotFound("Playlist not found".to_string())))?;

    if p.user_id != user_id {
        return Err(ApiError(AppError::Auth("Forbidden: Not your playlist".to_string())));
    }

    // 批量更新位置，利用 UNNEST 避免循环 SQL 查询以提升性能
    let positions: Vec<i32> = (1..=track_ids.len() as i32).collect();
    
    sqlx::query!(
        r#"
        UPDATE playlist_tracks AS pt
        SET position = new_pos
        FROM UNNEST($1::uuid[], $2::int[]) AS m(t_id, new_pos)
        WHERE pt.playlist_id = $3 AND pt.track_id = m.t_id
        "#,
        &track_ids,
        &positions,
        playlist_id
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::OK)
}

pub async fn remove_track(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((id, track_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let p = sqlx::query!("SELECT user_id FROM playlists WHERE id = $1", id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| ApiError(AppError::NotFound("Playlist not found".to_string())))?;

    if p.user_id != user_id {
        return Err(ApiError(AppError::Auth("Forbidden: Not your playlist".to_string())));
    }

    sqlx::query!(
        "DELETE FROM playlist_tracks WHERE playlist_id = $1 AND track_id = $2",
        id,
        track_id
    )
    .execute(&state.db)
    .await?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn update_playlist(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<CreatePlaylist>,
) -> Result<impl IntoResponse, ApiError> {
    let name = payload.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(ApiError(AppError::BadRequest(
            "Playlist name must be between 1 and 100 characters".to_string(),
        )));
    }

    let user_id = get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let playlist = sqlx::query_as!(
        Playlist,
        r#"UPDATE playlists SET name = $1, description = $2, is_public = $3, updated_at = NOW()
           WHERE id = $4 AND user_id = $5
           RETURNING id, user_id, name, description, is_public as "is_public!", created_at, updated_at"#,
        name,
        payload.description,
        payload.is_public.unwrap_or(false),
        id,
        user_id
    )
    .fetch_optional(&state.db).await?
    .ok_or_else(|| ApiError(AppError::NotFound("Playlist not found or access denied".to_string())))?;

    Ok(Json(playlist))
}

pub async fn delete_playlist(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let res = sqlx::query!(
        "DELETE FROM playlists WHERE id = $1 AND user_id = $2",
        id,
        user_id
    )
    .execute(&state.db)
    .await?;

    if res.rows_affected() == 0 {
        return Err(ApiError(AppError::NotFound(
            "Playlist not found or access denied".to_string(),
        )));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_playlist(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = get_user_id(&headers, &state).await;

    let playlist_opt = sqlx::query_as!(Playlist, r#"SELECT id, user_id, name, description, is_public as "is_public!", created_at, updated_at FROM playlists WHERE id = $1"#, id)
        .fetch_optional(&state.db).await?;

    let playlist = match playlist_opt {
        Some(p) => p,
        None => {
            tracing::warn!(
                "DATA ERROR: Playlist ID {} not found. Returning expired state.",
                id
            );
            Playlist {
                id,
                user_id: Uuid::nil(),
                name: "已失效的列表".to_string(),
                description: Some("该列表已在服务器上删除，请下拉刷新".to_string()),
                is_public: false,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
        }
    };

    if !playlist.is_public && playlist.user_id != Uuid::nil() && Some(playlist.user_id) != user_id {
        return Err(ApiError(AppError::Auth("Forbidden: Private playlist".to_string())));
    }

    let rows = sqlx::query(
        r#"
        SELECT t.id, t.title, t.album_id, t.artist_id,
               t.duration, t.track_number,
               t.disc_number, t.path, t.bitrate,
               t.format, t.size, t.bpm,
               t.musicbrainz_track_id,
               t.lyrics,
               t.created_at, t.updated_at,
               (f.user_id IS NOT NULL) as is_favorite,
               a.name as artist_name,
               a.image_url as artist_image_url,
               al.title as album_title,
               COALESCE(m.lyric_offset_ms, 0) as lyric_offset_ms
        FROM tracks t
        JOIN playlist_tracks pt ON t.id = pt.track_id
        LEFT JOIN user_favorites f ON t.id = f.track_id AND f.user_id = $2
        LEFT JOIN artists a ON t.artist_id = a.id
        LEFT JOIN albums al ON t.album_id = al.id
        LEFT JOIN user_track_metadata m ON t.id = m.track_id AND m.user_id = $2
        WHERE pt.playlist_id = $1
        ORDER BY pt.position
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;

    let tracks = rows
        .into_iter()
        .map(|row| TrackWithFavorite {
            track: Track {
                id: row.get("id"),
                title: row.get("title"),
                album_id: row.get("album_id"),
                artist_id: row.get("artist_id"),
                artist_name: row.get("artist_name"),
                album_title: row.get("album_title"),
                artist_image_url: row.get("artist_image_url"),
                duration: row.get("duration"),
                track_number: row.get("track_number"),
                disc_number: row.get::<Option<i32>, _>("disc_number").unwrap_or(1),
                path: row.get("path"),
                bitrate: row.get("bitrate"),
                format: row.get("format"),
                size: row.get("size"),
                bpm: row.get("bpm"),
                musicbrainz_track_id: row.get("musicbrainz_track_id"),
                lyrics: row.get("lyrics"),
                lyric_offset_ms: row.get::<i32, _>("lyric_offset_ms"),
                created_at: row.get("created_at"),
                updated_at: row.get("updated_at"),
            },
            is_favorite: row.get("is_favorite"),
        })
        .collect::<Vec<_>>();

    Ok(Json(PlaylistWithFavoriteTracks { playlist, tracks }))
}
