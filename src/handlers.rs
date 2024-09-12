use alloy::rpc::types::Log;
use anyhow::Result;

pub fn handle_log(log: Log) -> Result<()> {
    println!("Received: {:?}", log);
    Ok(())
}
