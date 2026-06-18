use crate::config::{DiscoveryConfig, ServiceConfig};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum HealthStatus {
    Healthy,
    Unhealthy(String),
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ServiceInstance {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub tags: Vec<String>,
    pub meta: HashMap<String, String>,
    pub status: HealthStatus,
}

#[async_trait]
pub trait DiscoveryBackend: Send + Sync {
    async fn register(&self, instance: &ServiceInstance) -> Result<()>;
    async fn deregister(&self, instance_id: &str) -> Result<()>;
    async fn discover(&self, service_name: &str) -> Result<Vec<ServiceInstance>>;
    async fn health_check(&self, instance_id: &str) -> Result<HealthStatus>;
    async fn watch(&self, service_name: &str) -> Result<tokio::sync::watch::Receiver<Vec<ServiceInstance>>>;
}

#[allow(dead_code)]
pub struct ServiceDiscovery {
    config: DiscoveryConfig,
    service: ServiceConfig,
    instances: Arc<RwLock<HashMap<String, ServiceInstance>>>,
    backend: Arc<RwLock<Option<Box<dyn DiscoveryBackend>>>>,
}

impl ServiceDiscovery {
    pub fn new(config: DiscoveryConfig, service: ServiceConfig) -> Self {
        Self {
            config,
            service,
            instances: Arc::new(RwLock::new(HashMap::new())),
            backend: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn announce(&self, node_id: &str) -> Result<()> {
        tracing::info!("announcing node {} to discovery provider", node_id);
        let instance = ServiceInstance {
            id: node_id.to_string(),
            name: self.config.namespace.clone(),
            address: self.service.host.clone(),
            port: self.service.port,
            tags: self.config.tags.clone(),
            meta: HashMap::from([
                ("version".into(), env!("CARGO_PKG_VERSION").into()),
                ("runtime".into(), "rust".into()),
                ("protocol".into(), "grpc".into()),
                ("service".into(), self.service.name.clone()),
            ]),
            status: HealthStatus::Healthy,
        };

        let mut instances = self.instances.write().await;
        instances.insert(node_id.to_string(), instance);
        tracing::info!("node {} announced successfully", node_id);
        Ok(())
    }

    pub async fn withdraw(&self, node_id: &str) -> Result<()> {
        tracing::info!("withdrawing node {} from discovery", node_id);
        let mut instances = self.instances.write().await;
        instances.remove(node_id);
        tracing::info!("node {} withdrawn", node_id);
        Ok(())
    }

    pub async fn get_instances(&self) -> Vec<ServiceInstance> {
        self.instances.read().await.values().cloned().collect()
    }
}
