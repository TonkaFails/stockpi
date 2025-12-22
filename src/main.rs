// Declare the other modules so we can use them
mod alpaca;
mod types;
mod kraken;

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
    routing::get,
    Router,
};
use std::net::SocketAddr;
use tokio::sync::broadcast;
use crate::types::AppState;
use crate::alpaca::ingest_alpaca_stream;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let api_key = std::env::var("KEY").expect("APCA_API_KEY_ID must be set");
    let api_secret = std::env::var("SECRET").expect("APCA_API_SECRET_KEY must be set");

    let (tx, _rx) = broadcast::channel(100);

    let app_state = AppState {
        tx: tx.clone()
    };

    // spawn the background ingestor tasks
    let tx_for_ingestor = tx.clone();
    tokio::spawn(async move {
        ingest_alpaca_stream(api_key, api_secret, tx_for_ingestor).await;
    });

    let tx_kraken = tx.clone();
    tokio::spawn(async move {
        kraken::ingest_kraken_stream(tx_kraken).await;
    });

    // Build the web server
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("🚀 Backend running on ws://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

// Handle the WebSocket upgrade request
async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_frontend_socket(socket, state))
}

// Manage the individual client connection
async fn handle_frontend_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();

    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg.into())).await.is_err() {
            // Client disconnected, break the loop to free resources
            break;
        }
    }
}