use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
pub struct Response {
    pub ticker: String,
    pub price: f64,
    pub time: String,
}

#[derive(Clone)]
pub struct AppState {
    pub tx: broadcast::Sender<String>,
}