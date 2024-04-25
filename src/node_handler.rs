use std::sync::Arc;

use actix_web::web::{Data, Json};
use actix_web::{delete, get, post, HttpResponse, Responder};
use ethers::abi::{encode, Token};
use ethers::prelude::*;
use ethers::utils::keccak256;
use k256::elliptic_curve::generic_array::sequence::Lengthen;

use crate::event_handler::run_job_listener_channel;
use crate::utils::{
    send_txn, AppState, CommonChainExecutors, CommonChainJobs, InjectKeyInfo, RegisterEnclaveInfo,
};

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok()
}

#[post("/inject-key")]
// Endpoint exposed to inject operator wallet's private key
async fn inject_key(Json(key): Json<InjectKeyInfo>, app_state: Data<AppState>) -> impl Responder {
    let mut executors_contract_guard = app_state.executors_contract_object.lock().unwrap();
    let mut jobs_contract_guard = app_state.jobs_contract_object.lock().unwrap();
    if executors_contract_guard.is_some() && jobs_contract_guard.is_some() {
        return HttpResponse::BadRequest().body("Secret key has already been injected");
    }

    let mut bytes32_key = [0u8; 32];
    if let Err(err) = hex::decode_to_slice(&key.operator_secret[2..], &mut bytes32_key) {
        return HttpResponse::BadRequest().body(format!(
            "Failed to hex decode the key into 32 bytes: {}",
            err
        ));
    }

    // Initialize local wallet with operator's key to send signed transactions to the common chain
    let signer_wallet = LocalWallet::from_bytes(&bytes32_key);
    let Ok(signer_wallet) = signer_wallet else {
        return HttpResponse::BadRequest().body(format!(
            "Invalid secret key provided: {}",
            signer_wallet.unwrap_err()
        ));
    };
    let signer_wallet = signer_wallet.with_chain_id(app_state.common_chain_id);
    let signer_address = signer_wallet.address();

    // Connect the rpc http provider with the operator's wallet
    let http_rpc_client = Provider::<Http>::try_connect(&app_state.http_rpc_url).await;
    let Ok(http_rpc_client) = http_rpc_client else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to connect to the http rpc server {}: {}",
            app_state.http_rpc_url,
            http_rpc_client.unwrap_err()
        ));
    };
    let http_rpc_client = Arc::new(
        http_rpc_client
            .with_signer(signer_wallet)
            .nonce_manager(signer_address),
    );

    // Initialize smart contract objects to interact with them using operator's wallet
    *executors_contract_guard = Some(CommonChainExecutors::new(
        app_state.executors_contract_addr,
        http_rpc_client.clone(),
    ));
    *jobs_contract_guard = Some(CommonChainJobs::new(
        app_state.jobs_contract_addr,
        http_rpc_client,
    ));

    HttpResponse::Ok().body("Secret key injected successfully")
}

#[post("/register")]
// Endpoint exposed to register the enclave on the common chain as a serverless executor
async fn register_enclave(
    Json(enclave_info): Json<RegisterEnclaveInfo>,
    app_state: Data<AppState>,
) -> impl Responder {
    if app_state
        .executors_contract_object
        .lock()
        .unwrap()
        .is_none()
        || app_state.jobs_contract_object.lock().unwrap().is_none()
    {
        return HttpResponse::BadRequest().body("Operator secret key not injected yet!");
    }

    let mut registered_guard = app_state.registered.lock().unwrap();
    if *registered_guard {
        return HttpResponse::BadRequest().body("Enclave node is already registered!");
    }

    // Encode and sign the job capacity of executor using its private key
    let hash = keccak256(encode(&[Token::Uint(app_state.job_capacity.into())]));
    let sig = app_state.enclave_signer_key.sign_prehash_recoverable(&hash);
    let Ok((rs, v)) = sig else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to sign the job capacity {}: {}",
            app_state.job_capacity,
            sig.unwrap_err()
        ));
    };
    let signature = rs.to_bytes().append(27 + v.to_byte()).to_vec();

    let Ok(attestation_bytes) = hex::decode(&enclave_info.attestation[2..]) else {
        return HttpResponse::BadRequest().body("Invalid attestation hex string");
    };
    let Ok(pcr_0_bytes) = hex::decode(&enclave_info.pcr_0[2..]) else {
        return HttpResponse::BadRequest().body("Invalid pcr0 hex string");
    };
    let Ok(pcr_1_bytes) = hex::decode(&enclave_info.pcr_1[2..]) else {
        return HttpResponse::BadRequest().body("Invalid pcr1 hex string");
    };
    let Ok(pcr_2_bytes) = hex::decode(&enclave_info.pcr_2[2..]) else {
        return HttpResponse::BadRequest().body("Invalid pcr2 hex string");
    };

    // Prepare the transaction to be send to the common chain for registration
    let txn = app_state
        .executors_contract_object
        .lock()
        .unwrap()
        .clone()
        .unwrap()
        .register_executor(
            attestation_bytes.into(),
            app_state.enclave_pub_key.clone().into(),
            pcr_0_bytes.into(),
            pcr_1_bytes.into(),
            pcr_2_bytes.into(),
            enclave_info.timestamp.into(),
            app_state.job_capacity.into(),
            signature.into(),
            enclave_info.stake_amount.into(),
        );

    let txn_result = send_txn(txn).await;
    let Ok(txn_receipt) = txn_result else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to register the enclave: {}",
            txn_result.unwrap_err()
        ));
    };

    *registered_guard = true;

    let app_state_clone = app_state.clone();
    // Start the listener to receive jobs emitted by the common chain contract
    tokio::spawn(async move { run_job_listener_channel(app_state_clone).await });

    HttpResponse::Ok().body(format!(
        "Enclave Node successfully registered on the common chain block {}, hash {:?}",
        txn_receipt.block_number.unwrap_or(0.into()),
        txn_receipt.transaction_hash
    ))
}

#[delete("/deregister")]
// Endpoint exposed to deregister the enclave from the common chain as an executor (Can be done manually but preferred this way)
async fn deregister_enclave(app_state: Data<AppState>) -> impl Responder {
    if app_state
        .executors_contract_object
        .lock()
        .unwrap()
        .is_none()
        || app_state.jobs_contract_object.lock().unwrap().is_none()
    {
        return HttpResponse::BadRequest().body("Operator secret key not injected yet!");
    }

    let mut registered_guard = app_state.registered.lock().unwrap();
    if *registered_guard == false {
        return HttpResponse::BadRequest().body("Enclave not registered yet!");
    }

    // Prepare the transaction to be send to the common chain for deregistration
    let txn = app_state
        .executors_contract_object
        .lock()
        .unwrap()
        .clone()
        .unwrap()
        .deregister_executor(app_state.enclave_pub_key.clone().into());

    let txn_result = send_txn(txn).await;
    let Ok(txn_receipt) = txn_result else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to deregister the enclave: {}",
            txn_result.unwrap_err()
        ));
    };

    *registered_guard = false;
    HttpResponse::Ok().body(format!(
        "Enclave Node successfully deregistered from the common chain block {}, hash {:?}",
        txn_receipt.block_number.unwrap_or(0.into()),
        txn_receipt.transaction_hash
    ))
}
