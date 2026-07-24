// We don't resolve managed MCPs from agent run CLI flows on WASM, so this code is unused there.
#![cfg_attr(target_family = "wasm", expect(dead_code))]

use anyhow::{anyhow, Result};
use async_trait::async_trait;
#[cfg(test)]
use mockall::automock;
use uuid::Uuid;
use warp_graphql::mutations::create_managed_mcp_client_config::CreateManagedMcpClientConfigOutput;

use super::ServerApi;

#[cfg_attr(test, automock)]
#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
pub trait ManagedMcpClient: 'static + Send + Sync {
    async fn create_managed_mcp_client_config(
        &self,
        uid: Uuid,
    ) -> Result<CreateManagedMcpClientConfigOutput>;
}

#[cfg_attr(not(target_family = "wasm"), async_trait)]
#[cfg_attr(target_family = "wasm", async_trait(?Send))]
impl ManagedMcpClient for ServerApi {
    async fn create_managed_mcp_client_config(
        &self,
        uid: Uuid,
    ) -> Result<CreateManagedMcpClientConfigOutput> {
        // ZYH local product (issue #29): managed MCP resolution, proxy tokens, and
        // server-side catalog requests are fail-closed. No GraphQL is issued.
        let _ = (self, uid);
        Err(anyhow!("{}", managed_resolution_unavailable_message()))
    }
}

fn managed_resolution_unavailable_message() -> &'static str {
    #[cfg(feature = "local_fs")]
    {
        crate::ai::mcp::local_mcp_surface().managed_resolution_unavailable_message()
    }
    #[cfg(not(feature = "local_fs"))]
    {
        "Managed MCP resolution is not available in the ZYH local product"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_resolution_message_is_explicit() {
        let message = managed_resolution_unavailable_message();
        assert!(message.contains("Managed MCP"));
        assert!(message.contains("not available") || message.contains("local"));
    }

    #[cfg(feature = "local_fs")]
    #[test]
    fn surface_policy_disallows_managed_resolution() {
        use crate::ai::mcp::local_mcp_surface;
        assert!(!local_mcp_surface().allows_managed_resolution());
    }
}
