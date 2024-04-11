use std::collections::HashSet;
use std::sync::Mutex;

use actix_web::web::Bytes;
use anyhow::{anyhow, Result};
use ethers::abi::{encode_packed, Token};
use ethers::contract::abigen;
use ethers::middleware::{NonceManagerMiddleware, SignerMiddleware};
use ethers::providers::{Http, Provider, Ws};
use ethers::signers::LocalWallet;
use ethers::types::{Address, U256};
use ethers::utils::keccak256;
use k256::ecdsa::SigningKey;
use serde::{Deserialize, Serialize};

use crate::cgroups::Cgroups;

abigen!(
    CommonChainExecutors,
    "CommonChainExecutors.json",
    derives(serde::Serialize, serde::Deserialize)
);

abigen!(
    CommonChainJobs,
    "CommonChainJobs.json",
    derives(serde::Serialize, serde::Deserialize)
);

pub type HttpSignerProvider = NonceManagerMiddleware<SignerMiddleware<Provider<Http>, LocalWallet>>;

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
    pub timeout_response: Option<(U256, U256)>,
}

pub struct ExecutionResponse {
    pub id: U256,
    pub req_chain_id: U256,
    pub output: Bytes,
    pub error_code: u8,
    pub total_time: u128,
    pub signature: Bytes,
}

pub fn pub_key_to_address(pub_key: &[u8]) -> Result<Address> {
    if pub_key.len() != 64 {
        return Err(anyhow!("Invalid public key length"));
    }

    let hash = keccak256(pub_key);
    let addr_bytes: [u8; 20] = hash[12..].try_into()?;
    Ok(Address::from_slice(&addr_bytes))
}

pub fn get_job_key(job_id: U256, req_chain_id: U256) -> Result<U256> {
    Ok(U256::from(keccak256(encode_packed(&[
        Token::Uint(job_id),
        Token::String("-".to_owned()),
        Token::Uint(req_chain_id),
    ])?)))
}
