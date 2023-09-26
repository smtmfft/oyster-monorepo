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
use std::net::SocketAddrV4;

use socket2::{SockAddr, Socket};

use raw_proxy::{
    new_ip_socket_with_backoff, new_vsock_socket_with_backoff, ProxyError, SocketError,
};

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
                .send_to(
                    &buf[total_sent..size],
                    // port does not matter
                    &"127.0.0.1:80".parse::<SocketAddrV4>().unwrap().into(),
                )
                .map_err(SocketError::WriteError)
                .map_err(ProxyError::IpError)?;
            total_sent += size;
        }
    }
}

fn main() -> anyhow::Result<()> {
    // get ip socket
    let device = "lo";
    let mut ip_socket = new_ip_socket_with_backoff(device);

    // get vsock socket
    let vsock_addr = &SockAddr::vsock(3, 1201);
    let mut vsock_socket = new_vsock_socket_with_backoff(vsock_addr);

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
                ip_socket = new_ip_socket_with_backoff(device);
            }
            Err(err @ ProxyError::VsockError(_)) => {
                println!("{:?}", anyhow::Error::from(err));

                // get vsock socket
                vsock_socket = new_vsock_socket_with_backoff(vsock_addr);
            }
            Err(err) => {
                // should never happen!
                unreachable!("connection handler exited with unknown error {err:?}");
            }
        }
    }
}
