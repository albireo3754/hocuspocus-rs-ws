// Portions of this module are adapted from the Hocuspocus JavaScript server
// (https://github.com/ueberdosis/hocuspocus) and y-sweet
// (https://github.com/y-sweet/y-sweet), both distributed under the MIT license.
// Adapted code retains the original license terms.

use crate::doc_connection::DocConnection;
use crate::sync::awareness::Awareness;
use crate::sync::Message;
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::error;
use uuid::Uuid;
use yrs::encoding::read::{Cursor, Read};
use yrs::updates::decoder::{Decode as _, Decoder, DecoderV1};

#[derive(Debug, Clone)]
pub struct DocConnectionConfig {
    pub read_only: bool,
    pub is_authenticated: bool,
}

impl Default for DocConnectionConfig {
    fn default() -> Self {
        Self {
            read_only: false,
            is_authenticated: true,
        }
    }
}

#[derive(Debug)]
pub struct MessageQueueEntry {
    pub data: Vec<u8>,
    pub document_name: String,
}

#[async_trait]
pub trait DocServer: Send + Sync {
    async fn fetch(&self, doc_id: &str) -> Result<Arc<RwLock<Awareness>>>;
    async fn authenticate(&self, doc_id: &str, token: &str) -> Result<DocConnectionConfig>;
}

pub struct ClientConnection {
    // Unique identifier for this connection
    socket_id: String,

    doc_server: Arc<dyn DocServer>,

    // Document connections by document name
    document_connections: Arc<Mutex<HashMap<String, Arc<DocConnection>>>>,

    // Callback for sending messages back to the client
    send_callback: mpsc::Sender<Vec<u8>>,
    // Timeout settings
    timeout: Duration,

    // Context for the connection
    context: Arc<Mutex<HashMap<String, String>>>,

    // Whether the connection is closed
    closed: Arc<Mutex<bool>>,
}

impl ClientConnection {
    pub fn new(
        doc_server: Arc<dyn DocServer>,
        send_callback: mpsc::Sender<Vec<u8>>,
        timeout: Duration,
        default_context: HashMap<String, String>,
    ) -> Self {
        Self {
            doc_server,
            socket_id: Uuid::new_v4().to_string(),
            document_connections: Arc::new(Mutex::new(HashMap::new())),
            send_callback,
            timeout,
            context: Arc::new(Mutex::new(default_context)),
            closed: Arc::new(Mutex::new(false)),
        }
    }

    pub fn socket_id(&self) -> &str {
        &self.socket_id
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.lock().unwrap()
    }

    pub fn close(&self) -> Result<()> {
        let mut closed = self.closed.lock().unwrap();
        if *closed {
            return Ok(());
        }
        *closed = true;

        // Close all document connections
        let connections = self.document_connections.lock().unwrap();
        for (_name, _connection) in connections.iter() {
            // DocConnection will handle cleanup in its Drop implementation
        }

        Ok(())
    }

    /// Handle incoming message for a specific document
    /// The document name must be provided separately as the ysweet protocol
    /// doesn't include document names in the message payload
    pub async fn handle_message(&self, data: &[u8]) -> Result<()> {
        if self.is_closed() {
            return Err(anyhow!("Connection is closed"));
        }

        let mut decoder = DecoderV1::new(Cursor::new(data));
        let document_name = decoder.read_string()?.to_owned();
        let msg = Message::decode_v1(decoder.read_to_end()?)?;

        let doc_connection = self.fetch_connection(&document_name).await;
        match doc_connection {
            Err(err) => {
                error!(
                    "Failed to fetch connection for document '{}': {}",
                    document_name, err
                );
                return Ok(());
            }
            Ok(connection) => {
                match connection.handle_msg(msg).await {
                    Ok(result_msg) => {
                        // Handle the result message if needed
                        if let Some(response) = result_msg {
                            connection.send_message(response).await?;
                        }
                    }
                    Err(err) => return Err(anyhow!("Failed to handle message: {}", err)),
                }
            }
        }

        // Queue the message if connection is not established yet
        // {
        //     let mut queue = self.incoming_message_queue.lock().unwrap();
        //     let entry = queue.entry(document_name.to_string()).or_default();
        //     entry.push(data.to_vec());
        // }

        // Check if this is the first message for this document
        // let mut established = self.document_connections_established.lock().unwrap();
        // if !established.contains_key(document_name) {
        //     established.insert(document_name.to_string(), false);

        //     // Initialize connection config for this document
        //     let mut configs = self.connection_configs.lock().unwrap();
        //     configs.insert(document_name.to_string(), ConnectionConfig::default());
        //     drop(configs);

        //     // Try to establish the connection

        // }

        // let establish_connection = self.try_establish_connection(document_name, msg).await;
        // if let Err(err) = establish_connection {
        //     error!(
        //         "Failed to establish connection for document '{}': {}",
        //         document_name, err
        //     );
        // }

        Ok(())
    }

    async fn fetch_connection(&self, document_name: &str) -> Result<Arc<DocConnection>> {
        // For now, we'll create a basic connection without authentication
        // In a real implementation, you'd handle authentication here
        {
            let connections = self.document_connections.lock().unwrap();
            let doc_connection = connections.get(document_name).cloned();
            if let Some(conn) = doc_connection {
                return Ok(conn.clone());
            }
        }

        let awareness = Arc::new(RwLock::new(self.doc_server.fetch(document_name).await?))
            .read()
            .unwrap()
            .clone();

        let send_callback = self.send_callback.clone();

        let connection = Arc::new(DocConnection::new(
            document_name.to_string(),
            self.doc_server.clone(),
            awareness.clone(),
            send_callback.clone(),
        ));

        {
            let mut connections = self.document_connections.lock().unwrap();
            connections.insert(document_name.to_string(), connection.clone());
        }

        Ok(connection)
    }

    pub fn get_context(&self, key: &str) -> Option<String> {
        let context = self.context.lock().unwrap();
        context.get(key).cloned()
    }

    pub fn set_context(&self, key: String, value: String) {
        let mut context = self.context.lock().unwrap();
        context.insert(key, value);
    }

    pub fn get_document_count(&self) -> usize {
        let connections = self.document_connections.lock().unwrap();
        connections.len()
    }

    pub fn has_document(&self, document_name: &str) -> bool {
        let connections = self.document_connections.lock().unwrap();
        connections.contains_key(document_name)
    }
}

impl Drop for ClientConnection {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {

    use yrs::updates::decoder::Decoder;

    use super::*;

    // #[tokio::test]
    // async fn test_client_connection_creation() {
    //     let (tx, _rx) = mpsc::channel(16);
    //     let connection = ClientConnection::new(tx, Duration::from_secs(30), HashMap::new());

    //     assert!(!connection.is_closed());
    //     assert_eq!(connection.get_document_count(), 0);
    // }

    // #[tokio::test]
    // async fn test_client_connection_close() {
    //     let (tx, _rx) = mpsc::channel(16);
    //     let connection = ClientConnection::new(tx, Duration::from_secs(30), HashMap::new());

    //     connection.close().unwrap();
    //     assert!(connection.is_closed());
    // }

    #[test]
    fn test_read_to_end() {
        let data = vec![1, 2, 3, 4, 5];
        let cursor = Cursor::new(&data);
        let mut decoder = DecoderV1::new(cursor);

        decoder.read_u8().unwrap(); // read one byte

        assert_eq!(decoder.read_to_end().unwrap(), &data[1..]);
    }
}
