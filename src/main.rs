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

    #[clap(long, value_parser, default_value = "./runtime/")]
    workerd_runtime_path: String,

    #[clap(long, value_parser, default_value = "1")]
    common_chain_id: u64,

    #[clap(long, value_parser, default_value = "")]
    http_rpc_url: String,

    #[clap(long, value_parser, default_value = "")]
    web_socket_url: String,

    #[clap(long, value_parser, default_value = "")]
    job_management_contract: String,

    #[clap(long, value_parser, default_value = "")]
    user_code_contract: String,

    #[clap(long, value_parser, default_value = "/app/id.sec")]
    enclave_signer_file: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Args::parse();

    let cgroups = Cgroups::new().context("Failed to retrieve cgroups")?;
    if cgroups.free.is_empty() {
        return Err(anyhow!("No cgroups found, make sure you have generated cgroups on your system using the instructions in the readme"));
    }

    let enclave_signer_key = SigningKey::from_slice(
        fs::read(cli.enclave_signer_file)
            .await
            .context("Failed to read the enclave signer key")?
            .as_slice(),
    )
    .context("Invalid enclave signer key")?;

    let app_data = Data::new(AppState {
        job_capacity: cgroups.free.len(),
        cgroups: cgroups.into(),
        common_chain_id: cli.common_chain_id,
        web_socket_url: cli.web_socket_url,
        job_management_contract: cli
            .job_management_contract
            .parse::<Address>()
            .context("Invalid job management contract address")?,
        contract_object: None.into(),
        user_code_contract: cli.user_code_contract,
        enclave_signer_key: enclave_signer_key,
        enclave_pub_key: String::new().into(),
        workerd_runtime_path: cli.workerd_runtime_path,
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
