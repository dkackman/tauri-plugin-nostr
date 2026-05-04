use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_tungstenite::{accept_async, tungstenite::Message};

type Store = Arc<Mutex<HashMap<String, serde_json::Value>>>;

pub struct MockRelay {
    addr: SocketAddr,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl MockRelay {
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = oneshot::channel::<()>();
        let store: Store = Arc::new(Mutex::new(HashMap::new()));

        tokio::spawn(run_relay(listener, store, rx));

        // Brief pause so the listener loop is ready before tests connect.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        MockRelay {
            addr,
            shutdown_tx: Some(tx),
        }
    }

    pub fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    pub async fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

async fn run_relay(
    listener: TcpListener,
    store: Store,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        tokio::spawn(handle_connection(stream, Arc::clone(&store)));
                    }
                    Err(_) => break,
                }
            }
            _ = &mut shutdown_rx => break,
        }
    }
}

async fn handle_connection(stream: TcpStream, store: Store) {
    let ws = match accept_async(stream).await {
        Ok(ws) => ws,
        Err(_) => return,
    };
    let (mut write, mut read) = ws.split();

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };

        match msg {
            Message::Text(text) => {
                let text = text.to_string();
                let json: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(v) => v,
                    Err(_) => continue,
                };
                let arr = match json.as_array() {
                    Some(a) => a.clone(),
                    None => continue,
                };
                let kind = match arr.first().and_then(|v| v.as_str()) {
                    Some(k) => k.to_string(),
                    None => continue,
                };

                match kind.as_str() {
                    "EVENT" => {
                        if let Some(event) = arr.get(1) {
                            let event_id = event["id"].as_str().unwrap_or("").to_string();
                            let key = dtag_key(event);
                            store.lock().unwrap().insert(key, event.clone());
                            let ok = serde_json::json!(["OK", event_id, true, ""]).to_string();
                            let _ = write.send(Message::Text(ok.into())).await;
                        }
                    }
                    "REQ" => {
                        if let Some(sub_id) = arr.get(1).and_then(|v| v.as_str()) {
                            let sub_id = sub_id.to_string();
                            let filter = arr.get(2).cloned().unwrap_or(serde_json::json!({}));
                            let events: Vec<serde_json::Value> = store
                                .lock()
                                .unwrap()
                                .values()
                                .filter(|e| matches_filter(e, &filter))
                                .cloned()
                                .collect();

                            for event in events {
                                let msg =
                                    serde_json::json!(["EVENT", sub_id, event]).to_string();
                                let _ = write.send(Message::Text(msg.into())).await;
                            }
                            let eose = serde_json::json!(["EOSE", sub_id]).to_string();
                            let _ = write.send(Message::Text(eose.into())).await;
                        }
                    }
                    "CLOSE" => {}
                    _ => {}
                }
            }
            // Respond to WebSocket pings to keep the connection alive.
            Message::Ping(data) => {
                let _ = write.send(Message::Pong(data)).await;
            }
            // Close frame: exit the handler loop.
            Message::Close(_) => break,
            // Binary, Pong, Frame: ignore.
            _ => {}
        }
    }
}

fn dtag_key(event: &serde_json::Value) -> String {
    if let Some(tags) = event["tags"].as_array() {
        for tag in tags {
            if let Some(arr) = tag.as_array() {
                if arr.first().and_then(|v| v.as_str()) == Some("d") {
                    if let Some(val) = arr.get(1).and_then(|v| v.as_str()) {
                        return val.to_string();
                    }
                }
            }
        }
    }
    event["id"].as_str().unwrap_or("").to_string()
}

fn matches_filter(event: &serde_json::Value, filter: &serde_json::Value) -> bool {
    if let Some(kinds) = filter["kinds"].as_array() {
        let ek = event["kind"].as_u64().unwrap_or(0);
        if !kinds.iter().any(|k| k.as_u64() == Some(ek)) {
            return false;
        }
    }
    if let Some(authors) = filter["authors"].as_array() {
        let pk = event["pubkey"].as_str().unwrap_or("");
        if !authors.iter().any(|a| a.as_str() == Some(pk)) {
            return false;
        }
    }
    if let Some(d_tags) = filter["#d"].as_array() {
        let dv = dtag_key(event);
        if !d_tags.iter().any(|d| d.as_str() == Some(&dv)) {
            return false;
        }
    }
    true
}
