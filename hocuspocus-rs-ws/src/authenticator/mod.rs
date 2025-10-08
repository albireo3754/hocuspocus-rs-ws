pub mod simple;

use crate::client_connection::DocConnectionConfig;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Authenticator: Send + Sync {
    async fn authenticate(
        &self,
        doc_id: &str,
        token: &str,
    ) -> Result<DocConnectionConfig>;
}
