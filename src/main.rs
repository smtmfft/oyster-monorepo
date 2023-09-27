use std::pin::Pin;
use std::task::{ready, Poll};

use axum::{routing::get, Router};
use hyper::server::accept::Accept;
use tokio_vsock::{VsockListener, VsockStream};

struct VsockServer {
    listener: VsockListener,
}

impl Accept for VsockServer {
    type Conn = VsockStream;
    type Error = std::io::Error;

    fn poll_accept(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let (conn, _addr) = ready!(self.listener.poll_accept(cx))?;
        Poll::Ready(Some(Ok(conn)))
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}
