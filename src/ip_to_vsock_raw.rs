use anyhow::{Context, Result};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

fn main() -> Result<()> {
    let vsock_socket =
        Socket::new(Domain::VSOCK, Type::STREAM, None).context("failed to create vsock socket")?;
    vsock_socket
        .connect(&SockAddr::vsock(3, 1200))
        .context("failed to connect vsock socket")?;

    let ip_socket = Socket::new(Domain::IPV4, Type::RAW, Protocol::TCP.into())
        .context("failed to create ip socket")?;
    ip_socket
        .bind_device("lo".as_bytes().into())
        .context("failed to bind ip socket")?;

    Ok(())
}
