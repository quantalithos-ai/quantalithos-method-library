//! Bootstrap binary reserved for the future operations job runner.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tracing::info!("operations job bootstrap");
    Ok(())
}
