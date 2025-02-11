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

/* msg examples:

[---2bits----|---------2bits----------|-4bits-|-32bits--]
[ msg type   | sub msg                |       |         ]
---------------------------------------------------------
[   bitmap   | BitmapMessage.Add      |       |checkbox ]
[   bitmap   | BitmapMessage.Sub      |       |checkbox ]
[   bitmap   | BitmapMessage.Set      | 0-15  |checkbox ]
---------------------------------------------------------
[online users|                        |       |users num]

*/

pub enum MessageType {
    Bitmap(u32, BitmapMessage),
    OnlineUsers(OnlineUsersMessage)
}

#[derive(Debug)]
pub enum BitmapMessage {
    Add,
    Sub,
    Set(u8),
}

pub struct OnlineUsersMessage(pub u32);
impl OnlineUsersMessage {
    pub fn ws_msg(&self) -> Vec<u8> {
        let bytes = self.0.to_le_bytes();        
        vec![0b01_000000, bytes[0], bytes[1], bytes[2], bytes[3]]
    }
}



impl MessageType {
    pub fn from_u8_array(msg: &Vec<u8>) -> Option<Self> {
        if msg.len() != 5 {
            return None;
        }

        let byte1 = msg[0];
        let msg_type = (byte1 >> 6) & 0b11; // first 2 bits
        let operation = (byte1 >> 4) & 0b11; // next 2 bits
        let i = u32::from_le_bytes([msg[1], msg[2], msg[3], msg[4]]); // last 32 bits

        match msg_type {
            0 => {
                if i as usize >= BITS_LEN {
                    return None;
                }
                let bitmap_msg = match operation {
                    0 => BitmapMessage::Add,
                    1 => BitmapMessage::Sub,
                    2 => {
                        let new_value = byte1 & 0b1111; // last 4 bits (byte1)
                        if new_value > 15 {
                            return None;
                        }
                        BitmapMessage::Set(new_value)
                    }
                    _ => return None,
                };
                Some(MessageType::Bitmap(i, bitmap_msg))
            }
            1 => Some(MessageType::OnlineUsers(OnlineUsersMessage(i))),
            _ => None
        }
    }
}

async fn handle_ws(
    socket: WebSocket,
    mut app_state: AppState,
) {
    let (mut tx, mut rx) = socket.split();
    let (stop_tx, mut stop_rx) = mpsc::channel(1);
    
    let mut app_state_cp = app_state.clone();

    tokio::spawn(async move {
        let mut broadcast_rx = app_state_cp.broadcast_rx();
        let _ = app_state_cp.cache.add_online_user().await;

        loop {
            select! {
                _ = stop_rx.recv() => break,
                msg = broadcast_rx.recv() => {
                    match msg {
                        Ok(msg) => if let Err(_) = tx.send(Message::Binary(msg)).await {
                            break
                        },
                        Err(_) => break   
                    }
                }
            }
        }
    });
    
    while let Some(Ok(Message::Binary(msg))) = rx.next().await {
        if let Some(MessageType::Bitmap(i, bitmap_msg)) = MessageType::from_u8_array(&msg) {
            if let Err(e) = app_state.cache.bitmap_modify(i as isize, bitmap_msg, &msg).await {
                println!("ERROR: {}", e);
            }
        }
    }
    
    // drop broadcast_rx
    let _ = app_state.cache.sub_online_user().await;
    let _ = stop_tx.send(());
}