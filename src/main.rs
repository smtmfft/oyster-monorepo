use actix_web::{web, App, HttpServer};
use std::error::Error;
use std::fs;

mod handlers;
mod types;

use clap::Parser;
use types::handlers::AppState;
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// path to enclave public key file
    #[arg(short, long)]
    enclavepublickey: String,

    /// path to secp private key file
    #[arg(short, long)]
    secpprivatekey: String,

    /// server ip
    #[arg(short, long)]
    ip: String,

    /// server port
    #[arg(short, long)]
    port: u16,
}

#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let enclave_public_key = fs::read(cli.enclavepublickey.clone())?;
    let secp_private_key = fs::read(cli.secpprivatekey.clone())?;
    let secp_private_key = secp256k1::SecretKey::from_slice(&secp_private_key)?;
    let secp = secp256k1::Secp256k1::new();
    let secp_public_key = secp_private_key.public_key(&secp).serialize_uncompressed();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                enclave_public_key: enclave_public_key.clone(),
                secp_private_key: secp_private_key.clone(),
                secp_public_key,
            }))
            .service(handlers::attestationdoc::verify)
    })
    .bind((cli.ip.clone(), cli.port))?
    .run();
    println!("api server running at {}:{}", cli.ip, cli.port);
    server.await?;
    Ok(())
}
