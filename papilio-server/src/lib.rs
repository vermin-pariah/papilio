pub mod handlers;
pub mod routes;

pub const SESSION_EXPIRATION: u64 = 7 * 24 * 60 * 60; // 7 days
pub const SESSION_PREFIX: &str = "session:";
pub const USER_SESSIONS_PREFIX: &str = "user_sessions:";

use axum::{
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use papilio_core::auth::verify_token;
use papilio_core::error::{AppError, ErrorResponse};
use sqlx::PgPool;
use uuid::Uuid;

use papilio_core::metadata::MetadataService;
use redis::aio::ConnectionManager;
use std::sync::Arc;

pub struct AppState {
    pub db: PgPool,
    pub redis: ConnectionManager,
    pub jwt_secret: String,
    pub metadata_service: Arc<MetadataService>,
}

// 定义 Server 本地的错误包装器
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(inner: AppError) -> Self {
        Self(inner)
    }
}

// 自动转换常用的 Result 错误
impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self(AppError::Database(err))
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> Self {
        Self(AppError::Io(err))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self.0 {
            AppError::Database(ref e) => {
                tracing::error!("Database error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
            AppError::Auth(m) => (StatusCode::UNAUTHORIZED, m),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            AppError::Io(ref e) => {
                tracing::error!("IO error: {:?}", e);
                (StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
            }
            AppError::Metadata(m) => (StatusCode::UNPROCESSABLE_ENTITY, m),
            AppError::Internal(m) => {
                tracing::error!("Internal error: {}", m);
                (StatusCode::INTERNAL_SERVER_ERROR, m)
            }
        };

        (status, Json(ErrorResponse { error: message })).into_response()
    }
}

pub async fn get_user_id(headers: &HeaderMap, state: &AppState) -> Option<Uuid> {
    let auth_header = match headers.get("Authorization") {
        Some(h) => h,
        None => {
            tracing::debug!("Auth: Missing Authorization header");
            return None;
        }
    };

    let auth_str = match auth_header.to_str() {
        Ok(s) => s,
        Err(_) => {
            tracing::warn!("Auth: Authorization header is not a valid string");
            return None;
        }
    };

    let token = if let Some(t) = auth_str.strip_prefix("Bearer ") {
        t
    } else {
        tracing::warn!("Auth: Missing Bearer prefix");
        return None;
    };

    // 1. 验证 JWT 基础有效性
    let claims = match verify_token(token, &state.jwt_secret) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Auth: JWT verification failed for token {}: {:?}", token, e);
            return None;
        }
    };

    // 2. 检查 Valkey 中是否存在该 Session
    let mut redis = state.redis.clone();
    use redis::AsyncCommands;
    let session_key = format!("{}{}", SESSION_PREFIX, token);
    let exists: bool = redis.exists(&session_key).await.unwrap_or(false);

    if !exists {
        tracing::warn!(
            "Auth: Session key NOT FOUND in Redis: {}. User: {}",
            session_key,
            claims.sub
        );
        return None;
    }

    // 3. 续签
    let _: () = redis
        .expire(&session_key, SESSION_EXPIRATION as i64)
        .await
        .unwrap_or(());

    Some(claims.sub)
}
