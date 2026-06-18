use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use std::path::Path;

const DEFAULT_BACKEND_HOST: &str = "0.0.0.0";
const DEFAULT_BACKEND_PORT: u16 = 8080;
const DEFAULT_LOG_LEVEL: &str = "info";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub enable_experimental: bool,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("{name} must be a valid TCP port between 1 and 65535, got {value:?}")]
    InvalidPort { name: &'static str, value: String },

    #[error("{name} must be a boolean value (true/false, 1/0, yes/no, on/off), got {value:?}")]
    InvalidBool { name: &'static str, value: String },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let host = lookup("TOT_BACKEND_HOST").unwrap_or_else(|| DEFAULT_BACKEND_HOST.into());
        let port = match lookup("TOT_BACKEND_PORT") {
            Some(value) => parse_port("TOT_BACKEND_PORT", &value)?,
            None => DEFAULT_BACKEND_PORT,
        };
        let log_level = lookup("TOT_LOG_LEVEL").unwrap_or_else(|| DEFAULT_LOG_LEVEL.into());
        let enable_experimental = match lookup("TOT_ENABLE_EXPERIMENTAL") {
            Some(value) => parse_bool("TOT_ENABLE_EXPERIMENTAL", &value)?,
            None => false,
        };

        Ok(Self {
            host,
            port,
            log_level,
            enable_experimental,
        })
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: DEFAULT_BACKEND_HOST.into(),
            port: DEFAULT_BACKEND_PORT,
            log_level: DEFAULT_LOG_LEVEL.into(),
            enable_experimental: false,
        }
    }
}

fn parse_port(name: &'static str, value: &str) -> Result<u16, ConfigError> {
    match value.parse::<u16>() {
        Ok(port) if port > 0 => Ok(port),
        _ => Err(ConfigError::InvalidPort {
            name,
            value: value.into(),
        }),
    }
}

fn parse_bool(name: &'static str, value: &str) -> Result<bool, ConfigError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidBool {
            name,
            value: value.into(),
        }),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub version: String,
    pub host: String,
    pub port: u16,
    pub tls_enabled: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    pub backend: String,
    pub endpoints: Vec<String>,
    pub heartbeat_interval_ms: u64,
    pub ttl_seconds: u64,
    pub replication_factor: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryConfig {
    pub provider: String,
    pub namespace: String,
    pub tags: Vec<String>,
    pub health_check_path: String,
    pub health_check_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagingConfig {
    pub broker_type: String,
    pub uris: Vec<String>,
    pub consumer_group: String,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub batch_size: u32,
    pub compression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    pub service: ServiceConfig,
    pub registry: RegistryConfig,
    pub discovery: DiscoveryConfig,
    pub messaging: MessagingConfig,
}

impl Default for RootConfig {
    fn default() -> Self {
        Self {
            service: ServiceConfig {
                name: "tent-backend".into(),
                version: "0.1.0".into(),
                host: DEFAULT_BACKEND_HOST.into(),
                port: DEFAULT_BACKEND_PORT,
                tls_enabled: false,
                tls_cert_path: None,
                tls_key_path: None,
            },
            registry: RegistryConfig {
                backend: "etcd".into(),
                endpoints: vec!["localhost:2379".into()],
                heartbeat_interval_ms: 5000,
                ttl_seconds: 30,
                replication_factor: 3,
            },
            discovery: DiscoveryConfig {
                provider: "consul".into(),
                namespace: "tent".into(),
                tags: vec!["microservice".into(), "orchestration".into()],
                health_check_path: "/health".into(),
                health_check_interval_ms: 10000,
            },
            messaging: MessagingConfig {
                broker_type: "kafka".into(),
                uris: vec!["localhost:9092".into()],
                consumer_group: "tent-consumers".into(),
                max_retries: 3,
                retry_backoff_ms: 1000,
                batch_size: 500,
                compression: "snappy".into(),
            },
        }
    }
}

pub async fn load_config(path: &str) -> Result<RootConfig> {
    let path = Path::new(path);
    if path.exists() {
        let contents = tokio::fs::read_to_string(path).await?;
        let config: RootConfig = toml::from_str(&contents)?;
        tracing::info!("configuration loaded from {}", path.display());
        Ok(config)
    } else {
        tracing::warn!(
            "config file {} not found, using defaults",
            path.display()
        );
        Ok(RootConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError};
    use std::collections::HashMap;

    fn config_from(entries: &[(&str, &str)]) -> Result<Config, ConfigError> {
        let values: HashMap<&str, String> = entries
            .iter()
            .map(|(key, value)| (*key, (*value).to_string()))
            .collect();

        Config::from_lookup(|name| values.get(name).cloned())
    }

    #[test]
    fn from_env_defaults_are_safe_for_local_development() {
        assert_eq!(
            config_from(&[]).unwrap(),
            Config {
                host: "0.0.0.0".into(),
                port: 8080,
                log_level: "info".into(),
                enable_experimental: false,
            }
        );
    }

    #[test]
    fn from_env_accepts_valid_overrides() {
        assert_eq!(
            config_from(&[
                ("TOT_BACKEND_HOST", "127.0.0.1"),
                ("TOT_BACKEND_PORT", "18080"),
                ("TOT_LOG_LEVEL", "debug"),
                ("TOT_ENABLE_EXPERIMENTAL", "yes"),
            ])
            .unwrap(),
            Config {
                host: "127.0.0.1".into(),
                port: 18080,
                log_level: "debug".into(),
                enable_experimental: true,
            }
        );
    }

    #[test]
    fn from_env_rejects_invalid_ports() {
        assert_eq!(
            config_from(&[("TOT_BACKEND_PORT", "70000")]).unwrap_err(),
            ConfigError::InvalidPort {
                name: "TOT_BACKEND_PORT",
                value: "70000".into(),
            }
        );

        assert_eq!(
            config_from(&[("TOT_BACKEND_PORT", "0")]).unwrap_err(),
            ConfigError::InvalidPort {
                name: "TOT_BACKEND_PORT",
                value: "0".into(),
            }
        );
    }

    #[test]
    fn from_env_rejects_invalid_booleans() {
        assert_eq!(
            config_from(&[("TOT_ENABLE_EXPERIMENTAL", "maybe")]).unwrap_err(),
            ConfigError::InvalidBool {
                name: "TOT_ENABLE_EXPERIMENTAL",
                value: "maybe".into(),
            }
        );
    }
}
