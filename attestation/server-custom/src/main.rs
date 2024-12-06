use std::collections::HashMap;
use std::error::Error;

use axum::{extract::Query, http::StatusCode, routing::get, Router};
use clap::Parser;
use oyster_attestation_server_custom::{get_attestation_doc, get_hex_attestation_doc};

async fn handle_raw(
    Query(query): Query<HashMap<String, String>>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let public_key = query
        .get("public_key")
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode public key: {e:?}"),
            )
        })?;
    let user_data = query
        .get("user_data")
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode user data: {e:?}"),
            )
        })?;
    let nonce = query
        .get("nonce")
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode nonce: {e:?}"),
            )
        })?;

    get_attestation_doc(
        public_key.as_deref(),
        user_data.as_deref(),
        nonce.as_deref(),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate attestation doc: {e:?}"),
        )
    })
}

async fn handle_hex(
    Query(query): Query<HashMap<String, String>>,
) -> Result<String, (StatusCode, String)> {
    let public_key = query
        .get("public_key")
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode public key: {e:?}"),
            )
        })?;
    let user_data = query
        .get("user_data")
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode user data: {e:?}"),
            )
        })?;
    let nonce = query
        .get("nonce")
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode nonce: {e:?}"),
            )
        })?;

    get_hex_attestation_doc(
        public_key.as_deref(),
        user_data.as_deref(),
        nonce.as_deref(),
    )
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to generate attestation doc: {e:?}"),
        )
    })
}

/// http server for handling attestation document requests
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// ip address of the server
    #[arg(short, long, default_value = "127.0.0.1:1350")]
    ip_addr: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    let app = Router::new()
        .route("/attestation/raw", get(handle_raw))
        .route("/attestation/hex", get(handle_hex));
    let listener = tokio::net::TcpListener::bind(&cli.ip_addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}
