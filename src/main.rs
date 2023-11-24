#[macro_use]
extern crate lazy_static;
use actix_web::{web, App, HttpServer};
use std::error::Error;
use std::fs;

mod config;
mod handlers;
mod types;

use types::handlers::AppState;

// global config
lazy_static! {
    static ref CONFIG: config::Configuration =
        config::Configuration::new().expect("config can be loaded");
}
#[actix_web::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let enclave_public_key = fs::read(CONFIG.enclave.publickeypath.clone())?;
    let scep_private_key = fs::read(CONFIG.scep.privatekeypath.clone())?;
    let scep_private_key = secp256k1::SecretKey::from_slice(&scep_private_key)?;
    let secp = secp256k1::Secp256k1::new();
    let scep_public_key = scep_private_key.public_key(&secp).serialize_uncompressed();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(AppState {
                enclave_public_key: enclave_public_key.clone(),
                scep_private_key: scep_private_key.clone(),
                scep_public_key,
            }))
            .service(handlers::attestationdoc::verify)
    })
    .bind((CONFIG.server.ip.clone(), CONFIG.server.port))?
    .run();
    println!(
        "api server running at {}:{}",
        CONFIG.server.ip, CONFIG.server.port
    );
    server.await?;
    Ok(())
}
