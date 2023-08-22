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
        // using read for now, investigate read_vectored for better perf
        let size = ip_socket
            .read(&mut buf)
            .map_err(SocketError::ReadError)
            .map_err(ProxyError::IpError)?;

        if size == 0 {
            Err(SocketError::EofError).map_err(ProxyError::IpError)?;
        }

        // get src and dst addr
        let src_addr = u32::from_be_bytes(buf[12..16].try_into().unwrap());
        let dst_addr = u32::from_be_bytes(buf[16..20].try_into().unwrap());

        // ignore packets not originating from 127.0.0.1
        if src_addr != 0x7f000001 {
            continue;
        }

        // https://en.wikipedia.org/wiki/Reserved_IP_addresses
        // ignore packets sent to
        // 0.0.0.0/8
        if (dst_addr & 0xff000000) == 0x00000000 ||
            // 10.0.0.0/8
            (dst_addr & 0xff000000) == 0x0a000000 ||
            // 100.64.0.0/10
            (dst_addr & 0xffc00000) == 0x64400000 ||
            // 127.0.0.0/8
            (dst_addr & 0xff000000) == 0x7f000000 ||
            // 169.254.0.0/16
            (dst_addr & 0xffff0000) == 0xa9fe0000 ||
            // 172.16.0.0/12
            (dst_addr & 0xfff00000) == 0xac100000 ||
            // 192.0.0.0/24
            (dst_addr & 0xffffff00) == 0xc0000000 ||
            // 192.0.2.0/24
            (dst_addr & 0xffffff00) == 0xc0000200 ||
            // 192.88.99.0/24
            (dst_addr & 0xffffff00) == 0xc0586300 ||
            // 192.168.0.0/16
            (dst_addr & 0xffff0000) == 0xc0a80000 ||
            // 198.18.0.0/15
            (dst_addr & 0xfffe0000) == 0xc6120000 ||
            // 198.51.100.0/24
            (dst_addr & 0xffffff00) == 0xc6336400 ||
            // 203.0.113.0/24
            (dst_addr & 0xffffff00) == 0xcb007100 ||
            // 224.0.0.0/4
            (dst_addr & 0xf0000000) == 0xe0000000 ||
            // 233.252.0.0/24
            (dst_addr & 0xffffff00) == 0xe9fc0000 ||
            // 240.0.0.0/4
            (dst_addr & 0xf0000000) == 0xf0000000 ||
            // 255.255.255.255/32
            (dst_addr & 0xffffffff) == 0xffffffff
        {
            continue;
        }

        let ip_header_size = usize::from((buf[0] & 0x0f) * 4);
        let src_port =
            u16::from_be_bytes(buf[ip_header_size..ip_header_size + 2].try_into().unwrap());

        if src_port != 80 && src_port != 443 && (src_port < 1024 || src_port > 61439) {
            // silently drop
            continue;
        }

        // send through vsock
        let mut total_sent = 0;
        while total_sent < size {
            let size = conn_socket
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
        .shutdown(std::net::Shutdown::Read)
        .map_err(|e| SocketError::ShutdownError {
            side: std::net::Shutdown::Read,
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
    // shutdown does not work since socket is not connected, set send buffer to 0 instead
    ip_socket
        .set_send_buffer_size(0)
        .map_err(|e| SocketError::ShutdownError {
            side: std::net::Shutdown::Write,
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
    let vsock_addr = &SockAddr::vsock(3, 1200);
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
        }
    }
}
