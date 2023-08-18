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

use std::ffi::CStr;
use std::io::Read;

use anyhow::{anyhow, Context, Result};
use libc::{freeifaddrs, getifaddrs, ifaddrs, strncmp};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

fn get_eth_interface() -> Result<String> {
    let mut ifap: *mut ifaddrs = std::ptr::null_mut();
    let res = unsafe { getifaddrs(&mut ifap) };

    if res < 0 {
        return Err(anyhow!("failed to query interfaces"));
    }

    let mut ifap_iter = ifap;
    let mut ifname = "".to_owned();
    while !ifap_iter.is_null() {
        let name = unsafe { CStr::from_ptr((*ifap_iter).ifa_name) };
        if unsafe { strncmp(name.as_ptr(), "eth".as_ptr().cast(), 3) } == 0
            || unsafe { strncmp(name.as_ptr(), "ens".as_ptr().cast(), 3) } == 0
        {
            ifname = name.to_str().context("non utf8 interface")?.to_owned();
            break;
        }
        ifap_iter = unsafe { (*ifap_iter).ifa_next };
    }

    unsafe { freeifaddrs(ifap) };

    if ifname == "" {
        Err(anyhow!("no matching interface found"))
    } else {
        Ok(ifname)
    }
}

fn handle_conn(conn_socket: &mut Socket, conn_addr: SockAddr) -> Result<()> {
    println!("handling connection from {:?}", conn_addr);
    let mut buf = vec![0u8; 65536].into_boxed_slice();

    // define nat table data structure, likely hash table

    loop {
        let size = conn_socket
            .read(&mut buf)
            .context("failed to read from conn socket")?;

        println!("{:?}", &buf[0..size]);

        // src_addr is assumed to be 127.0.0.1
        // we only NAT (src_port, dst_addr, dst_port) tuple
        // luckily fits in u64

        // calculate key

        // check if flow already exists

        // if not assign a port and start tracking flow

        // perform NAT

        // forward
    }
}

fn main() -> Result<()> {
    // get ethernet interface
    let ifname = get_eth_interface().context("could not get ethernet interface")?;
    println!("detected ethernet interface: {}", ifname);

    // set up ip socket in interface
    let ip_socket = Socket::new(Domain::IPV4, Type::RAW, Protocol::TCP.into())
        .context("failed to create ip socket")?;
    ip_socket
        .bind_device(ifname.as_bytes().into())
        .context("failed to bind ip socket")?;

    // shut down read side since we are only going to write
    // set zero buffer instead of shutdown since latter was not working
    ip_socket
        .set_recv_buffer_size(0)
        .context("failed to shut down read side")?;

    // set up vsock socket
    let vsock_socket =
        Socket::new(Domain::VSOCK, Type::STREAM, None).context("failed to create vsock socket")?;
    vsock_socket
        .bind(&SockAddr::vsock(3, 1200))
        .context("failed to bind vsock socket")?;
    vsock_socket
        .listen(0)
        .context("failed to listen using vsock socket")?;

    loop {
        let (mut conn_socket, conn_addr) = vsock_socket
            .accept()
            .context("failed to accept connection")?;

        let res =
            handle_conn(&mut conn_socket, conn_addr).context("error while handling connection");
        println!(
            "{:?}",
            res.err().unwrap_or(anyhow!("connection closed gracefully"))
        );
    }
}
