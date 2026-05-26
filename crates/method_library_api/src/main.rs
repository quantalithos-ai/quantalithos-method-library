//! Binary entrypoint for the method-library HTTP service.

use method_library_api::router;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let listen_addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, router()).await?;
    Ok(())
}
