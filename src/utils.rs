use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use actix_web::web::Bytes;
use anyhow::{anyhow, Context, Result};
use ethers::contract::{abigen, FunctionCall};
use ethers::middleware::{NonceManagerMiddleware, SignerMiddleware};
use ethers::providers::{Http, Provider, Ws};
use ethers::signers::LocalWallet;
use ethers::types::{Address, TransactionReceipt, H160, U256};
use ethers::utils::keccak256;
use k256::ecdsa::SigningKey;
use serde::{Deserialize, Serialize};

use crate::cgroups::Cgroups;

// Generate type-safe ABI bindings for the Jobs contract at compile time
abigen!(
    Jobs,
    "Jobs.json",
    derives(serde::Serialize, serde::Deserialize)
);

pub type HttpSignerProvider = NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>;

// App data struct containing the necessary fields to run the executor
pub struct AppState {
    pub job_capacity: usize,
    pub cgroups: Mutex<Cgroups>,
    pub registered: Mutex<bool>,
    pub register_listener_active: Mutex<bool>,
    pub num_selected_executors: u8,
    pub common_chain_id: u64,
    pub http_rpc_url: String,
    pub http_rpc_client: Mutex<Option<Arc<HttpSignerProvider>>>,
    pub web_socket_client: Provider<Ws>,
    pub executors_contract_addr: Address,
    pub jobs_contract_addr: Address,
    pub code_contract_addr: String,
    pub enclave_owner: Mutex<Option<H160>>,
    pub enclave_address: H160,
    pub enclave_signer: SigningKey,
    pub workerd_runtime_path: String,
    pub job_requests_running: Mutex<HashSet<U256>>,
    pub execution_buffer_time: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InjectInfo {
    pub owner_address: String,
    pub gas_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Attestation {
    pub timestamp: usize,
}

pub struct JobResponse {
    pub job_output: Option<JobOutput>,
    pub timeout_response: Option<U256>,
}

pub struct JobOutput {
    pub signature: Bytes,
    pub id: U256,
    pub execution_response: ExecutionResponse,
    pub sign_timestamp: U256,
}

pub struct ExecutionResponse {
    pub output: Bytes,
    pub error_code: u8,
    pub total_time: U256,
}

// Convert the 64 bytes 'secp256k1' public key to 20 bytes unique address
pub fn pub_key_to_address(pub_key: &[u8]) -> Result<Address> {
    if pub_key.len() != 64 {
        return Err(anyhow!("Public key is not 64 bytes"));
    }

    let hash = keccak256(pub_key);
    let addr_bytes: [u8; 20] = hash[12..].try_into()?;
    Ok(Address::from_slice(&addr_bytes))
}

// Send a signed transaction to the rpc network and report its confirmation or rejection
pub async fn send_txn(
    txn: FunctionCall<Arc<HttpSignerProvider>, HttpSignerProvider, ()>,
) -> Result<TransactionReceipt> {
    let pending_txn = txn
        .send()
        .await
        .context("Failed to send the transaction to the network")?;

    let txn_hash = pending_txn.tx_hash();
    let Some(txn_receipt) = pending_txn
        .confirmations(1) // TODO: FIX CONFIRMATIONS REQUIRED
        .await
        .context("Failed to confirm the transaction")?
    else {
        return Err(anyhow!(
            "Transaction with hash {:?} has been dropped from mempool!",
            txn_hash
        ));
    };

    Ok(txn_receipt)
}
