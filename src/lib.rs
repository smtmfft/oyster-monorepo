use std::thread::sleep;
use std::time::Duration;

use thiserror::Error;

use nfq::{Queue, Verdict};
use socket2::{Domain, Protocol, Type};

#[derive(Error, Debug)]
pub enum ProxyError {
    #[error("ip socket error")]
    IpError(#[source] SocketError),
    #[error("vsock socket error")]
    VsockError(#[source] SocketError),
    #[error("nfqueue error")]
    NfqError(#[source] SocketError),
}

#[derive(Error, Debug)]
pub enum SocketError {
    #[error(
        "failed to create socket with domain {domain:?}, type {r#type:?}, protocol {protocol:?}"
    )]
    CreateError {
        domain: Domain,
        r#type: Type,
        protocol: Option<Protocol>,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to bind socket to {addr}")]
    BindError {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to listen with socket on {addr}")]
    ListenError {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to accept with socket on {addr}")]
    AcceptError {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to connect socket to {addr}")]
    ConnectError {
        addr: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read from socket")]
    ReadError(#[source] std::io::Error),
    #[error("failed to write to socket")]
    WriteError(#[source] std::io::Error),
    #[error("failed to shutdown {side:?}")]
    ShutdownError {
        side: std::net::Shutdown,
        #[source]
        source: std::io::Error,
    },
    #[error("unexpected eof")]
    EofError,
    #[error("failed to open socket {0}")]
    OpenError(String, #[source] std::io::Error),
    #[error("failed to set verdict {0:?}")]
    VerdictError(Verdict, #[source] std::io::Error),
    #[error("failed to set option {0}")]
    OptionError(String, #[source] std::io::Error),
}

pub fn run_with_backoff<P: Clone, R, F: Fn(P) -> Result<R, ProxyError>>(
    f: F,
    p: P,
    backoff: &mut u64,
    max_backoff: u64,
) -> R {
    loop {
        match f(p.clone()) {
            Ok(r) => return r,
            Err(err) => {
                println!("{:?}", anyhow::Error::from(err));

                sleep(Duration::from_secs(*backoff));
                *backoff = (*backoff * 2).clamp(1, max_backoff);
            }
        };
    }
}

fn new_nfq(addr: u16) -> Result<Queue, ProxyError> {
    let mut queue = Queue::open()
        .map_err(|e| SocketError::OpenError(addr.to_string(), e))
        .map_err(ProxyError::NfqError)?;
    queue
        .bind(addr)
        .map_err(|e| SocketError::BindError {
            addr: addr.to_string(),
            source: e,
        })
        .map_err(ProxyError::NfqError)?;

    Ok(queue)
}

pub fn new_nfq_with_backoff(addr: u16, backoff: &mut u64) -> Queue {
    run_with_backoff(new_nfq, addr, backoff, 64)
}
