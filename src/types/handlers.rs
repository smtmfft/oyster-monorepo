use serde::Deserialize;

pub struct AppState {
    pub enclave_public_key: Vec<u8>,
    pub secp_private_key: secp256k1::SecretKey,
    pub secp_public_key: [u8; 65],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttestationVerificationResponse {
    pub signed_message: String,
}
