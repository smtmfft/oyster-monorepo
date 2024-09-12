use alloy::rpc::types::Log;
use anyhow::Result;
use tracing::info;

pub fn handle_log(log: Log) -> Result<()> {
    info!("Received: {:?}", log);
    Ok(())
}
