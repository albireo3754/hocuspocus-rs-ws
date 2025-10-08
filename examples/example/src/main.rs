use anyhow::Result;
use hocuspocus_rs_ws::hocuspocus::Server;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    let port = 3000;
    info!("Launching Hocuspocus WebSocket server on 0.0.0.0:{}", port);
    Server::start(port).await?;

    Ok(())
}
