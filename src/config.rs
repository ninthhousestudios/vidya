use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub store_path: PathBuf,
    pub log_level: String,
    pub http_addr: String,
    pub http_port: u16,
}

impl Config {
    pub fn from_env() -> Self {
        let home = vidya_home();
        Self {
            store_path: env::var("VIDYA_STORE_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| home.join("store")),
            log_level: env_or("VIDYA_LOG_LEVEL", "info"),
            http_addr: env_or("VIDYA_HTTP_ADDR", "127.0.0.1"),
            http_port: env_or("VIDYA_HTTP_PORT", "3300").parse().unwrap_or(3300),
        }
    }
}

pub fn vidya_home() -> PathBuf {
    env::var("VIDYA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| dirs().join(".vidya"))
}

fn dirs() -> PathBuf {
    env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
