use std::ffi::OsStr;
use std::pin::Pin;
use std::task::{ready, Poll};

use anyhow::Context;

use axum::{routing::get, Router};
use clap::{builder::TypedValueParser, error::ErrorKind, Arg, Command, Parser};
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

#[derive(Clone)]
pub struct VsockAddrParser {}

impl TypedValueParser for VsockAddrParser {
    type Value = (u32, u32);

    fn parse_ref(
        &self,
        cmd: &Command,
        _: Option<&Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, clap::Error> {
        let value = value
            .to_str()
            .ok_or(clap::Error::new(ErrorKind::InvalidUtf8).with_cmd(cmd))?;

        let (cid, port) = value
            .split_once(':')
            .ok_or(clap::Error::new(ErrorKind::ValueValidation).with_cmd(cmd))?;

        let cid = cid
            .parse::<u32>()
            .map_err(|_| clap::Error::new(ErrorKind::ValueValidation).with_cmd(cmd))?;
        let port = port
            .parse::<u32>()
            .map_err(|_| clap::Error::new(ErrorKind::ValueValidation).with_cmd(cmd))?;

        Ok((cid, port))
    }
}

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Cli {
    /// vsock address to listen on <cid:port>
    #[clap(short, long, value_parser = VsockAddrParser{})]
    vsock_addr: (u32, u32),
    /// job id served by the enclave
    #[clap(short, long, value_parser)]
    job_id: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Cli::parse();

    let app = Router::new().route("/job", get(|| async { cli.job_id }));

    axum::Server::builder(VsockServer {
        listener: VsockListener::bind(cli.vsock_addr.0, cli.vsock_addr.1)
            .context("failed to create vsock listener")?,
    })
    .serve(app.into_make_service())
    .await
    .context("server exited with error")?;

    Ok(())
}
