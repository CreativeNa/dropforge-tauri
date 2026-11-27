//! Сервис «просмотра» на Rust: плейлист → вариант → HEAD по сегментам → spade.
//!
//!   dropforge-watcher-svc --channel <login> [--minutes 5] [--interval 60]
//!
//! Печатает события строками JSON; видео не скачивается.

use base64::Engine;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

const GQL: &str = "https://gql.twitch.tv/gql";
const USHER: &str = "https://usher.ttvnw.net/api/channel/hls";
const SPADE: &str = "https://spade.twitch.tv/track";
const CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";
const HASH_PLAYBACK: &str = "ed230aa1e33e07eebb8928504583da78a5173989fadfb1ac94be06a04f3cdbe9";
const HASH_CURRENT_DROP: &str = "4d06b702d25d652afb9ef835d2a550031f1cf762b193523a92166f40ea3d142b";


/// Строка события сразу уходит читателю: stdout в пайпе буферизуется блоками.
fn emit(value: &Value) {
    use std::io::Write;
    println!("{value}");
    let _ = std::io::stdout().flush();
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36")
        .build()
        .new_agent()
}

fn playback_token(agent: &ureq::Agent, login: &str, auth: &str) -> Result<(String, String), String> {
    let body = json!([{
        "operationName": "PlaybackAccessToken",
        "extensions": { "persistedQuery": { "version": 1, "sha256Hash": HASH_PLAYBACK } },
        "variables": {
            "isLive": true,
            "isVod": false,
            "login": login,
            "platform": "web",
            "playerType": "site",
            "vodID": ""
        }
    }]);
    // Токен обязателен: анонимный PlaybackAccessToken не создаёт сессию
    // просмотра, и Twitch не засчитывает минуты в дропы.
    let mut request = agent
        .post(GQL)
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "text/plain;charset=UTF-8")
        .header("Origin", "https://www.twitch.tv");
    if !auth.is_empty() {
        request = request.header("Authorization", &format!("OAuth {auth}"));
    }
    let response: Value = request
        .send(body.to_string())
        .map_err(|error| format!("gql: {error}"))?
        .body_mut()
        .read_json()
        .map_err(|error| format!("gql json: {error}"))?;

    let token = response
        .get(0)
        .and_then(|item| item.get("data"))
        .and_then(|data| data.get("streamPlaybackAccessToken"))
        .ok_or_else(|| "токен плейлиста не получен".to_string())?;
    let value = token.get("value").and_then(Value::as_str).unwrap_or("");
    let signature = token.get("signature").and_then(Value::as_str).unwrap_or("");
    if value.is_empty() || signature.is_empty() {
        return Err("пустой токен плейлиста".into());
    }
    Ok((value.to_string(), signature.to_string()))
}

struct StreamMeta {
    broadcast_id: String,
    game_id: String,
    game_name: String,
}

fn stream_meta(agent: &ureq::Agent, login: &str, token: &str) -> Option<StreamMeta> {
    let body = json!({
        "query": "query($login:String!){ user(login:$login){ stream { id game { id name } } } }",
        "variables": { "login": login }
    });
    let mut request = agent
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "application/json");
    if !token.is_empty() {
        request = request.header("Authorization", &format!("OAuth {token}"));
    }
    let response: Value = request
        .send(body.to_string())
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    let stream = response
        .get("data")?
        .get("user")?
        .get("stream")?;
    let broadcast_id = stream.get("id").map(text_value).unwrap_or_default();
    if broadcast_id.is_empty() || broadcast_id == "null" {
        return None;
    }
    let game = stream.get("game");
    Some(StreamMeta {
        broadcast_id,
        game_id: game.and_then(|game| game.get("id")).map(text_value).unwrap_or_default(),
        game_name: game.and_then(|game| game.get("name")).map(text_value).unwrap_or_default(),
    })
}

fn text_value(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn http_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|line| line.trim())
        .filter(|line| line.starts_with("http"))
        .map(|line| line.to_string())
        .collect()
}


/// Прогресс текущего дропа на канале (DropCurrentSessionContext).
fn current_drop(agent: &ureq::Agent, channel_id: &str, token: &str) -> Option<Value> {
    if channel_id.is_empty() {
        return None;
    }
    let body = json!([{
        "operationName": "DropCurrentSessionContext",
        "extensions": { "persistedQuery": { "version": 1, "sha256Hash": HASH_CURRENT_DROP } },
        "variables": { "channelID": channel_id, "channelLogin": "" }
    }]);
    let mut request = agent
        .post(GQL)
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "text/plain;charset=UTF-8")
        .header("Origin", "https://www.twitch.tv");
    if !token.is_empty() {
        request = request.header("Authorization", &format!("OAuth {token}"));
    }
    let response: Value = request
        .send(body.to_string())
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    response
        .get(0)
        .and_then(|item| item.get("data"))
        .and_then(|data| data.get("currentUser"))
        .and_then(|user| user.get("dropCurrentSession"))
        .cloned()
}

fn watch_tick(
    agent: &ureq::Agent,
    login: &str,
    user_id: &str,
    channel_id: &str,
    token: &str,
    points_only: bool,
) -> Result<Value, String> {
    let (playback, signature) = playback_token(agent, login, token)?;
    let url = format!(
        "{USHER}/{login}.m3u8?allow_source=true&allow_audio_only=true&sig={}&token={}",
        urlencode(&signature),
        urlencode(&playback)
    );

    let master = agent
        .get(&url)
        .header("Connection", "close")
        .call()
        .map_err(|error| format!("master: {error}"))?
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("master body: {error}"))?;

    let variants = http_lines(&master);
    let Some(variant_url) = variants.last() else {
        return Err("в мастер-плейлисте нет вариантов".into());
    };

    let chunklist = agent
        .get(variant_url)
        .header("Connection", "close")
        .call()
        .map_err(|error| format!("variant: {error}"))?
        .body_mut()
        .read_to_string()
        .map_err(|error| format!("variant body: {error}"))?;

    let chunks: Vec<String> = http_lines(&chunklist).into_iter().take(40).collect();
    let mut ok = 0usize;
    for chunk in &chunks {
        match agent
            .head(chunk)
            .header("Referer", &format!("https://www.twitch.tv/{login}"))
            .header("Origin", "https://www.twitch.tv")
            .call()
        {
            Ok(response) if response.status().as_u16() < 400 => ok += 1,
            _ => {}
        }
    }

    let Some(meta) = stream_meta(agent, login, token) else {
        return Err("нет данных текущего эфира".into());
    };
    let payload = json!([{
        "event": "minute-watched",
        "properties": {
            "channel_id": channel_id,
            "broadcast_id": meta.broadcast_id,
            "player": "site",
            "channel": login,
            "client_time": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "game": meta.game_name,
            "game_id": meta.game_id,
            "hidden": false,
            "is_live": true,
            "live": true,
            "logged_in": true,
            "minutes_logged": 1,
            "muted": false,
            "user_id": user_id
        }
    }]);
    let encoded = base64::engine::general_purpose::STANDARD.encode(payload.to_string());
    let spade_status = match agent
        .post(SPADE)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Origin", "https://www.twitch.tv")
        .header("Referer", &format!("https://www.twitch.tv/{login}"))
        .send(format!("data={}", urlencode(&encoded)))
    {
        Ok(response) => response.status().as_u16(),
        Err(ureq::Error::StatusCode(code)) => code,
        Err(_) => 0,
    };

    let progress = if points_only {
        None
    } else {
        current_drop(agent, channel_id, &token).map(|session| {
        json!({
            "dropID": session.get("dropID"),
            "currentMinutesWatched": session.get("currentMinutesWatched"),
            "game": session.get("game").and_then(|game| game.get("displayName")),
        })
    })
    };

    Ok(json!({
        "event": "tick",
        "channel": login,
        "variants": variants.len(),
        "chunks": chunks.len(),
        "chunksOk": ok,
        "spadeStatus": spade_status,
        "progress": progress
    }))
}

fn urlencode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exit_on_close = args.iter().any(|arg| arg == "--exit-on-stdin-close");
    let value_of = |name: &str| -> Option<String> {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|index| args.get(index + 1))
            .cloned()
    };

    let login = value_of("--channel").ok_or("нужен --channel <login>")?;
    let minutes: u64 = value_of("--minutes").and_then(|v| v.parse().ok()).unwrap_or(5);
    let interval: u64 = value_of("--interval").and_then(|v| v.parse().ok()).unwrap_or(60);
    let user_id = value_of("--user").unwrap_or_default();
    // токен приходит первой строкой stdin: аргументы командной строки видны другим процессам
    let mut token = value_of("--token").unwrap_or_default();
    if args.iter().any(|arg| arg == "--stdin-token") {
        use std::io::BufRead;
        let mut line = String::new();
        if std::io::stdin().lock().read_line(&mut line).is_ok() {
            token = line.trim().to_string();
        }
    }
    let channel_id = value_of("--channel-id").unwrap_or_default();
    let points_only = args.iter().any(|arg| arg == "--points-only");
    // сторож stdin запускаем только после чтения токена, иначе он съест первую строку
    if exit_on_close {
        watch_parent();
    }

    let agent = agent();
    let deadline = Instant::now() + Duration::from_secs(minutes * 60);
    let mut ticks = 0u32;
    let mut errors = 0u32;

    while Instant::now() < deadline {
        match watch_tick(&agent, &login, &user_id, &channel_id, &token, points_only) {
            Ok(event) => {
                ticks += 1;
                errors = 0;
                emit(&event);
            }
            Err(message) => {
                errors += 1;
                emit(&json!({ "event": "error", "message": message }));
                if errors >= 4 {
                    emit(&json!({ "event": "stop", "reason": "stream-errors", "ticks": ticks }));
                    break;
                }
            }
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let sleep_for = remaining.min(Duration::from_secs(interval));
        std::thread::sleep(sleep_for);
    }

    emit(&json!({ "event": "stop", "ticks": ticks }));
    Ok(())
}
