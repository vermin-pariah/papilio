use crate::{ApiError, AppState};
use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use papilio_core::error::AppError;
use papilio_core::scanner::organizer::Organizer;
use serde::Deserialize;
use serde_json::json;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct UpdateConfigPayload {
    pub key: String,
    pub value: serde_json::Value,
}

pub async fn get_admin_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    let rows = sqlx::query("SELECT key, value FROM system_config")
        .fetch_all(&state.db)
        .await
        .map_err(|e| ApiError(AppError::Database(e)))?;

    let mut config = std::collections::HashMap::new();
    for row in rows {
        let key: String = row.get::<String, _>("key");
        let value: serde_json::Value = row.get::<serde_json::Value, _>("value");
        config.insert(key, value);
    }

    Ok(Json(config))
}

pub async fn update_admin_config(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<UpdateConfigPayload>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    sqlx::query("INSERT INTO system_config (key, value, updated_at) VALUES ($1, $2, NOW()) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()")
        .bind(&payload.key)
        .bind(&payload.value)
        .execute(&state.db).await
        .map_err(|e| ApiError(AppError::Database(e)))?;

    Ok(Json(json!({"status": "success"})))
}

pub async fn get_system_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    Ok(Json(json!({
        "status": "online",
    })))
}

#[derive(Deserialize)]
pub struct UpdateUserRolePayload {
    pub is_admin: bool,
}

pub async fn list_users(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    let users = sqlx::query!(
        r#"
        SELECT id, username, email, is_admin, created_at, nickname, avatar
        FROM users
        ORDER BY created_at DESC
        "#
    )
    .fetch_all(&state.db)
    .await?;

    let result = users
        .into_iter()
        .map(|row| {
            json!({
                "id": row.id,
                "username": row.username,
                "email": row.email,
                "is_admin": row.is_admin.unwrap_or(false),
                "created_at": row.created_at,
                "nickname": row.nickname,
                "avatar": row.avatar,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(result))
}

pub async fn update_user_role(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<Uuid>,
    Json(payload): Json<UpdateUserRolePayload>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    sqlx::query!(
        "UPDATE users SET is_admin = $1, updated_at = NOW() WHERE id = $2",
        payload.is_admin,
        user_id
    )
    .execute(&state.db)
    .await?;

    tracing::info!(
        "ADMIN: User {} role updated to is_admin: {}",
        user_id,
        payload.is_admin
    );

    Ok(Json(json!({"status": "success"})))
}

pub async fn delete_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    // 防止管理员删除自己
    let current_admin_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    if user_id == current_admin_id {
        return Err(ApiError(AppError::BadRequest(
            "Cannot delete your own account".to_string(),
        )));
    }

    sqlx::query!("DELETE FROM users WHERE id = $1", user_id)
        .execute(&state.db)
        .await?;

    tracing::info!("ADMIN: User {} has been deleted", user_id);

    Ok(Json(json!({"status": "success"})))
}

async fn check_admin(headers: &HeaderMap, state: &AppState) -> Result<(), ApiError> {
    let user_id = crate::get_user_id(headers, state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let is_admin = sqlx::query_scalar!("SELECT is_admin FROM users WHERE id = $1", user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|e| ApiError(AppError::Database(e)))?;

    if is_admin != Some(true) {
        return Err(ApiError(AppError::Auth(
            "Forbidden: Admin access required".to_string(),
        )));
    }

    Ok(())
}

pub async fn trigger_artist_sync(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;
    tracing::info!("ADMIN: trigger_artist_sync called");

    // 检查是否已有同步任务在运行
    let is_syncing: bool =
        sqlx::query_scalar("SELECT is_syncing FROM artist_sync_status WHERE id = 1")
            .fetch_one(&state.db)
            .await?;

    if is_syncing {
        return Err(ApiError(AppError::BadRequest(
            "A sync task is already in progress".to_string(),
        )));
    }

    // 查找库中所有歌手并触发元数据同步
    let artists_to_sync = sqlx::query!("SELECT id FROM artists")
        .fetch_all(&state.db)
        .await?;

    let total = artists_to_sync.len() as i32;
    if total == 0 {
        return Ok(Json(
            json!({"status": "success", "message": "No artists need syncing"}),
        ));
    }

    // 初始化状态
    sqlx::query!(
        "UPDATE artist_sync_status SET is_syncing = TRUE, current_count = 0, total_count = $1, last_error = NULL WHERE id = 1",
        total
    ).execute(&state.db).await?;

    let state_clone = state.clone();
    tokio::spawn(async move {
        tracing::info!(
            "ADMIN: Background artist sync thread started. Total artists: {}",
            total
        );
        let mut current = 0;

        for artist in artists_to_sync {
            current += 1;
            println!(
                "ADMIN_DEBUG: Syncing artist {}/{} (ID: {})",
                current, total, artist.id
            );
            tracing::info!(
                "ADMIN: Syncing artist {}/{} (ID: {})",
                current,
                total,
                artist.id
            );

            // 单次同步超时保护 (30秒)，防止单个异常请求阻塞队列
            let metadata_service = state_clone.metadata_service.clone();
            let sync_future = metadata_service.fetch_and_update_artist(artist.id);
            
            match tokio::time::timeout(std::time::Duration::from_secs(30), sync_future).await {
                Ok(Ok(_)) => {
                    tracing::info!("ADMIN: Sync success for artist {}", artist.id);
                }
                Ok(Err(e)) => {
                    tracing::error!("ADMIN: Batch sync failed for artist {}: {:?}", artist.id, e);
                    let _ = sqlx::query!(
                        "UPDATE artist_sync_status SET last_error = $1 WHERE id = 1",
                        format!("Artist {}: {:?}", artist.id, e)
                    )
                    .execute(&state_clone.db)
                    .await;
                }
                Err(_) => {
                    tracing::error!("ADMIN: Batch sync TIMEOUT for artist {}", artist.id);
                    let _ = sqlx::query!(
                        "UPDATE artist_sync_status SET last_error = $1 WHERE id = 1",
                        format!("Timeout syncing artist {}", artist.id)
                    )
                    .execute(&state_clone.db)
                    .await;
                }
            }

            // 更新进度
            let _ = sqlx::query!(
                "UPDATE artist_sync_status SET current_count = $1 WHERE id = 1",
                current
            )
            .execute(&state_clone.db)
            .await;

            // 增加请求间隔以符合 MusicBrainz API 频率限制建议
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }

        tracing::info!("ADMIN: Batch sync completed successfully.");
        let _ = sqlx::query!(
            "UPDATE artist_sync_status SET is_syncing = FALSE, last_sync_at = NOW() WHERE id = 1"
        )
        .execute(&state_clone.db)
        .await;
    });

    Ok(Json(json!({"status": "success", "total": total})))
}

pub async fn trigger_artist_sync_single(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(artist_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = state_clone
            .metadata_service
            .fetch_and_update_artist(artist_id)
            .await
        {
            tracing::error!("Single artist sync failed for {}: {:?}", artist_id, e);
        }
    });

    Ok(Json(json!({"status": "success"})))
}

pub async fn get_artist_sync_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    let row = sqlx::query!("SELECT is_syncing, current_count, total_count, last_sync_at, last_error FROM artist_sync_status WHERE id = 1")
        .fetch_one(&state.db).await?;

    Ok(Json(json!({
        "is_syncing": row.is_syncing,
        "current_count": row.current_count,
        "total_count": row.total_count,
        "last_sync_at": row.last_sync_at,
        "last_error": row.last_error,
    })))
}

pub async fn trigger_artist_sync_missing(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&_headers, &state).await?;

    let is_syncing: bool =
        sqlx::query_scalar("SELECT is_syncing FROM artist_sync_status WHERE id = 1")
            .fetch_one(&state.db)
            .await?;

    if is_syncing {
        return Err(ApiError(AppError::BadRequest(
            "A sync task is already in progress".to_string(),
        )));
    }

    // 关键差异：只查找没有图片的歌手
    let artists_to_sync =
        sqlx::query!("SELECT id FROM artists WHERE image_url IS NULL OR image_url = ''")
            .fetch_all(&state.db)
            .await?;

    let total = artists_to_sync.len() as i32;
    if total == 0 {
        return Ok(Json(
            json!({"status": "success", "message": "All artists already have images"}),
        ));
    }

    sqlx::query!(
        "UPDATE artist_sync_status SET is_syncing = TRUE, current_count = 0, total_count = $1, last_error = NULL WHERE id = 1",
        total
    ).execute(&state.db).await?;

    let state_clone = state.clone();
    tokio::spawn(async move {
        tracing::info!("ADMIN: Missing artist sync started. Total: {}", total);
        let mut current = 0;
        for artist in artists_to_sync {
            current += 1;
            if let Err(e) = state_clone
                .metadata_service
                .fetch_and_update_artist(artist.id)
                .await
            {
                tracing::error!("Sync failed for artist {}: {:?}", artist.id, e);
            }
            let _ = sqlx::query!(
                "UPDATE artist_sync_status SET current_count = $1 WHERE id = 1",
                current
            )
            .execute(&state_clone.db)
            .await;
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        }
        let _ = sqlx::query!(
            "UPDATE artist_sync_status SET is_syncing = FALSE, last_sync_at = NOW() WHERE id = 1"
        )
        .execute(&state_clone.db)
        .await;
    });

    Ok(Json(json!({"status": "success", "total": total})))
}

pub async fn upload_artist_avatar(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(artist_id): axum::extract::Path<Uuid>,
    mut multipart: axum::extract::Multipart,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&headers, &state).await?;

    if let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError(AppError::BadRequest(e.to_string())))?
    {
        let data = field
            .bytes()
            .await
            .map_err(|e| ApiError(AppError::BadRequest(e.to_string())))?;

        if data.len() > 10 * 1024 * 1024 {
            return Err(ApiError(AppError::BadRequest(
                "Artist avatar too large (max 10MB)".to_string(),
            )));
        }

        // 校验文件头（Magic Number）
        let kind = infer::get(&data).ok_or_else(|| {
            ApiError(AppError::BadRequest(
                "Unknown file type. Only images are allowed.".to_string(),
            ))
        })?;

        if !kind.mime_type().starts_with("image/") {
            return Err(ApiError(AppError::BadRequest(format!(
                "Invalid file type: {}. Only images are allowed.",
                kind.mime_type()
            ))));
        }

        let extension = kind.extension();
        // 净化文件名，防止路径穿越攻击
        let filename = sanitize_filename::sanitize(format!("artist_{}.{}", artist_id, extension));
        let base_dir =
            std::env::var("AVATAR_DIR").unwrap_or_else(|_| "data/avatars".to_string());
        let full_path = std::path::Path::new(&base_dir).join(&filename);

        if !std::path::Path::new(&base_dir).exists() {
            tokio::fs::create_dir_all(&base_dir).await?;
        }
        
        tokio::fs::write(full_path, data).await?;

        sqlx::query!(
            "UPDATE artists SET image_url = $1 WHERE id = $2",
            filename,
            artist_id
        )
        .execute(&state.db)
        .await?;

        tracing::info!(
            "ADMIN: Manual avatar upload success for artist {}",
            artist_id
        );
        return Ok(Json(json!({"status": "success", "image_url": filename})));
    }

    Err(ApiError(AppError::BadRequest(
        "No file uploaded".to_string(),
    )))
}

pub async fn trigger_library_organize(
    State(state): State<Arc<AppState>>,
    _headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    check_admin(&_headers, &state).await?;

    let music_root = std::env::var("MUSIC_DIR").unwrap_or_else(|_| "data/music".to_string());
    let organizer = Organizer::new(state.db.clone(), music_root.into());

    tokio::spawn(async move {
        tracing::warn!("ADMIN: Library reorganization started by administrator.");
        if let Err(e) = organizer.organize().await {
            tracing::error!("ADMIN: Library reorganization failed: {:?}", e);
        } else {
            tracing::info!("ADMIN: Library reorganization finished successfully.");
        }
    });

    Ok(Json(json!({
        "status": "success",
        "message": "Library reorganization task has been queued in the background."
    })))
}
