use thiserror::Error;

use nfq::Verdict;
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
}
