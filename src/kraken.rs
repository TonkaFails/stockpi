use crate::types::{KrakenTickerUpdate, Response};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::json;
use tokio::sync::broadcast;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TungsteniteMessage};

const KRAKEN_URL: &str = "wss://ws.kraken.com/v2";
const KRAKEN_SYMBOLS: &[&str] = &["XMR/USD"];

pub async fn ingest_kraken_stream(tx: broadcast::Sender<String>) {
    loop {
        tracing::info!("connecting to Kraken");

        match connect_async(KRAKEN_URL).await {
            Ok((ws_stream, _)) => {
                let (mut write, mut read) = ws_stream.split();
                
                let sub_msg = json!({
                    "method": "subscribe",
                    "params": {
                        "channel": "ticker",
                        "symbol": KRAKEN_SYMBOLS
                    }
                });

                if let Err(e) = write.send(TungsteniteMessage::Text(sub_msg.to_string().into())).await {
                    tracing::error!("kraken subscription failed: {}", e);
                    sleep(Duration::from_secs(5)).await;
                    continue;
                }

                tracing::info!("subscribed: {:?}", KRAKEN_SYMBOLS);

                while let Some(message) = read.next().await {
                    match message {
                        Ok(TungsteniteMessage::Text(text)) => {
                            if let Ok(update) = serde_json::from_str::<KrakenTickerUpdate>(&text) {
                                for d in update.data {
                                    let clean_data = Response {
                                        ticker: d.symbol,
                                        price: d.last,
                                        // add time manually
                                        time: chrono::Utc::now().to_rfc3339(),
                                    };

                                    if let Ok(json_output) = serde_json::to_string(&clean_data) {
                                        let _ = tx.send(json_output);
                                    }
                                }
                            }
                        }
                        Ok(TungsteniteMessage::Ping(p)) => {
                            let _ = write.send(TungsteniteMessage::Pong(p)).await;
                        }
                        Ok(TungsteniteMessage::Close(_)) => break,
                        Err(_) => break,
                        _ => {}
                    }
                }
            }
            Err(e) => tracing::error!("kraken conn failed: {}", e),
        }

        tracing::warn!("Kraken conn lost. Retrying in 5s");
        sleep(Duration::from_secs(5)).await;
    }
}