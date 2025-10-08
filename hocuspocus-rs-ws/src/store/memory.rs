use dashmap::DashMap;

use crate::store::{Result, Store};

pub struct MemoryStore {
    store: DashMap<String, Vec<u8>>,
}

impl MemoryStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self.store.get(key).map(|v| v.clone()))
    }

    async fn set(&self, key: &str, value: Vec<u8>) -> Result<()> {
        self.store.insert(key.to_owned(), value);
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<()> {
        self.store.remove(key);
        Ok(())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(self.store.contains_key(key))
    }
}

#[async_trait::async_trait]
impl Store for MemoryStore {
    async fn init(&self) -> Result<()> {
        Ok(())
    }
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.get(key).await
    }
    async fn set(&self, key: &str, value: Vec<u8>) -> Result<()> {
        self.set(key, value).await
    }
    async fn remove(&self, key: &str) -> Result<()> {
        self.remove(key).await
    }
    async fn exists(&self, key: &str) -> Result<bool> {
        self.exists(key).await
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self {
            store: DashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MemoryStore;

    #[tokio::test]
    async fn memory_store_basic_flow() {
        let store = MemoryStore::default();

        assert!(!store.exists("missing").await.unwrap());

        store
            .set("key", vec![1, 2, 3])
            .await
            .expect("set should succeed");

        assert!(store.exists("key").await.unwrap());

        let value = store.get("key").await.unwrap();
        assert_eq!(value, Some(vec![1, 2, 3]));

        store.remove("key").await.unwrap();

        assert!(!store.exists("key").await.unwrap());
        assert_eq!(store.get("key").await.unwrap(), None);
    }
}
