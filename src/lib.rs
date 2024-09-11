use std::future::Future;

use alloy::primitives::Address;
use alloy::providers::Provider;
use alloy::pubsub::PubSubFrontend;
use alloy::rpc::types::eth::Log;
use anyhow::Result;
use tokio_stream::StreamExt;

pub trait LogsProvider {
    fn logs<'a>(
        &'a self,
        client: &'a impl Provider<PubSubFrontend>,
    ) -> impl Future<Output = Result<impl StreamExt<Item = Log> + 'a>>;
}

#[derive(Clone)]
pub struct AlloyProvider {
    pub contract: Address,
}

impl LogsProvider for AlloyProvider {
    async fn logs<'a>(
        &'a self,
        client: &'a impl Provider<PubSubFrontend>,
    ) -> Result<impl StreamExt<Item = Log> + 'a> {
        todo!()
    }
}
