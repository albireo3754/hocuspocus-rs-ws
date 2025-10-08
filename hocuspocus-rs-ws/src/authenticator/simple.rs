use anyhow::Result;
use async_trait::async_trait;

use crate::client_connection::DocConnectionConfig;

use super::Authenticator;

#[derive(Debug, Default)]
pub struct SimpleAuthenticator;

impl SimpleAuthenticator {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Authenticator for SimpleAuthenticator {
    async fn authenticate(
        &self,
        _doc_id: &str,
        _token: &str,
    ) -> Result<DocConnectionConfig> {
        Ok(DocConnectionConfig {
            is_authenticated: true,
            ..DocConnectionConfig::default()
        })
    }
}
