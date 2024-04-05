use std::sync::Arc;

use actix_web::web::{Data, Json};
use actix_web::{delete, get, post, HttpResponse, Responder};
use ethers::abi::{encode, Token};
use ethers::prelude::*;
use ethers::utils::keccak256;
use k256::elliptic_curve::generic_array::sequence::Lengthen;

use crate::event_handler::run_job_listener_channel;
use crate::utils::{
    AppState, CommonChainExecutors, CommonChainJobs, InjectKeyInfo, RegisterEnclaveInfo,
};

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok()
}

#[post("/inject-key")]
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

    let signer_wallet = LocalWallet::from_bytes(&bytes32_key);
    let Ok(signer_wallet) = signer_wallet else {
        return HttpResponse::BadRequest().body(format!(
            "Invalid secret key provided: {}",
            signer_wallet.unwrap_err()
        ));
    };
    let signer_wallet = signer_wallet.with_chain_id(app_state.common_chain_id);
    let signer_address = signer_wallet.address();

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

    let mut enclave_pub_key_bytes = [0u8; 64];
    if let Err(err) = hex::decode_to_slice(
        &enclave_info.enclave_pub_key[2..],
        &mut enclave_pub_key_bytes,
    ) {
        return HttpResponse::BadRequest().body(format!(
            "Failed to hex decode the enclave public key into 64 bytes: {}",
            err
        ));
    }
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

    let txn = app_state
        .executors_contract_object
        .lock()
        .unwrap()
        .clone()
        .unwrap()
        .register_executor(
            attestation_bytes.into(),
            enclave_pub_key_bytes.into(),
            pcr_0_bytes.into(),
            pcr_1_bytes.into(),
            pcr_2_bytes.into(),
            enclave_info.timestamp.into(),
            app_state.job_capacity.into(),
            signature.into(),
            enclave_info.stake_amount.into(),
        );

    let pending_txn = txn.send().await;
    let Ok(pending_txn) = pending_txn else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to send transaction for registering the enclave node: {}",
            pending_txn.unwrap_err()
        ));
    };

    let txn_hash = pending_txn.tx_hash();
    let Ok(Some(txn_receipt)) = pending_txn.confirmations(1).await else {
        // TODO: FIX CONFIRMATIONS REQUIRED
        return HttpResponse::InternalServerError().body(format!(
            "Failed to confirm transaction with hash {}",
            txn_hash
        ));
    };

    *app_state.enclave_pub_key.lock().unwrap() = enclave_pub_key_bytes.to_vec().into();
    *registered_guard = true;

    let app_state_clone = app_state.clone();
    tokio::spawn(async move { run_job_listener_channel(app_state_clone).await });

    HttpResponse::Ok().body(format!(
        "Enclave Node successfully registered on the common chain block {}, hash {}",
        txn_receipt.block_number.unwrap_or(0.into()),
        txn_receipt.transaction_hash
    ))
}

#[delete("/deregister")]
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

    let txn = app_state
        .executors_contract_object
        .lock()
        .unwrap()
        .clone()
        .unwrap()
        .deregister_executor(app_state.enclave_pub_key.lock().unwrap().clone().into());
    let pending_txn = txn.send().await;
    let Ok(pending_txn) = pending_txn else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to send transaction for deregistering the enclave node: {}",
            pending_txn.unwrap_err()
        ));
    };

    let txn_hash = pending_txn.tx_hash();
    let Ok(Some(txn_receipt)) = pending_txn.confirmations(1).await else {
        // TODO: FIX CONFIRMATIONS REQUIRED
        return HttpResponse::InternalServerError().body(format!(
            "Failed to confirm transaction with hash {}",
            txn_hash
        ));
    };

    *registered_guard = false;
    HttpResponse::Ok().body(format!(
        "Enclave Node successfully deregistered from the common chain block {}, hash {}",
        txn_receipt.block_number.unwrap_or(0.into()),
        txn_receipt.transaction_hash
    ))
}
