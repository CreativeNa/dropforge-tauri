//! PubSub-клиент Twitch на Rust: живой прогресс дропов вместо опроса.
//!
//!   dropforge-pubsub-svc --token <oauth> --user <id> [--seconds 20]
//!
//! Подключается к `wss://pubsub-edge.twitch.tv/v1`, слушает
//! `user-drop-events.<id>`, отвечает на PING и печатает события строками JSON.

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::time::Duration;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const PUBSUB_URL: &str = "wss://pubsub-edge.twitch.tv/v1";
const PING_EVERY: Duration = Duration::from_secs(60);

/// Строка события сразу уходит читателю: stdout в пайпе буферизуется блоками.
fn emit(value: &Value) {
    use std::io::Write;
    println!("{value}");
    let _ = std::io::stdout().flush();
}

/// Выходим, когда родитель закрыл stdin: так сервис не живёт после приложения.
fn watch_parent() {
    std::thread::spawn(|| {
        use std::io::Read;
        let mut buffer = [0u8; 1];
        let mut stdin = std::io::stdin();
        loop {
            match stdin.read(&mut buffer) {
                Ok(0) | Err(_) => std::process::exit(0),
                Ok(_) => {}
            }
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // rustls без явного провайдера паникует: выбираем ring
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install rustls provider");

    let args: Vec<String> = std::env::args().skip(1).collect();
    let exit_on_close = args.iter().any(|arg| arg == "--exit-on-stdin-close");
    let value_of = |name: &str| -> Option<String> {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };

    // токен приходит первой строкой stdin: аргументы командной строки видны другим процессам
    let mut token = value_of("--token").unwrap_or_default();
    if args.iter().any(|arg| arg == "--stdin-token") {
        use std::io::BufRead;
        let mut line = String::new();
        if std::io::stdin().lock().read_line(&mut line).is_ok() {
            token = line.trim().to_string();
        }
    }
    if exit_on_close {
        watch_parent();
    }
    let user = value_of("--user").unwrap_or_default();
    // id каналов, за которыми следим: --stream <channel_id> (можно несколько)
    let mut stream_ids: Vec<String> = Vec::new();
    for (index, arg) in args.iter().enumerate() {
        if arg == "--stream" {
            if let Some(value) = args.get(index + 1) {
                if !value.is_empty() && !stream_ids.contains(value) {
                    stream_ids.push(value.clone());
                }
            }
        }
    }
    let seconds: u64 = value_of("--seconds")
        .and_then(|value| value.parse().ok())
        .unwrap_or(20);

    let (socket, _) = connect_async(PUBSUB_URL).await?;
    let (mut sink, mut stream) = socket.split();

    let mut topics: Vec<String> = Vec::new();
    if !user.is_empty() {
        topics.push(format!("user-drop-events.{user}"));
    }
    for id in &stream_ids {
        topics.push(format!("video-playback-by-id.{id}"));
    }

    let nonce = format!("dropforge-{}", std::process::id());
    if !topics.is_empty() {
        let listen = json!({
            "type": "LISTEN",
            "nonce": nonce,
            "data": {
                "topics": topics,
                "auth_token": token,
            }
        });
        sink.send(Message::Text(listen.to_string().into())).await?;
        emit(&json!({ "event": "listen-sent", "topics": topics }));
    } else {
        emit(&json!({ "event": "connected", "topics": [] }));
    }

    let deadline = tokio::time::sleep(Duration::from_secs(seconds));
    tokio::pin!(deadline);
    let mut ping = tokio::time::interval(PING_EVERY);

    loop {
        tokio::select! {
            _ = &mut deadline => {
                emit(&json!({ "event": "stop", "reason": "timeout" }));
                break;
            }
            _ = ping.tick() => {
                sink.send(Message::Text(json!({ "type": "PING" }).to_string().into())).await?;
                emit(&json!({ "event": "ping-sent" }));
            }
            message = stream.next() => {
                let Some(message) = message else { break };
                match message? {
                    Message::Text(text) => {
                        let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                        let kind = parsed.get("type").and_then(Value::as_str).unwrap_or("unknown");
                        let topic = parsed
                            .get("data")
                            .and_then(|data| data.get("topic"))
                            .and_then(Value::as_str)
                            .unwrap_or("");
                        match kind {
                            "PING" => {
                                sink.send(Message::Text(json!({ "type": "PONG" }).to_string().into())).await?;
                                emit(&json!({ "event": "pong-sent" }));
                            }
                            "MESSAGE" => {
                                let payload = parsed
                                    .get("data")
                                    .and_then(|data| data.get("message"))
                                    .and_then(Value::as_str)
                                    .unwrap_or("");
                                // video-playback-by-id присылает JSON: stream-up / stream-down / viewcount
                                if topic.starts_with("video-playback-by-id.") {
                                    let kind = serde_json::from_str::<Value>(payload)
                                        .ok()
                                        .and_then(|value| value.get("type").cloned())
                                        .and_then(|value| value.as_str().map(str::to_string))
                                        .unwrap_or_default();
                                    let channel = topic.trim_start_matches("video-playback-by-id.").to_string();
                                    emit(&json!({
                                        "event": "stream",
                                        "channel": channel,
                                        "kind": kind,
                                        "payload": payload
                                    }));
                                } else {
                                    emit(&json!({ "event": "message", "topic": topic, "payload": payload }));
                                }
                            }
                            _ => emit(&json!({ "event": kind, "topic": topic, "error": parsed.get("error") })),
                        }
                    }
                    Message::Ping(payload) => {
                        sink.send(Message::Pong(payload)).await?;
                    }
                    Message::Close(frame) => {
                        emit(&json!({ "event": "closed", "frame": format!("{frame:?}") }));
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}
