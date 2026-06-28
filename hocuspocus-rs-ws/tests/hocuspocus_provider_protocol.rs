use anyhow::Result;
use async_trait::async_trait;
use hocuspocus_rs_ws::{
    client_connection::{ClientConnection, DocConnectionConfig, DocServer},
    sync::{AUTH_TOKEN, AUTHENTICATED, MSG_AUTH, Message, SyncMessage, awareness::Awareness},
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};
use tokio::{sync::mpsc, time::timeout};
use yrs::{
    Doc, GetString, ReadTxn, Text, Transact,
    encoding::read::{Cursor, Read},
    encoding::write::Write,
    updates::{
        decoder::{Decode, DecoderV1},
        encoder::{Encode, Encoder, EncoderV1},
    },
};

const DOC_NAME: &str = "hocuspocus-test";

#[derive(Default)]
struct TestDocServer {
    docs: Mutex<HashMap<String, Arc<RwLock<Awareness>>>>,
    auth_calls: Mutex<Vec<(String, String)>>,
    read_only: bool,
    is_authenticated: bool,
}

impl TestDocServer {
    fn new(read_only: bool) -> Self {
        Self {
            read_only,
            is_authenticated: true,
            ..Self::default()
        }
    }

    fn doc_text(&self, document_name: &str, text_name: &str) -> Option<String> {
        let doc = self.docs.lock().unwrap().get(document_name).cloned()?;
        let awareness = doc.read().unwrap();
        let txn = awareness.doc().transact();
        let text = txn.get_text(text_name)?;

        Some(text.get_string(&txn))
    }

    fn auth_calls(&self) -> Vec<(String, String)> {
        self.auth_calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl DocServer for TestDocServer {
    async fn fetch(&self, doc_id: &str) -> Result<Arc<RwLock<Awareness>>> {
        let mut docs = self.docs.lock().unwrap();
        Ok(docs
            .entry(doc_id.to_owned())
            .or_insert_with(|| Arc::new(RwLock::new(Awareness::new(Doc::new()))))
            .clone())
    }

    async fn authenticate(&self, doc_id: &str, token: &str) -> Result<DocConnectionConfig> {
        self.auth_calls
            .lock()
            .unwrap()
            .push((doc_id.to_owned(), token.to_owned()));

        Ok(DocConnectionConfig {
            read_only: self.read_only,
            is_authenticated: self.is_authenticated,
        })
    }
}

fn encode_frame(document_name: &str, message: Message) -> Vec<u8> {
    let mut encoder = EncoderV1::new();
    encoder.write_string(document_name);
    message.encode(&mut encoder);
    encoder.to_vec()
}

fn encode_provider_auth_frame(document_name: &str, token: &str) -> Vec<u8> {
    let mut encoder = EncoderV1::new();
    encoder.write_string(document_name);
    encoder.write_var(MSG_AUTH);
    encoder.write_var(AUTH_TOKEN);
    encoder.write_string(token);
    encoder.write_string("3.2.4");
    encoder.to_vec()
}

fn decode_frame(data: &[u8]) -> Result<(String, Message)> {
    let mut decoder = DecoderV1::new(Cursor::new(data));
    let document_name = decoder.read_string()?.to_owned();
    let message = Message::decode(&mut decoder)?;

    Ok((document_name, message))
}

fn decode_auth_response(data: &[u8]) -> Result<(String, u8, String)> {
    let mut decoder = DecoderV1::new(Cursor::new(data));
    let document_name = decoder.read_string()?.to_owned();
    let message_type: u8 = decoder.read_var()?;
    let auth_type: u8 = decoder.read_var()?;
    let scope = decoder.read_string()?.to_owned();

    assert_eq!(message_type, MSG_AUTH);
    Ok((document_name, auth_type, scope))
}

async fn recv_frame(receiver: &mut mpsc::Receiver<Vec<u8>>) -> Vec<u8> {
    timeout(Duration::from_secs(1), receiver.recv())
        .await
        .expect("server should send a frame")
        .expect("frame channel should stay open")
}

fn client_connection(server: Arc<TestDocServer>) -> (ClientConnection, mpsc::Receiver<Vec<u8>>) {
    let (tx, rx) = mpsc::channel(16);
    (
        ClientConnection::new(server, tx, Duration::from_secs(30), HashMap::new()),
        rx,
    )
}

async fn authenticate(
    connection: &ClientConnection,
    receiver: &mut mpsc::Receiver<Vec<u8>>,
    token: &str,
) -> (String, String) {
    connection
        .handle_message(&encode_provider_auth_frame(DOC_NAME, token))
        .await
        .expect("auth frame should be handled");

    let (document_name, auth_type, scope) =
        decode_auth_response(&recv_frame(receiver).await).expect("auth response should decode");
    assert_eq!(document_name, DOC_NAME);
    assert_eq!(auth_type, AUTHENTICATED);

    let (document_name, message) =
        decode_frame(&recv_frame(receiver).await).expect("initial sync step should decode");
    assert_eq!(document_name, DOC_NAME);
    assert!(matches!(message, Message::Sync(SyncMessage::SyncStep1(_))));

    (document_name, scope)
}

#[tokio::test]
async fn provider_auth_frame_returns_hocuspocus_authenticated_scope() {
    let server = Arc::new(TestDocServer::new(false));
    let (connection, mut receiver) = client_connection(server.clone());

    let (_, scope) = authenticate(&connection, &mut receiver, "writer-token").await;

    assert_eq!(scope, "read-write");
    assert_eq!(
        server.auth_calls(),
        vec![(DOC_NAME.to_owned(), "writer-token".to_owned())]
    );
}

#[tokio::test]
async fn provider_auth_frame_preserves_readonly_scope() {
    let server = Arc::new(TestDocServer::new(true));
    let (connection, mut receiver) = client_connection(server);

    let (_, scope) = authenticate(&connection, &mut receiver, "readonly-token").await;

    assert_eq!(scope, "read-only");
}

#[tokio::test]
async fn provider_sync_step_one_gets_sync_step_two_response() {
    let server = Arc::new(TestDocServer::new(false));
    let (connection, mut receiver) = client_connection(server);
    authenticate(&connection, &mut receiver, "writer-token").await;

    let client_doc = Doc::new();
    let state_vector = client_doc.transact().state_vector();
    connection
        .handle_message(&encode_frame(
            DOC_NAME,
            Message::Sync(SyncMessage::SyncStep1(state_vector)),
        ))
        .await
        .expect("sync step one should be handled");

    let (document_name, message) = decode_frame(&recv_frame(&mut receiver).await)
        .expect("sync step two response should decode");

    assert_eq!(document_name, DOC_NAME);
    assert!(matches!(message, Message::Sync(SyncMessage::SyncStep2(_))));
}

#[tokio::test]
async fn provider_update_frame_applies_doc_update_and_acks_sync_status() {
    let server = Arc::new(TestDocServer::new(false));
    let (connection, mut receiver) = client_connection(server.clone());
    authenticate(&connection, &mut receiver, "writer-token").await;

    let update = {
        let doc = Doc::with_client_id(42);
        let text = doc.get_or_insert_text("test");
        let mut txn = doc.transact_mut();
        text.push(&mut txn, "hello from provider");
        txn.encode_update_v1()
    };

    connection
        .handle_message(&encode_frame(
            DOC_NAME,
            Message::Sync(SyncMessage::Update(update)),
        ))
        .await
        .expect("update frame should be handled");

    let mut saw_update_broadcast = false;
    let mut saw_sync_status = false;

    for _ in 0..2 {
        let (document_name, message) =
            decode_frame(&recv_frame(&mut receiver).await).expect("update response should decode");
        assert_eq!(document_name, DOC_NAME);

        match message {
            Message::Sync(SyncMessage::Update(_)) => saw_update_broadcast = true,
            Message::SyncStatus(true) => saw_sync_status = true,
            other => panic!("unexpected update response: {other:?}"),
        }
    }

    assert!(saw_update_broadcast);
    assert!(saw_sync_status);
    assert_eq!(
        server.doc_text(DOC_NAME, "test"),
        Some("hello from provider".to_owned())
    );
}

#[tokio::test]
async fn provider_query_awareness_gets_awareness_response() {
    let server = Arc::new(TestDocServer::new(false));
    let (connection, mut receiver) = client_connection(server);
    authenticate(&connection, &mut receiver, "writer-token").await;

    connection
        .handle_message(&encode_frame(DOC_NAME, Message::AwarenessQuery))
        .await
        .expect("awareness query should be handled");

    let (document_name, message) =
        decode_frame(&recv_frame(&mut receiver).await).expect("awareness response should decode");

    assert_eq!(document_name, DOC_NAME);
    assert!(matches!(message, Message::Awareness(_)));
}
