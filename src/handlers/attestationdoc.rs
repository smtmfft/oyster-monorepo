use crate::types::handlers::AppState;
use actix_web::{error, http::StatusCode, post, web, App, HttpServer, Responder, Result};
use derive_more::{Display, Error};
use libsodium_sys::crypto_sign;
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

#[post("/verify/attestation")]
async fn verify(
    attestation: web::Json<VerifyAttestation>,
    state: web::Data<AppState>,
) -> actix_web::Result<impl Responder, UserError> {
    let result = match oyster::verify(
        attestation.attestation_doc.clone(),
        attestation.pcrs.clone(),
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
            return Err(UserError::InternalServerError);
        }
    }
    Ok(web::Json(VerifyAttestationResponse { sig }))
}
