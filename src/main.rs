use std::collections::HashSet;

use actix_web::web::Data;
use actix_web::{App, HttpServer};
use anyhow::{anyhow, Context, Result};
use clap::Parser;
use ethers::providers::{Provider, Ws};
use ethers::types::Address;
use k256::ecdsa::SigningKey;
use tokio::fs;

use serverless::cgroups::Cgroups;
use serverless::node_handler::{export, index, inject};
use serverless::utils::{pub_key_to_address, AppState};

// EXECUTOR CONFIGURATION PARAMETERS
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

    #[clap(long, value_parser, default_value = "/app/id.pub")]
    enclave_pub_key_file: String,

    #[clap(long, value_parser, default_value = "10")]
    execution_buffer_time: u64, // time in seconds

    #[clap(long, value_parser, default_value = "1")]
    num_selected_executors: u8,
}

#[tokio::main]
// Program to run the executor
async fn main() -> Result<()> {
    let cli = Args::parse();

    // Initialize the 'cgroups' available inside the enclave to execute user code
    let cgroups = Cgroups::new().context("Failed to retrieve cgroups")?;
    if cgroups.free.is_empty() {
        return Err(anyhow!("No cgroups found, make sure you have generated cgroups on your system using the instructions in the readme"));
    }

    // Read the 'secp256k1' private and public key of the enclave instance generated by keygen
    let enclave_signer_key = SigningKey::from_slice(
        fs::read(cli.enclave_signer_file)
            .await
            .context("Failed to read the enclave signer key")?
            .as_slice(),
    )
    .context("Invalid enclave signer key")?;

    let enclave_pub_key = fs::read(cli.enclave_pub_key_file)
        .await
        .context("Failed to read the enclave public key")?;

    let enclave_address =
        pub_key_to_address(&enclave_pub_key).context("Failed to calculate enclave address")?;

    // Connect to the rpc web socket provider
    let web_socket_client = Provider::<Ws>::connect_with_reconnects(cli.web_socket_url, 5)
        .await
        .context("Failed to connect to the common chain websocket provider")?;

    // Initialize App data that will be shared across multiple threads and tasks
    let app_data = Data::new(AppState {
        job_capacity: cgroups.free.len(),
        cgroups: cgroups.into(),
        registered: false.into(),
        register_listener_active: false.into(),
        num_selected_executors: cli.num_selected_executors,
        common_chain_id: cli.common_chain_id,
        http_rpc_url: cli.http_rpc_url,
        http_rpc_client: None.into(),
        web_socket_client: web_socket_client,
        executors_contract_addr: cli
            .executors_contract_addr
            .parse::<Address>()
            .context("Invalid common chain executors contract address")?,
        jobs_contract_addr: cli
            .jobs_contract_addr
            .parse::<Address>()
            .context("Invalid common chain jobs contract address")?,
        code_contract_addr: cli.code_contract_addr,
        enclave_owner: None.into(),
        enclave_address: enclave_address,
        enclave_signer: enclave_signer_key,
        workerd_runtime_path: cli.workerd_runtime_path,
        job_requests_running: HashSet::new().into(),
        execution_buffer_time: cli.execution_buffer_time,
    });

    // Start actix server to expose the executor outside the enclave
    let server = HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .service(index)
            .service(inject)
            .service(export)
    })
    .bind(("0.0.0.0", cli.port))
    .context(format!("could not bind to port {}", cli.port))?
    .run();

    println!("Node server started on port {}", cli.port);

    server.await?;

    Ok(())
}
