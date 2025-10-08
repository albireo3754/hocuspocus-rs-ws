// Portions of this module and its submodules are adapted from the Hocuspocus
// JavaScript server (https://github.com/ueberdosis/hocuspocus) and y-sweet
// (https://github.com/y-sweet/y-sweet), both distributed under the MIT license.
// Adapted code retains the original license terms.

pub mod memory;
pub mod s3;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StoreError {
    #[error("Store bucket does not exist. {0}")]
    BucketDoesNotExist(String),
    #[error("Object does not exist. {0}")]
    DoesNotExist(String),
    #[error("Not authorized to access store. {0}")]
    NotAuthorized(String),
    #[error("Error connecting to store. {0}")]
    ConnectionError(String),
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[async_trait]
pub trait Store: Send + Sync {
    async fn init(&self) -> Result<()>;
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    async fn set(&self, key: &str, value: Vec<u8>) -> Result<()>;
    async fn remove(&self, key: &str) -> Result<()>;
    async fn exists(&self, key: &str) -> Result<bool>;
}
