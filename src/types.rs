use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::broadcast;

#[derive(Debug, Deserialize)]
pub struct AlpacaQuote {
    #[serde(rename = "S")]
    pub symbol: String,

    // bid price
    #[serde(rename = "bp")]
    pub bid_price: f64,

    // ask price
    #[serde(rename = "ap")]
    pub ask_price: f64,

    #[serde(rename = "t")]
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct Response {
    pub ticker: String,
    pub price: f64,
    pub time: String,
}

#[derive(Clone)]
pub struct AppState {
    pub tx: broadcast::Sender<String>,
    pub db: SqlitePool
}

#[derive(Debug, Deserialize)]
pub struct KrakenTickerUpdate {
    pub channel: String,
    pub r#type: String,
    pub data: Vec<KrakenTickerData>,
}

#[derive(Debug, Deserialize)]
pub struct KrakenTickerData {
    pub symbol: String,
    pub last: f64,
}

#[derive(serde::Deserialize)]
pub struct HistoryParams {
    pub ticker: String,
    pub limit: Option<i64>
}