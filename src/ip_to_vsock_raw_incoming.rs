// Summarizing NAT insights
//
// v1: track (src_port, dst_addr, dst_port)
// or any form of stateful NAT for that matter
//
// 1. tracking and assigning ports is a headache
// 2. does not easily scale to many threads and I want to avoid tokio/async if possible
// 3. there should be a fast path
//
// Host does not have any real services running on it
// Therefore, we have a lot of latitude in port assignment
//
// Let us direct map some port ranges directly to skip lookups
// 80, 443, 1024-61439 of enclave -> 80, 443, 1024-61439 of host
//
// Connections to and from the enclave now work directly
// More importantly, we do not need a stateful NAT!
// This means no lookups affecting performance
// This also means the NAT can easily be multi threaded without needing locks
//
// On the enclave, we set ephemeral ports to stay within the same range
// It seems to already be the case in my local system, the max is 60999
//
// Only downside - some ports need to be reserved for the host to use
// 61440-65535 is available for that
// This means the enclave cannot use these ports to reach the internet
// While this should not be an issue in most cases since ephemeral ports do not extend there
// and most applications use ports lower than ephemeral, it _is_ a breaking change

use std::io::Read;
use std::thread::sleep;
use std::time::Duration;

use socket2::{Domain, Protocol, SockAddr, Socket, Type};

use raw_proxy::{ProxyError, SocketError};

fn handle_conn(conn_socket: &mut Socket, ip_socket: &mut Socket) -> Result<(), ProxyError> {
    let mut buf = vec![0u8; 65535].into_boxed_slice();
    loop {
        // read till total size
        conn_socket
            .read_exact(&mut buf[0..4])
            .map_err(SocketError::ReadError)
            .map_err(ProxyError::VsockError)?;

        let size: usize = u16::from_be_bytes(buf[2..4].try_into().unwrap()).into();

        // read till full frame
        conn_socket
            .read_exact(&mut buf[4..size])
            .map_err(SocketError::ReadError)
            .map_err(ProxyError::VsockError)?;

        // send through ip sock
        let mut total_sent = 0;
        while total_sent < size {
            let size = ip_socket
                .send(&buf[total_sent..size])
                .map_err(SocketError::WriteError)
                .map_err(ProxyError::IpError)?;
            total_sent += size;
        }
    }
}

fn new_vsock_socket(addr: &SockAddr) -> Result<Socket, ProxyError> {
    let vsock_socket = Socket::new(Domain::VSOCK, Type::STREAM, None)
        .map_err(|e| SocketError::CreateError {
            domain: Domain::VSOCK,
            r#type: Type::STREAM,
            protocol: None,
            source: e,
        })
        .map_err(ProxyError::VsockError)?;
    vsock_socket
        .connect(addr)
        .map_err(|e| SocketError::ConnectError {
            addr: format!("{:?}, {:?}", addr.domain(), addr.as_vsock_address()),
            source: e,
        })
        .map_err(ProxyError::VsockError)?;
    vsock_socket
        .shutdown(std::net::Shutdown::Write)
        .map_err(|e| SocketError::ShutdownError {
            side: std::net::Shutdown::Write,
            source: e,
        })
        .map_err(ProxyError::VsockError)?;

    Ok(vsock_socket)
}

fn new_ip_socket(device: &str) -> Result<Socket, ProxyError> {
    let ip_socket = Socket::new(Domain::IPV4, Type::RAW, Protocol::TCP.into())
        .map_err(|e| SocketError::CreateError {
            domain: Domain::IPV4,
            r#type: Type::RAW,
            protocol: Protocol::TCP.into(),
            source: e,
        })
        .map_err(ProxyError::IpError)?;
    ip_socket
        .bind_device(device.as_bytes().into())
        .map_err(|e| SocketError::BindError {
            addr: device.to_owned(),
            source: e,
        })
        .map_err(ProxyError::IpError)?;
    ip_socket
        .set_header_included(true)
        .map_err(|e| SocketError::OptionError("IP_HDRINCL".to_owned(), e))
        .map_err(ProxyError::IpError)?;
    // shutdown does not work since socket is not connected, set buffer size to 0 instead
    ip_socket
        .set_recv_buffer_size(0)
        .map_err(|e| SocketError::ShutdownError {
            side: std::net::Shutdown::Read,
            source: e,
        })
        .map_err(ProxyError::IpError)?;

    Ok(ip_socket)
}

fn new_vsock_socket_with_backoff(addr: &SockAddr, backoff: &mut u64) -> Socket {
    loop {
        match new_vsock_socket(addr) {
            Ok(vsock_socket) => return vsock_socket,
            Err(err) => {
                println!("{:?}", anyhow::Error::from(err));

                sleep(Duration::from_secs(*backoff));
                *backoff = (*backoff * 2).clamp(1, 64);
            }
        };
    }
}

fn new_ip_socket_with_backoff(device: &str, backoff: &mut u64) -> Socket {
    loop {
        match new_ip_socket(device) {
            Ok(ip_socket) => return ip_socket,
            Err(err) => {
                println!("{:?}", anyhow::Error::from(err));

                sleep(Duration::from_secs(*backoff));
                *backoff = (*backoff * 2).clamp(1, 64);
            }
        };
    }
}

fn main() -> anyhow::Result<()> {
    let mut backoff = 1u64;

    // get ip socket
    let device = "lo";
    let mut ip_socket = new_ip_socket_with_backoff(device, &mut backoff);

    // reset backoff on success
    backoff = 1;

    // get vsock socket
    let vsock_addr = &SockAddr::vsock(3, 1201);
    let mut vsock_socket = new_vsock_socket_with_backoff(vsock_addr, &mut backoff);

    // reset backoff on success
    backoff = 1;

    loop {
        // do proxying
        // on errors, simply reset the erroring socket
        match handle_conn(&mut vsock_socket, &mut ip_socket) {
            Ok(_) => {
                // should never happen!
                unreachable!("connection handler exited without error");
            }
            Err(err @ ProxyError::IpError(_)) => {
                println!("{:?}", anyhow::Error::from(err));

                // get ip socket
                ip_socket = new_ip_socket_with_backoff(device, &mut backoff);

                // reset backoff on success
                backoff = 1;
            }
            Err(err @ ProxyError::VsockError(_)) => {
                println!("{:?}", anyhow::Error::from(err));

                // get vsock socket
                vsock_socket = new_vsock_socket_with_backoff(vsock_addr, &mut backoff);

                // reset backoff on success
                backoff = 1;
            }
            Err(err) => {
                // should never happen!
                unreachable!("connection handler exited with unknown error {err:?}");
            }
        }
    }
}
