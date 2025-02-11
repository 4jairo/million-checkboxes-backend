use futures::StreamExt;
use redis::{aio::MultiplexedConnection, AsyncCommands, RedisResult};
use tokio::sync::broadcast;

use crate::routes::ws::{BitmapMessage, OnlineUsersMessage};


#[derive(Clone)]
pub struct Cache {
    client: MultiplexedConnection
}

pub const BITS_PER_CHECKBOX: usize = 4;
pub const BITS_LEN: usize = 1_000_000 * BITS_PER_CHECKBOX;

const ONLINE_USERS_KEY: &str = "users";
const ONLINE_USERS_UDPATE_CHANNEL: &str = "users_update";
const BITMAP_KEY: &str = "state";
const BITMAP_UPDATE_CHANNEL: &str = "state_update";

impl Cache {
    pub async fn new(tx: broadcast::Sender<Vec<u8>>) -> RedisResult<Self> {
        let redis_url = env_var!("REDIS_URL");
        let conn = redis::Client::open(redis_url)?;
        let client = conn.get_multiplexed_tokio_connection().await?;

        let mut pubsub = conn.get_async_pubsub().await?;
    
        tokio::spawn(async move {
            pubsub.subscribe(BITMAP_UPDATE_CHANNEL).await?;
            pubsub.subscribe(ONLINE_USERS_UDPATE_CHANNEL).await?;
            
            while let Some(msg) = pubsub.on_message().next().await {
                let txt = msg.get_payload::<Vec<u8>>()?;
                let _ = tx.send(txt);
            }

            RedisResult::Ok(())
        });

        Ok(Cache { client })
    }

    pub async fn create_structure_if_null(&mut self) -> RedisResult<()> {
        let len: u32 = self.client.strlen(BITMAP_KEY).await?;
        if len == 0 {
            self.client.setbit(BITMAP_KEY, BITS_LEN - 1, false).await?;
        }

        let online_users: Option<u32> = self.client.get(ONLINE_USERS_KEY).await?;
        if online_users.is_none() {
            self.client.set(ONLINE_USERS_KEY, 0).await?;
        }

        Ok(())
    }

    pub async fn add_online_user(&mut self) -> RedisResult<()> {
        let new_value: u32 = self.client.incr(ONLINE_USERS_KEY, 1).await?;
        self.client.publish(ONLINE_USERS_UDPATE_CHANNEL, OnlineUsersMessage(new_value).ws_msg()).await?;
        Ok(())
    }

    pub async fn sub_online_user(&mut self) -> RedisResult<()> {
        let new_value: u32 = self.client.decr(ONLINE_USERS_KEY, 1).await?;
        self.client.publish(ONLINE_USERS_UDPATE_CHANNEL, OnlineUsersMessage(new_value).ws_msg()).await?;
        Ok(())
    }

    pub async fn bitmap_modify(&mut self, i: isize, msg_type: BitmapMessage, raw_msg: &Vec<u8>) -> RedisResult<()> {
        let byte = (i * 4) / 8;
        let first_4_bits = (i * 4) % 8 == 0;

        let result: Vec<u8> = self.client.getrange(BITMAP_KEY, byte, byte + 1).await?;
        assert!(result.len() > 0, "redis bitmap does not exist");

        let bits_value = match first_4_bits {
            true => result[0] >> 4,
            false => result[0] & 0b00001111
        };

        let new_value = match msg_type {
            BitmapMessage::Add => {
                if bits_value < 15 {
                    bits_value + 1
                } else {
                    0
                }
            }
            BitmapMessage::Sub => {
                if bits_value > 0 {
                    bits_value - 1
                } else {
                    15
                }
            }
            BitmapMessage::Set(new_value) => {
                if new_value == bits_value {
                    return Ok(());
                }
                new_value
            }
        };

        let new_byte = match first_4_bits {
            true => (result[0] & 0b00001111) | (new_value << 4),
            false => (result[0] & 0b11110000) | new_value
        };

        self.client.setrange(BITMAP_KEY, byte, &vec![new_byte]).await?;
        self.client.publish(BITMAP_UPDATE_CHANNEL, raw_msg).await?;

        Ok(())
    }

    pub async fn get_bitmap(&mut self) -> RedisResult<Vec<u8>> {
        self.client.get(BITMAP_KEY).await
    }
}