use std::env;

use redis::AsyncCommands;

const BITMAP_KEY: &str = "state";
const OLD_BITMAP_LEN: usize = 1_000_000;
const NEW_BITMAP_LEN: usize = 4_000_000;

#[tokio::main]
async fn main() {
    if cfg!(debug_assertions) {
        dotenvy::from_filename(".env.development").unwrap();
    } else {
        dotenvy::from_filename(".env.production").unwrap();
    }

    let client = redis::Client::open(env::var("REDIS_URL").unwrap()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let old_bitmap: Vec<u8> = conn.get(BITMAP_KEY).await.unwrap();
    if old_bitmap.len() != OLD_BITMAP_LEN / 8 {
        panic!("bitmap length is not 1_000_000");
    }

    let mut new_bitmap = vec![0u8; NEW_BITMAP_LEN / 8];

    for i in 0..OLD_BITMAP_LEN {
        // 1 bit -> 4 bits (1 -> 0001 | 0 -> 0000)
        let byte = i / 8;
        let bit = i % 8;
        let old_bit = (old_bitmap[byte] >> bit) & 1;

        let new_byte = i * 4 / 8;
        let new_bit = i * 4 % 8;

        new_bitmap[new_byte] |= old_bit << new_bit;      
    }

    let _: () = conn.setrange(BITMAP_KEY, 0, &new_bitmap).await.unwrap();
}