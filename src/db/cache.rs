use futures::StreamExt;
use redis::{aio::MultiplexedConnection, AsyncCommands, RedisResult};
use tokio::sync::broadcast;

use crate::routes::ws::MessageType;


#[derive(Clone)]
pub struct Cache {
    client: MultiplexedConnection
}

pub const BITS_PER_CHECKBOX: usize = 4;
pub const BITS_LEN: usize = 1_000_000 * BITS_PER_CHECKBOX;

const BITMAP_KEY: &str = "state";
const BITMAP_UPDATE_CHANNEL: &str = "state_update";

impl Cache {
    pub async fn new(tx: broadcast::Sender<String>) -> RedisResult<Self> {
        let redis_url = env_var!("REDIS_URL");
        let conn = redis::Client::open(redis_url)?;
        let client = conn.get_multiplexed_tokio_connection().await?;

        let mut pubsub = conn.get_async_pubsub().await?;
    
        tokio::spawn(async move {
            pubsub.subscribe(BITMAP_UPDATE_CHANNEL).await?;
            
            while let Some(msg) = pubsub.on_message().next().await {
                let txt = msg.get_payload::<String>()?;
                let _ = tx.send(txt);
            }

            RedisResult::Ok(())
        });

        Ok(Cache { client })
    }

    pub async fn create_bitmap_if_null(&mut self) -> RedisResult<()> {
        let len: u32 = self.client.strlen(BITMAP_KEY).await?;
        if len == 0 {
            self.client.setbit(BITMAP_KEY, BITS_LEN -1, false).await?;
        }

        Ok(())
    }

    pub async fn bitmap_modify(&mut self, i: isize, msg_type: MessageType, raw_msg: &String) -> RedisResult<()> {
        let byte = (i * 4) / 8;
        let first_4_bits = (i * 4) % 8 == 0;

        let result: Vec<u8> = self.client.getrange(BITMAP_KEY, byte, byte + 1).await?;
        assert!(result.len() > 0, "redis bitmap does not exist");

        let bits_value = match first_4_bits {
            true => result[0] >> 4,
            false => result[0] & 0b00001111
        };

        let new_value = match msg_type {
            MessageType::Add => {
                if bits_value < 15 {
                    bits_value + 1
                } else {
                    0
                }
            }
            MessageType::Sub => {
                if bits_value > 0 {
                    bits_value - 1
                } else {
                    15
                }
            }
            MessageType::Set(new_value) => {
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

// const TEST_AMMOUNT: isize = 5_000;
// async fn test_getrange(mut conn: MultiplexedConnection) {
//     let now = Instant::now();

//     for i in 0..TEST_AMMOUNT / 2 {
//         {
//             let result: String = conn.getrange(BITMAP_KEYS[0], i, i + 1).await.unwrap();
//             let result_u8 = result.chars().nth(0).unwrap() as u8;
            
//             // first 4 bits
//             let mut bits = [0; 4];
//             for j in 0..4 {
//                 bits[3 - j] = (result_u8 >> (j + 4)) & 1;
//             }
//         }
//     }

//     for i in 0..TEST_AMMOUNT / 2 {
//         {
//             let result: String = conn.getrange(BITMAP_KEYS[0], i, i + 1).await.unwrap();
//             let result_u8 = result.chars().nth(0).unwrap() as u8;
            
//             // last 4 bits
//             let mut bits = [0; 4];
//             for j in 0..4 {
//                 bits[3 - j] = (result_u8 >> j) & 1;
//             }
//         }
//     }

//     println!("GET RANGE {:?}", now.elapsed())
// }

// async fn test_getbit(mut conn: MultiplexedConnection) {
//     let now = Instant::now();

//     for i in 0..TEST_AMMOUNT as usize {
//         let mut result = [false; BITS_PER_CHECKBOX];
//         for level in 0..BITS_PER_CHECKBOX {
//             result[level] = conn.getbit(BITMAP_KEYS[level], i).await.unwrap();
//         }
//     }

//     println!("GET BIT {:?}", now.elapsed())
// }