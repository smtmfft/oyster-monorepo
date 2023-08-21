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

// threading model:
// two threads total
// one thread to handle packets coming from enclave going out
// one thread to handle packets coming to enclave going in
// NAT is stateless so they can work independently
// vsock connections are independent as well

// for incoming packets, we need to _intercept_ them and not just get a copy
// raw sockets do the latter, therefore we go with iptables and nfqueue
// iptables can be used to redirect packets to a nfqueue
// we read it here, do NAT and forward onwards

use std::ffi::CStr;
use std::io::Read;
use std::net::SocketAddrV4;

use anyhow::{anyhow, Context, Result};
use libc::{freeifaddrs, getifaddrs, ifaddrs, strncmp};
use nfq::{Queue, Verdict};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};

fn get_eth_interface() -> Result<(String, u32)> {
    let mut ifap: *mut ifaddrs = std::ptr::null_mut();
    let res = unsafe { getifaddrs(&mut ifap) };

    if res < 0 {
        return Err(anyhow!("failed to query interfaces"));
    }

    let mut ifap_iter = ifap;
    let mut ifname = "".to_owned();
    let mut ifaddr = 0;
    while !ifap_iter.is_null() {
        let name = unsafe { CStr::from_ptr((*ifap_iter).ifa_name) };
        if (unsafe { strncmp(name.as_ptr(), "eth".as_ptr().cast(), 3) } == 0
            || unsafe { strncmp(name.as_ptr(), "ens".as_ptr().cast(), 3) } == 0)
            && unsafe { (*(*ifap_iter).ifa_addr).sa_family == libc::AF_INET as u16 }
        {
            ifname = name.to_str().context("non utf8 interface")?.to_owned();
            ifaddr = unsafe {
                (*(*ifap_iter).ifa_addr.cast::<libc::sockaddr_in>())
                    .sin_addr
                    .s_addr
            };
            break;
        }
        ifap_iter = unsafe { (*ifap_iter).ifa_next };
    }

    unsafe { freeifaddrs(ifap) };

    if ifname == "" {
        Err(anyhow!("no matching interface found"))
    } else {
        Ok((ifname, ifaddr))
    }
}

fn handle_conn_outgoing(
    conn_socket: &mut Socket,
    conn_addr: SockAddr,
    ip_socket: &mut Socket,
    ifaddr: u32,
) -> Result<()> {
    println!("handling connection from {:?}", conn_addr);
    let mut buf = vec![0u8; 65535].into_boxed_slice();

    // does not matter what the address is, just has to be a publicly routed address
    let external_addr: SockAddr = "1.1.1.1:80".parse::<SocketAddrV4>().unwrap().into();

    loop {
        // read till total size
        conn_socket
            .read_exact(&mut buf[0..4])
            .context("failed to read size from conn socket")?;

        let size: usize = u16::from_be_bytes(buf[2..4].try_into().unwrap()).into();

        // read till full frame
        conn_socket
            .read_exact(&mut buf[4..size])
            .context("failed to read frame from conn socket")?;

        println!("{:?}", &buf[0..size]);

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

        buf[12..16].clone_from_slice(&ifaddr.to_ne_bytes());

        // TODO: tcp checksum has to be redone manually, figure out a way to offload
        buf[ip_header_size + 16..ip_header_size + 18].clone_from_slice(&[0, 0]);
        let mut csum = 0u32;
        for i in (12..20).step_by(2) {
            let word: u32 = u16::from_be_bytes(buf[i..i + 2].try_into().unwrap()).into();
            csum += word;
            println!("{:02x}, {:02x}", word, csum);
        }
        csum += u32::from(u16::from_be_bytes([0, buf[9]]));
        csum += (size - ip_header_size) as u16 as u32;
        for i in (ip_header_size..size - 1).step_by(2) {
            let word: u32 = u16::from_be_bytes(buf[i..i + 2].try_into().unwrap()).into();
            csum += word;
            println!("{:02x}, {:02x}", word, csum);
        }
        if size % 2 == 1 {
            csum += u32::from(u16::from_be_bytes([buf[size - 1], 0]));
        }
        csum = (csum >> 16) + (csum & 0xffff);
        println!("{:02x}", csum);
        csum = (csum >> 16) + (csum & 0xffff);
        println!("{:02x}", csum);
        csum = !csum;

        buf[ip_header_size + 16..ip_header_size + 18].clone_from_slice(&csum.to_be_bytes()[2..4]);

        ip_socket
            .send_to(&buf[0..size], &external_addr)
            .context("failed to send packet")?;
    }
}

fn handle_conn_incoming(conn_socket: &mut Socket, queue: &mut Queue) -> Result<()> {
    loop {
        let mut msg = queue.recv().context("nfqueue recv error")?;

        println!("{:?}", msg);

        // NAT

        // conn_socket.send(msg);

        msg.set_verdict(Verdict::Drop);
        queue.verdict(msg).context("failed to set verdict")?;
    }
}

fn handle_incoming(vsock_socket: Socket, mut queue: Queue) -> Result<()> {
    loop {
        let (mut conn_socket, _) = vsock_socket
            .accept()
            .context("failed to accept incoming connection")?;

        let res = handle_conn_incoming(&mut conn_socket, &mut queue)
            .context("error while handling incoming connection");
        println!(
            "{:?}",
            res.err()
                .unwrap_or(anyhow!("incoming connection closed gracefully"))
        );
    }
}

fn handle_outgoing(vsock_socket: Socket, mut ip_socket: Socket, ifaddr: u32) -> Result<()> {
    loop {
        let (mut conn_socket, conn_addr) = vsock_socket
            .accept()
            .context("failed to accept outgoing connection")?;

        let res = handle_conn_outgoing(&mut conn_socket, conn_addr, &mut ip_socket, ifaddr)
            .context("error while handling outgoing connection");
        println!(
            "{:?}",
            res.err()
                .unwrap_or(anyhow!("outgoing connection closed gracefully"))
        );
    }
}

fn main() -> Result<()> {
    // get ethernet interface
    let (ifname, ifaddr) = get_eth_interface().context("could not get ethernet interface")?;
    println!("detected ethernet interface: {}, {:#10x}", ifname, ifaddr);

    // set up ip socket for outgoing packets
    let ip_socket = Socket::new(Domain::IPV4, Type::RAW, Protocol::TCP.into())
        .context("failed to create ip socket")?;
    ip_socket
        .set_header_included(true)
        .context("failed to set IP_HDRINCL")?;
    ip_socket
        .bind_device(ifname.as_bytes().into())
        .context("failed to bind ip socket")?;

    // shut down read side since we are only going to write
    // set zero buffer instead of shutdown since latter was not working
    ip_socket
        .set_recv_buffer_size(0)
        .context("failed to shut down read side")?;

    // set up outgoing vsock socket for outgoing packets
    let vsock_socket_outgoing = Socket::new(Domain::VSOCK, Type::STREAM, None)
        .context("failed to create outgoing vsock socket")?;
    vsock_socket_outgoing
        .bind(&SockAddr::vsock(3, 1200))
        .context("failed to bind outgoing vsock socket")?;
    vsock_socket_outgoing
        .listen(0)
        .context("failed to listen using outgoing vsock socket")?;

    // nfqueue for incoming packets
    let mut queue = Queue::open().context("failed to open nfqueue")?;
    queue.bind(0).context("failed to bind to nfqueue 0")?;

    // set up incoming vsock socket for incoming packets
    let vsock_socket_incoming = Socket::new(Domain::VSOCK, Type::STREAM, None)
        .context("failed to create incoming vsock socket")?;
    vsock_socket_incoming
        .bind(&SockAddr::vsock(3, 1201))
        .context("failed to bind incoming vsock socket")?;
    vsock_socket_incoming
        .listen(0)
        .context("failed to listen using incoming vsock socket")?;

    let outgoing_handle =
        std::thread::spawn(move || handle_outgoing(vsock_socket_outgoing, ip_socket, ifaddr));
    let incoming_handle = std::thread::spawn(move || handle_incoming(vsock_socket_incoming, queue));

    // TODO: how are we supposed to handle this?
    let _ = outgoing_handle.join();
    let _ = incoming_handle.join();

    Ok(())
}
