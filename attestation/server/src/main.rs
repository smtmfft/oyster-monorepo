use std::error::Error;

use axum::{routing::get, Router};
use clap::Parser;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_vsock::{VsockAddr, VsockListener};

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
async fn main_deprecated() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();

    // leak in order to get a static slice
    // okay to do since it will get cleaned up on exit
    let pub_key = std::fs::read(cli.pub_key)?.leak::<'static>();
    println!("pub key: {:02x?}", pub_key);

    let app = Router::new()
        .route(
            "/attestation/raw",
            get(|| async { oyster_attestation_server::get_attestation_doc(pub_key) }),
        )
        .route(
            "/attestation/hex",
            get(|| async { oyster_attestation_server::get_hex_attestation_doc(pub_key) }),
        );
    let listener = tokio::net::TcpListener::bind(&cli.ip_addr).await?;

    axum::serve(listener, app).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let input_pub_key = std::fs::read(cli.pub_key)?.leak::<'static>();
    println!("load pub key: {:02x?}", input_pub_key);

    let mut listener = VsockListener::bind(VsockAddr::new(tokio_vsock::VMADDR_CID_ANY, 8080))?;
    println!("Listening on vsock port 8080");

    while let Ok((mut stream, addr)) = listener.accept().await {
        println!("Accepted connection from CID: {}", addr.cid());
        let pub_key = input_pub_key.to_vec();

        tokio::spawn(async move {
            let mut buf = [0; 1024];
            let n = match stream.read(&mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    eprintln!("Failed to read from connection: {}", e);
                    return;
                }
            };

            let request = String::from_utf8_lossy(&buf[..n]);
            let response = if request.contains("GET /attestation/raw") {
                let doc = oyster_attestation_server::get_attestation_doc(&pub_key);
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\n\r\n{:?}",
                    doc
                )
            } else if request.contains("GET /attestation/hex") {
                let doc = oyster_attestation_server::get_hex_attestation_doc(&pub_key);
                format!("HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n{}", doc)
            } else {
                "HTTP/1.1 404 Not Found\r\n\r\nNot Found".to_string()
            };

            if let Err(e) = stream.write_all(response.as_bytes()).await {
                eprintln!("Failed to write to connection: {}", e);
            }
        });
    }

    Ok(())
}
