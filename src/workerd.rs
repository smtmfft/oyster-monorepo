use std::process::Child;
use std::time::{Duration, Instant};

use actix_web::web::Bytes;
use reqwest::redirect::Policy;
use reqwest::Client;
use serde_json::{json, Value};
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::sleep;

use crate::cgroups::Cgroups;

#[derive(Error, Debug)]
pub enum ServerlessError {
    #[error("Failed to retrieve code transaction data")]
    TxDataRetrieve(#[source] reqwest::Error),
    #[error("Tx not found on the desired rpc")]
    TxNotFound,
    #[error("to field of transaction data is not an address")]
    InvalidTxToType,
    #[error("to address {0} does not match expected {1}")]
    InvalidTxToValue(String, String),
    #[error("calldata field of transaction data is not a string")]
    InvalidTxCalldataType,
    #[error("calldata field is not a valid hex string")]
    BadCalldata(#[from] hex::FromHexError),
    #[error("Failed to create/write to the code file")]
    CodeFileCreate(#[source] tokio::io::Error),
    #[error("Failed to create/write to the config file")]
    ConfigFileCreate(#[source] tokio::io::Error),
    #[error("Failed to execute cgroups")]
    Execute,
    #[error("Failed to send request to the workerd port")]
    WorkerRequestError(#[source] reqwest::Error),
    #[error("Failed to delete the code file")]
    CodeFileDelete(#[source] tokio::io::Error),
    #[error("Failed to delete the config file")]
    ConfigFileDelete(#[source] tokio::io::Error),
    #[error("Failed to retrieve port number from cgroup")]
    BadPort(#[source] std::num::ParseIntError),
}

async fn get_transaction_data(tx_hash: &str, rpc: &str) -> Result<Value, reqwest::Error> {
    let client = Client::new();
    let method = "eth_getTransactionByHash";
    let params = json!([&tx_hash]);
    let id = 1;

    let request = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id,
    });

    let response = client
        .post(rpc)
        .json(&request)
        .send()
        .await
        .map_err(|err| {
            eprintln!(
                "Failed to send the request to retrieve code transaction data: {}",
                err
            );
            err
        })?;

    let json_response = response.json::<Value>().await.map_err(|err| {
        eprintln!(
            "Failed to parse the response for the code transaction data: {}",
            err
        );
        err
    })?;

    Ok(json_response)
}

pub async fn create_code_file(
    tx_hash: &str,
    slug: &str,
    workerd_runtime_path: &str,
    rpc: &str,
    contract: &str,
) -> Result<(), ServerlessError> {
    // get tx data
    let mut tx_data = match get_transaction_data(tx_hash, rpc)
        .await
        .map_err(ServerlessError::TxDataRetrieve)?["result"]
        .take()
    {
        Value::Null => Err(ServerlessError::TxNotFound),
        other => Ok(other),
    }?;

    // get contract address
    let contract_address = match tx_data["to"].take() {
        Value::String(value) => Ok(value),
        _ => Err(ServerlessError::InvalidTxToType),
    }?;

    // check contract address matches expected
    if contract_address != contract {
        return Err(ServerlessError::InvalidTxToValue(
            contract_address,
            contract.to_owned(),
        ));
    }

    // get calldata
    let calldata = match tx_data["input"].take() {
        Value::String(calldata) => Ok(calldata),
        _ => Err(ServerlessError::InvalidTxCalldataType),
    }?;

    // hex decode calldata by skipping to the code bytes
    let mut calldata = hex::decode(&calldata[138..])?;

    // strip trailing zeros
    let idx = calldata.iter().rev().position(|x| *x != 0).unwrap_or(0);
    calldata.truncate(calldata.len() - idx);

    // write calldata to file
    let mut file =
        File::create(workerd_runtime_path.to_owned() + "/" + tx_hash + "-" + slug + ".js")
            .await
            .map_err(|err| {
                eprintln!("Failed to create the code file: {}", err);
                ServerlessError::CodeFileCreate(err)
            })?;
    file.write_all(calldata.as_slice()).await.map_err(|err| {
        eprintln!("Failed to write to the code file: {}", err);
        ServerlessError::CodeFileCreate(err)
    })?;

    Ok(())
}

pub async fn create_config_file(
    tx_hash: &str,
    slug: &str,
    workerd_runtime_path: &str,
    free_port: u16,
) -> Result<(), ServerlessError> {
    let capnp_data = format!(
        "
using Workerd = import \"/workerd/workerd.capnp\";

const oysterConfig :Workerd.Config = (
  services = [ (name = \"main\", worker = .oysterWorker) ],
  sockets = [ ( name = \"http\", address = \"*:{free_port}\", http = (), service = \"main\" ) ]
);

const oysterWorker :Workerd.Worker = (
  modules = [
    (name = \"main\", esModule = embed \"{tx_hash}-{slug}.js\")
  ],
  compatibilityDate = \"2023-03-07\",
);"
    );

    let mut file =
        File::create(workerd_runtime_path.to_owned() + "/" + tx_hash + "-" + slug + ".capnp")
            .await
            .map_err(|err| {
                eprintln!("Failed to create the workerd config file: {}", err);
                ServerlessError::ConfigFileCreate(err)
            })?;
    file.write_all(capnp_data.as_bytes()).await.map_err(|err| {
        eprintln!("Failed to write to the workerd config file: {}", err);
        ServerlessError::ConfigFileCreate(err)
    })?;
    Ok(())
}

pub fn get_port(cgroup: &str) -> Result<u16, ServerlessError> {
    u16::from_str_radix(&cgroup[8..], 10)
        .map(|x| x + 11000)
        .map_err(|err| {
            eprintln!(
                "Failed to get the port number for cgroup {}: {}",
                cgroup, err
            );
            ServerlessError::BadPort(err)
        })
}

// TODO: timeouts?
pub async fn execute(
    tx_hash: &str,
    slug: &str,
    workerd_runtime_path: &str,
    cgroup: &str,
) -> Result<Child, ServerlessError> {
    let args = [
        &(workerd_runtime_path.to_owned() + "/workerd"),
        "serve",
        &(workerd_runtime_path.to_owned() + "/" + tx_hash + "-" + slug + ".capnp"),
        "--verbose",
    ];

    Ok(Cgroups::execute(cgroup, args).map_err(|err| {
        eprintln!("Failed to execute cgroups or the workerd service: {}", err);
        ServerlessError::Execute
    })?)
}

pub async fn wait_for_port(port: u16) -> bool {
    let start_time = Instant::now();

    while start_time.elapsed() < Duration::from_secs(1) {
        match TcpStream::connect(format!("127.0.0.1:{}", port)).await {
            Ok(_) => return true,
            Err(_) => sleep(Duration::from_millis(1)).await,
        }
    }
    false
}

pub async fn cleanup_code_file(
    tx_hash: &str,
    slug: &str,
    workerd_runtime_path: &str,
) -> Result<(), ServerlessError> {
    tokio::fs::remove_file(workerd_runtime_path.to_owned() + "/" + tx_hash + "-" + slug + ".js")
        .await
        .map_err(|err| {
            eprintln!("Failed to clean up the code file: {}", err);
            ServerlessError::CodeFileDelete(err)
        })?;
    Ok(())
}

pub async fn cleanup_config_file(
    tx_hash: &str,
    slug: &str,
    workerd_runtime_path: &str,
) -> Result<(), ServerlessError> {
    tokio::fs::remove_file(workerd_runtime_path.to_owned() + "/" + tx_hash + "-" + slug + ".capnp")
        .await
        .map_err(|err| {
            eprintln!("Failed to clean up the config file: {}", err);
            ServerlessError::ConfigFileDelete(err)
        })?;
    Ok(())
}

pub async fn get_workerd_response(port: u16, inputs: Bytes) -> Result<Bytes, ServerlessError> {
    let port_str = port.to_string();
    let req_url = "http://127.0.0.1:".to_string() + &port_str + "/";

    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .map_err(|err| {
            eprintln!("Failed to build the reqwest client: {}", err);
            ServerlessError::WorkerRequestError(err)
        })?;

    let response = client
        .post(req_url)
        .body(inputs)
        .send()
        .await
        .map_err(|err| {
            eprintln!("Failed to send request to the workerd port: {}", err);
            ServerlessError::WorkerRequestError(err)
        })?;
    
    let mut response_bytes = format!("{}: ", response.status()).as_bytes().to_vec();    
    let response_body = response.bytes().await.map_err(|err| {
        eprintln!("Failed to parse response from the worker: {}", err);
        ServerlessError::WorkerRequestError(err)
    })?;
    response_bytes.extend(&response_body);
    
    Ok(response_bytes.into())
}
