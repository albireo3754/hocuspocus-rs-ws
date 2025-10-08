// Portions of this module are adapted from the Hocuspocus JavaScript server
// (https://github.com/ueberdosis/hocuspocus) and y-sweet
// (https://github.com/y-sweet/y-sweet), both distributed under the MIT license.
// Adapted code retains the original license terms.

use crate::api_types::Authorization;
use crate::client_connection::DocServer;
use crate::sync::Message;
use crate::sync::{
    self, DefaultProtocol, MSG_SYNC, MSG_SYNC_UPDATE, Protocol, SyncMessage, awareness::Awareness,
};
use anyhow::Result;
use std::sync::{Arc, OnceLock, RwLock};
use tokio::sync::mpsc;
use tracing::debug;
use yrs::updates::decoder::DecoderV1;
use yrs::{
    Subscription, Update,
    block::ClientID,
    encoding::write::Write,
    updates::{
        decoder::Decode,
        encoder::{Encode, Encoder, EncoderV1},
    },
};

// TODO: this is an implementation detail and should not be exposed.
pub const DOC_NAME: &str = "doc";
const SYNC_STATUS_MESSAGE: u8 = 102;

pub struct DocConnection<T: Protocol = DefaultProtocol> {
    doc_name: String,
    doc_server: Arc<dyn DocServer>,
    authorization: Arc<RwLock<Authorization>>,
    awareness: Arc<RwLock<Awareness>>,
    sender: mpsc::Sender<Vec<u8>>,
    protocol: T,
    closed: Arc<OnceLock<()>>,

    /// If the client sends an awareness state, this will be set to its client ID.
    /// It is used to clear the awareness state when a client disconnects.
    client_id: OnceLock<ClientID>,

    #[allow(unused)] // acts as RAII guard
    doc_subscription: Subscription,
    #[allow(unused)] // acts as RAII guard
    awareness_subscription: Subscription,
}

impl DocConnection {
    pub fn new(
        doc_name: String,
        doc_server: Arc<dyn DocServer>,
        awareness: Arc<RwLock<Awareness>>,
        callback: mpsc::Sender<Vec<u8>>,
    ) -> Self {
        Self::new_inner(doc_name, doc_server, awareness, callback)
    }

    pub fn new_inner(
        doc_name: String,
        doc_server: Arc<dyn DocServer>,
        awareness: Arc<RwLock<Awareness>>,
        callback: mpsc::Sender<Vec<u8>>,
    ) -> Self {
        let closed = Arc::new(OnceLock::new());

        let (doc_subscription, awareness_subscription) = {
            let mut awareness = awareness.write().unwrap();

            let doc_subscription = {
                let doc = awareness.doc();
                let callback = callback.clone();
                let doc_name = doc_name.clone();
                let closed = closed.clone();
                doc.observe_update_v1(move |_, event| {
                    if closed.get().is_some() {
                        return;
                    }
                    // https://github.com/y-crdt/y-sync/blob/56958e83acfd1f3c09f5dd67cf23c9c72f000707/src/net/broadcast.rs#L47-L52
                    let mut encoder = EncoderV1::new();
                    encoder.write_string(doc_name.as_str());
                    encoder.write_var(MSG_SYNC);
                    encoder.write_var(MSG_SYNC_UPDATE);
                    encoder.write_buf(&event.update);
                    let msg = encoder.to_vec();
                    callback.try_send(msg).expect("todo err handling");
                })
                .unwrap()
            };

            let callback = callback.clone();
            let closed = closed.clone();
            let doc_name = doc_name.clone();
            let awareness_subscription = awareness.on_update(move |awareness, e| {
                if closed.get().is_some() {
                    return;
                }

                debug!("awareneess update observed, sending to client");

                // https://github.com/y-crdt/y-sync/blob/56958e83acfd1f3c09f5dd67cf23c9c72f000707/src/net/broadcast.rs#L59
                let added = e.added();
                let updated = e.updated();
                let removed = e.removed();
                let mut changed = Vec::with_capacity(added.len() + updated.len() + removed.len());
                changed.extend_from_slice(added);
                changed.extend_from_slice(updated);
                changed.extend_from_slice(removed);

                if let Ok(u) = awareness.update_with_clients(changed) {
                    let mut encoder = EncoderV1::new();
                    encoder.write_string(doc_name.as_str());
                    Message::Awareness(u).encode(&mut encoder);
                    let msg = encoder.to_vec();
                    callback.try_send(msg).expect("todo err handling");
                }
            });

            (doc_subscription, awareness_subscription)
        };

        let protocol = DefaultProtocol;
        // Initial handshake is based on this:
        // https://github.com/y-crdt/y-sync/blob/56958e83acfd1f3c09f5dd67cf23c9c72f000707/src/sync.rs#L45-L54

        Self {
            doc_name,
            doc_server,
            awareness,
            doc_subscription,
            awareness_subscription,
            authorization: Arc::new(RwLock::new(Authorization::None)),
            protocol,
            sender: callback,
            client_id: OnceLock::new(),
            closed,
        }
    }

    pub async fn send(&self, mut update: DecoderV1<'_>) -> Result<(), anyhow::Error> {
        let msg = Message::decode(&mut update)?;
        self.send_message(msg).await?;

        Ok(())
    }

    pub async fn send_message(&self, msg: Message) -> Result<(), anyhow::Error> {
        let mut encoder = EncoderV1::new();
        encoder.write_string(self.doc_name.as_str());
        msg.encode(&mut encoder);
        self.send_raw(encoder.to_vec()).await
    }

    pub async fn send_raw(&self, msg: Vec<u8>) -> Result<(), anyhow::Error> {
        self.sender.send(msg).await?;

        Ok(())
    }

    // Adapted from:
    // https://github.com/y-crdt/y-sync/blob/56958e83acfd1f3c09f5dd67cf23c9c72f000707/src/net/conn.rs#L184C1-L222C1
    #[tracing::instrument(skip(self, msg), fields(doc_name = self.doc_name))]
    pub async fn handle_msg(&self, msg: Message) -> Result<Option<Message>, sync::Error> {
        debug!("Handling message for document: {:?}", msg);
        let protocol = &self.protocol;
        let awareness = &self.awareness;

        let can_write = matches!(*self.authorization.read().unwrap(), Authorization::Full);

        match msg {
            Message::Sync(msg) => match msg {
                SyncMessage::SyncStep1(sv) => {
                    let awareness = awareness.read().unwrap();
                    protocol.handle_sync_step1(&awareness, sv)
                }
                SyncMessage::SyncStep2(update) => {
                    if can_write {
                        let mut awareness = awareness.write().unwrap();
                        protocol.handle_sync_step2(&mut awareness, Update::decode_v1(&update)?)
                    } else {
                        Err(sync::Error::PermissionDenied {
                            reason: "Token does not have write access".to_string(),
                        })
                    }
                }
                SyncMessage::Update(update) => {
                    if can_write {
                        let mut awareness = awareness.write().unwrap();
                        protocol.handle_update(&mut awareness, Update::decode_v1(&update)?)
                    } else {
                        Err(sync::Error::PermissionDenied {
                            reason: "Token does not have write access".to_string(),
                        })
                    }
                }
            },
            Message::Auth(token, _) => {
                let token = token.unwrap_or_default();
                let config = self
                    .doc_server
                    .authenticate(&self.doc_name, token.as_str())
                    .await;

                let mut auth_failed = false;
                if let Ok(config) = config {
                    if config.is_authenticated {
                        if config.read_only {
                            *self.authorization.write().unwrap() = Authorization::ReadOnly;
                        } else {
                            *self.authorization.write().unwrap() = Authorization::Full;
                        }
                    } else {
                        *self.authorization.write().unwrap() = Authorization::None;
                        auth_failed = true;
                    }
                }

                if auth_failed {
                    tracing::warn!(
                        "Authentication failed for document: {}",
                        self.doc_name
                    );

                    let handle_auth_message =
                        protocol.handle_auth_fail(&self.awareness.read().unwrap());
                    self.send_message(handle_auth_message).await?;
                    return Err(sync::Error::PermissionDenied {
                        reason: "Authentication failed".to_string(),
                    });
                }

                let handle_auth_message =
                    protocol.handle_auth_success(&self.awareness.read().unwrap(), true);
                self.send_message(handle_auth_message).await?;

                if !self.awareness.read().unwrap().clients().is_empty() {
                    let awareness = protocol.awareness(&self.awareness.read().unwrap())?;
                    self.send_message(awareness).await?;
                } else {
                    debug!("No existing awareness states to send to client");
                }

                let sync1_message = protocol.sync_step1(&self.awareness.read().unwrap())?;
                self.send_message(sync1_message).await?;

                Ok(None)
            }
            Message::AwarenessQuery => {
                let awareness = awareness.read().unwrap();
                protocol.handle_awareness_query(&awareness)
            }
            Message::Awareness(update) => {
                if update.clients.len() == 1 {
                    let client_id = update.clients.keys().next().unwrap();
                    self.client_id.get_or_init(|| *client_id);
                } else {
                    tracing::warn!(
                        "Received awareness update with more than one client {:?}",
                        update.clients
                    );
                }
                let mut awareness = awareness.write().unwrap();
                protocol.handle_awareness_update(&mut awareness, update)
            }
            Message::SyncStatus(synced) => {
                debug!("Client sync status changed: synced={}", synced);

                Ok(None)
            }
            Message::Custom(SYNC_STATUS_MESSAGE, data) => {
                // Respond to the client with the same payload it sent.
                Ok(Some(Message::Custom(SYNC_STATUS_MESSAGE, data)))
            }
            Message::Custom(tag, data) => {
                let mut awareness = awareness.write().unwrap();
                protocol.missing_handle(&mut awareness, tag, data)
            }
        }
    }
}

impl<T: Protocol> Drop for DocConnection<T> {
    fn drop(&mut self) {
        self.closed.set(()).unwrap();

        // If this client had an awareness state, remove it.
        if let Some(client_id) = self.client_id.get() {
            let mut awareness = self.awareness.write().unwrap();
            awareness.remove_state(*client_id);
        }
    }
}
