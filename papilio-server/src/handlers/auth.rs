use crate::{ApiError, AppState};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use papilio_core::{
    auth::{create_token, hash_password, verify_password},
    error::AppError,
    models::user::{CreateUser, UpdateUser, User, UserResponse},
};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub async fn get_me(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let user = User::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| ApiError(AppError::NotFound("User not found".to_string())))?;

    tracing::debug!(
        "GET_ME: user_id={}, nickname={:?}, avatar={:?}",
        user.id,
        user.nickname,
        user.avatar
    );

    Ok(Json(UserResponse::from(user)))
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<UpdateUser>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    let password_hash = if let Some(p) = payload.password {
        Some(hash_password(&p).map_err(|e| ApiError(AppError::Internal(e.to_string())))?)
    } else {
        None
    };

    let user = User::update(
        &state.db,
        user_id,
        payload.nickname,
        payload.avatar,
        payload.email,
        password_hash,
    )
    .await?;
    Ok(Json(UserResponse::from(user)))
}

use axum::extract::Multipart;
use std::path::Path;
use tokio::fs;

pub async fn upload_avatar(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    // 1. 获取当前用户信息以进行物理删除
    let current_user = User::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| ApiError(AppError::NotFound("User not found".to_string())))?;

    let mut filename = None;

    while let Some(field) =
        multipart
            .next_field()
            .await
            .map_err(|e: axum::extract::multipart::MultipartError| {
                ApiError(AppError::Internal(e.to_string()))
            })?
    {
        if field.name() == Some("avatar") {
            let data =
                field
                    .bytes()
                    .await
                    .map_err(|e: axum::extract::multipart::MultipartError| {
                        ApiError(AppError::Internal(e.to_string()))
                    })?;

            if data.len() > 5 * 1024 * 1024 {
                return Err(ApiError(AppError::BadRequest(
                    "Avatar too large (max 5MB)".to_string(),
                )));
            }

            // 校验文件头（Magic Number）以判定真实 MIME 类型
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
            let safe_filename = sanitize_filename::sanitize(format!("{}.{}", user_id, extension));
            let avatars_dir = Path::new("data/avatars");
            let path = avatars_dir.join(&safe_filename);

            // 确保物理目录存在
            if !avatars_dir.exists() {
                fs::create_dir_all(avatars_dir).await.map_err(|e| {
                    ApiError(AppError::Internal(format!("Failed to create avatars dir: {}", e)))
                })?;
            }

            // 物理删除旧头像 (如果存在且文件名不同)
            if let Some(ref old_avatar) = current_user.avatar {
                if old_avatar != &safe_filename {
                    let old_path = avatars_dir.join(old_avatar);
                    if let Err(e) = fs::remove_file(&old_path).await {
                        tracing::warn!("Failed to remove old avatar file {:?}: {}", old_path, e);
                    }
                }
            }

            // 3. 写入文件
            fs::write(&path, data)
                .await
                .map_err(|e: std::io::Error| ApiError(AppError::Internal(e.to_string())))?;
            filename = Some(safe_filename);
        }
    }

    if let Some(name) = filename {
        let user = User::update(&state.db, user_id, None, Some(name), None, None).await?;
        Ok(Json(UserResponse::from(user)))
    } else {
        Err(ApiError(AppError::BadRequest(
            "No avatar file provided".to_string(),
        )))
    }
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateUser>,
) -> Result<impl IntoResponse, ApiError> {
    let password_hash = hash_password(&payload.password)
        .map_err(|e| ApiError(AppError::Internal(e.to_string())))?;

    let user = User::create(&state.db, payload, password_hash).await?;
    Ok((StatusCode::CREATED, Json(UserResponse::from(user))))
}

use serde::Deserialize;

#[derive(Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginPayload>,
) -> Result<impl IntoResponse, ApiError> {
    let user = User::find_by_username(&state.db, &payload.username)
        .await?
        .ok_or_else(|| ApiError(AppError::Auth("Invalid credentials".to_string())))?;

    let is_valid = verify_password(&payload.password, &user.password_hash).map_err(|e| {
        ApiError(AppError::Internal(format!(
            "Password verification system error: {}",
            e
        )))
    })?;

    if !is_valid {
        return Err(ApiError(AppError::Auth("Invalid credentials".to_string())));
    }

    let token = create_token(user.id, user.username.clone(), &state.jwt_secret)
        .map_err(|e| ApiError(AppError::Internal(e.to_string())))?;

    let mut redis = state.redis.clone();
    use redis::AsyncCommands;
    let session_key = format!("{}{}", crate::SESSION_PREFIX, token);
    let user_sessions_key = format!("{}{}", crate::USER_SESSIONS_PREFIX, user.id);

    let _: () = redis
        .set_ex(&session_key, user.id.to_string(), crate::SESSION_EXPIRATION)
        .await
        .map_err(|e| ApiError(AppError::Internal(format!("Valkey error: {}", e))))?;

    let _: () = redis
        .sadd(&user_sessions_key, &token)
        .await
        .map_err(|e| ApiError(AppError::Internal(format!("Valkey error: {}", e))))?;

    let _: () = redis
        .expire(&user_sessions_key, crate::SESSION_EXPIRATION as i64)
        .await
        .unwrap_or(());

    tracing::debug!(
        "LOGIN_SUCCESS: username={}, nickname={:?}",
        user.username,
        user.nickname
    );

    Ok(Json(json!({
        "token": token,
        "user": UserResponse::from(user)
    })))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let auth_header = headers.get("Authorization").and_then(|h| h.to_str().ok());
    if let Some(auth) = auth_header {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            let mut redis = state.redis.clone();
            use redis::AsyncCommands;

            if let Ok(claims) = papilio_core::auth::verify_token(token, &state.jwt_secret) {
                let session_key = format!("{}{}", crate::SESSION_PREFIX, token);
                let user_sessions_key = format!("{}{}", crate::USER_SESSIONS_PREFIX, claims.sub);

                let _: () = redis.del(&session_key).await.unwrap_or(());
                let _: () = redis.srem(&user_sessions_key, token).await.unwrap_or(());
            }
        }
    }

    Ok(StatusCode::OK)
}

pub async fn kick_user(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(target_user_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let user_id = crate::get_user_id(&headers, &state)
        .await
        .ok_or_else(|| ApiError(AppError::Auth("Unauthorized".to_string())))?;

    // 鉴权：允许管理员踢除任意用户，或普通用户踢除自己的其他会话
    let user_is_admin = sqlx::query_scalar!("SELECT is_admin FROM users WHERE id = $1", user_id)
        .fetch_one(&state.db)
        .await?
        .unwrap_or(false);

    if user_id != target_user_id && !user_is_admin {
        return Err(ApiError(AppError::Auth(
            "Forbidden: Not an administrator".to_string(),
        )));
    }

    let mut redis = state.redis.clone();
    use redis::AsyncCommands;
    let user_sessions_key = format!("{}{}", crate::USER_SESSIONS_PREFIX, target_user_id);

    let tokens: Vec<String> = redis
        .smembers(&user_sessions_key)
        .await
        .map_err(|e| ApiError(AppError::Internal(e.to_string())))?;

    for token in tokens {
        let _: () = redis
            .del(format!("{}{}", crate::SESSION_PREFIX, token))
            .await
            .unwrap_or(());
    }

    let _: () = redis.del(&user_sessions_key).await.unwrap_or(());

    Ok(StatusCode::OK)
}
