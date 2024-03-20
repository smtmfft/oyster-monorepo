use std::sync::Mutex;

use ethers::contract::abigen;
use ethers::middleware::{NonceManagerMiddleware, SignerMiddleware};
use ethers::providers::{Http, Provider};
use ethers::signers::LocalWallet;
use ethers::types::Address;
use k256::ecdsa::SigningKey;
use serde::{Deserialize, Serialize};

abigen!(
    JobManagementContract,
    "common_chain_contract.json",
    derives(serde::Serialize, serde::Deserialize)
);

type HttpSignerProvider = NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>;

pub struct AppState {
    pub job_capacity: usize,
    pub common_chain_id: u64,
    pub http_rpc_url: String,
    pub job_management_contract: Address,
    pub contract_object: Mutex<Option<JobManagementContract<HttpSignerProvider>>>,
    pub enclave_signer: SigningKey,
    pub enclave_pub_key: Mutex<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InjectKeyInfo {
    pub operator_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterEnclaveInfo {
    pub attestation: String,
    pub enclave_pub_key: String,
    pub pcr_0: String,
    pub pcr_1: String,
    pub pcr_2: String,
    pub enclave_cpus: usize,
    pub enclave_memory: usize,
    pub timestamp: usize,
    pub stake_amount: usize,
}
