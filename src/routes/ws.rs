use axum::response::IntoResponse;
use axum::extract::{State, WebSocketUpgrade, ws::{Message, WebSocket}};
use futures::{SinkExt, StreamExt};
use tokio::{select, sync::mpsc};

use crate::db::cache::BITS_LEN;
use super::AppState;

pub async fn ws_handler(
    upgrade: WebSocketUpgrade,
    State(app_state): State<AppState>,
) -> impl IntoResponse {
    upgrade.on_upgrade(|socket| async {
        handle_ws(socket, app_state).await
    })
}

pub enum MessageType {
    Add,
    Sub,
    Set(u8),
}
impl MessageType {
    pub fn from_str(txt: &str) -> Option<(usize, Self)> {
        let (i, value) = txt.split_once(";")?;       


        let i = i.parse::<usize>().ok()?;
        if i >= BITS_LEN {
            return None;
        }

        let msg_type = match value {
            "add" => Self::Add,
            "sub" => Self::Sub,
            _ if value.starts_with("set;") => {
                let (_, new_value) = value.split_once(";")?;
                let new_value = new_value.parse::<u8>().ok()?;
                if new_value > 15 {
                    return None;
                }
                Self::Set(new_value)
            },
            _ => return None
        };

        Some((i, msg_type))
    }
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
        /* msg types:
            1234;add
            1234;sub
            1234;set;15
        */

        if let Some((i, msg_type)) = MessageType::from_str(&txt) {
            println!("{txt}");
            if let Err(e) = app_state.cache.bitmap_modify(i as isize, msg_type, &txt).await {
                println!("e: {}", e);
            }
        }
    }
    
    // drop broadcast_rx
    let _ = stop_tx.send(());
}