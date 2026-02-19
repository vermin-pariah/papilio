use axum::{routing::get, Router};
use papilio_server::AppState;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "papilio_server=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")?;
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "default_secret".to_string());
    let cover_dir = std::env::var("COVER_DIR").unwrap_or_else(|_| "../data/covers".to_string());

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await?;
    let redis_client = redis::Client::open(redis_url)?;
    let redis_manager = redis::aio::ConnectionManager::new(redis_client).await?;
    let metadata_service =
        std::sync::Arc::new(papilio_core::metadata::MetadataService::new(pool.clone()));

    tracing::info!("Running database migrations...");
    sqlx::migrate!("../papilio-core/migrations")
        .run(&pool)
        .await?;

    // 启动时清理状态标志，防止因服务异常宕机导致的扫描状态挂起
    tracing::info!("Cleaning up stale scan/sync flags...");
    let _ = sqlx::query("UPDATE scan_status SET is_scanning = FALSE WHERE id = 1").execute(&pool).await;
    let _ = sqlx::query("UPDATE artist_sync_status SET is_syncing = FALSE WHERE id = 1").execute(&pool).await;

    // 如果库中不存在管理员，初始化默认账号
    let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_admin = TRUE")
        .fetch_one(&pool)
        .await?;

    if admin_count == 0 {
        tracing::info!("No admin user found. Initializing seed admin: chi");
        let password_hash = papilio_core::auth::hash_password("chi")
            .map_err(|e| anyhow::anyhow!("Failed to hash seed password: {}", e))?;
        
        sqlx::query!(
            "INSERT INTO users (username, nickname, password_hash, is_admin) VALUES ($1, $2, $3, $4)",
            "chi", "Default Admin", password_hash, true
        )
        .execute(&pool)
        .await?;
        tracing::info!("Seed admin user 'chi' created successfully.");
    }

    let state = Arc::new(AppState {
        db: pool,
        redis: redis_manager,
        jwt_secret,
        metadata_service,
    });

    let music_root = std::env::var("MUSIC_DIR").unwrap_or_else(|_| "data/music".to_string());

    // 定义 API 路由树
    let app = Router::new()
        .route("/api/health", get(|| async { "OK" }))
        .nest("/api/music", papilio_server::routes::music_routes())
        .nest("/api/playlists", papilio_server::routes::playlist_routes())
        .nest("/api/auth", papilio_server::routes::auth_routes())
        .nest("/api/admin", papilio_server::routes::admin_routes())
        .nest_service(
            "/data/covers",
            tower_http::services::ServeDir::new(cover_dir),
        )
        .nest_service(
            "/data/avatars",
            tower_http::services::ServeDir::new("data/avatars"),
        )
        .nest_service(
            "/data/music",
            tower_http::services::ServeDir::new(music_root),
        )
        .layer(axum::middleware::from_fn(
            |req: axum::extract::Request, next: axum::middleware::Next| async move {
                let method = req.method().clone();
                let uri = req.uri().clone();
                // 记录请求审计日志
                tracing::info!("REQ: {} {}", method, uri);
                let response = next.run(req).await;
                tracing::info!("RES: {} -> {}", uri, response.status());
                response
            },
        ))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
