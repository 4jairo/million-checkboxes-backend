use axum::response::IntoResponse;
use axum::extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}};
use futures::{SinkExt, StreamExt};
use tokio::{select, sync::mpsc};

use crate::db::cache::BITMAP_QTY;
use super::AppState;

pub async fn ws_handler(
    upgrade: WebSocketUpgrade,
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    upgrade.on_upgrade(|socket| async {
        handle_ws(socket, app_state).await
    })
}

async fn handle_ws(
    socket: WebSocket,
    mut app_state: AppState,
) {
    let (mut tx, mut rx) = socket.split();
    let (stop_tx, mut stop_rx) = mpsc::channel(1);
    
    let mut broadcast_rx = app_state.broadcast_rx();

    tokio::spawn(async move {
        loop {
            select! {
                _ = stop_rx.recv() => break,
                msg = broadcast_rx.recv() => {
                    match msg {
                        Ok(msg) => if let Err(_) = tx.send(Message::Text(msg)).await {
                            break
                        },
                        Err(_) => break   
                    }
                }
            }
        }
    });
    
    while let Some(Ok(Message::Text(txt))) = rx.next().await {   
        // txt -> "2134;true"

        if let Some((i, value)) = parse_msg(&txt) {
            println!("{i} -> {value}");

            if let Err(e) = app_state.cache.set(i, value, &txt).await {
                println!("e: {}", e);
            }
        }
    }
    
    // drop broadcast_rx
    let _ = stop_tx.send(());
}


fn parse_msg(txt: &String) -> Option<(usize, bool)> {
    let (i, value) = txt.split_once(";")?;

    let i = i.parse::<usize>().ok()?;
    if i >= BITMAP_QTY {
        return None;
    }

    let value = value.parse::<bool>().ok()?;

    Some((i, value))
}
