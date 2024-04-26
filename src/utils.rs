use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use actix_web::web::Bytes;
use anyhow::{anyhow, Context, Result};
use ethers::contract::{abigen, FunctionCall};
use ethers::middleware::{NonceManagerMiddleware, SignerMiddleware};
use ethers::providers::{Http, Provider, Ws};
use ethers::signers::LocalWallet;
use ethers::types::{Address, TransactionReceipt, U256};
use ethers::utils::keccak256;
use k256::ecdsa::SigningKey;
use serde::{Deserialize, Serialize};

use crate::cgroups::Cgroups;

// Generate type-safe ABI bindings for the Executors contract at compile time
abigen!(
    CommonChainExecutors,
    "CommonChainExecutors.json",
    derives(serde::Serialize, serde::Deserialize)
);

// Generate type-safe ABI bindings for the Jobs contract at compile time
abigen!(
    CommonChainJobs,
    "CommonChainJobs.json",
    derives(serde::Serialize, serde::Deserialize)
);

pub type HttpSignerProvider = NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>;

// App data struct containing the necessary fields to run the executor
pub struct AppState {
    pub job_capacity: usize,
    pub cgroups: Mutex<Cgroups>,
    pub registered: Mutex<bool>,
    pub common_chain_id: u64,
    pub http_rpc_url: String,
    pub executors_contract_addr: Address,
    pub executors_contract_object: Mutex<Option<CommonChainExecutors<HttpSignerProvider>>>,
    pub jobs_contract_addr: Address,
    pub jobs_contract_object: Mutex<Option<CommonChainJobs<HttpSignerProvider>>>,
    pub code_contract_addr: String,
    pub web_socket_client: Provider<Ws>,
    pub enclave_signer_key: SigningKey,
    pub enclave_pub_key: Bytes,
    pub workerd_runtime_path: String,
    pub job_requests_running: Mutex<HashSet<U256>>,
    pub execution_buffer_time: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InjectKeyInfo {
    pub operator_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterEnclaveInfo {
    pub attestation: String,
    pub pcr_0: String,
    pub pcr_1: String,
    pub pcr_2: String,
    pub timestamp: usize,
    pub stake_amount: usize,
}

pub struct JobResponse {
    pub execution_response: Option<ExecutionResponse>,
    pub timeout_response: Option<U256>,
}

pub struct ExecutionResponse {
    pub id: U256,
    pub output: Bytes,
    pub error_code: u8,
    pub total_time: u128,
    pub signature: Bytes,
}

// Convert the 64 bytes 'secp256k1' public key to 20 bytes unique address
pub fn pub_key_to_address(pub_key: &[u8]) -> Result<Address> {
    if pub_key.len() != 64 {
        return Err(anyhow!("Invalid public key length"));
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
