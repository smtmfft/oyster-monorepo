use std::error::Error;
use std::num::TryFromIntError;

use actix_web::{error, get, http::StatusCode, web, Responder};
use libsodium_sys::crypto_sign_verify_detached;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub struct AppState {
    pub secp256k1_secret: secp256k1::SecretKey,
    pub secp256k1_public: [u8; 65],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttestationVerificationResponse {
    pub signed_message: String,
}

#[derive(Deserialize, Serialize)]
struct VerifyAttestation {
    attestation: String,
    pcrs: Vec<String>,
    min_cpus: usize,
    min_mem: usize,
    max_age: usize,
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
    AttestationDecode(hex::FromHexError),
    #[error("error while verifying attestation")]
    AttestationVerification(oyster::AttestationError),
    #[error("error while decoding secp256k1 key from hex")]
    Secp256k1Decode(hex::FromHexError),
    #[error("error while encoding signature")]
    SignatureEncoding(ethers::abi::EncodePackedError),
    #[error("error while decoding signature")]
    SignatureDecoding(hex::FromHexError),
    #[error("Signature verification failed")]
    SignatureVerification,
    #[error("Message generation failed")]
    MessageGeneration(secp256k1::Error),
    #[error("error while decoding pcrs")]
    PCRDecode(hex::FromHexError),
    #[error("invalid recovery id")]
    InvalidRecovery(TryFromIntError),
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
    enclave_pubkey: &[u8; 65],
    pcr_0: Vec<u8>,
    pcr_1: Vec<u8>,
    pcr_2: Vec<u8>,
    enclave_cpu: usize,
    enclave_mem: usize,
) -> Vec<u8> {
    ethers::abi::encode(&[
        ethers::abi::Token::String(prefix),
        ethers::abi::Token::Address(address_from_pubkey(enclave_pubkey)),
        ethers::abi::Token::Bytes(pcr_0),
        ethers::abi::Token::Bytes(pcr_1),
        ethers::abi::Token::Bytes(pcr_2),
        ethers::abi::Token::Uint(enclave_cpu.into()),
        ethers::abi::Token::Uint(enclave_mem.into()),
    ])
}

fn address_from_pubkey(pub_key: &[u8; 65]) -> ethers::types::Address {
    let hash = ethers::utils::keccak256(&pub_key[1..]);
    ethers::types::Address::from_slice(&hash[12..])
}

#[get("/verify")]
async fn verify(
    req: web::Json<VerifyAttestation>,
    state: web::Data<AppState>,
) -> actix_web::Result<impl Responder, UserError> {
    let attestationdoc_bytes =
        hex::decode(&req.attestation).map_err(UserError::AttestationDecode)?;
    let requester_pub_key = oyster::verify(
        attestationdoc_bytes,
        req.pcrs.clone(),
        req.min_cpus,
        req.min_mem,
        req.max_age,
    )
    .map_err(UserError::AttestationVerification)?;

    let secp256k1_pubkey =
        hex::decode(&req.secp256k1_public).map_err(UserError::Secp256k1Decode)?;
    let msg = ethers::abi::encode_packed(&[
        ethers::abi::Token::String("attestation-verification-".to_string()),
        ethers::abi::Token::Bytes(secp256k1_pubkey),
    ])
    .map_err(UserError::SignatureEncoding)?;
    let sig_bytes = hex::decode(&req.signature).map_err(UserError::SignatureDecoding)?;

    unsafe {
        let is_verified = crypto_sign_verify_detached(
            sig_bytes.clone().as_mut_ptr(),
            msg.as_ptr(),
            msg.len() as u64,
            requester_pub_key.as_ptr(),
        );
        if is_verified != 0 {
            return Err(UserError::SignatureVerification);
        }
    }

    let mut pubkey_bytes = [0u8; 65];
    hex::decode_to_slice(&req.secp256k1_public, &mut pubkey_bytes)
        .map_err(UserError::Secp256k1Decode)?;

    let abi_encoded = abi_encode(
        "Enclave Attestation Verified".to_string(),
        &pubkey_bytes,
        hex::decode(req.pcrs[0].clone()).map_err(UserError::PCRDecode)?,
        hex::decode(req.pcrs[1].clone()).map_err(UserError::PCRDecode)?,
        hex::decode(req.pcrs[2].clone()).map_err(UserError::PCRDecode)?,
        req.min_cpus,
        req.min_mem,
    );

    let msg_to_sign = ethers::utils::keccak256(abi_encoded);
    let msg_to_sign = secp256k1::Message::from_digest_slice(&msg_to_sign)
        .map_err(UserError::MessageGeneration)?;

    let secp = secp256k1::Secp256k1::new();
    let (recid, sig) = secp
        .sign_ecdsa_recoverable(&msg_to_sign, &state.secp256k1_secret)
        .serialize_compact();

    let sig = hex::encode(sig);
    let recid: u8 = recid
        .to_i32()
        .try_into()
        .map_err(UserError::InvalidRecovery)?;
    let recid = hex::encode([recid]);

    Ok(web::Json(VerifyAttestationResponse {
        signature: sig + &recid,
        secp256k1_public: hex::encode(state.secp256k1_public),
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
        let enclave_priv_key = fs::read("./enclave_private.key").unwrap();
        let secp_priv_key = fs::read("./secret.key").unwrap();
        let secp_priv_key = secp256k1::SecretKey::from_slice(&secp_priv_key).unwrap();
        let secp = secp256k1::Secp256k1::new();

        let secp_pub_key = secp_priv_key.public_key(&secp).serialize_uncompressed();
        println!("address : {}", address_from_pubkey(&secp_pub_key));
        let msg_to_sign = ethers::abi::encode_packed(&[
            ethers::abi::Token::String("attestation-verification-".to_string()),
            ethers::abi::Token::Bytes(secp_pub_key.to_vec()),
        ])
        .unwrap();
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
                    secp256k1_secret: secp_priv_key.clone(),
                    secp256k1_public: secp_pub_key.clone(),
                }))
                .service(verify),
        )
        .await;
        let mut pcrs = Vec::new();
        pcrs.push("3a2c64486fc890a1f65e82c195632b35a1b97d7595c666b8c83e91b56b92568abbeca0829269e40e4b76a6df963157da".to_string());
        pcrs.push("be9dc8acb9b26e67f2919fe877f94271c79289989455013c66a5f2cc637a9355665bc9d89b7aed986f7b4c269acc1233".to_string());
        pcrs.push("2cd79888cf800407c2bdd2165be71b8484561430942b314832cb11208ce774c757767893a84f52c46a41185f2248989f".to_string());

        let req_data = VerifyAttestation {
            attestation: hex::encode(fs::read("./attestation_doc").unwrap()),
            pcrs,
            min_cpus: 2,
            min_mem: 4134580224,
            max_age: 300000000,
            signature: hex::encode(sig),
            secp256k1_public: hex::encode(&secp_pub_key).clone(),
        };
        let req = test::TestRequest::post()
            .uri("/verify/attestation")
            .set_json(req_data)
            .to_request();

        let resp: VerifyAttestationResponse = test::call_and_read_body_json(&app, req).await;

        println!("resp sig: {}", resp.signature);
        println!("resp secpkey: {}", resp.secp256k1_public);
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
        let msg_to_sign = ethers::abi::encode_packed(&[
            ethers::abi::Token::String("attestation-verification-".to_string()),
            ethers::abi::Token::Bytes(secp_pub_key.to_vec()),
        ])
        .unwrap();
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
