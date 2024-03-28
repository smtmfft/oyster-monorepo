use std::sync::atomic::Ordering;
use std::sync::Arc;

use actix_web::web::{Data, Json};
use actix_web::{delete, get, post, HttpResponse, Responder};
use ethers::prelude::*;
use hex::FromHex;
use k256::elliptic_curve::generic_array::sequence::Lengthen;
use tiny_keccak::{Hasher, Keccak};

use crate::event_handler::run_job_listener_channel;
use crate::utils::{AppState, InjectKeyInfo, JobManagementContract, RegisterEnclaveInfo};

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok()
}

#[post("/inject-key")]
async fn inject_key(Json(key): Json<InjectKeyInfo>, app_state: Data<AppState>) -> impl Responder {
    if app_state.contract_object.lock().unwrap().is_some() {
        return HttpResponse::BadRequest().body("Secret key has already been injected");
    }

    let mut bytes32_key = [0u8; 32];
    if let Err(err) = hex::decode_to_slice(&key.operator_secret, &mut bytes32_key) {
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
    let http_rpc_client = http_rpc_client
        .with_signer(signer_wallet)
        .nonce_manager(signer_address);

    *app_state.contract_object.lock().unwrap() = Some(JobManagementContract::new(
        app_state.job_management_contract,
        Arc::new(http_rpc_client),
    ));

    HttpResponse::Ok().body("Secret key injected successfully")
}

#[post("/register")]
async fn register_enclave(
    Json(enclave_info): Json<RegisterEnclaveInfo>,
    app_state: Data<AppState>,
) -> impl Responder {
    if app_state.contract_object.lock().unwrap().is_none() {
        return HttpResponse::BadRequest().body("Operator secret key not injected yet!");
    }

    let mut hasher = Keccak::v256();
    hasher.update(b"|jobCapacity|");
    hasher.update(&app_state.job_capacity.to_be_bytes());
    let mut hash = [0u8; 32];
    hasher.finalize(&mut hash);

    let sig = app_state.enclave_signer_key.sign_prehash_recoverable(&hash);
    let Ok((rs, v)) = sig else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to sign the job capacity {}: {}",
            app_state.job_capacity,
            sig.unwrap_err()
        ));
    };

    let Ok(signature) = Bytes::from_hex(hex::encode(rs.to_bytes().append(27 + v.to_byte()))) else {
        return HttpResponse::InternalServerError()
            .body("Failed to parse the signature into eth bytes");
    };

    let txn = app_state
        .contract_object
        .lock()
        .unwrap()
        .clone()
        .unwrap()
        .register_executor(
            enclave_info.attestation.into(),
            enclave_info.enclave_pub_key.clone().into(),
            enclave_info.pcr_0.into(),
            enclave_info.pcr_1.into(),
            enclave_info.pcr_2.into(),
            enclave_info.enclave_cpus.into(),
            enclave_info.enclave_memory.into(),
            enclave_info.timestamp.into(),
            app_state.job_capacity.into(),
            signature,
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

    *app_state.enclave_pub_key.lock().unwrap() = enclave_info.enclave_pub_key;
    app_state.registered.store(true, Ordering::Relaxed);

    run_job_listener_channel(app_state.clone()).await;

    HttpResponse::Ok().body(format!(
        "Enclave Node successfully registered on the common chain block {}, hash {}",
        txn_receipt.block_number.unwrap_or(0.into()),
        txn_receipt.transaction_hash
    ))
}

#[delete("/deregister")]
async fn deregister_enclave(app_state: Data<AppState>) -> impl Responder {
    if app_state.contract_object.lock().unwrap().is_none() {
        return HttpResponse::BadRequest().body("Operator secret key not injected yet!");
    }

    if !app_state.registered.load(Ordering::Relaxed) {
        return HttpResponse::BadRequest().body("Enclave not registered yet!");
    }

    let txn = app_state
        .contract_object
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

    app_state.registered.store(false, Ordering::Relaxed);
    HttpResponse::Ok().body(format!(
        "Enclave Node successfully deregistered from the common chain block {}, hash {}",
        txn_receipt.block_number.unwrap_or(0.into()),
        txn_receipt.transaction_hash
    ))
}
