use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use vidya::{
    config::{Config, vidya_home},
    db,
    tools::load,
};

#[derive(Debug, Parser)]
#[command(name = "vidya-seed", about = "Load a domain seed file into vidya")]
struct Cli {
    /// Path to the seed JSON file
    path: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_path(vidya_home().join(".env"));
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();
    let cfg = Config::from_env();

    let pool = db::connect(&cfg).await.context("connecting to database")?;
    db::run_migrations(&pool).await.context("running migrations")?;

    let content = std::fs::read_to_string(&cli.path)
        .with_context(|| format!("reading {}", cli.path.display()))?;
    let payload: serde_json::Value =
        serde_json::from_str(&content).context("parsing JSON")?;

    let args = load::LoadArgs { payload };
    let result = load::handle(&pool, args).await.map_err(|e| anyhow::anyhow!("{e}"))?;

    println!("Loaded domain: {}", result.domain);
    println!("  entity_kinds:    {}", result.entity_kinds);
    println!("  relation_kinds:  {}", result.relation_kinds);
    println!("  claim_templates: {}", result.claim_templates);
    println!("  traditions:      {}", result.traditions);
    println!("  sources:         {}", result.sources);
    println!("  entities:        {}", result.entities);
    println!("  claims:          {}", result.claims);
    println!("  assertions:      {}", result.assertions);
    println!("  relations:       {}", result.relations);

    Ok(())
}
