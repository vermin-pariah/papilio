use papilio_core::scanner::organizer::Organizer;
use sqlx::postgres::PgPoolOptions;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let music_root = std::env::var("MUSIC_DIR").expect("MUSIC_DIR must be set");

    println!("QA_TOOL: Connecting to DB: {}", database_url);
    let pool = PgPoolOptions::new().connect(&database_url).await?;

    let organizer = Organizer::new(pool, PathBuf::from(music_root));

    println!("QA_TOOL: Starting aggressive reorganization...");
    organizer.organize().await?;
    println!("QA_TOOL: Reorganization finished successfully!");

    Ok(())
}
