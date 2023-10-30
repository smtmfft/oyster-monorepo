use crate::types::handlers::AppState;
use actix_web::{error, http::StatusCode, post, web, Responder};
use derive_more::{Display, Error};
use ethers;
use hex;
use libsodium_sys::crypto_sign;
use libsodium_sys::crypto_sign_verify_detached;
use oyster;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, Bytes};

#[derive(Deserialize)]
struct VerifyAttestation {
    attestation_doc: Vec<u8>,
    pcrs: Vec<String>,
    min_cpus: usize,
    min_mem: usize,
    max_age: usize,
    signature: Vec<u8>,
}

#[serde_as]
#[derive(Serialize)]
struct VerifyAttestationResponse {
    #[serde_as(as = "Bytes")]
    sig: [u8; 64],
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
    attestation: Vec<u8>,
    source_pubkey: &[u8; 32],
    enclave_pubkey: &[u8; 32],
    pcr_0: Vec<u8>,
    pcr_1: Vec<u8>,
    pcr_2: Vec<u8>,
    enclave_cpu: usize,
    enclave_mem: usize,
) -> Vec<u8> {
    let mut encoded_data = Vec::new();
    encoded_data.push(ethers::abi::Token::Bytes(attestation));
    encoded_data.push(ethers::abi::Token::Address(address_from_pubkey(
        source_pubkey,
    )));
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

fn address_from_pubkey(pub_key: &[u8; 32]) -> ethers::types::Address {
    let hash = ethers::utils::keccak256(pub_key);
    ethers::types::Address::from_slice(&hash[12..])
}

fn verification_message(pubkey: &Vec<u8>) -> String {
    const PREFIX: &str = "attestation-verification-";
    format!("{}{:?}", PREFIX.to_string(), pubkey)
}
#[post("/verify/attestation")]
async fn verify(
    attestation: web::Json<VerifyAttestation>,
    state: web::Data<AppState>,
) -> actix_web::Result<impl Responder, UserError> {
    let msg = verification_message(&state.scep_public_key);
    unsafe {
        let is_verified = crypto_sign_verify_detached(
            attestation.signature.clone().as_mut_ptr(),
            msg.as_ptr(),
            msg.len() as u64,
            state.enclave_public_key.as_ptr(),
        );
        if is_verified != 0 {
            return Err(UserError::InternalServerError);
        }
    }
    let pub_key = oyster::verify(
        attestation.attestation_doc.clone(),
        attestation.pcrs.clone(),
        attestation.min_cpus,
        attestation.min_mem,
        attestation.max_age,
    )
    .map_err(|_| UserError::InternalServerError)?;

    let abi_encoded = abi_encode(
        attestation.attestation_doc.clone(),
        &pub_key,
        &pub_key,
        attestation.pcrs[0].clone().into_bytes(),
        attestation.pcrs[1].clone().into_bytes(),
        attestation.pcrs[2].clone().into_bytes(),
        attestation.min_cpus,
        attestation.min_mem,
    );
    let mut sig = [0u8; 64];
    const SIG_PREFIX: &str = "signed-attestation-verification-";
    let msg_to_sign = format!("{}{}", SIG_PREFIX.to_string(), hex::encode(abi_encoded));
    unsafe {
        let is_signed = crypto_sign(
            sig.as_mut_ptr(),
            std::ptr::null_mut(),
            msg_to_sign.as_ptr(),
            msg_to_sign.len() as u64,
            state.scep_private_key.as_ptr(),
        );
        if is_signed != 0 {
            return Err(UserError::InternalServerError);
        }
    }
    Ok(web::Json(VerifyAttestationResponse { sig }))
}

#[cfg(test)]

mod tests {
    use super::*;
    use std::fs;
    #[actix_web::test]
    async fn test_attestation() {
        let attestation_doc = fs::read("./attestation_doc").unwrap();
        let mut pcrs = Vec::new();
        pcrs.push("55ba3fa530581218580584144ce29c62c1c92f93c0bfcefead49c5fa174f15ba49a66a037957377abe34591364cbe935".to_string());
        pcrs.push("be9dc8acb9b26e67f2919fe877f94271c79289989455013c66a5f2cc637a9355665bc9d89b7aed986f7b4c269acc1233".to_string());
        pcrs.push("f064a1f5d2c0f49e3023a2f121c58ff5567ed423180da1f232f42093074b32f3e471b6bc946b9003e4725c9c2168ff25".to_string());
        let result = oyster::verify(attestation_doc, pcrs, 2, 4134580224, 300000000).unwrap();
        println!("publickey: {:?}", result);
    }
}
