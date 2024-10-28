use axum::{routing::get, Router};
use tokio::sync::broadcast;
use crate::db::cache::Cache;

pub mod ws;
pub mod bits;

#[derive(Clone)]
pub struct AppState {
    broadcast_rx: broadcast::Sender<String>,
    pub cache: Cache,
}

impl AppState {
    pub async fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        let cache = Cache::new(tx.clone()).await.unwrap();

        AppState {
            broadcast_rx: tx,
            cache,
        }
    }

    pub fn broadcast_rx(&self) -> broadcast::Receiver<String> {
        self.broadcast_rx.subscribe()
    }
}


pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/ws",   get(ws::ws_handler))
        .route("/bits", get(bits::handle_bits))
        .route("/ping", get(|| async { "pong" }))
}