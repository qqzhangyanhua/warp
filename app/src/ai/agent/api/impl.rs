use std::sync::Arc;

use super::{ConvertToAPITypeError, RequestParams, ResponseStream};
use crate::server::server_api::ServerApi;

pub async fn generate_multi_agent_output(
    _server_api: Arc<ServerApi>,
    params: RequestParams,
    cancellation_rx: futures::channel::oneshot::Receiver<()>,
) -> Result<ResponseStream, ConvertToAPITypeError> {
    super::local_provider::generate_local_provider_output(params, cancellation_rx).await
}
