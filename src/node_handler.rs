use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use actix_web::web::{Data, Json};
use actix_web::{get, post, HttpResponse, Responder};
use ethers::abi::{encode, encode_packed, Token};
use ethers::prelude::*;
use ethers::utils::keccak256;
use k256::elliptic_curve::generic_array::sequence::Lengthen;
use serde_json::json;

use crate::event_handler::events_listener;
use crate::utils::{AppState, ImmutableConfig, MutableConfig};

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok()
}

#[post("/immutable-config")]
// Endpoint exposed to inject immutable executor config parameters
async fn inject_immutable_config(
    Json(immutable_config): Json<ImmutableConfig>,
    app_state: Data<AppState>,
) -> impl Responder {
    let mut immutable_params_injected_guard = app_state.immutable_params_injected.lock().unwrap();
    if *immutable_params_injected_guard == true {
        return HttpResponse::BadRequest().body("Immutable params already configured!");
    }

    let owner_address = H160::from_str(&immutable_config.owner_address_hex);
    let Ok(owner_address) = owner_address else {
        return HttpResponse::BadRequest().body(format!(
            "Invalid owner address provided: {:?}",
            owner_address.unwrap_err()
        ));
    };

    // Initialize owner address for the enclave
    *app_state.enclave_owner.lock().unwrap() = owner_address;
    *immutable_params_injected_guard = true;

    HttpResponse::Ok().body("Immutable params configured!")
}

#[post("/mutable-config")]
// Endpoint exposed to inject mutable executor config parameters
async fn inject_mutable_config(
    Json(mutable_config): Json<MutableConfig>,
    app_state: Data<AppState>,
) -> impl Responder {
    let mut mutable_params_injected_guard = app_state.mutable_params_injected.lock().unwrap();

    let mut bytes32_gas_key = [0u8; 32];
    if let Err(err) = hex::decode_to_slice(&mutable_config.gas_key_hex, &mut bytes32_gas_key) {
        return HttpResponse::BadRequest().body(format!(
            "Failed to hex decode the gas private key into 32 bytes: {:?}",
            err
        ));
    }

    // Initialize local wallet with operator's gas key to send signed transactions to the common chain
    let gas_wallet = LocalWallet::from_bytes(&bytes32_gas_key);
    let Ok(gas_wallet) = gas_wallet else {
        return HttpResponse::BadRequest().body(format!(
            "Invalid gas private key provided: {:?}",
            gas_wallet.unwrap_err()
        ));
    };
    let gas_wallet = gas_wallet.with_chain_id(app_state.common_chain_id);
    let gas_address = gas_wallet.address();

    // Connect the rpc http provider with the operator's gas wallet
    let http_rpc_client = Provider::<Http>::try_connect(&app_state.http_rpc_url).await;
    let Ok(http_rpc_client) = http_rpc_client else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to connect to the http rpc server {}: {:?}",
            app_state.http_rpc_url,
            http_rpc_client.unwrap_err()
        ));
    };
    let http_rpc_client = http_rpc_client
        .with_signer(gas_wallet)
        .nonce_manager(gas_address);

    // Initialize HTTP RPC client for sending signed transactions
    *app_state.http_rpc_client.lock().unwrap() = Some(Arc::new(http_rpc_client));
    *mutable_params_injected_guard = true;

    HttpResponse::Ok().body("Mutable params configured!")
}

#[get("/signed-registration-message")]
// Endpoint exposed to retrieve the metadata required to register the enclave on the common chain
async fn export_signed_registration_message(app_state: Data<AppState>) -> impl Responder {
    if *app_state.immutable_params_injected.lock().unwrap() == false {
        return HttpResponse::BadRequest().body("Immutable params not configured yet!");
    }

    if *app_state.mutable_params_injected.lock().unwrap() == false {
        return HttpResponse::BadRequest().body("Mutable params not configured yet!");
    }

    let job_capacity = app_state.job_capacity;
    let owner = app_state.enclave_owner.lock().unwrap().clone();
    let sign_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

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
        Token::Uint(job_capacity.into()),
        Token::Uint(sign_timestamp.into()),
    ]));

    // Create the digest
    let digest = encode_packed(&[
        Token::String("\x19\x01".to_string()),
        Token::FixedBytes(domain_separator.to_vec()),
        Token::FixedBytes(hash_struct.to_vec()),
    ]);
    let Ok(digest) = digest else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to encode the registration message for signing: {:?}",
            digest.unwrap_err()
        ));
    };
    let digest = keccak256(digest);

    // Sign the digest using enclave key
    let sig = app_state.enclave_signer.sign_prehash_recoverable(&digest);
    let Ok((rs, v)) = sig else {
        return HttpResponse::InternalServerError().body(format!(
            "Failed to sign the registration message using enclave key: {:?}",
            sig.unwrap_err()
        ));
    };
    let signature = hex::encode(rs.to_bytes().append(27 + v.to_byte()).to_vec());

    let mut events_listener_active_guard = app_state.events_listener_active.lock().unwrap();
    if *events_listener_active_guard == false {
        let Ok(current_block_number) = app_state
            .http_rpc_client
            .lock()
            .unwrap()
            .clone()
            .unwrap()
            .get_block_number()
            .await
        else {
            return HttpResponse::InternalServerError().body(format!("Failed to fetch the latest block number of the common chain for initiating event listening!"));
        };

        *events_listener_active_guard = true;
        drop(events_listener_active_guard);

        tokio::spawn(async move {
            events_listener(app_state, current_block_number).await;
        });
    }

    HttpResponse::Ok().json(json!({
        "job_capacity": job_capacity,
        "sign_timestamp": sign_timestamp,
        "owner": owner,
        "signature": signature,
    }))
}
