use serde::Deserialize;

pub struct AppState {
    pub enclave_private_key: Vec<u8>,
    pub enclave_public_key: Vec<u8>,
    pub scep_private_key: secp256k1::SecretKey,
    pub scep_public_key: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttestationVerificationResponse {
    pub signed_message: String,
}
