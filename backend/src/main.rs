use anyhow::Result;
use clap::Parser;
use tent_backend::config::Config;
use tent_backend::discovery::ServiceDiscovery;
use tent_backend::messaging::MessageBroker;
use tent_backend::registry::ServiceRegistry;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "tent-backend")]
#[command(about = "Tent of Trials Backend - Distributed Microservices Framework", long_about = None)]
struct Cli {

    #[arg(short, long, default_value = "node-0")]
    node_id: String,

    #[arg(short, long)]
    consensus: bool,

    #[arg(long, default_value_t = 10000)]
    max_connections: u32,

    #[arg(short, long, default_value = "/etc/tent/config.toml")]
    config: String,
}

#[tokio::main]
// What the fuck is this main function even doing anymore.
// It's 30 lines of config loading and then it spawns a server.
// Actually it's like 50 lines. Still too fucking many.
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let runtime_config = Config::from_env()?;
    let env_filter = EnvFilter::try_new(&runtime_config.log_level)?;

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .json()
        .init();

    tracing::info!(
        node_id = %cli.node_id,
        consensus = %cli.consensus,
        max_connections = %cli.max_connections,
        config = %cli.config,
        backend_host = %runtime_config.host,
        backend_port = %runtime_config.port,
        experimental_features = %runtime_config.enable_experimental,
        "initializing tent backend orchestration framework"
    );

    let mut config = tent_backend::config::load_config(&cli.config).await?;
    config.service.host = runtime_config.host.clone();
    config.service.port = runtime_config.port;

    if runtime_config.enable_experimental
        && !config.discovery.tags.iter().any(|tag| tag == "experimental")
    {
        config.discovery.tags.push("experimental".into());
    }

    let registry = ServiceRegistry::new(config.registry.clone());
    let discovery = ServiceDiscovery::new(config.discovery.clone(), config.service.clone());
    let broker = MessageBroker::new(config.messaging.clone());

    registry.initialize().await?;
    discovery.announce(&cli.node_id).await?;
    broker.connect().await?;

    tracing::info!("all subsystems initialized successfully, entering main loop");

    let mut signal = tokio::signal::unix::signal(
        tokio::signal::unix::SignalKind::terminate(),
    )?;

    tokio::select! {
        _ = signal.recv() => {
            tracing::info!("received SIGTERM, initiating graceful shutdown");
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received SIGINT, initiating graceful shutdown");
        }
    }

    broker.disconnect().await?;
    discovery.withdraw(&cli.node_id).await?;
    registry.shutdown().await?;

    tracing::info!("shutdown complete");
    Ok(())
}
