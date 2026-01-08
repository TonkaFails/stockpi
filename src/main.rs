// Declare the other modules so we can use them
mod alpaca;
mod types;
mod kraken;

use axum::{extract::ws::{Message, WebSocket, WebSocketUpgrade}, extract::State, response::IntoResponse, routing::get, Json, Router};
use std::net::SocketAddr;
use axum::extract::Query;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::sync::broadcast;
use crate::types::{AppState, HistoryParams, Response};
use crate::alpaca::ingest_alpaca_stream;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().ok();

    let api_key = std::env::var("KEY").expect("APCA_API_KEY_ID must be set");
    let api_secret = std::env::var("SECRET").expect("APCA_API_SECRET_KEY must be set");

    // init db
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("failed to connect DB");

    sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await.expect("what?");
    // create table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS quotes (
            id INTEGER PRIMARY KEY,
            ticker TEXT NOT NULL,
            price REAL NOT NULL,
            time TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ticker_time ON quotes(ticker, time);"
    )
        .execute(&pool)
        .await
        .unwrap();

    let (tx, _rx) = broadcast::channel(100);

    let app_state = AppState {
        tx: tx.clone(),
        db: pool.clone()
    };

    let mut db_rx = tx.subscribe();
    let db_pool = pool.clone();

    // spawn db writer
    tokio::spawn(async move {
        tracing::info!("db writer");
        while let Ok(msg_json) = db_rx.recv().await {
            if let Ok(data) = serde_json::from_str::<Response>(&msg_json) {
                let _ = sqlx::query("INSERT INTO quotes (ticker, price, time) VALUES (?, ?, ?)")
                    .bind(&data.ticker)
                    .bind(data.price)
                    .bind(&data.time)
                    .execute(&db_pool)
                    .await
                    .map_err(|e| tracing::error!("write error: {}", e));
            } else {
                tracing::warn!("message isn't a valid response: {}", msg_json);
            }
        }
    });

    // spawn background ingestor tasks
    let tx_for_ingestor = tx.clone();
    tokio::spawn(async move {
        ingest_alpaca_stream(api_key, api_secret, tx_for_ingestor).await;
    });

    let tx_kraken = tx.clone();
    tokio::spawn(async move {
        kraken::ingest_kraken_stream(tx_kraken).await;
    });

    // build web server
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/history", get(get_history_handler))
        .with_state(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("running on ws://{}", addr);

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

async fn handle_frontend_socket(mut socket: WebSocket, state: AppState) {
    let mut rx = state.tx.subscribe();

    while let Ok(msg) = rx.recv().await {
        if socket.send(Message::Text(msg.into())).await.is_err() {
            // client disconnected
            break;
        }
    }
}

async fn get_history_handler(
    State(state): State<AppState>,
    Query(params): Query<HistoryParams>,
) -> Json<Vec<Response>> {
    let limit = params.limit.unwrap_or(100);

    let rows = sqlx::query_as::<_, Response>(
        "SELECT ticker, price, time FROM quotes WHERE ticker = ? ORDER BY id DESC LIMIT ?"
    )
        .bind(params.ticker)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .unwrap_or_else(|_| vec![]);

    Json(rows)
}