use std::{env, fs::File, io::Read};

use redis::{aio::MultiplexedConnection, AsyncCommands};
use tokio::fs;

const BITMAP_KEY: &str = "state";
const BITMAP_LEN: usize = 4_000_000;

async fn get_bitmap(conn: &mut MultiplexedConnection) -> Vec<u8> {
    let bitmap: Vec<u8> = conn.get(BITMAP_KEY).await.unwrap();
    if bitmap.len() != BITMAP_LEN / 8 {
        panic!("bitmap length is not 4_000_000");
    }

    bitmap
}

async fn get_bitmap2() -> Vec<u8> {
    let mut bitmap = vec![0u8; BITMAP_LEN / 8];
    let mut file = File::open("./old").unwrap();

    let i = file.read_to_end(&mut bitmap).unwrap();
    if i != BITMAP_LEN / 8 {
        panic!("bitmap length is not 4_000_000");
    }

    bitmap
}

#[tokio::main]
async fn main() {
    if cfg!(debug_assertions) {
        dotenvy::from_filename(".env.development").unwrap();
    } else {
        dotenvy::from_filename(".env.production").unwrap();
    }

    let client = redis::Client::open(env::var("REDIS_URL").unwrap()).unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();

    let bitmap = get_bitmap(&mut conn).await;
    let mut new_bitmap = vec![0u8; bitmap.len()];
    update_bitmap(&bitmap, &mut new_bitmap);

    let _: () = conn.setrange(BITMAP_KEY, 0, &bitmap).await.unwrap();
}


fn update_bitmap(bitmap: &Vec<u8>, new_bitmap: &mut Vec<u8>) {
    let old_bytes_per_row = 40;
    let new_bytes_per_row = 50;
    let mut current = 0;

    let mut temp_bitmap= vec![0u8; (bitmap.len() / (old_bytes_per_row / 2)) * (new_bytes_per_row / 2)];
    for i in (0..bitmap.len()).step_by(old_bytes_per_row) {
        for j in 0..new_bytes_per_row {
            if j < old_bytes_per_row {
                temp_bitmap[current] = bitmap[i + j];
            } else {
                temp_bitmap[current] = 0;
            }
            current += 1;
        }
    }

    let need_to_remove = temp_bitmap.len() - bitmap.len();
    new_bitmap[..500_000 / 2].copy_from_slice(&temp_bitmap[..500_000 / 2]);
    new_bitmap[500_000 / 2..].copy_from_slice(&temp_bitmap[(500_000 / 2) + need_to_remove..]);
}