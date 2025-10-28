use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;

use dashmap::DashMap;
use futures_util::StreamExt;
use hocuspocus_extension_database::types::{FetchContext, StoreContext};
use hocuspocus_extension_database::DatabaseExtension;
use tokio::sync::mpsc as tokio_mpsc;
use tokio::time::{sleep_until, Instant};
use yrs::encoding::read::{Cursor as YCursor, Read as YRead};
use yrs::encoding::write::Write as YWrite;
use yrs::sync::{Awareness, DefaultProtocol, Message as YMsg, Protocol};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, StateVector, Transact, Update};

#[cfg(feature = "redis")]
use hocuspocus_extension_redis::RedisBroadcaster;

pub struct AppState<E: DatabaseExtension> {
    pub db: Arc<E>,
    pub debounce_ms: u64,
    pub max_debounce_ms: u64,
    pub doc_counts: DashMap<String, usize>,
    pub doc_latest: DashMap<String, Vec<u8>>, // last full state per doc
    #[cfg(feature = "redis")]
    pub redis: Option<Arc<RedisBroadcaster>>, // optional broadcaster
    // Optional authentication provider; when None, auth is disabled
    pub auth: Option<Arc<dyn AuthProvider + Send + Sync>>,
}

pub async fn ws_handler<E: DatabaseExtension + 'static>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState<E>>>,
) -> impl IntoResponse {
    tracing::debug!("upgrade request");
    ws.on_upgrade(move |socket| on_ws::<E>(socket, state))
}

const MSG_SYNC: u32 = 0;
const MSG_AWARENESS: u32 = 1;
const MSG_AUTH: u32 = 2;
const MSG_QUERY_AWARENESS: u32 = 3;
const MSG_SYNC_STATUS: u32 = 8;

enum WorkerCmd {
    ApplyState(Vec<u8>),
    InboundWs(Vec<u8>),
    SetReadonly(bool),
    Stop,
}

enum WorkerEvent {
    OutgoingWs(Vec<u8>),
    StoreState(Vec<u8>),
}

fn worker_thread(
    cmd_rx: std::sync::mpsc::Receiver<WorkerCmd>,
    ev_tx: tokio_mpsc::Sender<WorkerEvent>,
) {
    // Rationale:
    // - Use Hocuspocus framing (varstring document name + varuint message type) instead of
    //   raw yrs protocol. This enables multiplexing multiple documents over a single socket and
    //   supports Hocuspocus-specific message kinds (e.g. SyncReply, SyncStatus, Stateless) that
    //   yrs::sync does not define.
    // - Libraries like yrs-axum speak pure yrs (no outer envelope). That approach is simpler,
    //   but incompatible with Hocuspocus Provider semantics and our routing/debounce/storage flow.
    // - We therefore parse the outer envelope here and handle inner y-sync/awareness with yrs
    //   types. Auth is ignored for the MVP, and we persist full state blobs only (no increments),
    //   while the server side debounces store operations.
    // Invariants:
    // - All inbound/outbound frames are prefixed with the document name.
    // - Storage events carry the full document state; the database extension remains stateless.
    let doc = Doc::new();
    let mut awareness = Awareness::new(doc.clone());
    let protocol = DefaultProtocol;
    let mut is_readonly = false;

    // when doc updates, compute state and send StoreState
    let ev_tx_updates = ev_tx.clone();
    let doc_for_obs = doc.clone();
    let doc_for_txn = doc.clone();
    doc_for_obs
        .observe_update_v1(move |_txn, _u| {
            let bytes = {
                let txn = doc_for_txn.transact();
                let sv = StateVector::default();
                txn.encode_state_as_update_v1(&sv)
            };
            let _ = ev_tx_updates.try_send(WorkerEvent::StoreState(bytes));
        })
        .ok();

    while let Ok(cmd) = cmd_rx.recv() {
        match cmd {
            WorkerCmd::ApplyState(bytes) => {
                if let Ok(update) = Update::decode_v1(&bytes) {
                    if let Err(e) = doc.transact_mut().apply_update(update) {
                        tracing::warn!(?e, "failed to apply initial state");
                    }
                }
            }
            WorkerCmd::InboundWs(data) => {
                if data.is_empty() {
                    continue;
                }
                // read incoming document name (varstring) and outer message type using yrs encoding
                let mut cur = YCursor::new(&data);
                let frame_doc_name = cur.read_string().unwrap_or("").to_string();
                let t: u32 = cur.read_var().unwrap_or(0);
                tracing::debug!(doc_name = %frame_doc_name, msg_type = t, len = data.len(), "inbound frame");
                let body = &data[cur.next..];
                match t {
                    MSG_SYNC => {
                        // Manual parse of y-sync submessage
                        if body.is_empty() {
                            tracing::debug!("empty y-sync body");
                        } else {
                            let mut bcur = YCursor::new(body);
                            let subtag: u32 = bcur.read_var().unwrap_or(u32::MAX);
                            match subtag {
                                0 => {
                                    // SyncStep1(sv)
                                    if let Ok(sv_bytes) = bcur.read_buf() {
                                        if let Ok(sv) = StateVector::decode_v1(sv_bytes) {
                                            // reply with SyncStep2 from doc state diff
                                            let update = {
                                                let txn = doc.transact();
                                                txn.encode_state_as_update_v1(&sv)
                                            };
                                            let mut out = Vec::new();
                                            out.write_string(&frame_doc_name);
                                            out.write_var(MSG_SYNC);
                                            // subtag 1: SyncStep2(update)
                                            out.write_var(1u32);
                                            // writeVarUint8Array(update)
                                            out.write_buf(&update);
                                            tracing::debug!(len = out.len(), out_doc = %frame_doc_name, manual = true, "outbound sync step2 reply");
                                            let _ =
                                                ev_tx.blocking_send(WorkerEvent::OutgoingWs(out));
                                        }
                                    }
                                }
                                1 | 2 => {
                                    // SyncStep2 or Update
                                    if let Ok(upd_bytes) = bcur.read_buf() {
                                        if is_readonly {
                                            // In read-only mode, ignore incoming updates but still acknowledge to avoid client loops.
                                            let mut ack = Vec::new();
                                            ack.write_string(&frame_doc_name);
                                            ack.write_var(MSG_SYNC_STATUS);
                                            ack.write_var(1u32);
                                            tracing::debug!(out_doc = %frame_doc_name, manual = true, "ack in readonly mode (no apply)");
                                            let _ =
                                                ev_tx.blocking_send(WorkerEvent::OutgoingWs(ack));
                                        } else {
                                            // apply incoming update (v1 only)
                                            if let Ok(update) = Update::decode_v1(upd_bytes) {
                                                if let Err(e) =
                                                    doc.transact_mut().apply_update(update)
                                                {
                                                    tracing::warn!(?e, "failed to apply update");
                                                }
                                                // send SyncStatus applied=1
                                                let mut ack = Vec::new();
                                                ack.write_string(&frame_doc_name);
                                                ack.write_var(MSG_SYNC_STATUS);
                                                ack.write_var(1u32);
                                                tracing::debug!(out_doc = %frame_doc_name, manual = true, "outbound sync status ack");
                                                let _ = ev_tx
                                                    .blocking_send(WorkerEvent::OutgoingWs(ack));

                                                // also emit StoreState immediately to ensure persistence
                                                let bytes = {
                                                    let txn = doc.transact();
                                                    let sv = StateVector::default();
                                                    txn.encode_state_as_update_v1(&sv)
                                                };
                                                let _ = ev_tx
                                                    .blocking_send(WorkerEvent::StoreState(bytes));
                                            } else {
                                                tracing::debug!(
                                                    "failed to decode update bytes (v1 only)"
                                                );
                                            }
                                        }
                                    }
                                }
                                _ => {
                                    tracing::debug!(subtag, "unknown y-sync submessage");
                                }
                            }
                        }
                    }
                    2 => {
                        // Auth messages are handled in the outer task (on_ws). Worker shouldn't receive them.
                        tracing::debug!("auth message received by worker; ignoring");
                    }
                    MSG_AWARENESS => {
                        if let Ok(inner) = YMsg::decode_v1(body) {
                            if let YMsg::Awareness(update) = inner {
                                let reply = protocol
                                    .handle_awareness_update(&mut awareness, update)
                                    .ok()
                                    .flatten();
                                if let Some(msg) = reply {
                                    let mut out = Vec::new();
                                    out.write_string(&frame_doc_name);
                                    out.write_var(MSG_AWARENESS);
                                    out.extend(msg.encode_v1());
                                    tracing::debug!(len = out.len(), out_doc = %frame_doc_name, "outbound awareness echo");
                                    let _ = ev_tx.blocking_send(WorkerEvent::OutgoingWs(out));
                                }
                            }
                        } else {
                            tracing::debug!("failed to decode awareness message");
                        }
                    }
                    MSG_QUERY_AWARENESS => {
                        if let Ok(Some(reply)) = protocol.handle_awareness_query(&awareness) {
                            let mut out = Vec::new();
                            out.write_string(&frame_doc_name);
                            out.write_var(MSG_AWARENESS);
                            out.extend(reply.encode_v1());
                            tracing::debug!(len = out.len(), out_doc = %frame_doc_name, "outbound awareness reply");
                            let _ = ev_tx.blocking_send(WorkerEvent::OutgoingWs(out));
                        }
                    }
                    _ => {}
                }
            }
            WorkerCmd::SetReadonly(ro) => {
                is_readonly = ro;
            }
            WorkerCmd::Stop => break,
        }
    }
}

async fn on_ws<E: DatabaseExtension + 'static>(mut socket: WebSocket, state: Arc<AppState<E>>) {
    tracing::debug!("connection established");

    // channels for worker communication
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<WorkerCmd>();
    let (ev_tx, mut ev_rx) = tokio_mpsc::channel::<WorkerEvent>(64);

    // spawn worker thread
    std::thread::spawn(move || worker_thread(cmd_rx, ev_tx));

    // debounce state
    let mut pending = false;
    let mut first_pending_at: Option<Instant> = None;
    let mut next_deadline: Option<Instant> = None;
    let mut latest_state_bytes: Option<Vec<u8>> = None;
    let mut selected_doc_name: Option<String> = None;
    let mut is_authenticated: bool = state.auth.is_none();
    let mut loaded_state: bool = false;
    #[cfg(feature = "redis")]
    let mut redis_sub_handle: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        let sleep_fut = if let Some(deadline) = next_deadline {
            Some(Box::pin(sleep_until(deadline)))
        } else {
            None
        };

        tokio::select! {
            // WebSocket input
            maybe_msg = socket.next() => {
                match maybe_msg {
                    Some(Ok(Message::Binary(b))) => {
                        // detect document name from first frame and load state before forwarding
                        let mut handled_by_auth = false;
                        if selected_doc_name.is_none() {
                            let mut cur = YCursor::new(b.as_ref());
                            if let Ok(name_str) = cur.read_string() {
                                let name = name_str.to_string();
                                tracing::debug!(document_name = %name, "first frame received");
                                // increment connection count for this doc
                                state.doc_counts.entry(name.clone()).and_modify(|c| *c += 1).or_insert(1);
                                selected_doc_name = Some(name.clone());

                                // If auth is enabled, require auth before loading state or forwarding messages
                                if state.auth.is_some() && !is_authenticated {
                                    // Peek message type
                                    let mtype: u32 = cur.read_var().unwrap_or(u32::MAX);
                                    if mtype == MSG_AUTH {
                                        // subtag
                                        let sub: u32 = cur.read_var().unwrap_or(u32::MAX);
                                        if sub == 0 {
                                            // token
                                            if let Ok(token_str) = cur.read_string() {
                                                if let Some(provider) = state.auth.as_ref() {
                                                    match provider.on_authenticate(&name, &token_str) {
                                                        Ok(scope) => {
                                                            // send authenticated reply
                                                            let readonly = matches!(scope, AuthScope::ReadOnly);
                                                            send_auth_authenticated(&mut socket, &name, readonly).await;
                                                            is_authenticated = true;
                                                            let _ = cmd_tx.send(WorkerCmd::SetReadonly(readonly));
                                                            handled_by_auth = true; // don't forward auth frame
                                                        }
                                                        Err(e) => {
                                                            send_auth_permission_denied(&mut socket, &name, Some(&format!("{}", e))).await;
                                                            // Keep the connection open; ask for token again.
                                                            send_auth_token_request(&mut socket, &name).await;
                                                            handled_by_auth = true;
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            // unexpected auth subtype
                                            send_auth_permission_denied(&mut socket, &name, Some("invalid-auth-message")).await;
                                            // Keep the connection open; ask for token again.
                                            send_auth_token_request(&mut socket, &name).await;
                                            handled_by_auth = true;
                                        }
                                    } else {
                                        // Not an auth message: request token
                                        send_auth_token_request(&mut socket, &name).await;
                                        handled_by_auth = true; // don't forward non-auth
                                    }
                                }

                                // subscribe to redis after doc is known
                                #[cfg(feature = "redis")]
                                if let Some(bc) = state.redis.as_ref() {
                                    let (handle, mut rx) = bc.subscribe(name.clone()).await.expect("redis subscribe");
                                    redis_sub_handle = Some(handle);
                                    let tx_clone = cmd_tx.clone();
                                    tokio::spawn(async move {
                                        while let Some((_is_sync, body)) = rx.recv().await {
                                            let _ = tx_clone.send(WorkerCmd::InboundWs(body.to_vec()));
                                        }
                                    });
                                }
                            } else {
                                tracing::debug!("failed to read document name from first frame");
                            }
                        } else if state.auth.is_some() && !is_authenticated {
                            // Post-selection but pre-auth: only process auth frames
                            let mut cur = YCursor::new(b.as_ref());
                            let _ = cur.read_string(); // skip name
                            let mtype: u32 = cur.read_var().unwrap_or(u32::MAX);
                            if mtype == MSG_AUTH {
                                let name = selected_doc_name.as_ref().unwrap().clone();
                                let sub: u32 = cur.read_var().unwrap_or(u32::MAX);
                                if sub == 0 {
                                    if let Ok(token_str) = cur.read_string() {
                                        if let Some(provider) = state.auth.as_ref() {
                                            match provider.on_authenticate(&name, &token_str) {
                                                Ok(scope) => {
                                                    let readonly = matches!(scope, AuthScope::ReadOnly);
                                                    send_auth_authenticated(&mut socket, &name, readonly).await;
                                                    is_authenticated = true;
                                                    let _ = cmd_tx.send(WorkerCmd::SetReadonly(readonly));
                                                    handled_by_auth = true;
                                                }
                                                Err(e) => {
                                                    send_auth_permission_denied(&mut socket, &name, Some(&format!("{}", e))).await;
                                                    // Keep the connection open; ask for token again, same as Hocuspocus.
                                                    send_auth_token_request(&mut socket, &name).await;
                                                    handled_by_auth = true;
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    send_auth_permission_denied(&mut socket, &name, Some("invalid-auth-message")).await;
                                    // Keep the connection open; ask for token again, same as Hocuspocus.
                                    send_auth_token_request(&mut socket, &name).await;
                                    handled_by_auth = true;
                                }
                            } else {
                                // request token again and ignore
                                let name = selected_doc_name.as_ref().unwrap().clone();
                                send_auth_token_request(&mut socket, &name).await;
                                handled_by_auth = true;
                            }
                        }

                        // If we just authenticated or auth is disabled, ensure state is loaded once
                        if !loaded_state {
                            if let Some(name) = selected_doc_name.as_ref() {
                                if is_authenticated {
                                    tracing::debug!(document_name = %name, "loading state after auth");
                                    if let Err(e) = load_and_send_state(&*state.db, name, &cmd_tx).await {
                                        tracing::warn!(error = %e, document_name = %name, "failed to load/apply state");
                                    }
                                    loaded_state = true;
                                }
                            }
                        }

                        if !handled_by_auth {
                            let _ = cmd_tx.send(WorkerCmd::InboundWs(b.to_vec()));
                        }
                    }
                    Some(Ok(Message::Ping(p))) => {
                        tracing::debug!(size = %p.len(), "ping received");
                        let _ = socket.send(Message::Pong(p)).await;
                    }
                    Some(Ok(Message::Close(frame))) => {
                        tracing::debug!(pending = pending, ?frame, "closing connection");
                        // decrement connection count and if last, force save latest known state
                        if let Some(name) = selected_doc_name.as_ref() {
                            if let Some(mut entry) = state.doc_counts.get_mut(name) {
                                *entry -= 1;
                                let remaining = *entry;
                                drop(entry);
                                if remaining == 0 {
                                    let to_store = latest_state_bytes
                                        .as_ref()
                                        .cloned()
                                        .or_else(|| state.doc_latest.get(name).map(|v| v.clone()));
                                    if let Some(bytes) = to_store {
                                        tracing::debug!(document_name = %name, "last client left; force storing state");
                                        let _ = store_bytes(&*state.db, name, &bytes).await;
                                    }
                                    state.doc_counts.remove(name);
                                    state.doc_latest.remove(name);
                                }
                            }
                        }
                        if pending {
                            tracing::debug!(pending = pending, "storing state on close");
                            if let (Some(bytes), Some(name)) = (latest_state_bytes.as_ref(), selected_doc_name.as_ref()) {
                                let _ = store_bytes(&*state.db, name, bytes).await;
                            }
                        }
                        let _ = cmd_tx.send(WorkerCmd::Stop);
                        #[cfg(feature = "redis")]
                        if let Some(h) = redis_sub_handle.take() { h.abort(); }
                        break;
                    }
                    None => {
                        tracing::debug!(pending = pending, "socket closed by peer");
                        // decrement connection count and if last, force save latest known state
                        if let Some(name) = selected_doc_name.as_ref() {
                            if let Some(mut entry) = state.doc_counts.get_mut(name) {
                                *entry -= 1;
                                let remaining = *entry;
                                drop(entry);
                                if remaining == 0 {
                                    let to_store = latest_state_bytes
                                        .as_ref()
                                        .cloned()
                                        .or_else(|| state.doc_latest.get(name).map(|v| v.clone()));
                                    if let Some(bytes) = to_store {
                                        tracing::debug!(document_name = %name, "last client left; force storing state");
                                        let _ = store_bytes(&*state.db, name, &bytes).await;
                                    }
                                    state.doc_counts.remove(name);
                                    state.doc_latest.remove(name);
                                }
                            }
                        }
                        if pending {
                            tracing::debug!(pending = pending, "storing state on close");
                            if let (Some(bytes), Some(name)) = (latest_state_bytes.as_ref(), selected_doc_name.as_ref()) {
                                let _ = store_bytes(&*state.db, name, bytes).await;
                            }
                        }
                        let _ = cmd_tx.send(WorkerCmd::Stop);
                        #[cfg(feature = "redis")]
                        if let Some(h) = redis_sub_handle.take() { h.abort(); }
                        break;
                    }
                    _ => {}
                }
            }
            // Worker events
            Some(ev) = ev_rx.recv() => {
                match ev {
                    WorkerEvent::OutgoingWs(bytes) => {
                        tracing::debug!(len = bytes.len(), "sending binary to client");
                        let _ = socket
                            .send(Message::Binary(axum::body::Bytes::from(bytes.clone())))
                            .await;
                        #[cfg(feature = "redis")]
                        if let (Some(name), Some(bc)) = (selected_doc_name.as_ref(), state.redis.as_ref()) {
                            // determine message type after varstring
                            let mut cur = YCursor::new(&bytes);
                            let _ = cur.read_string();
                            let mtype: u32 = cur.read_var().unwrap_or(u32::MAX);
                            let payload = &bytes[..];
                            // publish full framed message so other instances can forward as-is
                            if mtype == MSG_SYNC {
                                let _ = bc.publish_sync(name, payload).await;
                            } else if mtype == MSG_AWARENESS {
                                let _ = bc.publish_awareness(name, payload).await;
                            }
                        }
                    }
                    WorkerEvent::StoreState(bytes) => {
                        tracing::debug!(state_len = bytes.len(), "doc update observed; scheduling store");
                        if let Some(name) = selected_doc_name.as_ref() {
                            state.doc_latest.insert(name.clone(), bytes.clone());
                        }
                        latest_state_bytes = Some(bytes);
                        let now = Instant::now();
                        pending = true;
                        if first_pending_at.is_none() { first_pending_at = Some(now); }
                        let debounced = now + Duration::from_millis(state.debounce_ms);
                        let cap = first_pending_at.unwrap() + Duration::from_millis(state.max_debounce_ms);
                        let target = if debounced > cap { cap } else { debounced };
                        next_deadline = Some(target);
                    }
                }
            }
            // Debounce timer
            _ = async { if let Some(fut) = sleep_fut { fut.await } } , if next_deadline.is_some() => {
                if pending {
                    if let (Some(bytes), Some(name)) = (latest_state_bytes.as_ref(), selected_doc_name.as_ref()) {
                        tracing::debug!(document_name = %name, "debounce elapsed; storing state");
                        if let Err(e) = store_bytes(&*state.db, name, bytes).await {
                            tracing::warn!(error = %e, document_name = %name, "failed to store state");
                        }
                    }
                    pending = false;
                }
                first_pending_at = None;
                next_deadline = None;
            }
        }
    }
}

async fn load_and_send_state<E: DatabaseExtension>(
    db: &E,
    name: &str,
    cmd_tx: &std::sync::mpsc::Sender<WorkerCmd>,
) -> Result<()> {
    tracing::debug!(document_name = %name, "fetching state");
    if let Some(bytes) = db
        .fetch(FetchContext {
            document_name: name.to_string(),
        })
        .await?
    {
        tracing::debug!(document_name = %name, bytes = bytes.len(), "applying fetched state (worker)");
        let _ = cmd_tx.send(WorkerCmd::ApplyState(bytes));
    } else {
        tracing::debug!(document_name = %name, "no stored state; starting empty doc");
    }
    Ok(())
}

async fn store_bytes<E: DatabaseExtension>(db: &E, name: &str, bytes: &[u8]) -> Result<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    db.store(StoreContext {
        document_name: name.to_string(),
        state: bytes,
        updated_at_millis: now,
    })
    .await?;
    Ok(())
}

// ===== Auth support (Hocuspocus-compatible) =====

#[derive(Clone, Copy, Debug)]
pub enum AuthScope {
    ReadOnly,
    ReadWrite,
}

pub trait AuthProvider {
    fn on_authenticate(&self, document_name: &str, token: &str) -> anyhow::Result<AuthScope>;
}

pub struct StaticTokenAuth {
    pub token: String,
    pub scope: AuthScope,
}

impl AuthProvider for StaticTokenAuth {
    fn on_authenticate(&self, _document_name: &str, token: &str) -> anyhow::Result<AuthScope> {
        if token == self.token {
            Ok(self.scope)
        } else {
            anyhow::bail!("permission-denied");
        }
    }
}

async fn send_auth_token_request(socket: &mut WebSocket, name: &str) {
    let mut out = Vec::new();
    out.write_string(name);
    out.write_var(MSG_AUTH);
    // AuthMessageType.Token = 0
    out.write_var(0u32);
    let _ = socket
        .send(Message::Binary(axum::body::Bytes::from(out)))
        .await;
}

async fn send_auth_authenticated(socket: &mut WebSocket, name: &str, readonly: bool) {
    let mut out = Vec::new();
    out.write_string(name);
    out.write_var(MSG_AUTH);
    // AuthMessageType.Authenticated = 2
    out.write_var(2u32);
    out.write_string(if readonly { "readonly" } else { "read-write" });
    let _ = socket
        .send(Message::Binary(axum::body::Bytes::from(out)))
        .await;
}

async fn send_auth_permission_denied(socket: &mut WebSocket, name: &str, reason: Option<&str>) {
    let mut out = Vec::new();
    out.write_string(name);
    out.write_var(MSG_AUTH);
    // AuthMessageType.PermissionDenied = 1
    out.write_var(1u32);
    out.write_string(reason.unwrap_or("permission-denied"));
    let _ = socket
        .send(Message::Binary(axum::body::Bytes::from(out)))
        .await;
}
