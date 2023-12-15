use crate::types::handlers::AppState;
use actix_web::{error, http::StatusCode, post, web, Responder};
use derive_more::{Display, Error};
use ethers;
use hex;
use libsodium_sys::crypto_sign_verify_detached;
use oyster;
use secp256k1;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

#[derive(Deserialize, Serialize)]
struct VerifyAttestation {
    attestation_doc: String,
    pcrs: Vec<String>,
    min_cpus: usize,
    min_mem: usize,
    max_age: usize,
    signature: String,
    secp_key: String,
}

#[serde_as]
#[derive(Serialize, Deserialize)]
struct VerifyAttestationResponse {
    sig: String,
    secp_key: String,
}

#[derive(Debug, Display, Error)]
pub enum UserError {
    InternalServerError,
}

impl error::ResponseError for UserError {
    fn error_response(&self) -> actix_web::HttpResponse<actix_web::body::BoxBody> {
        actix_web::HttpResponse::build(self.status_code())
            .insert_header(actix_web::http::header::ContentType::plaintext())
            .body(self.to_string())
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            UserError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

fn abi_encode(
    prefix: String,
    enclave_pubkey: &[u8; 65],
    pcr_0: Vec<u8>,
    pcr_1: Vec<u8>,
    pcr_2: Vec<u8>,
    enclave_cpu: usize,
    enclave_mem: usize,
) -> Vec<u8> {
    let mut encoded_data = Vec::new();
    encoded_data.push(ethers::abi::Token::String(prefix));
    encoded_data.push(ethers::abi::Token::Address(address_from_pubkey(
        enclave_pubkey,
    )));
    encoded_data.push(ethers::abi::Token::Bytes(pcr_0));
    encoded_data.push(ethers::abi::Token::Bytes(pcr_1));
    encoded_data.push(ethers::abi::Token::Bytes(pcr_2));
    encoded_data.push(ethers::abi::Token::Uint(enclave_cpu.into()));
    encoded_data.push(ethers::abi::Token::Uint(enclave_mem.into()));
    ethers::abi::encode(&encoded_data)
}

fn address_from_pubkey(pub_key: &[u8; 65]) -> ethers::types::Address {
    let hash = ethers::utils::keccak256(&pub_key[1..]);
    ethers::types::Address::from_slice(&hash[12..])
}

fn verification_message(pubkey: &String) -> String {
    const PREFIX: &str = "attestation-verification-";
    format!("{}{:?}", PREFIX.to_string(), pubkey)
}

#[post("/verify/attestation")]
async fn verify(
    req: web::Json<VerifyAttestation>,
    state: web::Data<AppState>,
) -> actix_web::Result<impl Responder, UserError> {
    let attestationdoc_bytes =
        hex::decode(&req.attestation_doc).map_err(|_| UserError::InternalServerError)?;
    let requester_pub_key = oyster::verify(
        attestationdoc_bytes,
        req.pcrs.clone(),
        req.min_cpus,
        req.min_mem,
        req.max_age,
    )
    .map_err(|_| UserError::InternalServerError)?;

    let msg = verification_message(&req.secp_key);
    let sig_bytes = hex::decode(&req.signature).map_err(|_| UserError::InternalServerError)?;

    unsafe {
        let is_verified = crypto_sign_verify_detached(
            sig_bytes.clone().as_mut_ptr(),
            msg.as_ptr(),
            msg.len() as u64,
            requester_pub_key.as_ptr(),
        );
        if is_verified != 0 {
            return Err(UserError::InternalServerError);
        }
    }

    let mut pubkey_bytes = [0u8; 65];
    hex::decode_to_slice(&req.secp_key, &mut pubkey_bytes)
        .map_err(|_| UserError::InternalServerError)?;

    let abi_encoded = abi_encode(
        "Enclave Attestation Verified".to_string(),
        &pubkey_bytes,
        hex::decode(req.pcrs[0].clone()).map_err(|_| UserError::InternalServerError)?,
        hex::decode(req.pcrs[1].clone()).map_err(|_| UserError::InternalServerError)?,
        hex::decode(req.pcrs[2].clone()).map_err(|_| UserError::InternalServerError)?,
        req.min_cpus,
        req.min_mem,
    );

    let msg_to_sign = ethers::utils::keccak256(abi_encoded);
    let msg_to_sign = secp256k1::Message::from_digest_slice(&msg_to_sign)
        .map_err(|_| UserError::InternalServerError)?;
    let secp = secp256k1::Secp256k1::new();
    let sig = secp
        .sign_ecdsa(&msg_to_sign, &state.secp_private_key)
        .serialize_compact();
    let sig = hex::encode(sig);
    let sig = format!("{}1c", sig);
    Ok(web::Json(VerifyAttestationResponse {
        sig,
        secp_key: hex::encode(state.secp_public_key),
    }))
}

#[cfg(test)]

mod tests {
    use super::*;
    use actix_web::{test, web, App};
    use libsodium_sys::crypto_sign_detached;
    use std::fs;

    #[actix_web::test]
    async fn test_handler() {
        let enclave_pub_key = fs::read("./enclave_public.key").unwrap();
        let enclave_priv_key = fs::read("./enclave_private.key").unwrap();
        let secp_priv_key = fs::read("./secret.key").unwrap();
        let secp_priv_key = secp256k1::SecretKey::from_slice(&secp_priv_key).unwrap();
        let secp = secp256k1::Secp256k1::new();

        let secp_pub_key = secp_priv_key.public_key(&secp).serialize_uncompressed();
        println!("address : {}", address_from_pubkey(&secp_pub_key));
        let msg_to_sign = verification_message(&hex::encode(&secp_pub_key));
        let mut sig = [0u8; 64];
        unsafe {
            let is_signed = crypto_sign_detached(
                sig.as_mut_ptr(),
                std::ptr::null_mut(),
                msg_to_sign.as_ptr(),
                msg_to_sign.len() as u64,
                enclave_priv_key.as_ptr(),
            );
            if is_signed != 0 {
                panic!("not signed");
            }
        }

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState {
                    enclave_public_key: enclave_pub_key.clone(),
                    secp_private_key: secp_priv_key.clone(),
                    secp_public_key: secp_pub_key.clone(),
                }))
                .service(verify),
        )
        .await;
        let mut pcrs = Vec::new();
        pcrs.push("3a2c64486fc890a1f65e82c195632b35a1b97d7595c666b8c83e91b56b92568abbeca0829269e40e4b76a6df963157da".to_string());
        pcrs.push("be9dc8acb9b26e67f2919fe877f94271c79289989455013c66a5f2cc637a9355665bc9d89b7aed986f7b4c269acc1233".to_string());
        pcrs.push("2cd79888cf800407c2bdd2165be71b8484561430942b314832cb11208ce774c757767893a84f52c46a41185f2248989f".to_string());

        let req_data = VerifyAttestation {
            attestation_doc: hex::encode(fs::read("./attestation_doc").unwrap()),
            pcrs,
            min_cpus: 2,
            min_mem: 4134580224,
            max_age: 300000000,
            signature: hex::encode(sig),
            secp_key: hex::encode(&secp_pub_key).clone(),
        };
        let req = test::TestRequest::post()
            .uri("/verify/attestation")
            .set_json(req_data)
            .to_request();

        let resp: VerifyAttestationResponse = test::call_and_read_body_json(&app, req).await;

        println!("resp sig: {}", resp.sig);
        println!("resp secpkey: {}", resp.secp_key);
    }

    #[actix_web::test]
    async fn test_attestation() {
        println!("testing");
        let attestation_doc = fs::read("./attestation_doc").unwrap();
        let mut pcrs = Vec::new();
        pcrs.push("3a2c64486fc890a1f65e82c195632b35a1b97d7595c666b8c83e91b56b92568abbeca0829269e40e4b76a6df963157da".to_string());

        pcrs.push("be9dc8acb9b26e67f2919fe877f94271c79289989455013c66a5f2cc637a9355665bc9d89b7aed986f7b4c269acc1233".to_string());
        pcrs.push("2cd79888cf800407c2bdd2165be71b8484561430942b314832cb11208ce774c757767893a84f52c46a41185f2248989f".to_string());
        let result = oyster::verify(attestation_doc, pcrs, 2, 4134580224, 300000000).unwrap();
        println!("publickey: {:?}", result);
    }

    #[actix_web::test]
    async fn test_signature_verification() {
        let enclave_pub_key = fs::read("./enclave_public.key").unwrap();
        let enclave_priv_key = fs::read("./enclave_private.key").unwrap();
        let secp_priv_key = fs::read("./secret.key").unwrap();
        let secp_priv_key = secp256k1::SecretKey::from_slice(&secp_priv_key).unwrap();
        let secp = secp256k1::Secp256k1::new();
        let secp_pub_key = secp_priv_key.public_key(&secp).serialize_uncompressed();
        let msg_to_sign = verification_message(&hex::encode(secp_pub_key));
        let mut sig = [0u8; 64];
        unsafe {
            let is_signed = crypto_sign_detached(
                sig.as_mut_ptr(),
                std::ptr::null_mut(),
                msg_to_sign.as_ptr(),
                msg_to_sign.len() as u64,
                enclave_priv_key.as_ptr(),
            );
            if is_signed != 0 {
                panic!("not signed");
            }
        }

        unsafe {
            let is_verified = crypto_sign_verify_detached(
                sig.clone().as_mut_ptr(),
                msg_to_sign.as_ptr(),
                msg_to_sign.len() as u64,
                enclave_pub_key.as_ptr(),
            );
            if is_verified != 0 {
                panic!("not verified");
            }
        }
    }
}
