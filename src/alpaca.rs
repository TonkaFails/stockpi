use crate::types::{AlpacaQuote, Response};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use tokio::sync::broadcast;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TungsteniteMessage};
use url::Url;

const ALPACA_URL: &str = "wss://stream.data.alpaca.markets/v1beta3/crypto/us";

const SYMBOLS: &[&str] = &["BTC/USD", "ETH/USD", "LTC/USD"];

pub async fn ingest_alpaca_stream(key: String, secret: String, tx: broadcast::Sender<String>) {
    let url_str = ALPACA_URL;

    loop {
        tracing::info!("starting connection");
        let url = Url::parse(url_str).expect("invalid alpaca URL");

        match connect_async(url.to_string()).await {
            Ok((ws_stream, _)) => {
                tracing::info!("conn to {}", ALPACA_URL);
                let (mut write, mut read) = ws_stream.split();

                let auth_msg = json!({
                    "action": "auth",
                    "key": key,
                    "secret": secret
                });

                if let Err(e) = write
                    .send(TungsteniteMessage::Text(auth_msg.to_string().into()))
                    .await
                {
                    tracing::error!("failed auth: {}", e);
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }

                let sub_msg = json!({
                    "action": "subscribe",
                    "quotes": SYMBOLS
                });

                if let Err(e) = write
                    .send(TungsteniteMessage::Text(sub_msg.to_string().into()))
                    .await
                {
                    tracing::error!("failed subscription: {}", e);
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }

                tracing::info!("subscribed: {:?}", SYMBOLS);

                while let Some(message) = read.next().await {
                    match message {
                        Ok(TungsteniteMessage::Text(text)) => {

                            if let Ok(quotes) = serde_json::from_str::<Vec<AlpacaQuote>>(&text) {
                                for q in quotes {
                                    let price = if q.bid_price > 0.0 && q.ask_price > 0.0 {
                                        (q.bid_price + q.ask_price) / 2.0
                                    } else if q.bid_price > 0.0 {
                                        q.bid_price
                                    } else {
                                        q.ask_price
                                    };

                                    let clean_data = Response {
                                        ticker: q.symbol,
                                        price,
                                        time: q.timestamp,
                                    };

                                    if let Ok(json_output) = serde_json::to_string(&clean_data) {
                                        let _ = tx.send(json_output);
                                    }
                                }
                            }
                        }
                        Ok(TungsteniteMessage::Ping(ping)) => {
                            let _ = write.send(TungsteniteMessage::Pong(ping)).await;
                        }
                        Ok(TungsteniteMessage::Close(_)) => {
                            tracing::warn!("server closed connection.");
                            break;
                        }
                        Err(e) => {
                            tracing::error!("socket error: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Err(e) => {
                tracing::error!("init connection failed: {}", e);
            }
        }

        tracing::warn!("conn lost. retrying in 5s");
        sleep(Duration::from_secs(5)).await;
    }
}