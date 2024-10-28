use axum::{extract::State, response::IntoResponse};
use super::AppState;
 

pub async fn handle_bits(
    State(mut app_state): State<AppState>,
) -> impl IntoResponse {
    app_state.cache.get_bitmap().await.unwrap_or_default()
}