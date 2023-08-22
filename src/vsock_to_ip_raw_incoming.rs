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

use anyhow::{anyhow, Context, Result};
use nfq::{Queue, Verdict};
use socket2::{Domain, SockAddr, Socket, Type};

fn handle_conn_incoming(conn_socket: &mut Socket, queue: &mut Queue) -> Result<()> {
    loop {
        let mut msg = queue.recv().context("nfqueue recv error")?;

        println!("{:?}", msg);
        let payload = msg.get_payload_mut();

        // NAT
        payload[16..20].clone_from_slice(&0x7f000001u32.to_be_bytes());

        // TODO: handle incorrect checksums?

        // send
        let mut total_sent = 0;
        while total_sent < payload.len() {
            let size = conn_socket
                .send(payload)
                .context("failed to send incoming packet")?;
            total_sent += size;
        }

        // verdicts
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

fn new_nfq() -> Result<Queue, ProxyError> {
    let mut queue = Queue::open()
        .map_err(|e| SocketError::OpenError("0".to_owned(), e))
        .map_err(ProxyError::NfqError)?;
    queue
        .bind(0)
        .map_err(|e| SocketError::BindError {
            addr: "0".to_owned(),
            source: e,
        })
        .map_err(ProxyError::NfqError)?;

    Ok(queue)
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
        .bind(addr)
        .map_err(|e| SocketError::BindError {
            addr: format!("{:?}, {:?}", addr.domain(), addr.as_vsock_address()),
            source: e,
        })
        .map_err(ProxyError::VsockError)?;
    vsock_socket
        .listen(0)
        .map_err(|e| SocketError::ListenError {
            addr: format!("{:?}, {:?}", addr.domain(), addr.as_vsock_address()),
            source: e,
        })
        .map_err(ProxyError::VsockError)?;

    Ok(vsock_socket)
}

fn new_nfq_with_backoff(backoff: &mut u64) -> Queue {
    loop {
        match new_nfq() {
            Ok(queue) => return queue,
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

    // nfqueue for incoming packets
    let queue = new_nfq_with_backoff(&mut backoff);

    // reset backoff on success
    backoff = 1;

    // set up incoming vsock socket for incoming packets
    let vsock_addr = &SockAddr::vsock(3, 1201);
    let vsock_socket = new_vsock_socket(vsock_addr)?;

    handle_incoming(vsock_socket_incoming, queue)
}
