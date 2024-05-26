use std::str::FromStr;
use std::sync::Arc;

use actix_web::web::{Data, Json};
use actix_web::{get, post, HttpResponse, Responder};
use ethers::abi::{encode, encode_packed, Token};
use ethers::prelude::*;
use ethers::utils::keccak256;
use k256::elliptic_curve::generic_array::sequence::Lengthen;
use serde_json::json;

use crate::event_handler::register_listener;
use crate::utils::{AppState, Attestation, InjectInfo};

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok()
}

#[post("/inject")]
// Endpoint exposed to inject owner's address and gas wallet private key
async fn inject(Json(key): Json<InjectInfo>, app_state: Data<AppState>) -> impl Responder {
    let mut http_rpc_client_guard = app_state.http_rpc_client.lock().unwrap();
    let mut enclave_owner_guard = app_state.enclave_owner.lock().unwrap();
    if http_rpc_client_guard.is_some() && enclave_owner_guard.is_some() {
        return HttpResponse::BadRequest().body("Injection already done!");
    }

    let owner_address = H160::from_str(&key.owner_address[2..]);
    let Ok(owner_address) = owner_address else {
        return HttpResponse::BadRequest().body(format!(
            "Invalid owner address provided: {:?}",
            owner_address.unwrap_err()
        ));
    };

    let mut bytes32_gas_key = [0u8; 32];
    if let Err(err) = hex::decode_to_slice(&key.gas_key[2..], &mut bytes32_gas_key) {
        return HttpResponse::BadRequest().body(format!(
            "Failed to hex decode the gas key into 32 bytes: {:?}",
            err
        ));
    }

    // Initialize local wallet with operator's gas key to send signed transactions to the common chain
    let gas_wallet = LocalWallet::from_bytes(&bytes32_gas_key);
    let Ok(gas_wallet) = gas_wallet else {
        return HttpResponse::BadRequest().body(format!(
            "Invalid gas key provided: {:?}",
            gas_wallet.unwrap_err()
        ));
    };
    let gas_wallet = gas_wallet.with_chain_id(app_state.common_chain_id);
    let gas_address = gas_wallet.address();

    // Connect the rpc http provider with the operator's wallet
    let http_rpc_client = Provider::<Http>::try_connect(&app_state.http_rpc_url).await;
    let Ok(http_rpc_client) = http_rpc_client else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to connect to the http rpc server {}: {:?}",
            app_state.http_rpc_url,
            http_rpc_client.unwrap_err()
        ));
    };
    let http_rpc_client = Arc::new(
        http_rpc_client
            .with_signer(gas_wallet)
            .nonce_manager(gas_address),
    );

    // Initialize operator's wallet and http rpc client for sending transactions
    *enclave_owner_guard = Some(owner_address);
    *http_rpc_client_guard = Some(http_rpc_client);

    HttpResponse::Ok().body("Injection done successfully")
}

#[post("/export")]
// Endpoint exposed to retrieve the metadata required to register the enclave on the common chain
async fn export(
    Json(attestation_timestamp): Json<Attestation>,
    app_state: Data<AppState>,
) -> impl Responder {
    if app_state.enclave_owner.lock().unwrap().is_none()
        || app_state.http_rpc_client.lock().unwrap().is_none()
    {
        return HttpResponse::BadRequest().body("Injection not done yet!");
    }

    let job_capacity = app_state.job_capacity;
    let owner = app_state.enclave_owner.lock().unwrap().clone().unwrap();

    // Encode and hash the job capacity of executor following EIP712 format
    let domain_separator = keccak256(encode(&[
        Token::FixedBytes(keccak256("EIP712Domain(string name,string version)").to_vec()),
        Token::FixedBytes(keccak256("marlin.oyster.Executors").to_vec()),
        Token::FixedBytes(keccak256("1").to_vec()),
    ]));
    let register_typehash =
        keccak256("Register(address owner,uint256 jobCapacity,uint256 signTimestamp)");

    let hash_struct = keccak256(encode(&[
        Token::FixedBytes(register_typehash.to_vec()),
        Token::Address(owner),
        Token::Uint(app_state.job_capacity.into()),
        Token::Uint(attestation_timestamp.timestamp.into()),
    ]));

    // Create the digest
    let digest = encode_packed(&[
        Token::String("\x19\x01".to_string()),
        Token::FixedBytes(domain_separator.to_vec()),
        Token::FixedBytes(hash_struct.to_vec()),
    ]);
    let Ok(digest) = digest else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to encode the job capacity {} for signing: {:?}",
            app_state.job_capacity,
            digest.unwrap_err()
        ));
    };
    let digest = keccak256(digest);

    // Sign the digest using enclave key
    let sig = app_state.enclave_signer.sign_prehash_recoverable(&digest);
    let Ok((rs, v)) = sig else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to sign the job capacity {}: {:?}",
            app_state.job_capacity,
            sig.unwrap_err()
        ));
    };
    let signature = rs.to_bytes().append(27 + v.to_byte()).to_vec();

    if *app_state.register_listener_active.lock().unwrap() == false {
        tokio::spawn(async move {
            *app_state.register_listener_active.lock().unwrap() = true;
            register_listener(app_state.clone()).await;
            *app_state.register_listener_active.lock().unwrap() = false;
        });
    }

    HttpResponse::Ok().json(json!({
        "job_capacity": job_capacity,
        "sign_timestamp": attestation_timestamp.timestamp,
        "owner": owner,
        "signature": hex::encode(signature),
    }))
}
