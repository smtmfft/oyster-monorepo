use std::array::TryFromSliceError;
use std::error::Error;
use std::num::TryFromIntError;

use actix_web::{error, http::StatusCode, post, web, Responder};
use ethers::types::U256;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub struct AppState {
    pub secp256k1_secret: secp256k1::SecretKey,
    pub secp256k1_public: [u8; 64],
}

#[derive(Deserialize, Serialize)]
struct RawAttestation {
    attestation: String,
}

#[derive(Deserialize, Serialize)]
struct HexAttestation {
    attestation: String,
}

#[derive(Serialize, Deserialize)]
struct VerifyAttestationResponse {
    signature: String,
    secp256k1_public: String,
    pcr0: String,
    pcr1: String,
    pcr2: String,
    timestamp: usize,
    verifier_secp256k1_public: String,
}

#[derive(Error)]
pub enum UserError {
    #[error("error while decoding attestation doc from hex")]
    AttestationDecode(#[source] hex::FromHexError),
    #[error("error while verifying attestation")]
    AttestationVerification(#[source] oyster::AttestationError),
    #[error("error while decoding secp256k1 key from hex")]
    Secp256k1Decode(#[source] hex::FromHexError),
    #[error("invalid secp256k1 length, expected 64")]
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

// keccak256(
//     abi.encode(
//         keccak256("EIP712Domain(string name,string version)"),
//         keccak256("marlin.oyster.AttestationVerifier"),
//         keccak256("1")
//     )
// )
const DOMAIN_SEPARATOR: [u8; 32] =
    hex_literal::hex!("0de834feb03c214f785e75b2828ffeceb322312d4487e2fb9640ca5fc32542c7");

// keccak256("Attestation(bytes enclavePubKey,bytes PCR0,bytes PCR1,bytes PCR2,uint256 timestampInMilliseconds)")
const ATTESTATION_TYPEHASH: [u8; 32] =
    hex_literal::hex!("6889df476ca38f3f4b417c17eb496682eb401b4f41a2259741a78acc481ea805");

fn compute_digest(
    enclave_pubkey: &[u8],
    pcr0: &[u8],
    pcr1: &[u8],
    pcr2: &[u8],
    timestamp: usize,
) -> [u8; 32] {
    let mut encoded_struct = Vec::new();
    encoded_struct.reserve_exact(32 * 6);
    encoded_struct.extend_from_slice(&ATTESTATION_TYPEHASH);
    encoded_struct.extend_from_slice(&ethers::utils::keccak256(enclave_pubkey));
    encoded_struct.extend_from_slice(&ethers::utils::keccak256(pcr0));
    encoded_struct.extend_from_slice(&ethers::utils::keccak256(pcr1));
    encoded_struct.extend_from_slice(&ethers::utils::keccak256(pcr2));
    encoded_struct.resize(32 * 6, 0);
    U256::from(timestamp).to_big_endian(&mut encoded_struct[32 * 5..32 * 6]);

    let hash_struct = ethers::utils::keccak256(encoded_struct);

    let mut encoded_message = Vec::new();
    encoded_message.reserve_exact(2 + 32 * 2);
    encoded_message.extend_from_slice(&[0x19, 0x01]);
    encoded_message.extend_from_slice(&DOMAIN_SEPARATOR);
    encoded_message.extend_from_slice(&hash_struct);

    ethers::utils::keccak256(encoded_message)
}

fn verify(
    attestation: Vec<u8>,
    secret: &secp256k1::SecretKey,
    public: &[u8; 64],
) -> actix_web::Result<impl Responder, UserError> {
    let parsed = oyster::decode_attestation(attestation.clone())
        .map_err(UserError::AttestationVerification)?;
    oyster::verify_with_timestamp(attestation, parsed.pcrs.clone(), parsed.timestamp)
        .map_err(UserError::AttestationVerification)?;

    let requester_secp256k1_public: [u8; 64] = parsed
        .public_key
        .as_slice()
        .try_into()
        .map_err(UserError::InvalidSecp256k1Length)?;

    let digest = compute_digest(
        &requester_secp256k1_public,
        &parsed.pcrs[0],
        &parsed.pcrs[1],
        &parsed.pcrs[2],
        parsed.timestamp,
    );

    let response_msg =
        secp256k1::Message::from_digest_slice(&digest).map_err(UserError::MessageGeneration)?;

    let secp = secp256k1::Secp256k1::new();
    let (recid, sig) = secp
        .sign_ecdsa_recoverable(&response_msg, secret)
        .serialize_compact();

    let sig = hex::encode(sig);
    let recid: u8 = recid
        .to_i32()
        .try_into()
        .map_err(UserError::InvalidRecovery)?;
    let recid = hex::encode([recid + 27]);

    Ok(web::Json(VerifyAttestationResponse {
        signature: sig + &recid,
        secp256k1_public: hex::encode(public),
        pcr0: hex::encode(parsed.pcrs[0]),
        pcr1: hex::encode(parsed.pcrs[1]),
        pcr2: hex::encode(parsed.pcrs[2]),
        timestamp: parsed.timestamp,
        verifier_secp256k1_public: hex::encode(public),
    }))
}

#[post("/verify/raw")]
async fn verify_raw(
    req: web::Bytes,
    state: web::Data<AppState>,
) -> actix_web::Result<impl Responder, UserError> {
    // let attestation = hex::decode(&req.attestation).map_err(UserError::AttestationDecode)?;

    verify(
        req.to_vec(),
        &state.secp256k1_secret,
        &state.secp256k1_public,
    )
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
                .service(verify_raw),
        )
        .await;

        let attestation = std::fs::read_to_string("./src/test/attestation.json").unwrap();
        println!("{}", attestation);

        let req = test::TestRequest::post()
            .uri("/verify")
            .insert_header(("Content-Type", "application/json"))
            .set_payload(attestation)
            .to_request();

        println!(
            "{}",
            String::from_utf8_lossy(&test::call_and_read_body(&app, req).await)
        );
        // let resp: VerifyAttestationResponse = test::call_and_read_body(&app, req).await.unwrap();

        // assert_eq!(resp.signature, "26a910db11f7aeba592ac151ee4f81ea03026dd3d7f8ff261533a5d0b4818df663b34889688609b97add2eec8fb66296c2dfdf818eadc8bb8b503e6ad3ab0e241b");
        // assert_eq!(resp.secp256k1_public, "89b14cb02441b6850534580800bd0a33e6ca483a9ea8f0f55de0a99fbf4a4f02a525d6bb48a7a7a80928af68e0d4ad859d699b49538a425cd35403cd1fbdf956");
    }
}
