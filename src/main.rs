use actix_web::web::Data;
use actix_web::{App, HttpServer};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use ethers::types::Address;
use k256::ecdsa::SigningKey;
use tokio::fs;

use serverless::cgroups::Cgroups;
use serverless::node_handler::{deregister_enclave, index, inject_key, register_enclave};
use serverless::utils::AppState;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    //  TODO: ADD DEFAULT CONFIGURATIONS
    #[clap(long, value_parser, default_value = "6001")]
    port: u16,

    #[clap(long, value_parser, default_value = "1")]
    common_chain_id: u64,

    #[clap(
        long,
        value_parser,
        default_value = "https://sepolia-rollup.arbitrum.io/rpc"
    )]
    http_rpc_url: String,

    #[clap(long, value_parser, default_value = "")]
    job_management_contract: String,

    #[clap(long, value_parser, default_value = "/app/id.sec")]
    signer: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Args::parse();

    let cgroups = Cgroups::new().context("Failed to construct cgroups")?;
    if cgroups.free.is_empty() {
        return Err(anyhow!("no cgroups found, make sure you have generated cgroups on your system using instructions in the readme"));
    }

    let signer = SigningKey::from_slice(
        fs::read(cli.signer)
            .await
            .context("Failed to read the enclave signer key")?
            .as_slice(),
    )
    .context("Invalid enclave signer key")?;

    let app_data = Data::new(AppState {
        job_capacity: cgroups.free.len(),
        common_chain_id: cli.common_chain_id,
        http_rpc_url: cli.http_rpc_url,
        job_management_contract: cli
            .job_management_contract
            .parse::<Address>()
            .context("Invalid job management contract address")?,
        contract_object: None.into(),
        enclave_signer: signer,
        enclave_pub_key: String::new().into(),
    });

    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .service(index)
            .service(inject_key)
            .service(register_enclave)
            .service(deregister_enclave)
    })
    .bind(("0.0.0.0", cli.port))
    .context(format!("could not bind to port {}", cli.port))?
    .run();

    println!("Node server started on port {}", cli.port);

    server.await?;

    Ok(())
}
