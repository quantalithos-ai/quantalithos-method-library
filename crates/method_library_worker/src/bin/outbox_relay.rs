//! Bootstrap binary reserved for the future outbox relay worker.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("outbox relay bootstrap");
    Ok(())
}
