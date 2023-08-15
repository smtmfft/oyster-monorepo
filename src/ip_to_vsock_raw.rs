use anyhow::{Context, Result};
use socket2::{Domain, SockAddr, Socket, Type};

fn main() -> Result<()> {
    let vsock_socket =
        Socket::new(Domain::VSOCK, Type::STREAM, None).context("failed to create vsock socket")?;
    vsock_socket
        .connect(&SockAddr::vsock(3, 1200))
        .context("failed to connect vsock socket")?;

    Ok(())
}
