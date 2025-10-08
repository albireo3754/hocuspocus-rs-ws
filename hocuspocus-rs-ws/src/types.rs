use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use yrs::
    encoding::read::Error as DecodeError
;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageType {
    Sync = 0,
    Awareness = 1,
    Auth = 2,
    QueryAwareness = 3,
    SyncReply = 4,          // ?
    Stateless = 5,          // ?
    BroadcastStateless = 6, // ?
    SyncStatus = 7,         // ?
    Close = 8,              // ?
}

impl From<u8> for MessageType {
    fn from(value: u8) -> Self {
        match value {
            0 => MessageType::Sync,
            1 => MessageType::Awareness,
            2 => MessageType::Auth,
            3 => MessageType::QueryAwareness,
            5 => MessageType::Stateless,
            6 => MessageType::BroadcastStateless,
            7 => MessageType::SyncStatus,
            8 => MessageType::Close,
            _ => panic!("Invalid message type: {}", value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum SyncType {
    Step1 = 0,
    Step2 = 1,
    Update = 2,
}

impl SyncType {
    pub fn from_u8(value: u8) -> Result<Self, DecodeError> {
        match value {
            0 => Ok(SyncType::Step1),
            1 => Ok(SyncType::Step2),
            2 => Ok(SyncType::Update),
            _ => panic!("Invalid sync type: {}", value),
        }
    }

    pub fn from_u64(value: u64) -> Result<Self, DecodeError> {
        Self::from_u8(value as u8)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClientAwareness {
    pub client_id: u64,
    pub clock: u32,
    pub state: Option<serde_json::Value>,
}

// // Awareness 프로토콜 메시지
// #[derive(Debug)]
// pub struct AwarenessUpdate {
//     pub clients: Vec<AwarenessClient>,
// }

// #[derive(Debug, Clone)]
// pub struct AwarenessClient {
//     pub client_id: u64,
//     pub clock: u32,
//     pub state: Option<Vec<u8>>, // JSON as bytes
// }

// // Awareness 상태 관리
// #[derive(Debug, Clone)]
// pub struct Awareness {
//     pub states: HashMap<u64, AwarenessState>,
//     pub local_client_id: u64,
// }

// #[derive(Debug, Clone)]
// pub struct AwarenessState {
//     pub clock: u32,
//     pub state: serde_json::Value,
//     pub last_updated: std::time::Instant,
// }

// impl Awareness {
//     pub fn new(client_id: u64) -> Self {
//         Self {
//             states: HashMap::new(),
//             local_client_id: client_id,
//         }
//     }

//     pub fn set_local_state(&mut self, state: serde_json::Value) {
//         let entry = self
//             .states
//             .entry(self.local_client_id)
//             .or_insert(AwarenessState {
//                 clock: 0,
//                 state: serde_json::Value::Null,
//                 last_updated: std::time::Instant::now(),
//             });

//         entry.clock += 1;
//         entry.state = state;
//         entry.last_updated = std::time::Instant::now();
//     }

//     pub fn get_states(&self) -> &HashMap<u64, AwarenessState> {
//         &self.states
//     }

//     pub fn apply_update(&mut self, update: AwarenessUpdate) -> Vec<u64> {
//         let mut changed = Vec::new();

//         for client in update.clients {
//             let should_update = self
//                 .states
//                 .get(&client.client_id)
//                 .map(|s| s.clock < client.clock)
//                 .unwrap_or(true);

//             if should_update {
//                 if let Some(state_bytes) = client.state {
//                     if let Ok(state) = serde_json::from_slice(&state_bytes) {
//                         self.states.insert(
//                             client.client_id,
//                             AwarenessState {
//                                 clock: client.clock,
//                                 state,
//                                 last_updated: std::time::Instant::now(),
//                             },
//                         );
//                         changed.push(client.client_id);
//                     }
//                 } else {
//                     // Client disconnected
//                     self.states.remove(&client.client_id);
//                     changed.push(client.client_id);
//                 }
//             }
//         }

//         changed
//     }

//     pub fn encode_update(&self, clients: &[u64]) -> Vec<u8> {
//         let mut encoder = Encoder::new();

//         // Number of clients
//         encoder.write_var_uint(clients.len() as u64);

//         for &client_id in clients {
//             encoder.write_var_uint(client_id);

//             if let Some(state) = self.states.get(&client_id) {
//                 encoder.write_var_uint(state.clock as u64);
//                 let json_bytes = serde_json::to_vec(&state.state).unwrap_or_default();
//                 encoder.write_var_string(&String::from_utf8_lossy(&json_bytes));
//             } else {
//                 encoder.write_var_uint(0); // clock = 0 means removed
//                 encoder.write_var_uint(0); // empty state
//             }
//         }

//         encoder.to_vec()
//     }

//     pub fn decode_update(data: &[u8]) -> Result<AwarenessUpdate, DecodeError> {
//         let mut decoder = Decoder::new(data);
//         let num_clients = decoder.read_var_uint()? as usize;
//         let mut clients = Vec::with_capacity(num_clients);

//         for _ in 0..num_clients {
//             let client_id = decoder.read_var_uint()?;
//             let clock = decoder.read_var_uint()? as u32;

//             let state = if clock > 0 {
//                 let state_str = decoder.read_var_string()?;
//                 Some(state_str.as_bytes().to_vec())
//             } else {
//                 None
//             };

//             clients.push(AwarenessClient {
//                 client_id,
//                 clock,
//                 state,
//             });
//         }

//         Ok(AwarenessUpdate { clients })
//     }
// }

// 더 자세한 Sync 메시지 구조
#[derive(Debug)]
pub enum SyncMessage<'a> {
    Step1 { state_vector: &'a [u8] },
    Step2 { update: &'a [u8] },
    Update { update: &'a [u8] },
}

impl<'a> SyncMessage<'a> {
    pub fn decode(sync_type: SyncType, data: &'a [u8]) -> Result<Self, DecodeError> {
        match sync_type {
            SyncType::Step1 => Ok(SyncMessage::Step1 { state_vector: data }),
            SyncType::Step2 => Ok(SyncMessage::Step2 { update: data }),
            SyncType::Update => Ok(SyncMessage::Update { update: data }),
        }
    }
}

// // Separate payload structs for each message type
// #[derive(Debug, Clone)]
// pub struct SyncPayload {
//     pub sync_type: SyncType,
//     pub data: Vec<u8>,
// }

// #[derive(Debug, Clone)]
// pub struct SyncReplyPayload {
//     pub sync_type: SyncType,
//     pub data: Vec<u8>,
// }

// #[derive(Debug, Clone)]
// pub struct AwarenessPayload {
//     pub clients: Vec<ClientAwareness>,
// }

// #[derive(Debug, Clone)]
// pub struct AuthPayload {
//     pub token: String,
// }

// #[derive(Debug, Clone)]
// pub struct QueryAwarenessPayload {
//     // No additional data needed
// }

// #[derive(Debug, Clone)]
// pub struct StatelessPayload {
//     pub payload: Vec<u8>,
// }

// #[derive(Debug, Clone)]
// pub struct BroadcastStatelessPayload {
//     pub payload: Vec<u8>,
// }

// #[derive(Debug, Clone)]
// pub struct ClosePayload {
//     pub code: u16,
//     pub reason: String,
// }

// #[derive(Debug)]
// pub enum MessageTypeV2 {
//     Sync(SyncPayload),
//     SyncReply(SyncReplyPayload),
//     Awareness(AwarenessPayload),
//     Auth(AuthPayload),
//     QueryAwareness(QueryAwarenessPayload),
//     Stateless(StatelessPayload),
//     BroadcastStateless(BroadcastStatelessPayload),
//     Close(ClosePayload),
// }

// impl MessageTypeV2 {
//     pub fn decode(buffer: &[u8], skip_document: bool) -> Result<Self, DecodeError> {
//         let mut decoder = DecoderV2::new(Cursor::new(buffer)).unwrap();

//         // Skip document name if present in the buffer
//         if !skip_document {
//             let _document = decoder.read_string()?;
//         }

//         let msg_type = decoder.read_u8()?;

//         match msg_type {
//             0 => {
//                 let sync_type = SyncType::from_u8(decoder.read_u8()?)?;
//                 Ok(MessageTypeV2::Sync(SyncPayload {
//                     sync_type,
//                     data: decoder.read_buf()?.to_vec(),
//                 }))
//             }
//             1 => {
//                 let clients = decode_awareness(&mut decoder)?;
//                 Ok(MessageTypeV2::Awareness(AwarenessPayload { clients }))
//             }
//             2 => {
//                 let token = decoder.read_str()?.to_string();
//                 Ok(MessageTypeV2::Auth(AuthPayload { token }))
//             }
//             3 => Ok(MessageTypeV2::QueryAwareness(QueryAwarenessPayload {})),
//             4 => {
//                 let sync_type = SyncType::from_u8(decoder.read_u8()?)?;
//                 Ok(MessageTypeV2::SyncReply(SyncReplyPayload {
//                     sync_type,
//                     data: decoder.read_buf()?.to_vec(),
//                 }))
//             }
//             5 => Ok(MessageTypeV2::Stateless(StatelessPayload {
//                 payload: decoder.read_buf()?.to_vec(),
//             })),
//             6 => Ok(MessageTypeV2::BroadcastStateless(
//                 BroadcastStatelessPayload {
//                     payload: decoder.read_buf()?.to_vec(),
//                 },
//             )),
//             8 => {
//                 let code = decoder.read_u16()?;
//                 let reason = decoder.read_str()?.to_string();
//                 Ok(MessageTypeV2::Close(ClosePayload { code, reason }))
//             }
//             _ => Err(DecodeError::UnknownMessageType(msg_type)),
//         }
//     }

//     pub fn message_type(&self) -> MessageType {
//         match self {
//             MessageTypeV2::Sync(_) => MessageType::Sync,
//             MessageTypeV2::SyncReply(_) => MessageType::SyncReply,
//             MessageTypeV2::Awareness(_) => MessageType::Awareness,
//             MessageTypeV2::Auth(_) => MessageType::Auth,
//             MessageTypeV2::QueryAwareness(_) => MessageType::QueryAwareness,
//             MessageTypeV2::Stateless(_) => MessageType::Stateless,
//             MessageTypeV2::BroadcastStateless(_) => MessageType::BroadcastStateless,
//             MessageTypeV2::Close(_) => MessageType::Close,
//         }
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMessageType {
    Step1 = 0,
    Step2 = 1,
    Update = 2,
}

impl From<u8> for SyncMessageType {
    fn from(value: u8) -> Self {
        match value {
            0 => SyncMessageType::Step1,
            1 => SyncMessageType::Step2,
            2 => SyncMessageType::Update,
            _ => panic!("Invalid sync message type: {}", value),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfiguration {
    pub id: String,
    pub is_authenticated: bool,
    pub readonly: bool,
    pub user_id: Option<String>,
    pub socket_id: String,
    pub context: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentConfiguration {
    pub name: String,
    pub gc: bool,
}

impl Default for DocumentConfiguration {
    fn default() -> Self {
        Self {
            name: String::new(),
            gc: true,
        }
    }
}

#[derive(Clone)]
pub struct HocuspocusConfiguration {
    pub name: Option<String>,
    pub timeout: u64,
    pub debounce: u64,
    pub max_debounce: u64,
    pub quiet: bool,
    pub extensions: Vec<Arc<dyn Extension>>,
}

impl Default for HocuspocusConfiguration {
    fn default() -> Self {
        Self {
            name: None,
            timeout: 30000,
            debounce: 2000,
            max_debounce: 10000,
            quiet: false,
            extensions: Vec::new(),
        }
    }
}

#[async_trait::async_trait]
pub trait Extension: Send + Sync {
    async fn on_configure(
        &self,
        _configuration: &mut HocuspocusConfiguration,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_listen(&self, _port: u16) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_upgrade(
        &self,
        _request: &axum::http::Request<axum::body::Body>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_connect(&self, _data: ConnectEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_authenticate(
        &self,
        _data: AuthenticateEventData,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::Value::Null)
    }

    async fn connected(&self, _data: ConnectedEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_create_document(&self, _data: CreateDocumentEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_load_document(
        &self,
        _data: LoadDocumentEventData,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn before_handle_message(
        &self,
        _data: BeforeHandleMessageEventData,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn before_broadcast(&self, _data: BeforeBroadcastEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_change(&self, _data: ChangeEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_store_document(&self, _data: StoreDocumentEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn after_store_document(&self, _data: AfterStoreDocumentEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_awareness_update(&self, _data: AwarenessUpdateEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn before_send_stateless(
        &self,
        _data: BeforeSendStatelessEventData,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn before_send_awareness(
        &self,
        _data: BeforeSendAwarenessEventData,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_disconnect(&self, _data: DisconnectEventData) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_destroy(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn after_unload_document(
        &self,
        _data: AfterUnloadDocumentEventData,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn on_stateless(&self, _data: StatelessEventData) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct ConnectEventData {
    pub socket_id: String,
    pub connection: ConnectionConfiguration,
    pub request_headers: HashMap<String, String>,
    pub request_path: String,
}

#[derive(Clone, Debug)]
pub struct AuthenticateEventData {
    pub doc_id: String,
    pub token: String,
    // pub socket_id: String,
}

#[derive(Clone)]
pub struct ConnectedEventData {
    pub socket_id: String,
    // pub context: HashMap<String, serde_json::Value>,
}

#[derive(Clone)]
pub struct CreateDocumentEventData {
    pub document_name: String,
    pub socket_id: String,
    pub connection: ConnectionConfiguration,
}

#[derive(Clone)]
pub struct LoadDocumentEventData {
    pub document_name: String,
    pub socket_id: String,
}

#[derive(Clone)]
pub struct BeforeHandleMessageEventData {
    pub message: Vec<u8>,
    pub socket_id: String,
    pub connection: ConnectionConfiguration,
}

#[derive(Clone)]
pub struct BeforeBroadcastEventData {
    pub document_name: String,
    pub exclude: Vec<String>,
    pub message: Vec<u8>,
}

#[derive(Clone)]
pub struct ChangeEventData {
    pub document_name: String,
    pub socket_id: String,
    pub update: Vec<u8>,
}

#[derive(Clone)]
pub struct StoreDocumentEventData {
    pub document_name: String,
    pub state: Vec<u8>,
}

#[derive(Clone)]
pub struct AfterStoreDocumentEventData {
    pub document_name: String,
}

#[derive(Clone)]
pub struct AwarenessUpdateEventData {
    pub document_name: String,
    pub awareness: Vec<u8>,
    pub added: Vec<u64>,
    pub updated: Vec<u64>,
    pub removed: Vec<u64>,
}

#[derive(Clone)]
pub struct BeforeSendStatelessEventData {
    pub document_name: String,
    pub socket_id: String,
    pub payload: String,
}

#[derive(Clone)]
pub struct BeforeSendAwarenessEventData {
    pub document_name: String,
    pub socket_id: String,
    pub awareness: Vec<u8>,
}

#[derive(Clone)]
pub struct DisconnectEventData {
    pub socket_id: String,
}

#[derive(Clone)]
pub struct AfterUnloadDocumentEventData {
    pub document_name: String,
}

#[derive(Clone)]
pub struct StatelessEventData {
    pub document_name: String,
    pub socket_id: String,
    pub payload: String,
}

pub type ChannelId = String;
pub type DocumentName = String;
pub type SocketId = String;
