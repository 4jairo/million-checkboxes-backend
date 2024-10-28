use futures::StreamExt;
use redis::{aio::MultiplexedConnection, AsyncCommands, RedisResult};
use tokio::sync::broadcast;



#[derive(Clone)]
pub struct Cache {
    client: MultiplexedConnection
}

pub const BITMAP_QTY: usize = 1_000_000;

// const REDIS_URL: &str = "redis://redis-cache:6379"; // TODO: move to .env
const REDIS_URL: &str = "redis://localhost:6379"; // TODO: move to .env
const BITMAP_KEY: &str = "state"; 
const BITMAP_UPDATE_CHANNEL: &str = "state_update";

impl Cache {
    pub async fn new(tx: broadcast::Sender<String>) -> RedisResult<Self> {
        let conn = redis::Client::open(REDIS_URL)?;
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
            self.client.setbit(BITMAP_KEY, BITMAP_QTY -1, false).await?;
        }

        Ok(())
    }

    pub async fn set(&mut self, i: usize, value: bool, raw_msg: &String) -> RedisResult<()> {
        self.client.setbit(BITMAP_KEY, i, value).await?;
        self.client.publish(BITMAP_UPDATE_CHANNEL, raw_msg).await?;

        Ok(())
    }

    pub async fn get_bitmap(&mut self) -> RedisResult<Vec<u8>> {
        self.client.get(BITMAP_KEY).await
    }
}