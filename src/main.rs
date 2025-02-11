use axum::Router;
use routes::AppState;
use tokio::net::TcpListener;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;

#[macro_use]
mod dotenv;
mod routes;
mod db;


#[tokio::main]
async fn main() {
    if cfg!(debug_assertions) {
        dotenvy::from_filename(".env.development").unwrap();
    } else {
        dotenvy::from_filename(".env.production").unwrap();
    }

    let compression_layer = CompressionLayer::new()
        .br(true)
        .deflate(true)
        .gzip(true)
        .zstd(true);

    let cors = CorsLayer::very_permissive();
    let mut state = AppState::new().await;
    println!("redis cache connected");

    state.cache.create_structure_if_null().await.unwrap();

    let routes = Router::new()
        .merge(routes::routes())
        .with_state(state)
        .layer(compression_layer)
        .layer(cors)
        .into_make_service();

    let listener = TcpListener::bind("0.0.0.0:8900").await.unwrap();

    println!("ready to listen in 0.0.0.0:8900");

    axum::serve(listener, routes).await.unwrap()
}