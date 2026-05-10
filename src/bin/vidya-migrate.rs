use anyhow::{Context, Result};

use vidya::{config::{Config, vidya_home}, db};

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_path(vidya_home().join(".env"));
    let _ = dotenvy::dotenv();

    let cfg = Config::from_env();

    let pool = db::connect(&cfg).await.context("connecting to database")?;
    db::run_migrations(&pool)
        .await
        .context("running migrations")?;

    println!("Migrations applied successfully.");
    Ok(())
}
