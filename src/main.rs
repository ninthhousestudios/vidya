use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use rmcp::{ServiceExt, transport::stdio};
use tracing_subscriber::EnvFilter;
use vidya_core::{KnowledgeStore, ProvenanceFilter, QueryMode, ResolvedQuery};
use vidya_core::resolve;

use vidya::{
    config::{Config, vidya_home},
    format,
    mcp::VidyaServer,
};

#[derive(Debug, Parser)]
#[command(
    name = "vidya",
    version,
    about = "Structured knowledge graph with reasoning — MCP server",
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start the MCP server (stdio by default, --http for Streamable HTTP).
    Serve {
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
    },
    /// Install systemd user service for vidya.
    InstallServices {
        /// Enable and start the service after installing.
        #[arg(long)]
        enable: bool,
    },
    /// Load a domain from a Turtle (.ttl) file.
    Load {
        /// Domain name (e.g. "jyotish").
        domain: String,
        /// Path to .ttl file.
        file: PathBuf,
    },
    /// List loaded domains.
    Domains,
    /// Describe an entity — all properties and provenance.
    Describe {
        /// Domain name (or set VIDYA_DOMAIN).
        #[arg(short, long, env = "VIDYA_DOMAIN")]
        domain: String,
        /// Subject short name (e.g. "surya").
        subject: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        tradition: Option<String>,
        #[arg(long)]
        pramana: Option<String>,
    },
    /// Search entities by type.
    Search {
        /// Domain name (or set VIDYA_DOMAIN).
        #[arg(short, long, env = "VIDYA_DOMAIN")]
        domain: String,
        /// Kind short name (e.g. "Graha", "Rashi").
        kind: String,
        /// Attribute filter as key=value (repeatable, e.g. -f element=fire).
        #[arg(short, long = "filter", value_name = "KEY=VALUE")]
        filters: Vec<String>,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        tradition: Option<String>,
        #[arg(long)]
        pramana: Option<String>,
    },
    /// Walk relationships from an entity.
    Traverse {
        /// Domain name (or set VIDYA_DOMAIN).
        #[arg(short, long, env = "VIDYA_DOMAIN")]
        domain: String,
        /// Subject short name.
        subject: String,
        /// Predicate short name (e.g. "naturalFriend").
        predicate: String,
        /// Max traversal depth (default 1, max 10).
        #[arg(long, default_value_t = 1)]
        depth: u32,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        tradition: Option<String>,
        #[arg(long)]
        pramana: Option<String>,
    },
    /// List vocabulary tokens the NL resolver knows for a domain.
    Vocab {
        /// Domain name (or set VIDYA_DOMAIN).
        #[arg(short, long, env = "VIDYA_DOMAIN")]
        domain: String,
        #[arg(long)]
        json: bool,
    },
    /// Show epistemological metadata for a specific triple.
    Provenance {
        /// Domain name (or set VIDYA_DOMAIN).
        #[arg(short, long, env = "VIDYA_DOMAIN")]
        domain: String,
        /// Subject short name.
        subject: String,
        /// Predicate short name.
        predicate: String,
        /// Object short name or literal.
        object: String,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        tradition: Option<String>,
        #[arg(long)]
        pramana: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::from_path(vidya_home().join(".env"));
    let _ = dotenvy::dotenv();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve {
            http,
            http_addr,
            http_port,
            auth_token_file,
        } => {
            let cfg = Config::from_env();

            tracing_subscriber::fmt()
                .with_env_filter(
                    EnvFilter::try_new(&cfg.log_level)
                        .unwrap_or_else(|_| EnvFilter::new("info")),
                )
                .with_writer(std::io::stderr)
                .init();

            tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting vidya");

            std::fs::create_dir_all(&cfg.store_path).with_context(|| {
                format!("creating store directory {}", cfg.store_path.display())
            })?;

            let store = Arc::new(
                KnowledgeStore::open(&cfg.store_path)
                    .map_err(|e| anyhow::anyhow!("{e}"))
                    .context("opening knowledge store")?,
            );

            if http {
                serve_http(http_addr, http_port, auth_token_file, cfg, store).await
            } else {
                serve_stdio(store).await
            }
        }
        Commands::InstallServices { enable } => cmd_install_services(enable),
        Commands::Load { domain, file } => cmd_load(&domain, &file),
        Commands::Domains => cmd_domains(),
        Commands::Vocab { domain, json } => cmd_vocab(&domain, json),
        Commands::Describe {
            domain,
            subject,
            json,
            tradition,
            pramana,
        } => cmd_describe(&domain, &subject, json, tradition, pramana),
        Commands::Search {
            domain,
            kind,
            filters,
            json,
            tradition,
            pramana,
        } => cmd_search(&domain, &kind, &filters, json, tradition, pramana),
        Commands::Traverse {
            domain,
            subject,
            predicate,
            depth,
            json,
            tradition,
            pramana,
        } => cmd_traverse(&domain, &subject, &predicate, depth, json, tradition, pramana),
        Commands::Provenance {
            domain,
            subject,
            predicate,
            object,
            json,
            tradition,
            pramana,
        } => cmd_provenance(&domain, &subject, &predicate, &object, json, tradition, pramana),
    }
}

fn open_store_ro() -> Result<KnowledgeStore> {
    let cfg = Config::from_env();
    KnowledgeStore::open_read_only(&cfg.store_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("opening knowledge store (read-only)")
}

fn open_store_rw() -> Result<KnowledgeStore> {
    let cfg = Config::from_env();
    std::fs::create_dir_all(&cfg.store_path)
        .with_context(|| format!("creating store directory {}", cfg.store_path.display()))?;
    KnowledgeStore::open(&cfg.store_path)
        .map_err(|e| anyhow::anyhow!("{e}"))
        .context("opening knowledge store")
}

fn prov_filter(domain: &str, tradition: Option<String>, pramana: Option<String>) -> ProvenanceFilter {
    ProvenanceFilter {
        tradition: tradition.map(|t| vidya_core::ontology::resolve_iri(&t, domain)),
        pramana: pramana.map(|p| vidya_core::ontology::resolve_iri(&p, domain)),
    }
}

fn cmd_load(domain: &str, file: &PathBuf) -> Result<()> {
    let store = open_store_rw()?;
    store
        .load_domain_from_file(domain, file)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let count = store.triple_count().map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("loaded {domain} ({count} triples total)");
    Ok(())
}

fn cmd_domains() -> Result<()> {
    let store = open_store_ro()?;
    let domains = store.domains();
    if domains.is_empty() {
        println!("(no domains loaded)");
    } else {
        for d in domains {
            println!("{d}");
        }
    }
    Ok(())
}

fn cmd_vocab(domain: &str, json: bool) -> Result<()> {
    let store = open_store_ro()?;
    let result = vidya_core::query::vocab(&store, domain);
    output(&result, json, format::fmt_vocab)
}

fn cmd_describe(
    domain: &str,
    subject: &str,
    json: bool,
    tradition: Option<String>,
    pramana: Option<String>,
) -> Result<()> {
    let store = open_store_ro()?;
    let pf = prov_filter(domain, tradition, pramana);

    match store.describe(domain, subject, &pf) {
        Ok(result) => return output(&result, json, format::fmt_describe),
        Err(vidya_core::VidyaError::NotFound(_)) => {}
        Err(e) => return Err(anyhow::anyhow!("{e}")),
    }

    let report = nl_resolve(&store, domain, QueryMode::Describe, subject)?;
    match report.query {
        ResolvedQuery::Describe { ref subject_iri } => {
            let result = store
                .describe(domain, &iri_local_name(subject_iri), &pf)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            output(&result, json, format::fmt_describe)
        }
        _ => anyhow::bail!("NL resolution returned unexpected query mode"),
    }
}

fn cmd_search(
    domain: &str,
    kind: &str,
    filters: &[String],
    json: bool,
    tradition: Option<String>,
    pramana: Option<String>,
) -> Result<()> {
    let store = open_store_ro()?;
    let pf = prov_filter(domain, tradition, pramana);

    let parsed: Vec<(String, String)> = filters
        .iter()
        .filter_map(|f| {
            let (k, v) = f.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .collect();

    match store.search(domain, kind, &parsed, &pf) {
        Ok(result) => return output(&result, json, format::fmt_search),
        Err(vidya_core::VidyaError::NotFound(_) | vidya_core::VidyaError::InvalidArgument(_)) => {}
        Err(e) => return Err(anyhow::anyhow!("{e}")),
    }

    let mut nl_input = kind.to_string();
    for f in filters {
        if let Some((_, v)) = f.split_once('=') {
            nl_input.push(' ');
            nl_input.push_str(v);
        } else {
            nl_input.push(' ');
            nl_input.push_str(f);
        }
    }

    let report = nl_resolve(&store, domain, QueryMode::Search, &nl_input)?;
    match report.query {
        ResolvedQuery::Search {
            ref type_iri,
            ref filters,
        } => {
            let result = store
                .search(domain, &iri_local_name(type_iri), filters, &pf)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            output(&result, json, format::fmt_search)
        }
        _ => anyhow::bail!("NL resolution returned unexpected query mode"),
    }
}

fn cmd_traverse(
    domain: &str,
    subject: &str,
    predicate: &str,
    depth: u32,
    json: bool,
    tradition: Option<String>,
    pramana: Option<String>,
) -> Result<()> {
    let store = open_store_ro()?;
    let pf = prov_filter(domain, tradition, pramana);

    match store.traverse(domain, subject, predicate, depth, &pf) {
        Ok(result) => return output(&result, json, format::fmt_traverse),
        Err(vidya_core::VidyaError::NotFound(_)) => {}
        Err(e) => return Err(anyhow::anyhow!("{e}")),
    }

    let nl_input = format!("{subject} {predicate}");
    let report = nl_resolve(&store, domain, QueryMode::Traverse, &nl_input)?;
    match report.query {
        ResolvedQuery::Traverse {
            ref subject_iri,
            ref predicate_iri,
        } => {
            let result = store
                .traverse(domain, &iri_local_name(subject_iri), &iri_local_name(predicate_iri), depth, &pf)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            output(&result, json, format::fmt_traverse)
        }
        _ => anyhow::bail!("NL resolution returned unexpected query mode"),
    }
}

fn cmd_provenance(
    domain: &str,
    subject: &str,
    predicate: &str,
    object: &str,
    json: bool,
    tradition: Option<String>,
    pramana: Option<String>,
) -> Result<()> {
    let store = open_store_ro()?;
    let pf = prov_filter(domain, tradition, pramana);

    match store.provenance(domain, subject, predicate, object, &pf) {
        Ok(result) => return output(&result, json, format::fmt_provenance),
        Err(vidya_core::VidyaError::NotFound(_)) => {}
        Err(e) => return Err(anyhow::anyhow!("{e}")),
    }

    let nl_input = format!("{subject} {predicate} {object}");
    let report = nl_resolve(&store, domain, QueryMode::Provenance, &nl_input)?;
    match report.query {
        ResolvedQuery::Provenance {
            ref subject_iri,
            ref predicate_iri,
            ref object,
            object_is_literal,
        } => {
            let obj_str = if object_is_literal {
                object.clone()
            } else {
                iri_local_name(object)
            };
            let result = store
                .provenance(domain, &iri_local_name(subject_iri), &iri_local_name(predicate_iri), &obj_str, &pf)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            output(&result, json, format::fmt_provenance)
        }
        _ => anyhow::bail!("NL resolution returned unexpected query mode"),
    }
}

fn output<T: serde::Serialize>(result: &T, json: bool, fmt: fn(&T) -> String) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
    } else {
        print!("{}", fmt(result));
    }
    Ok(())
}

fn nl_resolve(
    store: &KnowledgeStore,
    domain: &str,
    mode: QueryMode,
    input: &str,
) -> Result<vidya_core::ResolutionReport> {
    let ctx = store.resolve_context(domain);
    let report = resolve::resolve(mode, input, &ctx.vocab, Some(&ctx.vsa), domain)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    eprintln!("  resolved: {}", report.resolution_details.join(", "));
    if !report.unknown_tokens.is_empty() {
        eprintln!("  unrecognized: {}", report.unknown_tokens.join(", "));
    }
    Ok(report)
}

fn iri_local_name(iri: &str) -> String {
    iri.rsplit_once('/')
        .map(|(_, local)| local.to_string())
        .unwrap_or_else(|| iri.to_string())
}

async fn serve_stdio(store: Arc<KnowledgeStore>) -> Result<()> {
    let server = VidyaServer::new(store);
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

async fn serve_http(
    http_addr: Option<String>,
    http_port: Option<u16>,
    auth_token_file: Option<PathBuf>,
    cfg: Config,
    store: Arc<KnowledgeStore>,
) -> Result<()> {
    use axum::routing::any_service;
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager,
        tower::{StreamableHttpServerConfig, StreamableHttpService},
    };
    use tokio_util::sync::CancellationToken;
    use tower_http::validate_request::ValidateRequestHeaderLayer;

    let token_path = auth_token_file.ok_or_else(|| {
        anyhow::anyhow!("--auth-token-file is required when running in --http mode")
    })?;
    let bearer_token = std::fs::read_to_string(&token_path)
        .with_context(|| format!("reading auth token from {}", token_path.display()))?
        .trim()
        .to_string();

    let cancel = CancellationToken::new();

    let http_addr = http_addr.unwrap_or_else(|| cfg.http_addr.clone());
    let http_port = http_port.unwrap_or(cfg.http_port);

    let config = StreamableHttpServerConfig::default()
        .with_cancellation_token(cancel.clone())
        .with_allowed_hosts(vec![
            "localhost".to_string(),
            "127.0.0.1".to_string(),
            "::1".to_string(),
        ]);

    let mut session_manager = LocalSessionManager::default();
    session_manager.session_config.keep_alive = None;
    let session_manager = Arc::new(session_manager);

    let store_clone = store.clone();
    let mcp_service = StreamableHttpService::new(
        move || Ok(VidyaServer::new(store_clone.clone())),
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

    let health_store = store.clone();
    #[allow(deprecated)]
    let app = axum::Router::new()
        .route(
            "/health",
            axum::routing::get(move || async move {
                let ok = health_store.triple_count().is_ok();
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

fn cmd_install_services(enable: bool) -> Result<()> {
    let home = std::env::var("HOME").map_err(|_| anyhow::anyhow!("HOME not set"))?;
    let vidya_bin = format!("{home}/.cargo/bin/vidya");

    let unit = service_unit_content(&vidya_bin);

    let service_dir = format!("{home}/.config/systemd/user");
    std::fs::create_dir_all(&service_dir)?;

    let service_path = format!("{service_dir}/vidya.service");
    std::fs::write(&service_path, unit)?;
    println!("Wrote {service_path}");

    let status = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status()?;
    if !status.success() {
        anyhow::bail!("systemctl --user daemon-reload failed");
    }
    println!("Reloaded systemd user daemon");

    if enable {
        let status = std::process::Command::new("systemctl")
            .args(["--user", "enable", "--now", "vidya.service"])
            .status()?;
        if !status.success() {
            anyhow::bail!("systemctl --user enable --now vidya.service failed");
        }
        println!("Enabled and started vidya.service");
    }

    Ok(())
}

fn service_unit_content(vidya_bin: &str) -> String {
    format!(
        r#"[Unit]
Description=vidya MCP server (HTTP)

[Service]
ExecStart={vidya_bin} serve --http --auth-token-file %h/.vidya/auth-token
Restart=on-failure
RestartSec=3

[Install]
WantedBy=default.target
"#
    )
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
