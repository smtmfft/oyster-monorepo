use std::array::TryFromSliceError;
use std::error::Error;
use std::num::TryFromIntError;

use actix_web::{error, http::StatusCode, post, web, Responder};
use libsodium_sys::crypto_sign_verify_detached;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub struct AppState {
    pub secp256k1_secret: secp256k1::SecretKey,
    pub secp256k1_public: [u8; 64],
}

#[derive(Deserialize, Serialize)]
struct VerifyAttestation {
    attestation: String,
    pcrs: Vec<String>,
    min_cpus: usize,
    min_mem: usize,
    timestamp: usize,
    signature: String,
    secp256k1_public: String,
}

#[derive(Serialize, Deserialize)]
struct VerifyAttestationResponse {
    signature: String,
    secp256k1_public: String,
}

#[derive(Error)]
pub enum UserError {
    #[error("error while decoding attestation doc from hex")]
    AttestationDecode(#[source] hex::FromHexError),
    #[error("error while verifying attestation")]
    AttestationVerification(#[source] oyster::AttestationError),
    #[error("error while decoding secp256k1 key from hex")]
    Secp256k1Decode(#[source] hex::FromHexError),
    #[error("invalid secp256k1 length, expected 65")]
    InvalidSecp256k1Length(#[source] TryFromSliceError),
    #[error("error while encoding signature")]
    SignatureEncoding(#[source] ethers::abi::EncodePackedError),
    #[error("invalid signature length, expected 64")]
    InvalidSignatureLength(#[source] TryFromSliceError),
    #[error("error while decoding signature")]
    SignatureDecoding(#[source] hex::FromHexError),
    #[error("Signature verification failed")]
    SignatureVerification,
    #[error("Message generation failed")]
    MessageGeneration(#[source] secp256k1::Error),
    #[error("error while decoding pcrs")]
    PCRDecode(#[source] hex::FromHexError),
    #[error("invalid recovery id")]
    InvalidRecovery(#[source] TryFromIntError),
}

impl error::ResponseError for UserError {
    fn error_response(&self) -> actix_web::HttpResponse<actix_web::body::BoxBody> {
        actix_web::HttpResponse::build(self.status_code())
            .insert_header(actix_web::http::header::ContentType::plaintext())
            .body(format!("{self:?}"))
    }

    fn status_code(&self) -> actix_web::http::StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl std::fmt::Debug for UserError {
    // pretty print like anyhow
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)?;

        if self.source().is_some() {
            writeln!(f, "\n\nCaused by:")?;
        }

        let mut err: &dyn Error = self;
        loop {
            let Some(source) = err.source() else { break };
            writeln!(f, "\t{}", source)?;

            err = source;
        }

        Ok(())
    }
}

fn abi_encode(
    prefix: String,
    enclave_pubkey: Vec<u8>,
    pcr_0: Vec<u8>,
    pcr_1: Vec<u8>,
    pcr_2: Vec<u8>,
    enclave_cpu: usize,
    enclave_mem: usize,
    timestamp: usize,
) -> Vec<u8> {
    ethers::abi::encode(&[
        ethers::abi::Token::String(prefix),
        ethers::abi::Token::Bytes(enclave_pubkey),
        ethers::abi::Token::Bytes(pcr_0),
        ethers::abi::Token::Bytes(pcr_1),
        ethers::abi::Token::Bytes(pcr_2),
        ethers::abi::Token::Uint(enclave_cpu.into()),
        ethers::abi::Token::Uint(enclave_mem.into()),
        ethers::abi::Token::Uint(timestamp.into()),
    ])
}

#[post("/verify")]
async fn verify(
    req: web::Json<VerifyAttestation>,
    state: web::Data<AppState>,
) -> actix_web::Result<impl Responder, UserError> {
    let attestation = hex::decode(&req.attestation).map_err(UserError::AttestationDecode)?;
    let requester_ed25519_public = oyster::verify_with_timestamp(
        attestation,
        req.pcrs.clone(),
        req.min_cpus,
        req.min_mem,
        req.timestamp,
    )
    .map_err(UserError::AttestationVerification)?;
    let requester_secp256k1_public =
        hex::decode(&req.secp256k1_public).map_err(UserError::Secp256k1Decode)?;
    let requester_signature: [u8; 64] = hex::decode(&req.signature)
        .map_err(UserError::SignatureDecoding)?
        .as_slice()
        .try_into()
        .map_err(UserError::InvalidSignatureLength)?;

    let requester_msg = ethers::abi::encode_packed(&[
        ethers::abi::Token::String("attestation-verification-".to_string()),
        ethers::abi::Token::Bytes(requester_secp256k1_public.clone()),
    ])
    .map_err(UserError::SignatureEncoding)?;
    let ret = unsafe {
        crypto_sign_verify_detached(
            requester_signature.as_ptr(),
            requester_msg.as_ptr(),
            requester_msg.len() as u64,
            requester_ed25519_public.as_ptr(),
        )
    };
    if ret != 0 {
        return Err(UserError::SignatureVerification);
    }

    let requester_secp256k1_public: [u8; 64] = requester_secp256k1_public
        .as_slice()
        .try_into()
        .map_err(UserError::InvalidSecp256k1Length)?;

    let abi_encoded = abi_encode(
        "Enclave Attestation Verified".to_string(),
        requester_secp256k1_public.into(),
        hex::decode(&req.pcrs[0]).map_err(UserError::PCRDecode)?,
        hex::decode(&req.pcrs[1]).map_err(UserError::PCRDecode)?,
        hex::decode(&req.pcrs[2]).map_err(UserError::PCRDecode)?,
        req.min_cpus,
        req.min_mem,
        req.timestamp,
    );

    let response_msg = ethers::utils::keccak256(abi_encoded);
    let response_msg = secp256k1::Message::from_digest_slice(&response_msg)
        .map_err(UserError::MessageGeneration)?;

    let secp = secp256k1::Secp256k1::new();
    let (recid, sig) = secp
        .sign_ecdsa_recoverable(&response_msg, &state.secp256k1_secret)
        .serialize_compact();

    let sig = hex::encode(sig);
    let recid: u8 = recid
        .to_i32()
        .try_into()
        .map_err(UserError::InvalidRecovery)?;
    let recid = hex::encode([recid + 27]);

    Ok(web::Json(VerifyAttestationResponse {
        signature: sig + &recid,
        secp256k1_public: hex::encode(state.secp256k1_public),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, web, App};

    #[actix_web::test]
    async fn test_handler() {
        let secp256k1_secret = std::fs::read("./src/test/secp256k1.sec").unwrap();
        let secp256k1_public = std::fs::read("./src/test/secp256k1.pub").unwrap();

        let secp256k1_secret = secp256k1::SecretKey::from_slice(&secp256k1_secret).unwrap();
        let secp256k1_public: [u8; 64] = secp256k1_public.try_into().unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(AppState {
                    secp256k1_secret,
                    secp256k1_public,
                }))
                .service(verify),
        )
        .await;

        let attestation = std::fs::read("./src/test/attestation.json").unwrap();
        let req = test::TestRequest::post()
            .uri("/verify")
            .insert_header(("Content-Type", "application/json"))
            .set_payload(attestation)
            .to_request();

        let resp: VerifyAttestationResponse = test::call_and_read_body_json(&app, req).await;

        assert_eq!(resp.signature, "26a910db11f7aeba592ac151ee4f81ea03026dd3d7f8ff261533a5d0b4818df663b34889688609b97add2eec8fb66296c2dfdf818eadc8bb8b503e6ad3ab0e241b");
        assert_eq!(resp.secp256k1_public, "89b14cb02441b6850534580800bd0a33e6ca483a9ea8f0f55de0a99fbf4a4f02a525d6bb48a7a7a80928af68e0d4ad859d699b49538a425cd35403cd1fbdf956");
    }
}
