use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;

use vidya::{
    config::{Config, vidya_home},
    db,
    mcp::VidyaServer,
};

#[derive(Debug, Parser)]
#[command(name = "vidya", version, about = "Structured knowledge graph with reasoning — MCP server")]
struct Cli {
    /// Run as a Streamable HTTP server instead of stdio.
    #[arg(long)]
    http: bool,

    /// HTTP listen address (used with --http).
    #[arg(long)]
    http_addr: Option<String>,

    /// HTTP listen port (used with --http).
    #[arg(long)]
    http_port: Option<u16>,

    /// Path to a file containing the bearer token for HTTP auth.
    #[arg(long)]
    auth_token_file: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_path(vidya_home().join(".env"));
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();
    let cfg = Config::from_env();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_new(&cfg.log_level).unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting vidya");

    let pool = db::connect(&cfg).await.context("connecting to database")?;
    db::run_migrations(&pool)
        .await
        .context("running migrations")?;

    if cli.http {
        serve_http(cli, cfg, pool).await
    } else {
        serve_stdio(pool).await
    }
}

async fn serve_stdio(pool: sqlx::PgPool) -> Result<()> {
    let server = VidyaServer::new(pool);
    let (stdin, stdout) = stdio();

    let service = server
        .serve((stdin, stdout))
        .await
        .context("starting MCP service over stdio")?;

    tokio::select! {
        res = service.waiting() => {
            res.context("MCP service terminated with error")?;
        }
        _ = shutdown_signal() => {
            tracing::info!("shutdown signal received; exiting");
        }
    }

    Ok(())
}

async fn serve_http(cli: Cli, cfg: Config, pool: sqlx::PgPool) -> Result<()> {
    use axum::routing::any_service;
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager,
        tower::{StreamableHttpServerConfig, StreamableHttpService},
    };
    use tokio_util::sync::CancellationToken;
    use tower_http::validate_request::ValidateRequestHeaderLayer;

    let token_path = cli.auth_token_file.ok_or_else(|| {
        anyhow::anyhow!("--auth-token-file is required when running in --http mode")
    })?;
    let bearer_token = std::fs::read_to_string(&token_path)
        .with_context(|| format!("reading auth token from {}", token_path.display()))?
        .trim()
        .to_string();

    let cancel = CancellationToken::new();

    let http_addr = cli.http_addr.unwrap_or_else(|| cfg.http_addr.clone());
    let http_port = cli.http_port.unwrap_or(cfg.http_port);

    let config = StreamableHttpServerConfig::default()
        .with_cancellation_token(cancel.clone())
        .with_allowed_hosts(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
        ]);

    let session_manager = Arc::new(LocalSessionManager::default());

    let pool_clone = pool.clone();
    let mcp_service = StreamableHttpService::new(
        move || Ok(VidyaServer::new(pool_clone.clone())),
        session_manager,
        config,
    );

    let normalize_accept = axum::middleware::from_fn(
        |mut req: axum::http::Request<axum::body::Body>, next: axum::middleware::Next| async move {
            use axum::http::header::ACCEPT;
            let needs_fix = req
                .headers()
                .get(ACCEPT)
                .and_then(|v| v.to_str().ok())
                .is_none_or(|v| {
                    !v.contains("application/json") || !v.contains("text/event-stream")
                });
            if needs_fix {
                req.headers_mut().insert(
                    ACCEPT,
                    "application/json, text/event-stream".parse().unwrap(),
                );
            }
            next.run(req).await
        },
    );

    let authed = axum::Router::new()
        .route("/mcp", any_service(mcp_service))
        .layer(normalize_accept)
        .layer(ValidateRequestHeaderLayer::bearer(&bearer_token));

    let health_pool = pool.clone();
    #[allow(deprecated)]
    let app = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(move || async move {
                let ok = sqlx::query_scalar::<_, i32>("SELECT 1")
                    .fetch_one(&health_pool)
                    .await
                    .is_ok();
                axum::Json(serde_json::json!({
                    "status": if ok { "ok" } else { "degraded" }
                }))
            }),
        )
        .merge(authed);

    let addr = format!("{http_addr}:{http_port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bind {addr}: {e}"))?;
    tracing::info!(%addr, "vidya HTTP server listening");

    let cancel_for_shutdown = cancel.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            cancel_for_shutdown.cancel();
        })
        .await
        .context("HTTP server exited with error")?;

    Ok(())
}

#[cfg(unix)]
async fn shutdown_signal() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut int = signal(SignalKind::interrupt()).expect("install SIGINT handler");
    let mut term = signal(SignalKind::terminate()).expect("install SIGTERM handler");
    tokio::select! {
        _ = int.recv() => {}
        _ = term.recv() => {}
    }
}

#[cfg(not(unix))]
async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
