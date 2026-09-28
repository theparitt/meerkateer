use meerkateer_sdk::{EventLevel, Meerkateer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    Meerkateer::from_env()?
        .event(
            "rust_sdk_connected",
            EventLevel::Info,
            Some("Rust SDK integration ready".to_owned()),
            1,
        )
        .await?;
    Ok(())
}
