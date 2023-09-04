use crate::types::handlers::AppState;
use actix_web::{post, web, App, HttpServer, Responder, Result};
use libsodium_sys::crypto_sign;
use oyster::verify;
use serde::Deserialize;
#[derive(Deserialize)]
struct VerifyAttestation {
    attestation_doc: Vec<u8>,
    pcrs: Vec<String>,
    min_cpus: usize,
    min_mem: usize,
    max_age: usize,
}

#[derive(Deserialize)]
struct VerifyAttestationResponse {
    sig: [u8; 64],
}

#[derive(Debug, Display, Error)]
pub enum UserError {
    InternalServerError,
}

#[post("/verify/attestation")]
async fn verify(
    attestation: web::Json<VerifyAttestation>,
    state: web::Data<AppState>,
) -> actix_web::Result<impl Responder, UserError> {
    let result = match verify(
        attestation.attestation_doc,
        attestation.pcrs,
        attestation.min_cpus,
        attestation.min_mem,
        attestation.max_age,
    ) {
        Ok(_) => "true",
        Err(_) => "false",
    };

    let mut sig = [0u8; 64];
    const SIG_PREFIX: &str = "signed-attestation-verification-";
    let msg_to_sign = format!("{}{}", SIG_PREFIX.to_string(), result);
    unsafe {
        let is_signed = crypto_sign(
            sig.as_mut_ptr(),
            std::ptr::null_mut(),
            msg_to_sign.as_ptr(),
            msg_to_sign.len() as u64,
            state.private_key.as_ptr(),
        );
        if is_signed != 0 {
            return UserError::InternalServerError;
        }
    }
    Ok(web::Json(VerifyAttestationResponse { sig }))
}
