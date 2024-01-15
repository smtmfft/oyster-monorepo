use serde::Deserialize;

pub struct AppState {
    pub ed25519_public_key: Vec<u8>,
    pub secp256k1_private_key: secp256k1::SecretKey,
    pub secp256k1_public_key: [u8; 65],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttestationVerificationResponse {
    pub signed_message: String,
}
