use std::collections::HashSet;
use std::sync::atomic::AtomicBool;

use actix_web::web::{Bytes, Data};
use actix_web::{App, HttpServer};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use ethers::providers::{Provider, Ws};
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
    executors_contract_addr: String,

    #[clap(long, value_parser, default_value = "")]
    jobs_contract_addr: String,

    #[clap(long, value_parser, default_value = "")]
    code_contract_addr: String,

    #[clap(long, value_parser, default_value = "/app/id.sec")]
    enclave_signer_file: String,

    #[clap(long, value_parser, default_value = "10")]
    execution_buffer_time: u64,
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

    let web_socket_client = Provider::<Ws>::connect_with_reconnects(cli.web_socket_url, 5)
        .await
        .context("Failed to connect to the common chain websocket provider")?;

    let app_data = Data::new(AppState {
        job_capacity: cgroups.free.len(),
        cgroups: cgroups.into(),
        registered: AtomicBool::new(false),
        common_chain_id: cli.common_chain_id,
        http_rpc_url: cli.http_rpc_url,
        executors_contract_addr: cli
            .executors_contract_addr
            .parse::<Address>()
            .context("Invalid common chain executors contract address")?,
        executors_contract_object: None.into(),
        jobs_contract_addr: cli
            .jobs_contract_addr
            .parse::<Address>()
            .context("Invalid common chain jobs contract address")?,
        jobs_contract_object: None.into(),
        code_contract_addr: cli.code_contract_addr,
        web_socket_client: web_socket_client,
        enclave_signer_key: enclave_signer_key,
        enclave_pub_key: Bytes::new().into(),
        workerd_runtime_path: cli.workerd_runtime_path,
        job_requests_running: HashSet::new().into(),
        execution_buffer_time: cli.execution_buffer_time,
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
