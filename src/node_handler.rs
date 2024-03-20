use std::str::FromStr;
use std::sync::Arc;

use actix_web::web::{Data, Json};
use actix_web::{delete, get, post, HttpResponse, Responder};
use ethers::prelude::*;
use hex::FromHex;
use k256::elliptic_curve::generic_array::sequence::Lengthen;

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

    let http_client = Provider::<Http>::try_connect(&app_state.http_rpc_url).await;
    let Ok(http_client) = http_client else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to connect to rpc server {}: {}",
            app_state.http_rpc_url,
            http_client.unwrap_err()
        ));
    };
    let http_client = http_client
        .with_signer(signer_wallet)
        .nonce_manager(signer_address);

    *app_state.contract_object.lock().unwrap() = Some(JobManagementContract::new(
        app_state.job_management_contract,
        Arc::new(http_client),
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

    let sig = app_state
        .enclave_signer
        .sign_prehash_recoverable(&app_state.job_capacity.to_be_bytes());
    let Ok((rs, v)) = sig else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to sign the job capacity {}: {}",
            app_state.job_capacity,
            sig.unwrap_err()
        ));
    };

    let Ok(attestation) = Bytes::from_str(&enclave_info.attestation) else {
        return HttpResponse::BadRequest().body("Failed to parse the attestation into eth bytes");
    };
    let Ok(enclave_pub_key) = Bytes::from_str(&enclave_info.enclave_pub_key) else {
        return HttpResponse::BadRequest()
            .body("Failed to parse the enclave public key into eth bytes");
    };
    let Ok(pcr_0) = Bytes::from_str(&enclave_info.pcr_0) else {
        return HttpResponse::BadRequest().body("Failed to parse pcr0 into eth bytes");
    };
    let Ok(pcr_1) = Bytes::from_str(&enclave_info.pcr_1) else {
        return HttpResponse::BadRequest().body("Failed to parse pcr1 into eth bytes");
    };
    let Ok(pcr_2) = Bytes::from_str(&enclave_info.pcr_2) else {
        return HttpResponse::BadRequest().body("Failed to parse pcr2 into eth bytes");
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
            attestation,
            enclave_pub_key,
            pcr_0,
            pcr_1,
            pcr_2,
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

    if app_state.enclave_pub_key.lock().unwrap().is_empty() {
        return HttpResponse::BadRequest().body("Enclave not registered yet!");
    }

    let Ok(enclave_pub_key) = Bytes::from_str(&app_state.enclave_pub_key.lock().unwrap()) else {
        return HttpResponse::BadRequest()
            .body("Failed to parse the enclave public key into eth bytes");
    };

    let txn = app_state
        .contract_object
        .lock()
        .unwrap()
        .clone()
        .unwrap()
        .deregister_executor(enclave_pub_key);
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

    HttpResponse::Ok().body(format!(
        "Enclave Node successfully deregistered from the common chain block {}, hash {}",
        txn_receipt.block_number.unwrap_or(0.into()),
        txn_receipt.transaction_hash
    ))
}
