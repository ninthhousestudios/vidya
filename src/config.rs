use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub log_level: String,
    pub db_max_connections: u32,
    pub db_acquire_timeout: Duration,
    pub db_idle_timeout: Duration,
    pub http_addr: String,
    pub http_port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            database_url: env_or("DATABASE_URL", "postgresql://localhost/vidya"),
            log_level: env_or("VIDYA_LOG_LEVEL", "info"),
            db_max_connections: env_or("VIDYA_DB_MAX_CONNECTIONS", "5").parse().unwrap_or(5),
            db_acquire_timeout: Duration::from_secs(
                env_or("VIDYA_DB_ACQUIRE_TIMEOUT_SECS", "5").parse().unwrap_or(5),
            ),
            db_idle_timeout: Duration::from_secs(
                env_or("VIDYA_DB_IDLE_TIMEOUT_SECS", "300").parse().unwrap_or(300),
            ),
            http_addr: env_or("VIDYA_HTTP_ADDR", "127.0.0.1"),
            http_port: env_or("VIDYA_HTTP_PORT", "3200").parse().unwrap_or(3200),
        }
    }
}

pub fn vidya_home() -> PathBuf {
    env::var("VIDYA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs().join(".vidya")
        })
}

fn dirs() -> PathBuf {
    env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/tmp"))
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
