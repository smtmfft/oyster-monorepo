use std::error::Error;

use axum::{extract::Query, http::StatusCode, routing::get, Router};
use clap::Parser;
use oyster_attestation_server_custom::{get_attestation_doc, get_hex_attestation_doc};

async fn handle_raw(
    public_key: Query<Option<String>>,
    user_data: Query<Option<String>>,
    nonce: Query<Option<String>>,
) -> Result<Vec<u8>, (StatusCode, String)> {
    let public_key = public_key
        .as_ref()
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode public key: {e:?}"),
            )
        })?;
    let user_data = user_data
        .as_ref()
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode user data: {e:?}"),
            )
        })?;
    let nonce = nonce
        .as_ref()
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode nonce: {e:?}"),
            )
        })?;

    Ok(get_attestation_doc(
        public_key.as_deref(),
        user_data.as_deref(),
        nonce.as_deref(),
    ))
}

async fn handle_hex(
    public_key: Query<Option<String>>,
    user_data: Query<Option<String>>,
    nonce: Query<Option<String>>,
) -> Result<String, (StatusCode, String)> {
    let public_key = public_key
        .as_ref()
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode public key: {e:?}"),
            )
        })?;
    let user_data = user_data
        .as_ref()
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode user data: {e:?}"),
            )
        })?;
    let nonce = nonce
        .as_ref()
        .map(|x| hex::decode(x.as_bytes()))
        .transpose()
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to decode nonce: {e:?}"),
            )
        })?;

    Ok(get_hex_attestation_doc(
        public_key.as_deref(),
        user_data.as_deref(),
        nonce.as_deref(),
    ))
}

/// http server for handling attestation document requests
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// ip address of the server (e.g. 127.0.0.1:1300)
    #[arg(short, long)]
    ip_addr: String,

    /// path to public key file (e.g. /app/id.pub)
    #[arg(short, long)]
    pub_key: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // leak in order to get a static slice
    // okay to do since it will get cleaned up on exit
    let pub_key = std::fs::read(cli.pub_key)?.leak::<'static>();
    println!("pub key: {:02x?}", pub_key);

    let app = Router::new()
        .route(
            "/attestation/raw",
            get(|| async { oyster_attestation_server_custom::get_attestation_doc(pub_key) }),
        )
        .route(
            "/attestation/hex",
            get(|| async { oyster_attestation_server_custom::get_hex_attestation_doc(pub_key) }),
        );
    let listener = tokio::net::TcpListener::bind(&cli.ip_addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}
