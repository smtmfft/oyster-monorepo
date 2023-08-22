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

fn main() -> Result<()> {
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

    handle_incoming(vsock_socket_incoming, queue)
}
