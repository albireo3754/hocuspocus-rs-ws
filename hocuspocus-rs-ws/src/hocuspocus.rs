use crate::{
    authenticator::Authenticator, client_connection::{ClientConnection, DocConnectionConfig, DocServer}, doc_sync::DocWithSyncKv, store::{memory::MemoryStore, Store}, sync::awareness::Awareness, sync_kv::SyncKv, types::HocuspocusConfiguration
};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use axum::{
    Router,
    extract::{
        State,
        ws::{Message as WsMessage, WebSocketUpgrade},
    },
    response::IntoResponse,
    routing::get,
};
use dashmap::{DashMap, mapref::one::MappedRef};
use futures::SinkExt;
use futures_util::StreamExt;
use std::sync::Arc;
use std::{sync::RwLock, time::Duration};
use tokio::sync::mpsc::{self, channel};
use tokio::{net::TcpListener, sync::mpsc::Receiver};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{Instrument, Level, info, span};
use url::Url;

pub type HocuspocusServer = Arc<Server>;

pub struct Server {
    docs: Arc<DashMap<String, DocWithSyncKv>>,
    doc_worker_tracker: TaskTracker,
    store: Arc<dyn Store>,
    checkpoint_freq: Duration,
    authenticator: Option<Arc<dyn Authenticator>>,
    cancellation_token: CancellationToken,
    doc_gc: bool,
    port: u16,
}

impl Server {
    pub fn new(
        store: Arc<dyn Store>,
        checkpoint_freq: Duration,
        authenticator: Option<Arc<dyn Authenticator>>,
        cancellation_token: CancellationToken,
        doc_gc: bool,
        port: u16,
    ) -> Self {
        Self {
            docs: Arc::new(DashMap::new()),
            doc_worker_tracker: TaskTracker::new(),
            store,
            checkpoint_freq,
            authenticator,
            cancellation_token,
            doc_gc,
            port,
        }
    }

    pub async fn start(port: u16) -> anyhow::Result<()> {
        let server = Arc::new(Server {
            docs: Arc::new(DashMap::new()),
            doc_worker_tracker: TaskTracker::new(),
            store: Arc::new(MemoryStore::default()), // Some(Arc::new(Box::new(MemoryStore::default()))) ,
            checkpoint_freq: Duration::from_secs(60), // 1분마다 GC 및 체크포인트
            authenticator: None,
            cancellation_token: CancellationToken::new(),
            doc_gc: true,
            port,
        });
        let app = Router::new()
            .route("/", get(ws_handler))
            .with_state(server.clone());

        let addr = format!("0.0.0.0:{}", server.port);
        let listener = TcpListener::bind(&addr).await?;

        tracing::info!("Hocuspocus server listening on {}", addr);

        axum::serve(listener, app).await?;

        Ok(())
    }

    // pub async fn doc_exists(&self, doc_id: &str) -> bool {
    //     if self.docs.contains_key(doc_id) {
    //         return true;
    //     }
    //     self.store
    //         .exists(&format!("{}/data.ysweet", doc_id))
    //         .await
    //         .unwrap_or_default()
    // }

    // pub async fn create_doc(&self) -> Result<String> {
    //     let doc_id = nanoid::nanoid!();
    //     self.load_doc(&doc_id).await?;
    //     tracing::info!(doc_id=?doc_id, "Created doc");
    //     Ok(doc_id)
    // }

    async fn load_doc(&self, doc_id: &str) -> Result<()> {
        let (send, recv) = channel(1024);

        let dwskv = DocWithSyncKv::new(doc_id, self.store.clone(), move || {
            send.try_send(()).unwrap();
        })
        .await?;

        dwskv
            .sync_kv()
            .persist()
            .await
            .map_err(|e| anyhow!("Error persisting: {:?}", e))?;

        {
            let sync_kv = dwskv.sync_kv();
            let checkpoint_freq = self.checkpoint_freq;
            let doc_id = doc_id.to_string();
            let cancellation_token = self.cancellation_token.clone();

            // Spawn a task to save the document to the store when it changes.
            self.doc_worker_tracker.spawn(
                Self::doc_persistence_worker(
                    recv,
                    sync_kv,
                    checkpoint_freq,
                    doc_id.clone(),
                    cancellation_token.clone(),
                )
                .instrument(span!(Level::INFO, "save_loop", doc_id=?doc_id)),
            );

            if self.doc_gc {
                self.doc_worker_tracker.spawn(
                    Self::doc_gc_worker(
                        self.docs.clone(),
                        doc_id.clone(),
                        checkpoint_freq,
                        cancellation_token,
                    )
                    .instrument(span!(Level::INFO, "gc_loop", doc_id=?doc_id)),
                );
            }
        }

        self.docs.insert(doc_id.to_string(), dwskv);
        Ok(())
    }

    async fn doc_gc_worker(
        docs: Arc<DashMap<String, DocWithSyncKv>>,
        doc_id: String,
        checkpoint_freq: Duration,
        cancellation_token: CancellationToken,
    ) {
        let mut checkpoints_without_refs = 0;

        loop {
            tokio::select! {
                _ = tokio::time::sleep(checkpoint_freq) => {
                    if let Some(doc) = docs.get(&doc_id) {
                        let awareness = Arc::downgrade(&doc.awareness());
                        if awareness.strong_count() > 1 {
                            checkpoints_without_refs = 0;
                            tracing::debug!("doc is still alive - it has {} references", awareness.strong_count());
                        } else {
                            checkpoints_without_refs += 1;
                            tracing::info!("doc has only one reference, candidate for GC. checkpoints_without_refs: {}", checkpoints_without_refs);
                        }
                    } else {
                        break;
                    }

                    if checkpoints_without_refs >= 2 {
                        tracing::info!("GCing doc");
                        if let Some(doc) = docs.get(&doc_id) {
                            doc.sync_kv().shutdown();
                        }

                        docs.remove(&doc_id);
                        break;
                    }
                }
                _ = cancellation_token.cancelled() => {
                    break;
                }
            };
        }
        tracing::info!("Exiting gc_loop");
    }

    async fn doc_persistence_worker(
        mut recv: Receiver<()>,
        sync_kv: Arc<SyncKv>,
        checkpoint_freq: Duration,
        doc_id: String,
        cancellation_token: CancellationToken,
    ) {
        let mut last_save = std::time::Instant::now();

        loop {
            let is_done = tokio::select! {
                v = recv.recv() => v.is_none(),
                _ = cancellation_token.cancelled() => true,
                _ = tokio::time::sleep(checkpoint_freq) => {
                    sync_kv.is_shutdown()
                }
            };

            tracing::info!("Received signal. done: {}", is_done);
            let now = std::time::Instant::now();
            if !is_done && now - last_save < checkpoint_freq {
                let sleep = tokio::time::sleep(checkpoint_freq - (now - last_save));
                tokio::pin!(sleep);
                tracing::info!("Throttling.");

                loop {
                    tokio::select! {
                        _ = &mut sleep => {
                            break;
                        }
                        v = recv.recv() => {
                            tracing::info!("Received dirty while throttling.");
                            if v.is_none() {
                                break;
                            }
                        }
                        _ = cancellation_token.cancelled() => {
                            tracing::info!("Received cancellation while throttling.");
                            break;
                        }

                    }
                    tracing::info!("Done throttling.");
                }
            }
            tracing::info!("Persisting.");
            if let Err(e) = sync_kv.persist().await {
                tracing::error!(?e, "Error persisting.");
            } else {
                tracing::info!("Done persisting.");
            }
            last_save = std::time::Instant::now();

            if is_done {
                break;
            }
        }
        tracing::info!("Terminating loop for {}", doc_id);
    }

    pub async fn get_or_create_doc(
        &self,
        doc_id: &str,
    ) -> Result<MappedRef<String, DocWithSyncKv, DocWithSyncKv>> {
        if !self.docs.contains_key(doc_id) {
            tracing::info!(doc_id=?doc_id, "Loading doc");
            self.load_doc(doc_id).await?;
        }

        Ok(self
            .docs
            .get(doc_id)
            .ok_or_else(|| anyhow!("Failed to get-or-create doc"))?
            .map(|d| d))
    }

    // pub fn check_auth(
    //     &self,
    //     auth_header: Option<TypedHeader<headers::Authorization<headers::authorization::Bearer>>>,
    // ) -> Result<()> {
    //     if let Some(auth) = &self.authenticator {
    //         if let Some(TypedHeader(headers::Authorization(bearer))) = auth_header {
    //             if let Ok(()) =
    //                 auth.verify_server_token(bearer.token(), current_time_epoch_millis())
    //             {
    //                 return Ok(());
    //             }
    //         }
    //         Err((StatusCode::UNAUTHORIZED, anyhow!("Unauthorized.")))?
    //     } else {
    //         Ok(())
    //     }
    // }

    // pub async fn redact_error_middleware(req: Request, next: Next) -> impl IntoResponse {
    //     let resp = next.run(req).await;
    //     if resp.status().is_server_error() || resp.status().is_client_error() {
    //         // If we should redact errors, copy over only the status code and
    //         // not the response body.
    //         return resp.status().into_response();
    //     }
    //     resp
    // }
}

#[async_trait]
impl DocServer for Server {
    async fn fetch(&self, doc_id: &str) -> Result<Arc<RwLock<Awareness>>> {
        Ok(self.get_or_create_doc(doc_id).await?.awareness())
    }

    async fn authenticate(&self, doc_id: &str, token: &str) -> Result<DocConnectionConfig> {
        if let Some(auth) = &self.authenticator {
            Ok(auth.authenticate(doc_id, token).await?)
        } else {
            Ok(DocConnectionConfig::default())
        }
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(hocuspocus): State<Arc<Server>>,
    _request: axum::http::Request<axum::body::Body>,
) -> impl IntoResponse {
    // let document_name = document.unwrap_or_else(|| "default".to_string());
    // let document_name = "".to_string(); // Default document name

    // Trigger onUpgrade hooks
    // for extension in &hocuspocus.configuration.extensions {
    //     if let Err(e) = extension.on_upgrade(&request).await {
    //         tracing::error!("onUpgrade hook failed: {}", e);
    //         return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    //     }
    // }

    ws.on_upgrade(move |socket| handle_websocket_upgrade(socket, hocuspocus))
}

async fn handle_websocket_upgrade(
    socket: axum::extract::ws::WebSocket,
    hocuspocus: Arc<Server>,
) {
    tracing::debug!("handle_websocket_upgrade : {:?}", socket);

    let (_close_tx, _close_rx) = mpsc::channel::<()>(1);
    let (mut sink, stream) = socket.split();

    let hocuspocus_clone = hocuspocus.clone();
    let (tx_to_ws, mut rx_to_ws) = mpsc::channel(16);

    let client_connection = ClientConnection::new(
        hocuspocus_clone,
        tx_to_ws.clone(),
        Duration::from_secs(300),
        Default::default(),
    );

    tokio::spawn(async move {
        loop {
            match rx_to_ws.recv().await {
                Some(msg) => {
                    let _ = sink.send(WsMessage::Binary(msg.into())).await;
                }
                None => {
                    info!("client connection already closed");
                    return;
                }
            }
        }
    });

    let mut stream = stream;
    loop {
        match stream.next().await {
            Some(Ok(WsMessage::Binary(data))) => {
                tracing::debug!("Received buffer: {:?}", data);
                let result = client_connection.handle_message(&data).await;

                if let Err(e) = result {
                    tracing::warn!("Failed to handle message: {}", e);
                }
                // if let Err(e) = message_receiver
                //     .handle_ws_bytes(&document.clone(), data)
                //     .await
                // {
                //     tracing::error!("Failed to handle message: {}", e);
                // }
            }
            Some(Ok(WsMessage::Close(_))) => {
                drop(stream);
                break;
            }
            Some(Err(e)) => {
                drop(stream);
                tracing::error!("WebSocket error: {}", e);
                break;
            }
            None => {
                drop(stream);
                break;
            }
            _ => {
                // Ignore other message types
            }
        }
    }
}

pub async fn start_server(
    _configuration: HocuspocusConfiguration,
    port: u16,
) -> anyhow::Result<()> {
    // let hocuspocus = Hocuspocus::new(configuration).await?;
    // let store = if let Some(store) = store {
    //     let store = get_store_from_opts(store)?;
    //     store.init().await?;
    //     Some(store)
    // } else {
    //     tracing::warn!("No store set. Documents will be stored in memory only.");
    //     None
    // };

    // if !prod {
    //     print_server_url(auth.as_ref(), url_prefix.as_ref(), addr);
    // }

    // let token = CancellationToken::new();

    // let auth = if let Some(auth) = Some("<auth_key>") {
    //     Some(Authenticator::new(auth)?)
    // } else {
    //     tracing::warn!("No auth key set. Only use this for local development!");
    //     None
    // };

    // let server = Server::new(
    //     None,
    //     std::time::Duration::from_secs(10),
    //     None,
    //     // url_prefix.clone(),
    //     None,
    //     token.clone(),
    //     true,
    //     // *max_body_size,
    //     None,
    //     port,
    // );

    Server::start(port).await
}
