use anyhow::Result;
use hocuspocus_rs_ws::hocuspocus::Server;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let port = 3000;
    info!("Launching Hocuspocus WebSocket server on 0.0.0.0:{}", port);
    Server::start(port).await?;

    Ok(())
}
