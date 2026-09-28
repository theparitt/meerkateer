use std::time::Duration;

use meerkateer_sdk::{HeartbeatStatus, Meerkateer};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Meerkateer::from_env()?;
    loop {
        if let Err(error) = client
            .heartbeat(HeartbeatStatus::Ok, Some("process responsive".to_owned()))
            .await
        {
            eprintln!("Meerkateer heartbeat failed: {error}");
        }
        sleep(Duration::from_secs(30)).await;
    }
}
