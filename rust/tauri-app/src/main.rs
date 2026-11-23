#![cfg_attr(windows, windows_subsystem = "windows")]

//! Шелл на Tauri: окно с текущим UI DropForge и данными из портативного профиля.
//!
//! Команды повторяют контракт preload-моста Electron. Пока сеть и фарм живут
//! в отдельных сервисах, Tauri читает профиль (`data/profile`) и отдаёт снимок;
//! кнопки, которым нужен движок, честно отвечают «пока не подключено».

use serde_json::{json, Map, Value};
use std::path::PathBuf;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Emitter, Manager, WindowEvent};

/// Пути и токены из окружения — только для отладочной сборки.
/// В релизе переменные игнорируются: иначе их можно использовать для подмены исполняемых файлов.
fn debug_env(name: &str) -> Option<String> {
    if cfg!(debug_assertions) {
        std::env::var(name).ok()
    } else {
        None
    }
}

fn portable_root() -> PathBuf {
    // DROPFORGE_DATA позволяет dev-сборке работать с профилем установленного приложения
    if let Some(dir) = debug_env("DROPFORGE_DATA") {
        let candidate = PathBuf::from(dir);
        if std::fs::create_dir_all(candidate.join("profile")).is_ok() {
            return candidate;
        }
    }
    // рядом с exe: <dir>/data/profile; иначе — %APPDATA%/DropForge/data/profile
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join("data");
            if candidate.is_dir() {
                return candidate;
            }
            if std::fs::create_dir_all(candidate.join("profile")).is_ok() {
                return candidate;
            }
        }
    }
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("DropForge").join("data")
}

fn profile_dir() -> PathBuf {
    portable_root().join("profile")
}

fn read_json(path: PathBuf) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    // файлы могли быть записаны с BOM — срезаем его перед разбором
    let text = text.trim_start_matches('\u{feff}');
    serde_json::from_str(text).ok()
}

fn text_of(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or("").to_string()
}

fn campaign_view(raw: &Value) -> Value {
    let game = raw.get("game").cloned().unwrap_or(Value::Null);
    // локальные кампании приходят уже в виде готовых дропов
    let ready_drops = raw.get("drops").and_then(Value::as_array).cloned();
    let drops: Vec<Value> = if let Some(existing) = ready_drops {
        existing
    } else {
        raw
        .get("timeBasedDrops")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|drop| {
            let required = drop
                .get("requiredMinutesWatched")
                .and_then(Value::as_f64)
                .or_else(|| drop.get("requiredMinutes").and_then(Value::as_f64))
                .unwrap_or(0.0);
            if let Some(existing) = drop.get("benefits").and_then(Value::as_array) {
                return json!({
                    "id": drop.get("id"),
                    "name": drop.get("name"),
                    "startsAt": drop.get("startsAt").or_else(|| drop.get("startAt")),
                    "endsAt": drop.get("endsAt").or_else(|| drop.get("endAt")),
                    "requiredMinutes": required,
                    "requiredSubs": drop.get("requiredSubs").cloned().unwrap_or(json!(0)),
                    "currentMinutes": 0,
                    "remainingMinutes": required,
                    "progress": 0,
                    "isClaimed": false,
                    "canClaim": false,
                    "note": drop.get("note").cloned().unwrap_or(json!("")),
                    "benefits": existing,
                });
            }
            let benefits: Vec<Value> = drop
                .get("benefitEdges")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|edge| edge.get("benefit"))
                .map(|benefit| {
                    let mut name = text_of(benefit.get("name"));
                    let mut image = benefit.get("imageAssetURL").cloned().unwrap_or(Value::Null);
                    let mut badge = benefit.get("badge").and_then(Value::as_bool).unwrap_or(false);
                    // у GTAVI локальная версия тоже отдаёт «сундуки»: подменяем на реальные награды
                    if text_of(raw.get("name")).to_lowercase().contains("gtavi") {
                        let subs = drop.get("requiredSubs").and_then(Value::as_f64).unwrap_or(0.0) as i64;
                        let is_badge_chest = name.to_lowercase().contains("badge") || badge;
                        let replacement: Option<(&str, &str, bool)> = match (subs, is_badge_chest) {
                            (1, true) => Some(("Welcome 2 Miami", "https://static-cdn.jtvnw.net/badges/v1/1c51d343-754e-4fee-a717-ec95cafb5482/3", true)),
                            (2, true) => Some(("Scored Big", "https://static-cdn.jtvnw.net/badges/v1/2acfc5b8-7907-4b05-9b30-e211a93ffc63/3", true)),
                            (3, true) => Some(("Bleed Miami", "https://static-cdn.jtvnw.net/badges/v1/c7b8fc7d-7ebd-46e1-8f57-cc58ce51a9d1/3", true)),
                            (1, false) => Some(("BustedPlz", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_fba1756b9e5f443c9725542b78f93b11/default/dark/3.0", false)),
                            (2, false) => Some(("DinoBusted", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_a021fa5191714969829fee79badc0756/default/dark/3.0", false)),
                            (3, false) => Some(("SabaBusted", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_bcdb69418a044bf982e1fed15c34ce63/default/dark/3.0", false)),
                            _ => None,
                        };
                        if let Some((new_name, url, is_badge)) = replacement {
                            name = new_name.to_string();
                            image = json!(url);
                            badge = is_badge;
                        } else if subs <= 0 && (name.to_lowercase().contains("chest") || name.to_lowercase().contains("dvd")) {
                            name = String::from("Ежедневный сундук с цепочкой");
                            image = Value::Null;
                        }
                    }
                    json!({
                        "id": benefit.get("id"),
                        "name": name,
                        "type": benefit.get("distributionType"),
                        "imageUrl": image,
                        "badge": badge,
                    })
                })
                .collect();
            let first_benefit_name = benefits
                .first()
                .and_then(|benefit| benefit.get("name"))
                .map(|value| text_of(Some(value)))
                .unwrap_or_default();
            let mut drop_name = text_of(drop.get("name"));
            let mut drop_note = text_of(drop.get("note"));
            if text_of(raw.get("name")).to_lowercase().contains("gtavi") {
                let mut watch_chest = false;
                if drop_name.to_lowercase().contains("dvd") {
                    drop_name = String::from("Ежедневный сундук с цепочкой");
                    drop_note = String::from("Проверяйте прогресс на Twitch");
                    watch_chest = true;
                }
                if !first_benefit_name.is_empty() && !watch_chest {
                    drop_name = first_benefit_name;
                }
            }
            json!({
                "id": drop.get("id"),
                "name": drop_name,
                "startsAt": drop.get("startAt"),
                "endsAt": drop.get("endAt"),
                "requiredMinutes": required,
                "requiredSubs": drop.get("requiredSubs").cloned().unwrap_or(json!(0)),
                "note": drop_note,
                "currentMinutes": 0,
                "remainingMinutes": required,
                "progress": 0,
                "isClaimed": false,
                "canClaim": false,
                "benefits": benefits,
            })
        })
        .collect()
    };

    json!({
        "id": raw.get("id"),
        "name": raw.get("name"),
        "status": raw.get("status").cloned().unwrap_or(json!("ACTIVE")),
        "game": {
            "slug": text_of(game.get("slug")),
            "name": game.get("name").cloned().unwrap_or(json!("Неизвестная игра")),
            "boxArtUrl": box_art_value(game.get("boxArtURL")),
        },
        "startsAt": raw.get("startAt"),
        "endsAt": raw.get("endAt"),
        "linkUrl": raw.get("accountLinkURL"),
        "reward": raw.get("rewardCampaign").cloned().unwrap_or(json!(false)),
        "webUrl": raw.get("webUrl"),
        "watchRewardIds": raw.get("watchRewardIds").cloned().unwrap_or(json!([])),
        "subRewardIds": raw.get("subRewardIds").cloned().unwrap_or(json!([])),
        "active": true,
        "upcoming": false,
        "expired": false,
        "canEarn": true,
        "selected": false,
        "farming": false,
        "local": raw.get("local").cloned().unwrap_or(json!(false)),
        "channel": raw.get("channel").cloned().unwrap_or(Value::Null),
        "drops": drops,
    })
}

const CATALOG_URL: &str = "https://raw.githubusercontent.com/SunkwiBOT/twitch-drops-api/main/drops.json";
const REWARDS_URL: &str = "https://raw.githubusercontent.com/SunkwiBOT/twitch-drops-api/main/rewards.json";
const REWARDS_PAGE: &str = "https://www.twitch.tv/drops/rewards";

const UPDATE_REPO: &str = "CreativeNa/dropforge-tauri";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Убираем из текста домашний каталог и имя пользователя.
fn mask_text(text: &str) -> String {
    let mut result = text.to_string();
    if let Some(home) = std::env::var_os("USERPROFILE").and_then(|value| value.to_str().map(str::to_string)) {
        result = result.replace(&home, "%USERPROFILE%");
        if let Some(name) = std::path::Path::new(&home).file_name().and_then(|value| value.to_str()) {
            result = result.replace(name, "%USER%");
        }
    }
    result
}

#[cfg(windows)]
fn reg_value(path: &str, name: &str) -> Option<String> {
    use windows_sys::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    let path: Vec<u16> = format!("{path}\0").encode_utf16().collect();
    let name: Vec<u16> = format!("{name}\0").encode_utf16().collect();
    let mut size: u32 = 0;
    let probe = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, path.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), std::ptr::null_mut(), &mut size)
    };
    if probe != 0 || size == 0 {
        return None;
    }
    let mut buffer = vec![0u16; (size as usize / 2) + 1];
    let read = unsafe {
        RegGetValueW(HKEY_LOCAL_MACHINE, path.as_ptr(), name.as_ptr(), RRF_RT_REG_SZ, std::ptr::null_mut(), buffer.as_mut_ptr() as *mut _, &mut size)
    };
    if read != 0 {
        return None;
    }
    let length = buffer.iter().position(|value| *value == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..length]))
}

#[cfg(not(windows))]
fn reg_value(_path: &str, _name: &str) -> Option<String> {
    None
}

fn webview2_version() -> String {
    reg_value("SOFTWARE\\WOW6432Node\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}", "pv")
        .or_else(|| reg_value("SOFTWARE\\Microsoft\\EdgeUpdate\\Clients\\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}", "pv"))
        .unwrap_or_else(|| "не найдена".to_string())
}

fn reachable(url: &str) -> String {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(8)))
        .user_agent("DropForge-Diagnostics/1.0")
        .build()
        .new_agent();
    match agent.head(url).call() {
        Ok(response) => format!("доступен (HTTP {})", response.status().as_u16()),
        Err(ureq::Error::StatusCode(code)) => format!("доступен (HTTP {code})"),
        Err(error) => format!("ошибка: {error}"),
    }
}

#[tauri::command]
fn diagnostics_report(app: tauri::AppHandle) -> Value {
    let profile = profile_dir();
    let dir = profile.join("diagnostics");
    let _ = std::fs::create_dir_all(&dir);
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("DropForge diagnostics"));
    lines.push(format!("version: {APP_VERSION}"));
    lines.push(format!("os: {} {}", std::env::consts::OS, std::env::consts::ARCH));
    lines.push(format!("windows: {} {} (build {})",
        reg_value("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "ProductName").unwrap_or_default(),
        reg_value("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "DisplayVersion").unwrap_or_default(),
        reg_value("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "CurrentBuild").unwrap_or_default()));
    lines.push(format!("webview2: {}", webview2_version()));
    lines.push(format!("exe: {}", mask_text(&std::env::current_exe().map(|path| path.display().to_string()).unwrap_or_default())));
    lines.push(format!("data: {}", mask_text(&profile.display().to_string())));
    let exe_dir = std::env::current_exe().ok().and_then(|path| path.parent().map(|dir| dir.to_path_buf())).unwrap_or_default();
    // DropForge.exe и dropforge-tauri.exe — одно и то же приложение под разными
    // именами (релизная и dev-сборка), отсутствие второго — норма, а не поломка.
    for name in ["DropForge.exe", "dropforge-tauri.exe"] {
        let path = exe_dir.join(name);
        if let Ok(meta) = std::fs::metadata(&path) {
            lines.push(format!("file: {name} — {} байт, изменён {}", meta.len(), chrono::DateTime::<chrono::Utc>::from(meta.modified().unwrap_or(std::time::SystemTime::now())).to_rfc3339()));
        }
    }
    for name in ["watcher-svc\\dropforge-watcher-svc.exe", "pubsub-svc\\dropforge-pubsub-svc.exe"] {
        let path = exe_dir.join(name);
        if let Ok(meta) = std::fs::metadata(&path) {
            lines.push(format!("file: {name} — {} байт, изменён {}", meta.len(), chrono::DateTime::<chrono::Utc>::from(meta.modified().unwrap_or(std::time::SystemTime::now())).to_rfc3339()));
        } else {
            lines.push(format!("file: {name} — ОТСУТСТВУЕТ"));
        }
    }
    let settings = load_settings();
    let games: Vec<String> = settings.get("games").and_then(Value::as_array).map(|items| items.iter().map(|item| text_of(Some(item))).collect()).unwrap_or_default();
    lines.push(format!("settings.games: {}", games.join(", ")));
    lines.push(format!("settings.flags: autoClaim={} spade={} tray={}",
        settings.get("autoClaim").and_then(Value::as_bool).unwrap_or(false),
        settings.get("sendSpadeEvents").and_then(Value::as_bool).unwrap_or(false),
        settings.get("minimizeToTray").and_then(Value::as_bool).unwrap_or(false)));
    lines.push(format!("auth: {}", if load_token().is_some() { "токен есть" } else { "токена нет" }));
    let status = STATUS.lock().unwrap().clone();
    lines.push(format!("status: {status}"));
    let child = CHILD.lock().unwrap().as_ref().and_then(|child| child.id().into());
    lines.push(format!("watcher pid: {}", child.map(|pid| pid.to_string()).unwrap_or_else(|| "нет".into())));
    let pubsub_state = app.state::<MinerState>();
    let pubsub_status = {
        let mut guard = pubsub_state.pubsub.lock().unwrap();
        match guard.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => format!("works (pid {})", child.id()),
                Ok(Some(status)) => format!("exited (code {:?}), watchdog will restart", status.code()),
                Err(error) => format!("poll error: {error}"),
            },
            None => String::from("not running"),
        }
    };
    lines.push(format!("pubsub: {pubsub_status}"));
    let progress = read_json(progress_path()).unwrap_or(Value::Null);
    lines.push(format!("progress.at: {}", text_of(progress.get("at"))));
    lines.push(format!("progress.drops: {}", progress.get("drops").and_then(Value::as_object).map(|map| map.len()).unwrap_or(0)));
    let pending: Vec<&Value> = progress
        .get("drops")
        .and_then(Value::as_object)
        .map(|drops| {
            drops
                .values()
                .filter(|state| {
                    let claimed = state.get("isClaimed").and_then(Value::as_bool).unwrap_or(false);
                    let current = state.get("currentMinutes").and_then(Value::as_f64).unwrap_or(0.0);
                    let required = state.get("requiredMinutes").and_then(Value::as_f64).unwrap_or(0.0);
                    !claimed && required > 0.0 && current >= required
                })
                .collect()
        })
        .unwrap_or_default();
    let pending_with_id = pending
        .iter()
        .filter(|state| state.get("instanceId").map(|value| !value.is_null()).unwrap_or(false))
        .count();
    lines.push(format!(
        "claims: ready to claim {} (with instance id: {})",
        pending.len(),
        pending_with_id
    ));
    // прогресс по дропам игр из очереди: минуты сейчас / сколько нужно
    if let Some(cache) = read_json(profile_dir().join("catalog-cache.json")) {
        let campaigns = cache.get("campaigns").and_then(Value::as_array).cloned().unwrap_or_default();
        let progress_drops = progress.get("drops").and_then(Value::as_object).cloned().unwrap_or_default();
        let mut shown = 0usize;
        for campaign in &campaigns {
            let game_slug = text_of(campaign.get("game").and_then(|game| game.get("slug")));
            if !games.iter().any(|slug| slug == &game_slug) {
                continue;
            }
            let campaign_id = text_of(campaign.get("id"));
            let campaign_name = text_of(campaign.get("name"));
            for drop in campaign.get("timeBasedDrops").and_then(Value::as_array).cloned().unwrap_or_default() {
                let drop_id = text_of(drop.get("id"));
                let Some(state) = progress_drops.get(&drop_id) else { continue };
                if text_of(state.get("campaignId")) != campaign_id {
                    continue;
                }
                let claimed = state.get("isClaimed").and_then(Value::as_bool).unwrap_or(false);
                lines.push(format!(
                    "drop progress: {campaign_name} / {} — {}/{} мин{}",
                    text_of(drop.get("name")),
                    state.get("currentMinutes").and_then(Value::as_f64).unwrap_or(0.0),
                    state.get("requiredMinutes").and_then(Value::as_f64).unwrap_or(0.0),
                    if claimed { " (получен)" } else { "" }
                ));
                shown += 1;
            }
        }
        if let Some(watch) = progress.get("watch") {
            let game = text_of(watch.get("game"));
            let minutes = watch.get("currentMinutes").and_then(Value::as_f64).unwrap_or(0.0);
            if games.iter().any(|slug| slug == &game) {
                lines.push(format!("watch progress: {game} — {minutes:.0} мин (без кампании в каталоге)"));
                shown += 1;
            }
        }
        if shown == 0 {
            if games.iter().any(|slug| slug.starts_with("local:")) {
                lines.push(String::from("drop progress: локальная кампания — Twitch не отдаёт прогресс по таким дропсам"));
            } else {
                lines.push(String::from("drop progress: нет данных по играм из очереди"));
            }
        }
    }
    lines.push(format!("network gql.twitch.tv: {}", reachable("https://gql.twitch.tv")));
    lines.push(format!("network api.github.com: {}", reachable("https://api.github.com")));
    lines.push(format!("network drops.sunkwi.com: {}", reachable("https://drops.sunkwi.com/drops.json")));
    lines.push(String::from("logs:"));
    for entry in LOGS.lock().unwrap().iter().rev().take(120).rev() {
        lines.push(format!("  {entry}"));
    }
    let report = mask_text(&lines.join("\n"));
    let path = dir.join("latest.txt");
    let _ = std::fs::write(&path, &report);
    let _ = app.emit("log", json!({ "at": "", "level": "info", "message": "Отчёт диагностики сохранён" }));
    json!({ "path": mask_text(&path.display().to_string()), "text": report })
}

fn version_parts(value: &str) -> Vec<u32> {
    value.trim_start_matches('v').split('.').map(|part| part.parse::<u32>().unwrap_or(0)).collect()
}

fn version_greater(latest: &str, current: &str) -> bool {
    let left = version_parts(latest);
    let right = version_parts(current);
    for index in 0..left.len().max(right.len()) {
        let a = *left.get(index).unwrap_or(&0);
        let b = *right.get(index).unwrap_or(&0);
        if a != b {
            return a > b;
        }
    }
    false
}

fn clipboard_copy(text: &str) -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::HANDLE;
        use windows_sys::Win32::System::DataExchange::{
            CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
        };
        use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
        use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = wide.len() * std::mem::size_of::<u16>();
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return false;
            }
            EmptyClipboard();
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if handle.is_null() {
                CloseClipboard();
                return false;
            }
            let pointer = GlobalLock(handle) as *mut u16;
            if pointer.is_null() {
                CloseClipboard();
                return false;
            }
            std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer, wide.len());
            GlobalUnlock(handle);
            if SetClipboardData(CF_UNICODETEXT as u32, handle as HANDLE).is_null() {
                CloseClipboard();
                return false;
            }
            CloseClipboard();
        }
        true
    }
    #[cfg(not(windows))]
    {
        let _ = text;
        false
    }
}

fn last_error_line() -> String {
    let logs = LOGS.lock().unwrap();
    for entry in logs.iter().rev() {
        if entry.get("level").and_then(Value::as_str) == Some("error") {
            return mask_text(&text_of(entry.get("message")));
        }
    }
    String::from("нет")
}

/// Требуемый бейдж для эволюции (Pokémon First Partners / Evolutions).
fn evolution_prereq(name: &str) -> Option<&'static str> {
    match name {
        "Ivysaur" => Some("Bulbasaur"),
        "Venusaur" => Some("Ivysaur"),
        "Charmeleon" => Some("Charmander"),
        "Charizard" => Some("Charmeleon"),
        "Wartortle" => Some("Squirtle"),
        "Blastoise" => Some("Wartortle"),
        "Haunter" => Some("Gastly"),
        "Gengar" => Some("Haunter"),
        "Raichu" => Some("Pikachu"),
        _ => None,
    }
}

fn short_summary() -> String {
    let settings = load_settings();
    let games: Vec<String> = settings.get("games").and_then(Value::as_array).map(|items| items.iter().map(|item| text_of(Some(item))).collect()).unwrap_or_default();
    let running = STATUS.lock().unwrap().get("running").and_then(Value::as_bool).unwrap_or(false);
    let windows = format!(
        "{} {} (build {})",
        reg_value("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "ProductName").unwrap_or_default(),
        reg_value("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "DisplayVersion").unwrap_or_default(),
        reg_value("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion", "CurrentBuild").unwrap_or_default()
    );
    format!(
        "DropForge {APP_VERSION} · {windows}\nочередь: {} · фарм: {} · вход: {}\nwebview2: {}\nпоследняя ошибка: {}",
        if games.is_empty() { "пусто".to_string() } else { games.join(", ") },
        if running { "идёт" } else { "не запущен" },
        if load_token().is_some() { "да" } else { "нет" },
        webview2_version(),
        last_error_line()
    )
}

#[tauri::command]
fn report_summary() -> Value {
    let text = short_summary();
    let copied = clipboard_copy(&text);
    json!({ "text": text, "copied": copied })
}

#[tauri::command]
fn report_issue(app: tauri::AppHandle, comment: String) -> Value {
    let diagnostics = diagnostics_report(app.clone());
    let body = diagnostics.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
    let summary = short_summary();
    let content = format!(
        "{summary}\n\n--- комментарий пользователя ---\n{}\n\n--- диагностика ---\n{body}\n",
        comment.trim()
    );
    let name = format!("DropForge-report-{}.txt", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let downloads = std::env::var_os("USERPROFILE")
        .map(std::path::PathBuf::from)
        .map(|home| home.join("Downloads"))
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(profile_dir);
    let path = downloads.join(&name);
    if let Err(error) = std::fs::write(&path, &content) {
        return json!({ "ok": false, "error": error.to_string() });
    }
    register_export(&path);
    let copied = clipboard_copy(&summary);
    let mut explore = Command::new("explorer");
    explore.arg(format!("/select,\"{}\"", path.display()));
    hide_console(&mut explore);
    let _ = explore.spawn();
    json!({ "ok": true, "path": mask_text(&path.display().to_string()), "summary": summary, "copied": copied })
}

#[tauri::command]
fn diagnostics_open() -> Value {
    let path = profile_dir().join("diagnostics").join("latest.txt");
    if !path.exists() {
        return json!({ "ok": false, "error": "отчёт ещё не собран" });
    }
    let mut explore = Command::new("explorer");
    explore.arg(format!("/select,\"{}\"", path.display()));
    hide_console(&mut explore);
    let _ = explore.spawn();
    if let Ok(content) = std::fs::read_to_string(&path) {
        let _ = clipboard_copy(&content);
    }
    json!({ "ok": true, "path": mask_text(&path.display().to_string()) })
}

#[tauri::command]
fn update_check() -> Value {
    let url = format!("https://api.github.com/repos/{UPDATE_REPO}/releases/latest");
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("DropForge-Updater/1.0")
        .build()
        .new_agent();
    let response = match agent.get(&url).header("Accept", "application/vnd.github+json").call() {
        Ok(response) => response,
        Err(ureq::Error::StatusCode(code)) => {
            return json!({ "current": APP_VERSION, "available": false, "error": format!("GitHub ответил {code}") });
        }
        Err(error) => {
            return json!({ "current": APP_VERSION, "available": false, "error": format!("нет связи: {error}") });
        }
    };
    let release: Value = match response.into_body().read_json() {
        Ok(value) => value,
        Err(error) => return json!({ "current": APP_VERSION, "available": false, "error": format!("плохой ответ: {error}") }),
    };
    let tag = text_of(release.get("tag_name"));
    let latest = tag.trim_start_matches('v').to_string();
    let mut asset_url = String::new();
    let mut asset_name = String::new();
    let mut asset_size = 0u64;
    let mut asset_digest = String::new();
    for asset in release.get("assets").and_then(Value::as_array).cloned().unwrap_or_default() {
        let name = text_of(asset.get("name"));
        let digest = text_of(asset.get("digest"));
        if name.starts_with("DropForge-") && name.ends_with("-win64.zip") && digest.starts_with("sha256:") {
            asset_url = text_of(asset.get("browser_download_url"));
            asset_name = name;
            asset_size = asset.get("size").and_then(Value::as_u64).unwrap_or(0);
            asset_digest = digest;
            break;
        }
    }
    json!({
        "current": APP_VERSION,
        "latest": latest,
        "available": !latest.is_empty() && version_greater(&latest, APP_VERSION),
        "notes": release.get("body").cloned().unwrap_or(Value::Null),
        "publishedAt": release.get("published_at").cloned().unwrap_or(Value::Null),
        "assetUrl": asset_url,
        "assetName": asset_name,
        "assetSize": asset_size,
        "assetDigest": asset_digest,
        "url": release.get("html_url").cloned().unwrap_or(Value::Null),
    })
}

fn powershell_path() -> std::path::PathBuf {
    if let Some(root) = std::env::var_os("SystemRoot") {
        let candidate = std::path::PathBuf::from(root)
            .join("System32")
            .join("WindowsPowerShell")
            .join("v1.0")
            .join("powershell.exe");
        if candidate.is_file() {
            return candidate;
        }
    }
    std::path::PathBuf::from("powershell")
}

fn updater_script() -> String {
    [
        "param([int]$AppPid, [string]$Zip, [string]$Dst, [string]$ExeName, [string]$WorkDir)",
        "$ErrorActionPreference = 'Stop'",
        "try {",
        "    try { Wait-Process -Id $AppPid -Timeout 90 -ErrorAction SilentlyContinue } catch {}",
        "    Start-Sleep -Milliseconds 700",
        "    $tmp = Join-Path $WorkDir 'extract'",
        "    Expand-Archive -LiteralPath $Zip -DestinationPath $tmp -Force",
        "    $src = Join-Path $tmp 'DropForge'",
        "    if (-not (Test-Path -LiteralPath $src)) { $src = $tmp }",
        "    Get-ChildItem $src -Force | ForEach-Object { Copy-Item $_.FullName -Destination $Dst -Recurse -Force }",
        "    $legacy = Join-Path $Dst 'DropForge.exe'",
        "    if ($ExeName -ne 'DropForge.exe' -and (Test-Path -LiteralPath $legacy)) {",
        "        Copy-Item $legacy (Join-Path $Dst $ExeName) -Force",
        "        Remove-Item $legacy -Force -ErrorAction SilentlyContinue",
        "    }",
        "} catch {",
        "    try { Set-Content -LiteralPath (Join-Path $Dst 'update-error.txt') -Value $_.Exception.Message -Encoding UTF8 } catch {}",
        "} finally {",
        "    try { Start-Process -FilePath (Join-Path $Dst $ExeName) } catch {}",
        "    Remove-Item -LiteralPath $WorkDir -Recurse -Force -ErrorAction SilentlyContinue",
        "}",
    ].join("\n")
}

/// SHA-256 в hex: сверяем скачанный архив с digest из GitHub Release.
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[tauri::command]
fn update_install(app: tauri::AppHandle, url: String, digest: Option<String>) -> Value {
    // только релизы этого репозитория: чужой zip не должен попасть в установку
    let allowed = format!("https://github.com/{UPDATE_REPO}/releases/download/");
    if !url.starts_with(&allowed) && !url.starts_with("https://objects.githubusercontent.com/") {
        return json!({ "started": false, "error": "обновления берём только из релизов DropForge" });
    }
    let expected = digest.unwrap_or_default().trim().to_lowercase();
    let expected = expected.strip_prefix("sha256:").unwrap_or(&expected).to_string();
    if expected.len() != 64 || !expected.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return json!({ "started": false, "error": "у обновления нет контрольной суммы — установка отменена" });
    }
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => return json!({ "started": false, "error": error.to_string() }),
    };
    let exe_name = exe.file_name().and_then(|name| name.to_str()).unwrap_or("DropForge.exe").to_string();
    let dir = exe.parent().map(|path| path.to_path_buf()).unwrap_or_default();
    let work = std::env::temp_dir().join(format!(
        "dropforge-update-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    if let Err(error) = std::fs::create_dir_all(&work) {
        return json!({ "started": false, "error": error.to_string() });
    }
    let zip_path = work.join("update.zip");
    let script = work.join("update.ps1");
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(180)))
        .user_agent("DropForge-Updater/1.0")
        .build()
        .new_agent();
    let mut response = match agent.get(&url).call() {
        Ok(response) => response,
        Err(error) => return json!({ "started": false, "error": format!("скачивание: {error}") }),
    };
    let bytes = match response.body_mut().with_config().limit(64 * 1024 * 1024).read_to_vec() {
        Ok(bytes) => bytes,
        Err(error) => return json!({ "started": false, "error": format!("чтение: {error}") }),
    };
    let actual = sha256_hex(&bytes);
    if actual != expected {
        let _ = std::fs::remove_dir_all(&work);
        return json!({ "started": false, "error": "контрольная сумма не совпала — обновление отменено" });
    }
    if let Err(error) = std::fs::write(&zip_path, &bytes) {
        return json!({ "started": false, "error": error.to_string() });
    }
    if let Err(error) = std::fs::write(&script, updater_script()) {
        return json!({ "started": false, "error": error.to_string() });
    }
    let mut command = Command::new(powershell_path());
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-WindowStyle", "Hidden", "-File"])
        .arg(&script)
        .arg("-AppPid")
        .arg(std::process::id().to_string())
        .arg("-Zip")
        .arg(&zip_path)
        .arg("-Dst")
        .arg(&dir)
        .arg("-ExeName")
        .arg(&exe_name)
        .arg("-WorkDir")
        .arg(&work);
    hide_console(&mut command);
    if let Err(error) = command.spawn() {
        let _ = std::fs::remove_dir_all(&work);
        return json!({ "started": false, "error": error.to_string() });
    }
    let handle = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(900));
        handle.exit(0);
    });
    json!({ "started": true, "script": mask_text(&script.display().to_string()) })
}

fn http_json(url: &str) -> Option<Value> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) DropForge/1.1")
        .build()
        .new_agent();
    let response = agent.get(url).call().ok()?;
    let value: Value = response.into_body().read_json().ok()?;
    Some(value)
}

fn text_or_null(value: Option<&Value>) -> Value {
    value.cloned().unwrap_or(Value::Null)
}

/// Апскейл обложки: каталог отдаёт 120x160, на карточках это мыло.
/// Twitch CDN умеет те же файлы в 600x800 — меняем суффикс размера.
fn hi_res_box_art(url: &str) -> String {
    let Some(dot) = url.rfind('.') else { return url.to_string() };
    let (head, ext) = url.split_at(dot);
    let Some(dash) = head.rfind('-') else { return url.to_string() };
    let size = &head[dash + 1..];
    let mut parts = size.split('x');
    let (Some(width), Some(height)) = (parts.next(), parts.next()) else {
        return url.to_string();
    };
    if parts.next().is_some() {
        return url.to_string();
    }
    let digits = |value: &str| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit());
    if digits(width) && digits(height) {
        format!("{}-600x800{}", &head[..dash], ext)
    } else {
        url.to_string()
    }
}

fn box_art_value(value: Option<&Value>) -> Value {
    match value.and_then(Value::as_str) {
        Some(url) if !url.is_empty() => json!(hi_res_box_art(url)),
        _ => Value::Null,
    }
}

/// Приводит кампанию каталога к виду, который читает snapshot_from_profile.
fn raw_from_catalog(group: &Value, campaign: &Value) -> Value {
    let game = campaign.get("game").cloned().unwrap_or(Value::Null);
    let allow = campaign.get("allow").cloned().unwrap_or(Value::Null);
    let drops: Vec<Value> = campaign
        .get("timeBasedDrops")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(index, drop)| {
            let edges: Vec<Value> = drop
                .get("benefitEdges")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|edge| {
                    let benefit = edge.get("benefit").cloned().unwrap_or(Value::Null);
                    json!({
                        "benefit": {
                            "id": text_or_null(benefit.get("id")),
                            "name": benefit.get("name").cloned().unwrap_or(json!("Награда")),
                            "distributionType": benefit.get("distributionType").cloned().unwrap_or(json!("DIRECT_ENTITLEMENT")),
                            "imageAssetURL": benefit.get("imageAssetURL").cloned().unwrap_or(Value::Null),
                        }
                    })
                })
                .collect();
            json!({
                "id": text_or_null(drop.get("id")),
                "name": drop.get("name").cloned().unwrap_or(json!(format!("Награда {}", index + 1))),
                "startAt": text_or_null(drop.get("startAt")),
                "endAt": text_or_null(drop.get("endAt")),
                "requiredMinutesWatched": text_or_null(drop.get("requiredMinutesWatched")),
                "preconditionDrops": [],
                "benefitEdges": edges,
            })
        })
        .collect();

    json!({
        "id": text_or_null(campaign.get("id")),
        "name": text_or_null(campaign.get("name")),
        "status": campaign.get("status").cloned().unwrap_or(json!("ACTIVE")),
        "startAt": text_or_null(campaign.get("startAt")),
        "endAt": text_or_null(campaign.get("endAt")),
        "accountLinkURL": text_or_null(campaign.get("accountLinkURL")),
        "game": {
            "id": game.get("id").or_else(|| group.get("gameId")).cloned().unwrap_or(Value::Null),
            "name": game.get("displayName").or_else(|| game.get("name"))
                .or_else(|| group.get("gameDisplayName")).cloned().unwrap_or(json!("Неизвестная игра")),
            "slug": text_or_null(game.get("slug")),
            "boxArtURL": box_art_value(group.get("gameBoxArtURL")),
        },
        "allow": {
            "isEnabled": allow.get("isEnabled").and_then(Value::as_bool).unwrap_or(false),
            "channels": allow.get("channels").cloned().unwrap_or(json!([])),
        },
        "timeBasedDrops": drops,
        "suggestedChannel": Value::Null,
        "source": "catalog",
        // кампания, привязанная к нескольким каналам, — локальная
        "local": allow
            .get("channels")
            .and_then(Value::as_array)
            .map(|channels| !channels.is_empty() && channels.len() <= 5)
            .unwrap_or(false),
        "channel": allow
            .get("channels")
            .and_then(Value::as_array)
            .and_then(|channels| channels.first())
            .map(|channel| json!({ "login": channel.get("name"), "id": channel.get("id") })),
    })
}

fn raw_from_reward(campaign: &Value) -> Value {
    let groups = campaign.get("rewardGroups").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut category = groups
        .first()
        .and_then(|group| group.get("categories"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or(Value::Null);
    // у GTAVI в фиде нет категории — без неё кампания выпадала и подхватывалась как «локальная»
    if category.is_null()
        && text_of(campaign.get("id")) == "b7669f9e-c69c-4a9b-a3b2-36266c1bdfc0"
    {
        category = json!({
            "id": "1867279146",
            "slug": "grand-theft-auto-vi",
            "displayName": "Grand Theft Auto VI",
        });
    }

    let mut drops = Vec::new();
    let campaign_title = text_of(campaign.get("name")).to_lowercase();
    let mut watch_reward_ids: Vec<String> = Vec::new();
    let mut sub_reward_ids: Vec<String> = Vec::new();
    for group in &groups {
        let minutes = group
            .get("unlockRequirements")
            .and_then(|req| req.get("minuteWatchedGoal"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let subs = group
            .get("unlockRequirements")
            .and_then(|req| req.get("subsGoal"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        for reward in group.get("rewards").and_then(Value::as_array).cloned().unwrap_or_default() {
            if let Some(reward_id) = reward.get("id").map(|value| text_of(Some(value))) {
                if !reward_id.is_empty() {
                    if minutes > 0.0 { watch_reward_ids.push(reward_id.clone()); }
                    if subs > 0.0 { sub_reward_ids.push(reward_id); }
                }
            }
            let image = reward
                .get("thumbnailImage")
                .and_then(|image| image.get("image1xURL"))
                .cloned()
                .unwrap_or(Value::Null);
            // бейджи приходят и как BADGE, и как QUEST_REWARD: отличаем по адресу ассета и ссылке
            let image_text = text_of(Some(&image));
            let redemption = text_of(reward.get("redemptionURL")).to_lowercase();
            let is_badge = image_text.contains("/badges/") || redemption.contains("badge");
            let reward_name = text_of(reward.get("name"));
            let mut final_name = reward_name.clone();
            let mut final_image = image.clone();
            let mut final_badge = is_badge;
            // у GTAVI «сундуки» подписок на самом деле содержат конкретные награды
            if campaign_title.contains("gtavi") && subs <= 0.0 && reward_name.to_lowercase().contains("dvd") {
                final_name = String::from("Ежедневный сундук с цепочкой (1 из 6)");
                final_image = Value::Null;
            }            // у GTAVI «сундуки» подписок на самом деле содержат конкретные награды
            if campaign_title.contains("gtavi") && subs > 0.0 {
                let is_badge_chest = reward_name.to_lowercase().contains("badge");
                let replacement: Option<(&str, &str, bool)> = match (subs as i64, is_badge_chest) {
                    (1, true) => Some(("Welcome 2 Miami", "https://static-cdn.jtvnw.net/badges/v1/1c51d343-754e-4fee-a717-ec95cafb5482/3", true)),
                    (2, true) => Some(("Scored Big", "https://static-cdn.jtvnw.net/badges/v1/2acfc5b8-7907-4b05-9b30-e211a93ffc63/3", true)),
                    (3, true) => Some(("Bleed Miami", "https://static-cdn.jtvnw.net/badges/v1/c7b8fc7d-7ebd-46e1-8f57-cc58ce51a9d1/3", true)),
                    (1, false) => Some(("BustedPlz", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_fba1756b9e5f443c9725542b78f93b11/default/dark/3.0", false)),
                    (2, false) => Some(("DinoBusted", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_a021fa5191714969829fee79badc0756/default/dark/3.0", false)),
                    (3, false) => Some(("SabaBusted", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_bcdb69418a044bf982e1fed15c34ce63/default/dark/3.0", false)),
                    _ => None,
                };
                if let Some((name, url, badge)) = replacement {
                    final_name = name.to_string();
                    final_image = json!(url);
                    final_badge = badge;
                }
            }
            let drop_minutes = if campaign_title.contains("gtavi") { 0.0 } else { minutes };
            let drop_note = if campaign_title.contains("gtavi") && subs <= 0.0 { "Проверяйте прогресс на Twitch" } else { "" };
            drops.push(json!({
                "id": text_or_null(reward.get("id")),
                "name": final_name,
                "startAt": text_or_null(campaign.get("startsAt")),
                "endAt": reward.get("earnableUntil").or_else(|| campaign.get("endsAt")).cloned().unwrap_or(Value::Null),
                "requiredMinutesWatched": drop_minutes,
                "requiredSubs": subs,
                "note": drop_note,
                "preconditionDrops": [],
                "benefitEdges": [{
                    "benefit": {
                        "id": text_or_null(reward.get("id")),
                        "name": final_name,
                        "distributionType": "QUEST_REWARD",
                        "imageAssetURL": final_image,
                        "badge": final_badge,
                    }
                }],
            }));
        }
    }

    let category_id = category.get("id").cloned().unwrap_or(Value::Null);
    json!({
        "id": text_or_null(campaign.get("id")),
        "name": text_or_null(campaign.get("name")),
        "status": campaign.get("status").cloned().unwrap_or(json!("ACTIVE")),
        "startAt": text_or_null(campaign.get("startsAt")),
        "endAt": text_or_null(campaign.get("endsAt")),
        "accountLinkURL": Value::Null,
        "game": {
            "id": category_id,
            "name": category.get("displayName").or_else(|| category.get("name")).cloned().unwrap_or(json!("Twitch")),
            "slug": text_or_null(category.get("slug")),
            "boxArtURL": if category_id.is_null() {
                Value::Null
            } else {
                json!(format!("https://static-cdn.jtvnw.net/ttv-boxart/{}_IGDB-285x380.jpg", text_of(Some(&category_id))))
            },
        },
        "allow": { "isEnabled": false, "channels": [] },
        "timeBasedDrops": drops,
        "rewardCampaign": true,
        "watchRewardIds": watch_reward_ids,
        "subRewardIds": sub_reward_ids,
        "webUrl": campaign.get("webUrl").cloned().unwrap_or(json!(REWARDS_PAGE)),
        "suggestedChannel": Value::Null,
        "source": "reward",
    })
}

fn progress_path() -> PathBuf {
    profile_dir().join("progress.json")
}

/// Минуты по каждому дропу кампании: DropCampaignDetails по кампаниям из инвентаря.
/// Награды, которые аккаунт уже получил (бейджи, дропы).
/// Полученные награды кампаний: id, тип, статус и дата.
fn earned_rewards(token: &str) -> Vec<Value> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36")
        .build()
        .new_agent();
    let body = json!([{
        "operationName": "Inventory",
        "extensions": { "persistedQuery": { "version": 1, "sha256Hash": "8337eb8541b314040b0edde0c09c5c7a2783ba1960aa9edfbf3bac16d0fec404" } },
        "variables": { "fetchRewardCampaigns": true }
    }]);
    let response: Value = agent
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko")
        .header("Content-Type", "text/plain;charset=UTF-8")
        .header("Origin", "https://www.twitch.tv")
        .header("Authorization", &format!("OAuth {token}"))
        .send(body.to_string())
        .ok()
        .and_then(|response| response.into_body().read_json::<Value>().ok())
        .unwrap_or(Value::Null);
    response
        .get(0)
        .and_then(|item| item.get("data"))
        .and_then(|data| data.get("currentUser"))
        .and_then(|user| user.get("inventory"))
        .and_then(|inventory| inventory.get("earnedDropRewards"))
        .and_then(|earned| earned.get("edges"))
        .and_then(Value::as_array)
        .map(|edges| {
            edges
                .iter()
                .filter_map(|edge| edge.get("node"))
                .map(|node| {
                    json!({
                        "campaignId": node.get("campaign").and_then(|campaign| campaign.get("id")),
                        "itemId": node.get("item").and_then(|item| item.get("id")),
                        "name": node.get("item").and_then(|item| item.get("name")),
                        "type": node.get("item").and_then(|item| item.get("distributionType")),
                        "status": node.get("status"),
                        "earnedAt": node.get("earnedAt"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Бейджи, доступные аккаунту (в том числе полученные за кампании).
fn available_badges(token: &str) -> Vec<Value> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36")
        .build()
        .new_agent();
    let query = "query { currentUser { availableBadges { id setID version title imageURL } } }";
    let response: Value = agent
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko")
        .header("Content-Type", "application/json")
        .header("Origin", "https://www.twitch.tv")
        .header("Authorization", &format!("OAuth {token}"))
        .send(json!({ "query": query, "variables": {} }).to_string())
        .ok()
        .and_then(|response| response.into_body().read_json::<Value>().ok())
        .unwrap_or(Value::Null);
    response
        .get("data")
        .and_then(|data| data.get("currentUser"))
        .and_then(|user| user.get("availableBadges"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn claimed_benefits(token: &str) -> Vec<String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("Mozilla/5.0 DropForge/1.1")
        .build()
        .new_agent();
    let body = json!([{
        "operationName": "Inventory",
        "extensions": { "persistedQuery": { "version": 1, "sha256Hash": "8337eb8541b314040b0edde0c09c5c7a2783ba1960aa9edfbf3bac16d0fec404" } },
        "variables": { "fetchRewardCampaigns": false }
    }]);
    let response: Value = agent
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko")
        .header("Content-Type", "text/plain;charset=UTF-8")
        .header("Origin", "https://www.twitch.tv")
        .header("Authorization", &format!("OAuth {token}"))
        .send(body.to_string())
        .ok()
        .and_then(|response| response.into_body().read_json::<Value>().ok())
        .unwrap_or(Value::Null);
    response
        .get(0)
        .and_then(|item| item.get("data"))
        .and_then(|data| data.get("currentUser"))
        .and_then(|user| user.get("inventory"))
        .and_then(|inventory| inventory.get("gameEventDrops"))
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|item| text_of(item.get("id"))).collect())
        .unwrap_or_default()
}

fn refresh_progress() -> Result<Value, String> {
    let Some(token) = load_token() else {
        return Err("нужен вход в Twitch".into());
    };

    let mut drops: serde_json::Map<String, Value> = serde_json::Map::new();
    let campaigns = inventory_campaigns(&token);
    let mut campaigns_seen = 0usize;

    for campaign in &campaigns {
        let id = text_of(campaign.get("id"));
        if id.is_empty() {
            continue;
        }
        campaigns_seen += 1;
        for drop in campaign
            .get("timeBasedDrops")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let drop_id = text_of(drop.get("id"));
            if drop_id.is_empty() {
                continue;
            }
            let self_state = drop.get("self").cloned().unwrap_or(Value::Null);
            drops.insert(
                drop_id,
                json!({
                    "campaignId": id,
                    "currentMinutes": self_state.get("currentMinutesWatched").cloned().unwrap_or(json!(0)),
                    "requiredMinutes": drop.get("requiredMinutesWatched").cloned().unwrap_or(json!(0)),
                    "isClaimed": self_state.get("isClaimed").cloned().unwrap_or(json!(false)),
                    "instanceId": self_state.get("dropInstanceID").cloned().unwrap_or(Value::Null),
                }),
            );
        }
    }

    let claimed = claimed_benefits(&token);
    let earned = earned_rewards(&token);
    let profile_badges = available_badges(&token);
    let payload = json!({
        "at": chrono::Utc::now().to_rfc3339(),
        "campaigns": campaigns_seen,
        "drops": drops,
        "claimedBenefits": claimed,
        "earnedRewards": earned,
        "availableBadges": profile_badges,
    });
    std::fs::create_dir_all(profile_dir()).ok();
    std::fs::write(
        progress_path(),
        serde_json::to_string(&payload).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(payload)
}

fn refresh_catalog() -> Result<Value, String> {
    if !is_logged_in() {
        return Err("нет активного входа".to_string());
    }
    let drops = http_json(CATALOG_URL).ok_or("каталог недоступен")?;
    let rewards = http_json(REWARDS_URL).unwrap_or(Value::Null);

    let mut campaigns: Vec<Value> = Vec::new();
    for group in drops.get("games").and_then(Value::as_array).cloned().unwrap_or_default() {
        for campaign in group.get("rewards").and_then(Value::as_array).cloned().unwrap_or_default() {
            if campaign.get("status").and_then(Value::as_str) == Some("EXPIRED") {
                continue;
            }
            let raw = raw_from_catalog(&group, &campaign);
            // в самом фиде бывают одинаковые кампании с разными id — оставляем одну
            let raw_name = text_of(raw.get("name")).to_lowercase();
            let raw_slug = text_of(raw.get("game").and_then(|game| game.get("slug")));
            let raw_end = text_of(raw.get("endAt"));
            let duplicate = campaigns.iter().any(|item| {
                text_of(item.get("name")).to_lowercase() == raw_name
                    && text_of(item.get("game").and_then(|game| game.get("slug"))) == raw_slug
                    && text_of(item.get("endAt")) == raw_end
            });
            if duplicate {
                continue;
            }
            campaigns.push(raw);
        }
    }
    for campaign in rewards.get("campaigns").and_then(Value::as_array).cloned().unwrap_or_default() {
        if campaign.get("status").and_then(Value::as_str) == Some("EXPIRED") {
            continue;
        }
        let raw = raw_from_reward(&campaign);
        let has_slug = raw
            .get("game")
            .and_then(|game| game.get("slug"))
            .map(|slug| !text_of(Some(slug)).is_empty())
            .unwrap_or(false);
        if has_slug {
            campaigns.push(raw);
        }
    }

    // кампании из инвентаря: их может не быть в публичном каталоге
    if let Some(token) = load_token() {
        for campaign in inventory_campaigns(&token) {
            if campaign.get("status").and_then(Value::as_str) == Some("EXPIRED") {
                continue;
            }
            let mut raw = raw_from_catalog(&Value::Null, &campaign);
            let slug = raw.get("game").and_then(|game| game.get("slug")).map(|slug| text_of(Some(slug))).unwrap_or_default();
            if slug.is_empty() {
                continue;
            }
            // та же кампания уже могла прийти из публичного каталога — не плодим копии
            let raw_name = text_of(raw.get("name")).to_lowercase();
            let raw_end = text_of(raw.get("endAt"));
            let duplicate = campaigns.iter().any(|item| {
                text_of(item.get("name")).to_lowercase() == raw_name
                    && text_of(item.get("game").and_then(|game| game.get("slug"))) == slug
                    && text_of(item.get("endAt")) == raw_end
            });
            if duplicate {
                continue;
            }
            // у кампаний из инвентаря часто нет обложки и названия игры — берём у известной кампании
            let missing_art = text_of(raw.get("game").and_then(|game| game.get("boxArtURL"))).is_empty();
            let missing_name = text_of(raw.get("game").and_then(|game| game.get("name"))).is_empty();
            if missing_art || missing_name {
                let donor = campaigns.iter().find(|item| {
                    text_of(item.get("game").and_then(|game| game.get("slug"))) == slug
                        && !text_of(item.get("game").and_then(|game| game.get("boxArtURL"))).is_empty()
                });
                if let Some(donor) = donor {
                    let art = donor.get("game").and_then(|game| game.get("boxArtURL")).cloned().unwrap_or(Value::Null);
                    let name = donor.get("game").and_then(|game| game.get("name")).cloned().unwrap_or(Value::Null);
                    if let Some(game) = raw.get_mut("game").and_then(Value::as_object_mut) {
                        if missing_art && !art.is_null() {
                            game.insert("boxArtURL".into(), art);
                        }
                        if missing_name && !name.is_null() {
                            game.insert("name".into(), name);
                        }
                    }
                }
            }
            let id = text_of(raw.get("id"));
            let known = campaigns.iter().any(|item| text_of(item.get("id")) == id);
            if !known {
                campaigns.push(raw);
            }
        }

        // локальные дропсы каналов, которые добавил пользователь
        for campaign in local_campaigns(&token) {
            let id = text_of(campaign.get("id"));
            if id.is_empty() {
                continue;
            }
            // кампания могла уже прийти из инвентаря — локальная версия точнее
            // (полный список дропов канала и обложка кампании)
            // не затираем каталожную/наградную версию: у неё бывают свои дропы и бейджи,
            // поэтому локальная копия живёт рядом с отдельным id
            // если такая кампания уже есть в глобальном каталоге — это не «локальная»,
            // а канальная версия глобальной: в локальные её не показываем
            let local_name = text_of(campaign.get("name")).to_lowercase();
            let duplicate_global = campaigns.iter().any(|item| {
                let global_id = text_of(item.get("id"));
                let global_name = text_of(item.get("name")).to_lowercase();
                global_id == id || (!global_name.is_empty() && global_name == local_name)
            });
            if duplicate_global {
                continue;
            }
            campaigns.push(campaign);
        }
    }

    let _ = refresh_progress();

    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or(0);
    let _ = at;
    let cache = json!({ "at": chrono::Utc::now().to_rfc3339(), "campaigns": campaigns });
    let dir = profile_dir();
    std::fs::create_dir_all(&dir).ok();
    std::fs::write(
        dir.join("catalog-cache.json"),
        serde_json::to_string(&cache).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    Ok(cache)
}

const DEVICE_CLIENT_ID: &str = "ue6666qo983tsx6so1t0vnawi233wa";
const DEVICE_UA: &str = "Mozilla/5.0 (Linux; Android 7.1; Smart Box C1) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

fn url_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn device_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent(DEVICE_UA)
        .build()
        .new_agent()
}

/// Запрос кода устройства: Twitch выдаёт код и ссылку активации.
fn device_start() -> Result<Value, String> {
    let body = format!("client_id={}&scopes=", url_encode(DEVICE_CLIENT_ID));
    let response = device_agent()
        .post("https://id.twitch.tv/oauth2/device")
        .header("Client-Id", DEVICE_CLIENT_ID)
        .header("Origin", "https://android.tv.twitch.tv")
        .header("Referer", "https://android.tv.twitch.tv/")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body)
        .map_err(|error| format!("Twitch не выдал код устройства: {error}"))?;
    let value: Value = response
        .into_body()
        .read_json()
        .map_err(|error| format!("ответ Twitch не разобран: {error}"))?;
    if value.get("device_code").is_none() {
        return Err(format!("Twitch ответил без кода: {value}"));
    }
    let user_code = text_of(value.get("user_code"));
    let uri = text_of(value.get("verification_uri"));
    let activation = if uri.contains("device-code=") {
        uri
    } else {
        format!("{uri}{}device-code={}", if uri.contains('?') { "&" } else { "?" }, url_encode(&user_code))
    };
    Ok(json!({
        "userCode": user_code,
        "uri": activation,
        "expiresIn": value.get("expires_in").cloned().unwrap_or(json!(1800)),
        "deviceCode": value.get("device_code").cloned().unwrap_or(Value::Null),
        "interval": value.get("interval").cloned().unwrap_or(json!(5)),
    }))
}

/// Ждём подтверждения в браузере и забираем токен.
fn device_poll(started: Value) -> Result<String, String> {
    let device_code = text_of(started.get("deviceCode"));
    let expires = started.get("expiresIn").and_then(Value::as_u64).unwrap_or(1800);
    let mut interval = started.get("interval").and_then(Value::as_u64).unwrap_or(5).max(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(expires);

    while std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_secs(interval));
        let body = format!(
            "client_id={}&device_code={}&grant_type={}",
            url_encode(DEVICE_CLIENT_ID),
            url_encode(&device_code),
            url_encode("urn:ietf:params:oauth:grant-type:device_code"),
        );
        let response = device_agent()
            .post("https://id.twitch.tv/oauth2/token")
            .header("Client-Id", DEVICE_CLIENT_ID)
            .header("Origin", "https://android.tv.twitch.tv")
            .header("Referer", "https://android.tv.twitch.tv/")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send(body);

        let value: Value = match response {
            Ok(response) => response.into_body().read_json().unwrap_or(Value::Null),
            Err(ureq::Error::StatusCode(_)) => Value::Null,
            Err(_) => Value::Null,
        };
        if let Some(token) = value.get("access_token").and_then(Value::as_str) {
            return Ok(token.to_string());
        }
        let message = text_of(value.get("message"));
        if message.to_lowercase().contains("slow") {
            interval = (interval + 2).min(30);
        } else if message.to_lowercase().contains("expired") {
            break;
        }
    }
    Err("Код истёк — попробуйте снова".into())
}

fn auth_path() -> PathBuf {
    profile_dir().join("tauri-auth.json")
}

#[cfg(windows)]
fn dpapi(data: &[u8], protect: bool) -> Option<Vec<u8>> {
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};
    use windows_sys::Win32::Foundation::LocalFree;

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let result = unsafe {
        if protect {
            CryptProtectData(&mut input, std::ptr::null(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut output)
        } else {
            CryptUnprotectData(&mut input, std::ptr::null_mut(), std::ptr::null(), std::ptr::null(), std::ptr::null(), 0, &mut output)
        }
    };
    if result == 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() };
    unsafe { LocalFree(output.pbData as *mut core::ffi::c_void) };
    Some(bytes)
}

#[cfg(not(windows))]
fn dpapi(data: &[u8], _protect: bool) -> Option<Vec<u8>> {
    Some(data.to_vec())
}

fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { TABLE[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { TABLE[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    fn value(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some((byte - b'A') as u32),
            b'a'..=b'z' => Some((byte - b'a' + 26) as u32),
            b'0'..=b'9' => Some((byte - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0u32;
    for byte in text.bytes() {
        if byte == b'=' {
            break;
        }
        let v = value(byte)?;
        buffer = (buffer << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xFF) as u8);
        }
    }
    Some(out)
}

fn validate_token(token: &str) -> Option<(String, String, String)> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .user_agent("Mozilla/5.0 DropForge/1.1")
        .build()
        .new_agent();
    let response: Value = agent
        .get("https://id.twitch.tv/oauth2/validate")
        .header("Authorization", &format!("OAuth {token}"))
        .call()
        .ok()?
        .into_body()
        .read_json()
        .ok()?;
    Some((
        text_of(response.get("login")),
        text_of(response.get("user_id")),
        text_of(response.get("client_id")),
    ))
}

fn fetch_avatar(token: &str, login: &str) -> Option<String> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .user_agent("Mozilla/5.0 DropForge/1.1")
        .build()
        .new_agent();
    let url = format!(
        "https://api.twitch.tv/helix/users?login={}",
        url_encode(&login.to_lowercase())
    );
    let response: Value = agent
        .get(&url)
        .header("Client-Id", &auth_client_id())
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .ok()?
        .into_body()
        .read_json()
        .ok()?;
    response
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|user| user.get("profile_image_url"))
        .map(|value| text_of(Some(value)))
        .filter(|value| !value.is_empty())
}

fn store_avatar(url: &str) {
    let Some(mut payload) = read_json(auth_path()) else { return };
    if let Some(object) = payload.as_object_mut() {
        object.insert("avatarUrl".into(), json!(url));
        let _ = std::fs::write(auth_path(), serde_json::to_string_pretty(&payload).unwrap_or_default());
    }
}

fn save_token(token: &str) -> Option<(String, String)> {
    let (login, user_id, client_id) = validate_token(token)?;
    let protected = dpapi(token.as_bytes(), true)?;
    let mut payload = json!({
        "token": format!("enc:{}", base64_encode(&protected)),
        "login": login,
        "userId": user_id,
        "clientId": client_id,
        "savedAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|span| span.as_secs()).unwrap_or(0),
    });
    std::fs::create_dir_all(profile_dir()).ok();
    std::fs::write(auth_path(), serde_json::to_string_pretty(&payload).ok()?).ok()?;
    let login_for_avatar = text_of(payload.get("login"));
    if let Some(avatar) = fetch_avatar(token, &login_for_avatar) {
        if let Some(object) = payload.as_object_mut() {
            object.insert("avatarUrl".into(), json!(avatar));
        }
        let _ = std::fs::write(auth_path(), serde_json::to_string_pretty(&payload).unwrap_or_default());
    }
    // Кэш авторизации мог запомнить пустое состояние до входа — обновляем его.
    set_auth_state(payload.clone());
    Some((text_of(payload.get("login")), text_of(payload.get("userId"))))
}

fn is_logged_in() -> bool {
    let auth = auth_state();
    auth.get("token").is_some() || load_token().is_some()
}

fn load_token() -> Option<String> {
    let raw = read_json(auth_path())?;
    let encoded = text_of(raw.get("token"));
    let encoded = encoded.strip_prefix("enc:")?;
    let bytes = base64_decode(encoded)?;
    let plain = dpapi(&bytes, false)?;
    String::from_utf8(plain).ok()
}

/* --------------------------------------------------- локальные дропсы канала */

const HASH_AVAILABLE_DROPS: &str =
    "782dad0f032942260171d2d80a654f88bdd0c5a9dddc392e9bc92218a0f42d20";
const DEVICE_CLIENT: &str = "ue6666qo983tsx6so1t0vnawi233wa";

/// Client-Id, с которым выдан токен: нужен для Helix.
fn auth_client_id() -> String {
    let id = read_json(auth_path())
        .map(|value| text_of(value.get("clientId")))
        .unwrap_or_default();
    if id.is_empty() {
        DEVICE_CLIENT.to_string()
    } else {
        id
    }
}

fn channel_id_by_login(login: &str) -> Option<String> {
    let token = load_token()?;
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .user_agent("Mozilla/5.0 DropForge/1.1")
        .build()
        .new_agent();
    let url = format!(
        "https://api.twitch.tv/helix/users?login={}",
        url_encode(&login.to_lowercase())
    );
    let response: Value = agent
        .get(&url)
        .header("Client-Id", &auth_client_id())
        .header("Authorization", &format!("Bearer {token}"))
        .call()
        .ok()?
        .into_body()
        .read_json()
        .ok()?;
    let id = response
        .get("data")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .map(|user| text_of(user.get("id")))
        .unwrap_or_default();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

/// Дропсы, доступные конкретному каналу (включая локальные кампании канала).
fn available_drops(token: &str, channel_id: &str) -> Vec<Value> {
    let data = gql_value(
        token,
        "DropsHighlightService_AvailableDrops",
        HASH_AVAILABLE_DROPS,
        json!({ "channelID": channel_id }),
    )
    .unwrap_or(Value::Null);
    data.get("channel")
        .and_then(|channel| channel.get("viewerDropCampaigns"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn local_channel() -> Option<String> {
    let settings = load_settings();
    let single = settings
        .get("localChannel")
        .map(|value| text_of(Some(value)))
        .unwrap_or_default();
    if !single.is_empty() {
        return Some(single.to_lowercase());
    }
    // совместимость со старым списком каналов
    settings
        .get("localChannels")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .map(|value| text_of(Some(value)).to_lowercase())
        .filter(|value| !value.is_empty())
}

/// Приводит локальную кампанию канала к виду, который читает snapshot_from_profile.
fn raw_from_local(login: &str, channel_id: &str, campaign: &Value) -> Value {
    let game = campaign.get("game").cloned().unwrap_or(Value::Null);
    let drops: Vec<Value> = campaign
        .get("timeBasedDrops")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()

        .map(|drop| {
            let required = drop
                .get("requiredMinutesWatched")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let benefits: Vec<Value> = drop
                .get("benefitEdges")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .iter()
                .filter_map(|edge| edge.get("benefit"))
                .map(|benefit| {
                    let mut name = text_of(benefit.get("name"));
                    let mut image = benefit.get("imageAssetURL").cloned().unwrap_or(Value::Null);
                    let mut badge = benefit.get("badge").and_then(Value::as_bool).unwrap_or(false);
                    if text_of(campaign.get("name")).to_lowercase().contains("gtavi") {
                        let subs = drop.get("requiredSubs").and_then(Value::as_f64).unwrap_or(0.0) as i64;
                        let is_badge_chest = name.to_lowercase().contains("badge") || badge;
                        let replacement: Option<(&str, &str, bool)> = match (subs, is_badge_chest) {
                            (1, true) => Some(("Welcome 2 Miami", "https://static-cdn.jtvnw.net/badges/v1/1c51d343-754e-4fee-a717-ec95cafb5482/3", true)),
                            (2, true) => Some(("Scored Big", "https://static-cdn.jtvnw.net/badges/v1/2acfc5b8-7907-4b05-9b30-e211a93ffc63/3", true)),
                            (3, true) => Some(("Bleed Miami", "https://static-cdn.jtvnw.net/badges/v1/c7b8fc7d-7ebd-46e1-8f57-cc58ce51a9d1/3", true)),
                            (1, false) => Some(("BustedPlz", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_fba1756b9e5f443c9725542b78f93b11/default/dark/3.0", false)),
                            (2, false) => Some(("DinoBusted", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_a021fa5191714969829fee79badc0756/default/dark/3.0", false)),
                            (3, false) => Some(("SabaBusted", "https://static-cdn.jtvnw.net/emoticons/v2/emotesv2_bcdb69418a044bf982e1fed15c34ce63/default/dark/3.0", false)),
                            _ => None,
                        };
                        if let Some((new_name, url, is_badge)) = replacement {
                            name = new_name.to_string();
                            image = json!(url);
                            badge = is_badge;
                        }
                    }
                    json!({
                        "id": benefit.get("id"),
                        "name": name,
                        "type": benefit.get("distributionType"),
                        "imageUrl": image,
                        "badge": badge,
                    })
                })
                .collect();
            let first_benefit_name = benefits
                .first()
                .and_then(|benefit| benefit.get("name"))
                .map(|value| text_of(Some(value)))
                .unwrap_or_default();
            let mut drop_name = text_of(drop.get("name"));
            let mut drop_note = text_of(drop.get("note"));
            if text_of(campaign.get("name")).to_lowercase().contains("gtavi") {
                let mut watch_chest = false;
                if drop_name.to_lowercase().contains("dvd") {
                    drop_name = String::from("Ежедневный сундук с цепочкой");
                    drop_note = String::from("Проверяйте прогресс на Twitch");
                    watch_chest = true;
                }
                if !first_benefit_name.is_empty() && !watch_chest {
                    drop_name = first_benefit_name;
                }
            }
            json!({
                "id": drop.get("id"),
                "name": drop_name,
                "startsAt": drop.get("startAt"),
                "endsAt": drop.get("endAt"),
                "requiredMinutes": required,
                "requiredSubs": drop.get("requiredSubs").cloned().unwrap_or(json!(0)),
                "note": drop_note,
                "currentMinutes": 0,
                "remainingMinutes": required,
                "progress": 0,
                "isClaimed": false,
                "canClaim": false,
                "benefits": benefits,
            })
        })
        .collect();

    let game_name = text_of(game.get("name"));
    let game_name = if game_name.is_empty() || game_name == "Special Events" {
        text_of(campaign.get("name"))
    } else {
        game_name
    };
    json!({
        "id": campaign.get("id"),
        "name": campaign.get("name"),
        "status": "ACTIVE",
        "imageURL": campaign.get("imageURL"),
        "game": {
            "id": game.get("id"),
            "name": game_name,
            "slug": format!("local:{login}"),
            "boxArtURL": campaign.get("imageURL"),
        },
        "allow": { "isEnabled": true, "channels": [{ "id": channel_id, "name": login }] },
        "timeBasedDrops": drops,
        "suggestedChannel": Value::Null,
        "source": "local",
        "local": true,
        "channel": { "login": login, "id": channel_id },
        "startAt": campaign.get("startAt"),
        "endAt": campaign.get("endAt"),
    })
}

/// Кампании всех каналов из settings.localChannels.
fn local_campaigns(token: &str) -> Vec<Value> {
    let Some(login) = local_channel() else {
        return Vec::new();
    };
    let Some(channel_id) = channel_id_by_login(&login) else {
        return Vec::new();
    };
    available_drops(token, &channel_id)
        .iter()
        .map(|campaign| raw_from_local(&login, &channel_id, campaign))
        .collect()
}

/// Обновляет только локальные кампании в кэше — без полной перезагрузки каталога.
fn refresh_local_into_cache(token: &str) -> usize {
    let path = profile_dir().join("catalog-cache.json");
    let mut cache = read_json(path.clone()).unwrap_or_else(|| json!({ "campaigns": [] }));
    let mut campaigns: Vec<Value> = cache
        .get("campaigns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    campaigns.retain(|item| item.get("source").and_then(Value::as_str) != Some("local"));
    let local = local_campaigns(token);
    let found = local.len();
    for campaign in local {
        let id = text_of(campaign.get("id"));
        match campaigns
            .iter()
            .position(|item| text_of(item.get("id")) == id)
        {
            Some(position) => campaigns[position] = campaign,
            None => campaigns.push(campaign),
        }
    }
    cache["campaigns"] = json!(campaigns);
    let _ = std::fs::write(path, serde_json::to_string(&cache).unwrap_or_default());
    found
}

fn inventory_campaigns(token: &str) -> Vec<Value> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("Mozilla/5.0 DropForge/1.1")
        .build()
        .new_agent();
    let body = json!([{
        "operationName": "Inventory",
        "extensions": { "persistedQuery": { "version": 1, "sha256Hash": "8337eb8541b314040b0edde0c09c5c7a2783ba1960aa9edfbf3bac16d0fec404" } },
        "variables": { "fetchRewardCampaigns": false }
    }]);
    let response: Value = agent
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko")
        .header("Content-Type", "text/plain;charset=UTF-8")
        .header("Origin", "https://www.twitch.tv")
        .header("Authorization", &format!("OAuth {token}"))
        .send(body.to_string())
        .ok()
        .and_then(|response| response.into_body().read_json::<Value>().ok())
        .unwrap_or(Value::Null);
    response
        .get(0)
        .and_then(|item| item.get("data"))
        .and_then(|data| data.get("currentUser"))
        .and_then(|user| user.get("inventory"))
        .and_then(|inventory| inventory.get("dropCampaignsInProgress"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

use std::process::{Child, Command};
use std::sync::Mutex;

struct MinerState {
    child: Mutex<Option<Child>>,
    pubsub: Mutex<Option<Child>>,
    status: Mutex<Value>,
}

struct TrayHolder(Mutex<Option<tauri::tray::TrayIcon>>);

static GAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static GAME_INDEX: Mutex<usize> = Mutex::new(0);
static STOPPING: Mutex<bool> = Mutex::new(false);
static WATCH_GEN: Mutex<u64> = Mutex::new(0);
static LAST_TICK: Mutex<Option<std::time::Instant>> = Mutex::new(None);
static WATCHDOG_EVENTS: Mutex<Vec<std::time::Instant>> = Mutex::new(Vec::new());
static PUBSUB_EVENTS: Mutex<Vec<std::time::Instant>> = Mutex::new(Vec::new());
static CHILD: Mutex<Option<Child>> = Mutex::new(None);
static STATUS: Mutex<Value> = Mutex::new(Value::Null);
static LOGS: Mutex<Vec<Value>> = Mutex::new(Vec::new());
static DEVICE: Mutex<Value> = Mutex::new(Value::Null);
static AUTH_CACHE: Mutex<Option<Value>> = Mutex::new(None);

fn pubsub_exe() -> Option<std::path::PathBuf> {
    if let Some(path) = debug_env("DROPFORGE_PUBSUB_EXE") {
        let candidate = std::path::PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("pubsub-svc").join("dropforge-pubsub-svc.exe"));
            candidates.push(dir.join("native").join("pubsub-svc").join("dropforge-pubsub-svc.exe"));
            candidates.push(dir.join("resources").join("pubsub-svc").join("dropforge-pubsub-svc.exe"));
            candidates.push(dir.join("dropforge-pubsub-svc.exe"));
        }
    }
    if let Some(found) = candidates.into_iter().find(|candidate| candidate.is_file()) {
        return Some(found);
    }
    let dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|path| path.to_path_buf()))?;
    find_service_exe(&dir.join("_up_"), "dropforge-pubsub-svc.exe", 6)
}

/// PubSub в отдельном процессе: события уходят в UI как есть.
/// Первый бесплатный неполученный бейдж, который можно фармить.
fn first_badge_target() -> Option<(String, String)> {
    let snapshot = snapshot_from_profile();
    let badges = snapshot.get("badges")?.as_array()?.clone();
    // что уже есть в профиле — нужно для проверки эволюций
    let progress = read_json(progress_path()).unwrap_or(Value::Null);
    let mut owned: std::collections::HashSet<String> = progress
        .get("availableBadges")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|item| text_of(item.get("title"))).collect())
        .unwrap_or_default();
    if let Some(items) = progress.get("earnedRewards").and_then(Value::as_array) {
        for item in items {
            owned.insert(text_of(item.get("name")));
        }
    }
    let now = chrono::Utc::now();
    for badge in badges {
        let paid = badge.get("paid").and_then(Value::as_bool).unwrap_or(true);
        let claimed = badge.get("claimed").and_then(Value::as_bool).unwrap_or(true);
        let minutes = badge.get("requiredMinutes").and_then(Value::as_f64).unwrap_or(0.0);
        let local = badge.get("local").and_then(Value::as_bool).unwrap_or(false);
        if paid || claimed || minutes <= 0.0 || local {
            continue;
        }
        // кампания уже закончилась — фармить бессмысленно
        let expired = badge
            .get("endsAt")
            .and_then(Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc) < now)
            .unwrap_or(false);
        if expired {
            continue;
        }
        // эволюция без нужного покемона недостижима
        let name = text_of(badge.get("name"));
        if let Some(required) = evolution_prereq(&name) {
            if !owned.contains(required) {
                continue;
            }
        }
        let slug = text_of(badge.get("gameSlug"));
        if slug.is_empty() {
            continue;
        }
        let name = text_of(badge.get("name"));
        return Some((slug, name));
    }
    None
}

/// Если вотчер жив, но тиков нет — перезапускаем (не больше 3 раз за 10 минут).
fn start_watchdog(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        let running = STATUS
            .lock()
            .unwrap()
            .get("running")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if !running {
            continue;
        }
        let idle = LAST_TICK
            .lock()
            .unwrap()
            .map(|at| at.elapsed())
            .unwrap_or_default();
        if idle < std::time::Duration::from_secs(180) {
            continue;
        }
        let allowed = {
            let mut events = WATCHDOG_EVENTS.lock().unwrap();
            events.retain(|at| at.elapsed() < std::time::Duration::from_secs(600));
            if events.len() >= 3 {
                false
            } else {
                events.push(std::time::Instant::now());
                true
            }
        };
        if !allowed {
            continue;
        }
        let games = GAMES.lock().unwrap().clone();
        let index = *GAME_INDEX.lock().unwrap();
        let slug = games
            .get(index % games.len().max(1))
            .cloned()
            .unwrap_or_default();
        push_log("warn", format!("Вотчер молчит {} с — перезапускаю", idle.as_secs()));
        stop_miner();
        std::thread::sleep(std::time::Duration::from_secs(2));
        *STOPPING.lock().unwrap() = false;
        let _ = start_miner(Some(app.clone()), &slug);
        *LAST_TICK.lock().unwrap() = Some(std::time::Instant::now());
        let _ = app.emit("state", snapshot_from_profile());
    });
}

/// PubSub-процесс мог завершиться сам (падение, закрытый stdin) — поднимаем
/// его обратно, иначе живые события Twitch пропадают до перезапуска приложения.
fn start_pubsub_watchdog(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(30));
        let Some(token) = load_token() else { continue };
        let user_id = read_json(auth_path())
            .and_then(|value| value.get("userId").cloned())
            .map(|value| text_of(Some(&value)))
            .unwrap_or_default();
        if user_id.is_empty() {
            continue;
        }
        let state = app.state::<MinerState>();
        let problem: Option<String> = {
            let mut guard = state.pubsub.lock().unwrap();
            match guard.as_mut() {
                None => Some(String::from("процесс не запущен")),
                Some(child) => match child.try_wait() {
                    Ok(None) => None,
                    Ok(Some(status)) => {
                        guard.take();
                        Some(match status.code() {
                            Some(code) => format!("код {code}"),
                            None => String::from("завершился"),
                        })
                    }
                    Err(error) => {
                        guard.take();
                        Some(format!("ошибка опроса: {error}"))
                    }
                },
            }
        };
        let Some(reason) = problem else { continue };
        let allowed = {
            let mut events = PUBSUB_EVENTS.lock().unwrap();
            events.retain(|at| at.elapsed() < std::time::Duration::from_secs(600));
            if events.len() >= 3 {
                false
            } else {
                events.push(std::time::Instant::now());
                true
            }
        };
        if !allowed {
            continue;
        }
        push_log("warn", format!("pubsub-svc остановился ({reason}) — перезапускаю"));
        start_pubsub(app.clone(), state.inner(), token, user_id);
    });
}

fn start_background(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        let mut last_refresh = std::time::Instant::now() - std::time::Duration::from_secs(3600);
        loop {
            std::thread::sleep(std::time::Duration::from_secs(30));
            let refresh_minutes = load_settings()
                .get("refreshMinutes")
                .and_then(Value::as_u64)
                .unwrap_or(10)
                .max(1);
            if last_refresh.elapsed() < std::time::Duration::from_secs(refresh_minutes * 60) {
                continue;
            }
            last_refresh = std::time::Instant::now();
            let refreshed = refresh_catalog();
            let settings = read_json(profile_dir().join("settings.json")).unwrap_or(Value::Null);
            let auto_claim = settings.get("autoClaim").and_then(Value::as_bool).unwrap_or(true);
            if auto_claim && refreshed.is_ok() {
                let result = claim_all();
                let _ = app.emit("log", json!({
                    "at": "",
                    "level": "info",
                    "message": format!("Автоклейм: {}", result)
                }));
            }
            let auto_badges = load_settings()
                .get("autoBadges")
                .and_then(Value::as_bool)
                .unwrap_or(true);
            let running = STATUS
                .lock()
                .unwrap()
                .get("running")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let queue_empty = load_settings()
                .get("games")
                .and_then(Value::as_array)
                .map(|items| items.is_empty())
                .unwrap_or(true);
            if auto_badges && !running && queue_empty {
                if let Some((slug, name)) = first_badge_target() {
                    push_log("info", format!("Беру бесплатный бейдж: {name}"));
                    let _ = start_miner(Some(app.clone()), &slug);
                }
            }
            let _ = app.emit("state", snapshot_from_profile());
        }
    });
}

fn update_tray_tooltip(app: &tauri::AppHandle) {
    let Some(holder) = app.try_state::<TrayHolder>() else { return };
    let guard = holder.0.lock().unwrap();
    let Some(tray) = guard.as_ref() else { return };
    let status = STATUS.lock().unwrap().clone();
    let running = status.get("running").and_then(Value::as_bool).unwrap_or(false);
    let game = text_of(status.get("game"));
    let channel = text_of(status.get("channel").and_then(|value| value.get("login")));
    let minutes = read_json(progress_path())
        .and_then(|progress| progress.get("watch").and_then(|watch| watch.get("currentMinutes")).and_then(Value::as_f64))
        .unwrap_or(0.0);
    let idle_seconds = LAST_TICK
        .lock()
        .unwrap()
        .map(|at| at.elapsed().as_secs())
        .unwrap_or(0);
    let text = if running {
        format!(
            "DropForge — фарм идёт: {} / {} · {} мин · проверка {} с назад",
            if channel.is_empty() { "канал" } else { &channel },
            if game.is_empty() { "игра" } else { &game },
            minutes.round(),
            idle_seconds
        )
    } else {
        format!("DropForge — простой · последняя проверка {} с назад", idle_seconds)
    };
    let short: String = text.chars().take(120).collect();
    let _ = tray.set_tooltip(Some(short.as_str()));
}

fn start_tray_updater(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        update_tray_tooltip(&app);
        std::thread::sleep(std::time::Duration::from_secs(15));
    });
}

fn start_pubsub(app: tauri::AppHandle, state: &MinerState, token: String, user_id: String) {
    let Some(exe) = pubsub_exe() else {
        let _ = app.emit("log", json!({ "at": "", "level": "warn", "message": "pubsub-svc не найден" }));
        return;
    };
    let mut command = Command::new(&exe);
    command
        .args(["--stdin-token", "--user", &user_id, "--seconds", "86400", "--exit-on-stdin-close"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    hide_console(&mut command);
    let Ok(mut child) = command.spawn() else {
        let _ = app.emit("log", json!({ "at": "", "level": "error", "message": "pubsub-svc не запустился" }));
        return;
    };
    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write;
        let _ = writeln!(stdin, "{token}");
    }
    let stdout = child.stdout.take();
    *state.pubsub.lock().unwrap() = Some(child);
    let handle = app.clone();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let Some(stdout) = stdout else { return };
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
            match event.get("event").and_then(Value::as_str) {
                Some("RESPONSE") => {
                    let _ = handle.emit("state", snapshot_from_profile());
                }
                Some("message") => {
                    let topic = text_of(event.get("topic"));
                    let payload = text_of(event.get("payload"));
                    let _ = handle.emit("log", json!({
                        "at": "",
                        "level": "info",
                        "message": format!("PubSub {topic}: {}", &payload[..payload.len().min(160)])
                    }));
                }
                _ => {}
            }
        }
    });
}

fn watcher_exe() -> Option<std::path::PathBuf> {
    if let Some(path) = debug_env("DROPFORGE_WATCHER_EXE") {
        let candidate = std::path::PathBuf::from(path);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            candidates.push(dir.join("watcher-svc").join("dropforge-watcher-svc.exe"));
            candidates.push(dir.join("native").join("watcher-svc").join("dropforge-watcher-svc.exe"));
            candidates.push(dir.join("resources").join("watcher-svc").join("dropforge-watcher-svc.exe"));
            candidates.push(dir.join("dropforge-watcher-svc.exe"));
        }
    }
    if let Some(found) = candidates.into_iter().find(|candidate| candidate.is_file()) {
        return Some(found);
    }
    let dir = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|path| path.to_path_buf()))?;
    find_service_exe(&dir.join("_up_"), "dropforge-watcher-svc.exe", 6)
}

/// Ищем исполняемый файл сервиса где угодно рядом с приложением.
fn find_service_exe(dir: &std::path::Path, name: &str, depth: usize) -> Option<std::path::PathBuf> {
    if depth == 0 {
        return None;
    }
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_service_exe(&path, name, depth - 1) {
                return Some(found);
            }
        }
    }
    None
}

fn gql_value(token: &str, operation: &str, hash: &str, variables: Value) -> Option<Value> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(15)))
        .user_agent("Mozilla/5.0 DropForge/1.1")
        .build()
        .new_agent();
    let body = json!([{
        "operationName": operation,
        "extensions": { "persistedQuery": { "version": 1, "sha256Hash": hash } },
        "variables": variables
    }]);
    let mut request = agent
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko")
        .header("Content-Type", "text/plain;charset=UTF-8")
        .header("Origin", "https://www.twitch.tv");
    if !token.is_empty() {
        request = request.header("Authorization", &format!("OAuth {token}"));
    }
    let response: Value = request
        .send(body.to_string())
        .ok()?
        .into_body()
        .read_json()
        .ok()?;
    Some(
        response
            .get(0)
            .and_then(|item| item.get("data"))
            .cloned()
            .unwrap_or(Value::Null),
    )
}

/// Живой канал категории с включёнными Drops: берём самого «дешёвого».
/// Кампании с allow.channels: смотреть можно только разрешённый канал.
fn gql_raw(token: &str, query: &str, variables: Value) -> Option<Value> {
    let agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(20)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36")
        .build()
        .new_agent();
    let mut request = agent
        .post("https://gql.twitch.tv/gql")
        .header("Client-Id", "kimne78kx3ncx6brgo4mv6wki5h1ko")
        .header("Content-Type", "application/json")
        .header("Origin", "https://www.twitch.tv");
    if !token.is_empty() {
        request = request.header("Authorization", &format!("OAuth {token}"));
    }
    let response: Value = request
        .send(json!({ "query": query, "variables": variables }).to_string())
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    Some(response)
}

/// У кампании есть список разрешённых каналов (не «все каналы категории»).
fn has_allowed_channels(slug: &str) -> bool {
    let Some(cache) = read_json(profile_dir().join("catalog-cache.json")) else { return false };
    let campaigns = cache.get("campaigns").and_then(Value::as_array).cloned().unwrap_or_default();
    campaigns.iter().any(|campaign| {
        let game_slug = campaign.get("game").and_then(|game| game.get("slug")).map(|value| text_of(Some(value))).unwrap_or_default();
        if game_slug != slug {
            return false;
        }
        campaign
            .get("allow")
            .and_then(|allow| allow.get("channels"))
            .and_then(Value::as_array)
            .map(|channels| !channels.is_empty())
            .unwrap_or(false)
    })
}

/// Канал из списка разрешённых, который сейчас реально в эфире.
fn allowed_channel_for(token: &str, slug: &str) -> Option<(String, String)> {
    let cache = read_json(profile_dir().join("catalog-cache.json"))?;
    let campaigns = cache.get("campaigns").and_then(Value::as_array)?;
    let mut candidates: Vec<(String, String)> = Vec::new();
    for campaign in campaigns {
        let game_slug = campaign
            .get("game")
            .and_then(|game| game.get("slug"))
            .map(|value| text_of(Some(value)))
            .unwrap_or_default();
        if game_slug != slug {
            continue;
        }
        let Some(channels) = campaign
            .get("allow")
            .and_then(|allow| allow.get("channels"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for channel in channels {
            let id = text_of(channel.get("id"));
            let name = text_of(channel.get("name"));
            if !id.is_empty() && !name.is_empty() && !candidates.iter().any(|(_, login)| login == &name) {
                candidates.push((id, name));
            }
        }
    }
    // проверяем живость пачками по 20: у Twitch нет массового фильтра по списку каналов
    for chunk in candidates.chunks(20) {
        let mut query = String::from("query {");
        for (index, (_, login)) in chunk.iter().enumerate() {
            query.push_str(&format!(" a{index}: user(login:\"{login}\") {{ stream {{ id }} }}",));
        }
        query.push_str(" }");
        let Some(response) = gql_raw(token, &query, json!({})) else { continue };
        let Some(data) = response.get("data") else { continue };
        for (index, (id, login)) in chunk.iter().enumerate() {
            let live = data
                .get(format!("a{index}"))
                .and_then(|user| user.get("stream"))
                .map(|stream| !stream.is_null())
                .unwrap_or(false);
            if live {
                return Some((id.clone(), login.clone()));
            }
        }
    }
    None
}

fn pick_channel(token: &str, slug: &str) -> Option<(String, String)> {
    // локальная кампания канала: смотрим именно этот канал
    if let Some(login) = slug.strip_prefix("local:") {
        let id = channel_id_by_login(login)?;
        return Some((id, login.to_string()));
    }
    // до 2 декабря у GTA VI приоритет — канал bratishkinoff, если он в эфире
    if slug == "grand-theft-auto-vi" {
        let deadline = chrono::DateTime::parse_from_rfc3339("2026-12-02T00:00:00Z")
            .map(|value| value.with_timezone(&chrono::Utc))
            .ok();
        let now = chrono::Utc::now();
        if deadline.map(|value| now < value).unwrap_or(false) {
            let query = "query($login:String!){user(login:$login){id stream{id}}}";
            if let Some(response) = gql_raw(token, query, json!({ "login": "bratishkinoff" })) {
                let user = response.get("data").and_then(|data| data.get("user"));
                let live = user
                    .and_then(|user| user.get("stream"))
                    .map(|stream| !stream.is_null())
                    .unwrap_or(false);
                let id = text_of(user.and_then(|user| user.get("id")));
                if live && !id.is_empty() {
                    return Some((id, String::from("bratishkinoff")));
                }
            }
        }
    }
    // кампания, ограниченная списком каналов: берём первый из разрешённых
    let restricted = has_allowed_channels(slug);
    if let Some(found) = allowed_channel_for(token, slug) {
        return Some(found);
    }
    // если список есть, но никто не в эфире — чужие каналы не помогут: прогресс не засчитают
    if restricted {
        return None;
    }
    // сначала ищем каналы с включёнными Drops, затем любые живые в категории:
    // у кампаний-наград (QUEST_REWARD) фильтр DROPS_ENABLED часто не выставлен
    for filters in [json!(["DROPS_ENABLED"]), json!([])] {
        let data = gql_value(
            token,
            "DirectoryPage_Game",
            "86bcceb4e8b1a51256ff8eed8bd8aae4acacf80d737efe904f84f3aeadf8cafd",
            json!({
                "limit": 30,
                "slug": slug,
                "imageWidth": 50,
                "includeCostreaming": false,
                "options": {
                    "broadcasterLanguages": [],
                    "freeformTags": Value::Null,
                    "includeRestricted": ["SUB_ONLY_LIVE"],
                    "recommendationsContext": { "platform": "web" },
                    "sort": "RELEVANCE",
                    "systemFilters": filters,
                    "tags": []
                },
                "sortTypeIsRecency": false
            }),
        );
        let Some(data) = data else { continue };
        let Some(edges) = data
            .get("game")
            .and_then(|game| game.get("streams"))
            .and_then(|streams| streams.get("edges"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        let mut channels: Vec<(i64, String, String)> = edges
            .iter()
            .filter_map(|edge| {
                let node = edge.get("node")?;
                let broadcaster = node.get("broadcaster")?;
                let id = text_of(broadcaster.get("id"));
                let login = text_of(broadcaster.get("login"));
                if id.is_empty() || login.is_empty() {
                    return None;
                }
                let viewers = node.get("viewersCount").and_then(Value::as_i64).unwrap_or(0);
                Some((viewers, id, login))
            })
            .collect();
        channels.sort_by_key(|(viewers, _, _)| *viewers);
        if let Some(found) = channels.first().map(|(_, id, login)| (id.clone(), login.clone())) {
            return Some(found);
        }
    }
    None
}

/// Следующая игра из очереди: перезапускаем вотчер, пока есть варианты.
fn advance_game(app: Option<tauri::AppHandle>) {
    std::thread::sleep(std::time::Duration::from_secs(5));
    let games = GAMES.lock().unwrap().clone();
    if games.len() <= 1 {
        eprintln!("rotation: одна игра в очереди — остаёмся");
        return;
    }
    let mut index = GAME_INDEX.lock().unwrap();
    *index = (*index + 1) % games.len();
    let slug = games[*index].clone();
    drop(index);
    push_log("info", format!("Переключаюсь на {slug}"));
    let result = start_miner(app, &slug);
    eprintln!("rotation: {result}");
}

fn start_miner(app: Option<tauri::AppHandle>, slug: &str) -> Value {
    let token = load_token().unwrap_or_default();
    let Some(exe) = watcher_exe() else {
        return json!({ "error": "watcher-svc не найден рядом с приложением" });
    };
    // локальная кампания: смотрим сам канал, а не ищем его по категории
    let channel = if let Some(local) = slug.strip_prefix("local:") {
        let login = local.to_string();
        match channel_id_by_login(&login) {
            Some(id) => Some((id, login)),
            None => None,
        }
    } else {
        pick_channel(&token, slug)
    };
    let Some((channel_id, login)) = channel else {
        let message = if has_allowed_channels(slug) {
            format!("нет живых каналов: у кампании {slug} свой список стримеров, сейчас никто не в эфире")
        } else {
            format!("нет живых каналов с Drops для {slug}")
        };
        push_log("warn", message.clone());
        return json!({ "error": message });
    };

    let mut guard = CHILD.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
    }
    let minutes = debug_env("DROPFORGE_WATCH_MINUTES").unwrap_or_else(|| "1440".to_string());
    let interval = debug_env("DROPFORGE_TICK_INTERVAL").unwrap_or_else(|| "60".to_string());
    let mut command = Command::new(&exe);
    command
        .args([
            "--channel",
            &login,
            "--channel-id",
            &channel_id,
            "--stdin-token",
            "--minutes",
            &minutes,
            "--interval",
            &interval,
            "--exit-on-stdin-close",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped());
    hide_console(&mut command);

    let is_local_campaign = slug.starts_with("local:");
    let watch_generation = {
        let mut generation = WATCH_GEN.lock().unwrap();
        *generation += 1;
        *generation
    };
    *LAST_TICK.lock().unwrap() = Some(std::time::Instant::now());
    match command.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            let slug_owned = slug.to_string();
            // токен уходит в stdin: в списке процессов его не видно
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                let _ = writeln!(stdin, "{token}");
            }
            if let Some(stdout) = child.stdout.take() {
                let handle = app.clone();
                let login_for_thread = login.clone();
                std::thread::spawn(move || {
                    use std::io::BufRead;
                    let reader = std::io::BufReader::new(stdout);
                    let tick_limit = debug_env("DROPFORGE_TICKS").and_then(|value| value.parse::<usize>().ok());
                    let mut ticks = 0usize;
                    for line in reader.lines().map_while(Result::ok) {
                        let Ok(event) = serde_json::from_str::<Value>(&line) else { continue };
                        if event.get("event").and_then(Value::as_str) != Some("tick") {
                            continue;
                        }
                        let minutes = event
                            .get("progress")
                            .and_then(|progress| progress.get("currentMinutesWatched"))
                            .and_then(Value::as_f64);
                        // прогресс: обновляем минуты и отдаём снимок в UI
                        record_watch_minute(&slug_owned, 1.0);
                        bump_gta_day(&slug_owned, 1.0);
                        if progress_refresh_due() {
                            let _ = refresh_progress();
                        }
                        let drop_id = event
                            .get("progress")
                            .and_then(|progress| progress.get("dropID"))
                            .map(|value| text_of(Some(value)))
                            .unwrap_or_default();
                        *LAST_TICK.lock().unwrap() = Some(std::time::Instant::now());
                        if let Some(minutes) = minutes {
                            record_watch_progress(&slug_owned, &drop_id, minutes);
                        }
                        if let Some(handle) = &handle {
                            let _ = handle.emit("state", snapshot_from_profile());
                        }
                        ticks += 1;
                        if handle.is_none() {
                            println!("tick: {line}");
                            if let Some(limit) = tick_limit {
                                if ticks >= limit {
                                    println!("ticks limit reached");
                                    std::process::exit(0);
                                }
                            }
                        }
                        if let Some(handle) = &handle {
                            let category = event
                                .get("progress")
                                .and_then(|progress| progress.get("game"))
                                .map(|game| text_of(Some(game)))
                                .unwrap_or_default();
                            let scope = if is_local_campaign { "локальная" } else { "глобальная" };
                            let _ = handle.emit("progress", json!({ "channel": login_for_thread, "game": category.clone(), "scope": scope }));
                            let category_label = if category.is_empty() {
                                String::from("категория неизвестна")
                            } else {
                                category.clone()
                            };
                            let message = match minutes {
                                Some(value) => format!("Смотрю {login_for_thread} · {category_label} · {scope} · {value:.0} мин"),
                                None => format!("Смотрю {login_for_thread} · {category_label} · {scope}"),
                            };
                            let _ = handle.emit("log", json!({ "at": "", "level": "info", "message": message }));
                            let _ = handle.emit("progress", json!({
                                "channel": login_for_thread,
                                "currentMinutes": minutes,
                                "dropId": event.get("progress").and_then(|progress| progress.get("dropID")),
                            }));
                        }
                    }
                    let stopping = *STOPPING.lock().unwrap();
                    let current_generation = *WATCH_GEN.lock().unwrap();
                    // если вотчер уже заменили (другая игра/канал) — это не «стрим закончился»
                    if !stopping && current_generation == watch_generation {
                        if let Some(handle) = &handle {
                            let _ = handle.emit("log", json!({ "at": "", "level": "info", "message": "Стрим закончился — беру следующую игру" }));
                        }
                        push_log("info", "Стрим закончился — беру следующую игру".to_string());
                        advance_game(handle.clone());
                    }
                });
            }
            *guard = Some(child);
            let scope = if is_local_campaign { "локальная кампания" } else { "глобальная кампания" };
            let message = format!("Смотрю {login} · {scope}");
            save_farm_state(true, slug, &login);
            *STATUS.lock().unwrap() = json!({
                "state": "watching",
                "running": true,
                "message": message.clone(),
                "channel": { "login": login.clone(), "url": format!("https://twitch.tv/{login}") },
                "game": slug,
            });
            push_log("info", message);
            if let Some(handle) = &app {
                let _ = handle.emit("state", snapshot_from_profile());
            }
            json!({ "ok": true, "pid": pid, "channel": login })
        }
        Err(error) => json!({ "error": format!("не удалось запустить watcher-svc: {error}") }),
    }
}

fn stop_miner() -> Value {
    save_farm_state(false, "", "");
    *STOPPING.lock().unwrap() = true;
    let mut guard = CHILD.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
    }
    *STATUS.lock().unwrap() = json!({
        "state": "idle",
        "running": false,
        "message": "Автофарм остановлен",
        "channel": Value::Null,
        "game": Value::Null,
    });
    push_log("info", "Автофарм остановлен".to_string());
    json!({ "ok": true })
}

fn push_log(level: &str, message: String) {
    let entry = json!({ "at": chrono::Utc::now().to_rfc3339(), "level": level, "message": message });
    let mut logs = LOGS.lock().unwrap();
    logs.push(entry);
    if logs.len() > 200 {
        let extra = logs.len() - 200;
        logs.drain(0..extra);
    }
}

/// Не даём запустить вторую копию: именованный мьютекс на процесс.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

/// Второй запуск: показываем уже работающее окно.
fn focus_existing_window() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        let title: Vec<u16> = "DropForge\0".encode_utf16().collect();
        let handle = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if !handle.is_null() {
            unsafe {
                ShowWindow(handle, SW_RESTORE);
                SetForegroundWindow(handle);
            }
        }
    }
}

fn single_instance() -> bool {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    let mut name: Vec<u16> = "Local\\DropForgeTauriSingleInstance".encode_utf16().collect();
    name.push(0);
    unsafe {
        let handle = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if handle.is_null() {
            return true;
        }
        GetLastError() == ERROR_ALREADY_EXISTS
    }
}

fn status_value() -> Value {
    let current = STATUS.lock().unwrap().clone();
    if current.is_null() {
        json!({ "state": "idle", "running": false, "message": "Готов к работе", "channel": null, "game": null })
    } else {
        current
    }
}

fn auth_state() -> Value {
    if let Some(cached) = AUTH_CACHE.lock().unwrap().clone() {
        return cached;
    }
    let state = read_json(auth_path()).unwrap_or(Value::Null);
    *AUTH_CACHE.lock().unwrap() = Some(state.clone());
    state
}

fn set_auth_state(state: Value) {
    *AUTH_CACHE.lock().unwrap() = Some(state);
}

fn snapshot_from_profile() -> Value {
    let profile = profile_dir();
    let cache = read_json(profile.join("catalog-cache.json")).unwrap_or(Value::Null);
    let auth = read_json(profile.join("auth.json")).unwrap_or(Value::Null);
    let settings = read_json(profile.join("settings.json")).unwrap_or(Value::Null);

    let progress = read_json(progress_path()).unwrap_or(Value::Null);
    let minutes = progress.get("drops").cloned().unwrap_or(Value::Null);

    let campaigns: Vec<Value> = cache
        .get("campaigns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|raw| {
            let mut view = campaign_view(raw);
            if let Some(drops) = view.get_mut("drops").and_then(Value::as_array_mut) {
                for drop in drops.iter_mut() {
                    let drop_id = text_of(drop.get("id"));
                    let Some(state) = minutes.get(&drop_id) else { continue };
                    let current = state.get("currentMinutes").and_then(Value::as_f64).unwrap_or(0.0);
                    let required = drop.get("requiredMinutes").and_then(Value::as_f64).unwrap_or(0.0);
                    let claimed = state.get("isClaimed").and_then(Value::as_bool).unwrap_or(false);
                    let progress_ratio = if required > 0.0 { (current / required).min(1.0) } else { 0.0 };
                    drop["currentMinutes"] = json!(current);
                    drop["remainingMinutes"] = json!((required - current).max(0.0));
                    drop["progress"] = json!(progress_ratio);
                    drop["isClaimed"] = json!(claimed);
                    drop["canClaim"] = json!(!claimed && current >= required && required > 0.0);
                }
            }
            view
        })
        .collect();

    let now = chrono::Utc::now();
    let campaigns: Vec<Value> = campaigns
        .into_iter()
        .map(|mut campaign| {
            let expired = campaign
                .get("endsAt")
                .and_then(Value::as_str)
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&chrono::Utc) < now)
                .unwrap_or(false);
            campaign["expired"] = json!(expired);
            campaign
        })
        .filter(|campaign| campaign.get("expired").and_then(Value::as_bool) != Some(true))
        .collect();

    let mut games: Map<String, Value> = Map::new();
    let mut drops_total = 0usize;
    for campaign in &campaigns {
        let slug = text_of(campaign.get("game").and_then(|game| game.get("slug")));
        if slug.is_empty() {
            continue;
        }
        let campaign_drops = campaign
            .get("drops")
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0);
        drops_total += campaign_drops;
        let entry = games.entry(slug.clone()).or_insert_with(|| {
            json!({
                "slug": slug,
                "name": campaign.get("game").and_then(|game| game.get("name")),
                "boxArtUrl": campaign.get("game").and_then(|game| game.get("boxArtUrl")),
                "campaignIds": [],
                "totalDrops": 0,
                "claimedDrops": 0,
                "endsAt": campaign.get("endsAt"),
                "active": true,
                "canEarn": true,
                "selected": false,
                "pinned": false,
                "farming": false,
                "rewardOnly": campaign.get("reward").and_then(Value::as_bool).unwrap_or(false),
            })
        });
        if let Some(ids) = entry.get_mut("campaignIds").and_then(Value::as_array_mut) {
            if let Some(id) = campaign.get("id").cloned() {
                ids.push(id);
            }
        }
        let total = entry.get("totalDrops").and_then(Value::as_u64).unwrap_or(0);
        entry["totalDrops"] = json!(total + campaign_drops as u64);
    }

    let selected: Vec<String> = settings
        .get("games")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|value| text_of(Some(value)))
        .collect();
    let pinned: Vec<String> = settings
        .get("pinned")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|value| text_of(Some(value)))
        .collect();
    // обложка: у кампаний из инвентаря её может не быть, берём у любой кампании той же игры
    let mut art_by_slug: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for campaign in &campaigns {
        let slug = text_of(campaign.get("game").and_then(|game| game.get("slug")));
        let art = text_of(campaign.get("game").and_then(|game| game.get("boxArtUrl")));
        if !slug.is_empty() && !art.is_empty() {
            art_by_slug.entry(slug).or_insert(art);
        }
    }
    let games: Vec<Value> = games
        .into_values()
        .map(|mut game| {
            let slug = text_of(game.get("slug"));
            let chosen = selected.iter().any(|item| item == &slug);
            let starred = pinned.iter().any(|item| item == &slug);
            game["selected"] = json!(chosen);
            game["pinned"] = json!(starred);
            if text_of(game.get("boxArtUrl")).is_empty() {
                if let Some(art) = art_by_slug.get(&slug) {
                    game["boxArtUrl"] = json!(art);
                }
            }
            game
        })
        .collect();

    let tauri_auth = auth_state();
    let mut stats = (0usize, 0usize, 0f64);
    for campaign in &campaigns {
        for drop in campaign.get("drops").and_then(Value::as_array).cloned().unwrap_or_default() {
            let claimed = drop.get("isClaimed").and_then(Value::as_bool).unwrap_or(false);
            let current = drop.get("currentMinutes").and_then(Value::as_f64).unwrap_or(0.0);
            stats.2 += current;
            if claimed {
                stats.0 += 1;
            }
            if drop.get("canClaim").and_then(Value::as_bool).unwrap_or(false) {
                stats.1 += 1;
            }
        }
    }

    // какая кампания и игра сейчас фармятся
    let current = STATUS.lock().unwrap().clone();
    let running = current.get("running").and_then(Value::as_bool).unwrap_or(false);
    let farming_slug = if running {
        text_of(current.get("game"))
    } else {
        String::new()
    };
    let mut newest: std::collections::HashMap<String, (chrono::DateTime<chrono::Utc>, String)> =
        std::collections::HashMap::new();
    for campaign in &campaigns {
        let slug = text_of(campaign.get("game").and_then(|game| game.get("slug")));
        if slug.is_empty() {
            continue;
        }
        let start = campaign
            .get("startsAt")
            .and_then(Value::as_str)
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&chrono::Utc))
            .unwrap_or(now);
        let id = text_of(campaign.get("id"));
        let entry = newest.entry(slug).or_insert((start, String::new()));
        if start >= entry.0 {
            *entry = (start, id);
        }
    }

    let campaigns: Vec<Value> = campaigns
        .into_iter()
        .map(|mut campaign| {
            let slug = text_of(campaign.get("game").and_then(|game| game.get("slug")));
            let id = text_of(campaign.get("id"));
            let is_newest = newest.get(&slug).map(|entry| entry.1 == id).unwrap_or(false);
            if !farming_slug.is_empty() && slug == farming_slug && is_newest {
                campaign["farming"] = json!(true);
                campaign["selected"] = json!(true);
            }
            campaign
        })
        .collect();
    let games: Vec<Value> = games
        .into_iter()
        .map(|mut game| {
            let slug = text_of(game.get("slug"));
            if !farming_slug.is_empty() && slug == farming_slug {
                game["farming"] = json!(true);
            }
            game
        })
        .collect();

    // бейджи профиля: дропсы с наградой типа BADGE
    let claimed_ids: std::collections::HashSet<String> = progress
        .get("claimedBenefits")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|value| text_of(Some(value))).collect())
        .unwrap_or_default();
    let owned_titles: std::collections::HashSet<String> = progress
        .get("availableBadges")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|item| text_of(item.get("title"))).collect())
        .unwrap_or_default();
    let earned_titles: std::collections::HashSet<String> = progress
        .get("earnedRewards")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|item| text_of(item.get("name"))).collect())
        .unwrap_or_default();
    let mut badges: Vec<Value> = Vec::new();
    for campaign in &campaigns {
        let campaign_name = text_of(campaign.get("name"));
        let campaign_id = text_of(campaign.get("id"));
        let game_slug = text_of(campaign.get("game").and_then(|game| game.get("slug")));
        let game_name = text_of(campaign.get("game").and_then(|game| game.get("name")));
        let campaign_url = text_of(campaign.get("webUrl"));
        let campaign_has_paid = campaign
            .get("drops")
            .and_then(Value::as_array)
            .map(|drops| {
                drops.iter().any(|drop| {
                    let subs = drop.get("requiredSubs").and_then(Value::as_f64).unwrap_or(0.0);
                    let minutes = drop.get("requiredMinutes").and_then(Value::as_f64).unwrap_or(0.0);
                    subs > 0.0 || minutes <= 0.0
                })
            })
            .unwrap_or(false);
        for drop in campaign.get("drops").and_then(Value::as_array).cloned().unwrap_or_default() {
            let minutes = drop.get("requiredMinutes").and_then(Value::as_f64).unwrap_or(0.0);
            let subs = drop.get("requiredSubs").and_then(Value::as_f64).unwrap_or(0.0);
            for benefit in drop.get("benefits").and_then(Value::as_array).cloned().unwrap_or_default() {
                let is_badge_type = text_of(benefit.get("type")) == "BADGE";
                let badge_flag = benefit.get("badge").and_then(Value::as_bool).unwrap_or(false);
                if !is_badge_type && !badge_flag {
                    continue;
                }
                let benefit_id = text_of(benefit.get("id"));
                let badge_name = text_of(benefit.get("name"));
                if badge_name.to_lowercase().contains("iguana") {
                    continue;
                }
                // этот бейдж сейчас не выдаётся — не показываем мёртвую кнопку «Фармить»
                if badge_name.to_lowercase().contains("themed poke ball") {
                    continue;
                }
                let claimed = claimed_ids.contains(&benefit_id) || owned_titles.contains(&badge_name);
                let paid = subs > 0.0 || minutes <= 0.0;
                let note = if subs > 0.0 {
                    "нужна подписка: оформляется на Twitch"
                } else if minutes <= 0.0 {
                    "награда за подписку или покупку"
                } else if campaign_has_paid {
                    "финальная награда кампании: нужны все награды, включая платные"
                } else {
                    ""
                };
                let prereq = evolution_prereq(&badge_name);
                let evolution_ready = prereq
                    .map(|required| {
                        owned_titles.contains(required) || earned_titles.contains(required)
                    })
                    .unwrap_or(false);
                let evolution_note = match prereq {
                    Some(required) if evolution_ready => {
                        format!("эволюция: {required} уже есть в профиле — можно получать")
                    }
                    Some(required) => format!("эволюция: нужен {required} — в профиле его нет"),
                    None => String::new(),
                };
                let note = if evolution_note.is_empty() { note.to_string() } else { evolution_note };
                badges.push(json!({
                    "id": benefit_id,
                    "name": benefit.get("name"),
                    "evolution": prereq.is_some(),
                    "evolutionRequires": prereq.unwrap_or_default(),
                    "evolutionReady": evolution_ready,
                    "imageUrl": benefit.get("imageUrl"),
                    "campaignId": campaign_id,
                    "campaign": campaign_name,
                    "gameSlug": game_slug,
                    "game": game_name,
                    "requiredMinutes": minutes,
                    "requiredSubs": subs,
                    "paid": paid,
                    "note": note,
                    "webUrl": campaign_url,
                    "claimed": claimed,
                    "endsAt": campaign.get("endsAt"),
                    "local": campaign.get("local").and_then(Value::as_bool).unwrap_or(false),
                }));
            }
        }
    }
    // коллекции наградных кампаний: сундуки за просмотр и подписки
    let earned_list = progress.get("earnedRewards").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut collections: Vec<Value> = Vec::new();
    for campaign in &campaigns {
        let campaign_id = text_of(campaign.get("id"));
        let campaign_earned_names: Vec<String> = earned_list
            .iter()
            .filter(|entry| text_of(entry.get("campaignId")) == campaign_id)
            .map(|entry| text_of(entry.get("name")))
            .collect();
        let watch_ids: Vec<String> = campaign
            .get("watchRewardIds")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(|item| text_of(Some(item))).collect())
            .unwrap_or_default();
        let sub_ids: Vec<String> = campaign
            .get("subRewardIds")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(|item| text_of(Some(item))).collect())
            .unwrap_or_default();
        if watch_ids.is_empty() && sub_ids.is_empty() {
            continue;
        }
        let mut chests = 0usize;
        let mut subs = 0usize;
        let mut final_earned = false;
        for entry in &earned_list {
            if text_of(entry.get("campaignId")) != campaign_id {
                continue;
            }
            let item_id = text_of(entry.get("itemId"));
            if watch_ids.iter().any(|id| id == &item_id) {
                chests += 1;
            }
            if sub_ids.iter().any(|id| id == &item_id) {
                subs += 1;
            }
            if text_of(entry.get("name")).to_lowercase().contains("iguana") {
                final_earned = true;
            }
        }
        let campaign_name = text_of(campaign.get("name"));
        let is_gta = campaign_name.to_lowercase().contains("gtavi");
        let available: Vec<Value> = progress
            .get("availableBadges")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if is_gta {
            // у наградных «сундуков» в earned лежат их содержимые, поэтому считаем по именам
            let chain_names = ["Purple Chain", "Bronze Chain", "Silver Chain", "Gold Chain", "Diamond Chain", "Miami Chain"];
            let sub_names = ["Welcome 2 Miami", "Scored Big", "Bleed Miami", "BustedPlz", "DinoBusted", "SabaBusted"];
            let mut chain_flags: Vec<bool> = chain_names.iter().map(|_| false).collect();
            let mut sub_count = 0usize;
            for entry in &earned_list {
                if text_of(entry.get("campaignId")) != campaign_id {
                    continue;
                }
                let name = text_of(entry.get("name"));
                if let Some(index) = chain_names.iter().position(|chain| *chain == name) {
                    chain_flags[index] = true;
                }
                if sub_names.iter().any(|sub| *sub == name) {
                    sub_count += 1;
                }
            }
            // точные данные из профиля: setID цепочек
            let chain_sets = ["purple-chain", "bronze-chain", "silver-chain", "gold-chain", "diamond-chain", "miami-chain"];
            let owned: Vec<&Value> = available
                .iter()
                .filter(|badge| {
                    let set = text_of(badge.get("setID"));
                    chain_sets.iter().any(|known| *known == set)
                })
                .collect();
            if !owned.is_empty() {
                for index in 0..chain_names.len() {
                    chain_flags[index] = owned.iter().any(|badge| text_of(badge.get("setID")) == chain_sets[index]);
                }
            }
            chests = chain_flags.iter().filter(|value| **value).count();
            subs = sub_count;
            let game_slug = text_of(campaign.get("game").and_then(|game| game.get("slug")));
            let today = chrono::Local::now().format("%Y-%m-%d").to_string();
            let watch_days = load_settings();
            let bucket = watch_days
                .get("watchDays")
                .and_then(|days| days.get(&game_slug))
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            let today_minutes = bucket.get(&today).and_then(Value::as_f64).unwrap_or(0.0);
            let days_done = bucket.values().filter(|value| value.as_f64().unwrap_or(0.0) >= 60.0).count();
            let mut settings_snapshot = watch_days.clone();
            let manual_closed = settings_snapshot
                .get("closedDays")
                .and_then(|days| days.get(&game_slug))
                .map(|value| text_of(Some(value)) == today)
                .unwrap_or(false);
            let seen_before = settings_snapshot
                .get("chainsSeen")
                .and_then(|days| days.get(&game_slug))
                .and_then(Value::as_f64)
                .unwrap_or(0.0) as usize;
            let day_closed = manual_closed || today_minutes >= 60.0 || chests > seen_before;
            if let Some(root) = settings_snapshot.as_object_mut() {
                let seen = root.entry("chainsSeen").or_insert_with(|| json!({}));
                if let Some(map) = seen.as_object_mut() {
                    map.insert(game_slug.clone(), json!(chests));
                }
                let _ = save_settings(&settings_snapshot);
            }
            // Twitch не отдаёт цепочки в API — разрешаем отметить вручную
            let manual = load_settings()
                .get("gtaChains")
                .and_then(Value::as_f64)
                .unwrap_or(0.0)
                .clamp(0.0, 6.0) as usize;
            if manual > chests {
                chests = manual;
                for index in 0..chain_flags.len() {
                    chain_flags[index] = index < manual;
                }
            }
            let mut chains: Vec<Value> = Vec::new();
            for (index, name) in chain_names.iter().enumerate() {
                let image = available
                    .iter()
                    .find(|badge| text_of(badge.get("setID")) == chain_sets[index])
                    .and_then(|badge| badge.get("imageURL"))
                    .cloned()
                    .unwrap_or(Value::Null);
                chains.push(json!({ "name": name, "earned": chain_flags[index], "imageUrl": image }));
            }
            collections.push(json!({
                "campaignId": campaign_id,
                "campaign": campaign_name,
                "gameSlug": campaign.get("game").and_then(|game| game.get("slug")),
                "chests": chests,
                "chestsTotal": 6,
                "subs": subs,
                "subsTotal": 6,
                "finalEarned": final_earned,
                "chains": chains,
                "earnedNames": campaign_earned_names,
                "todayMinutes": today_minutes,
                "daysDone": days_done,
                "dayClosed": day_closed,
                "gtaDay": watch_days.get("gtaDay").cloned().unwrap_or(Value::Null),
            }));
            continue;
        }
        let mut chains: Vec<Value> = Vec::new();
        let _ = &mut chains;
        collections.push(json!({
            "campaignId": campaign_id,
            "campaign": campaign_name,
            "gameSlug": campaign.get("game").and_then(|game| game.get("slug")),
            "chests": chests,
            "chestsTotal": watch_ids.len().min(6),
            "subs": subs,
            "subsTotal": sub_ids.len().min(6),
            "finalEarned": final_earned,
            "chains": chains,
            "earnedNames": campaign_earned_names,
        }));
    }
    let logged_in = auth.get("token").is_some() || tauri_auth.get("token").is_some();
    let login = if tauri_auth.get("login").is_some() {
        text_of(tauri_auth.get("login"))
    } else {
        text_of(auth.get("login"))
    };

    json!({
        "ready": true,
        "auth": {
            "loggedIn": logged_in,
            "user": if logged_in {
                json!({
                    "id": tauri_auth.get("userId").or_else(|| auth.get("userId")),
                    "login": login,
                    "displayName": login,
                    "avatarUrl": tauri_auth.get("avatarUrl").or_else(|| auth.get("avatarUrl")).cloned().unwrap_or(Value::Null)
                })
            } else {
                Value::Null
            },
            "profile": auth.get("profileKey"),
            "tokenPreview": Value::Null,
        },
        "status": status_value(),
        "pubsub": "offline",
        "lastRefresh": cache.get("at"),
        "settings": {
            "games": settings.get("games").cloned().unwrap_or(json!([])),
            "pinned": settings.get("pinned").cloned().unwrap_or(json!([])),
            "minimizeToTray": settings.get("minimizeToTray").cloned().unwrap_or(json!(true)),
            "sendSpadeEvents": settings.get("sendSpadeEvents").cloned().unwrap_or(json!(true)),
            "autoClaim": settings.get("autoClaim").cloned().unwrap_or(json!(true)),
            "refreshMinutes": settings.get("refreshMinutes").cloned().unwrap_or(json!(10)),
            "priorityMode": settings.get("priorityMode").cloned().unwrap_or(json!("ending-soonest")),
            "localChannel": settings.get("localChannel").cloned().unwrap_or(json!("")),
            "localCheck": settings.get("localCheck").cloned().unwrap_or(Value::Null),
            "dropsTab": settings.get("dropsTab").cloned().unwrap_or(json!("global")),
            "resumeMinutes": settings.get("resumeMinutes").cloned().unwrap_or(json!(15)),
            "autoBadges": settings.get("autoBadges").cloned().unwrap_or(json!(true)),
        },
        "stats": {
            "games": games.len(),
            "campaigns": campaigns.len(),
            "drops": drops_total,
            "claimed": stats.0,
            "claimable": stats.1,
            "minutesEarned": stats.2,
        },
        "games": games,
        "campaigns": campaigns,
        "logs": LOGS.lock().unwrap().clone(),
        "badges": badges,
        "collections": collections,
        "watch": progress.get("watch").cloned().unwrap_or(Value::Null),
        "legacy": legacy_state(),
        "device": DEVICE.lock().unwrap().clone(),
        "catalog": Value::Null,
        "demo": false,
        "platform": std::env::consts::OS,
    })
}

/// Минуты из тика вотчера: кампании может не быть в каталоге, но прогресс показать надо.
fn record_watch_progress(slug: &str, drop_id: &str, minutes: f64) {
    if slug.is_empty() || drop_id.is_empty() {
        return;
    }
    let mut progress = read_json(progress_path()).unwrap_or_else(|| json!({}));
    let Some(root) = progress.as_object_mut() else { return };
    let at = chrono::Utc::now().to_rfc3339();
    root.insert(
        "watch".into(),
        json!({ "game": slug, "dropId": drop_id, "currentMinutes": minutes, "at": at }),
    );
    let drops = root.entry("drops").or_insert_with(|| json!({}));
    if let Some(map) = drops.as_object_mut() {
        // у дропа уже может быть настоящая кампания из инвентаря — её не теряем
        let existing = map.get(drop_id).cloned();
        let campaign_id = existing
            .as_ref()
            .and_then(|value| value.get("campaignId").cloned())
            .unwrap_or(Value::Null);
        let required = existing
            .as_ref()
            .and_then(|value| value.get("requiredMinutes").cloned())
            .unwrap_or(Value::Null);
        let claimed = existing
            .as_ref()
            .and_then(|value| value.get("isClaimed"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        map.insert(
            drop_id.to_string(),
            json!({
                "campaignId": campaign_id,
                "currentMinutes": minutes,
                "requiredMinutes": required,
                "isClaimed": claimed,
                "game": slug,
            }),
        );
    }
    if let Ok(text) = serde_json::to_string(&progress) {
        let _ = std::fs::write(progress_path(), text);
    }
}

fn farm_state_path() -> PathBuf {
    profile_dir().join("farm-state.json")
}

fn save_farm_state(running: bool, game: &str, channel: &str) {
    let payload = json!({
        "running": running,
        "game": game,
        "channel": channel,
        "at": chrono::Utc::now().to_rfc3339(),
    });
    let _ = std::fs::write(farm_state_path(), serde_json::to_string(&payload).unwrap_or_default());
}

fn settings_path() -> PathBuf {
    profile_dir().join("settings.json")
}

fn load_settings() -> Value {
    read_json(settings_path()).unwrap_or_else(|| {
        json!({
            "games": [],
            "pinned": [],
            "localChannels": [],
            "minimizeToTray": true,
            "sendSpadeEvents": true,
            "autoClaim": true,
            "autoBadges": true,
            "refreshMinutes": 10,
            "priorityMode": "ending-soonest",
            "resumeMinutes": 15
        })
    })
}

fn save_settings(value: &Value) -> Result<(), String> {
    std::fs::create_dir_all(profile_dir()).ok();
    std::fs::write(
        settings_path(),
        serde_json::to_string_pretty(value).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

/// Сколько минут в какой день смотрели категорию — для оценки следующей цепочки.
/// Сессия «дня» GTA: стартует вручную, копит 60 минут и закрывается.
fn bump_gta_day(slug: &str, minutes: f64) {
    if slug != "grand-theft-auto-vi" && slug != "local:bratishkinoff" {
        return;
    }
    let mut settings = load_settings();
    let Some(root) = settings.as_object_mut() else { return };
    let day = root.entry("gtaDay").or_insert_with(|| json!({}));
    let Some(day) = day.as_object_mut() else { return };
    if day.get("startedAt").and_then(Value::as_str).is_none() {
        return;
    }
    if day.get("completedAt").and_then(Value::as_str).is_some() {
        return;
    }
    let watched = day.get("watched").and_then(Value::as_f64).unwrap_or(0.0) + minutes;
    day.insert("watched".into(), json!(watched.min(600.0)));
    if watched >= 60.0 {
        day.insert("completedAt".into(), json!(chrono::Utc::now().to_rfc3339()));
    }
    let _ = save_settings(&settings);
}

fn record_watch_minute(slug: &str, minutes: f64) {
    if slug.is_empty() {
        return;
    }
    let mut settings = load_settings();
    let date = chrono::Local::now().format("%Y-%m-%d").to_string();
    let Some(root) = settings.as_object_mut() else { return };
    let days = root
        .entry("watchDays")
        .or_insert_with(|| json!({}))
        .as_object_mut();
    let Some(days) = days else { return };
    let entry = days.entry(slug.to_string()).or_insert_with(|| json!({}));
    let Some(bucket) = entry.as_object_mut() else { return };
    let current = bucket.get(&date).and_then(Value::as_f64).unwrap_or(0.0);
    bucket.insert(date, json!((current + minutes).min(600.0)));
    let _ = save_settings(&settings);
}

fn settings_games() -> Vec<String> {
    load_settings()
        .get("games")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|value| text_of(Some(value))).collect())
        .unwrap_or_default()
}

#[tauri::command]
fn settings_save(app: tauri::AppHandle, patch: Value) -> Value {
    let mut settings = load_settings();
    let refresh_changed = patch.get("refreshMinutes").is_some();
    if let (Some(target), Some(source)) = (settings.as_object_mut(), patch.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    match save_settings(&settings) {
        Ok(()) => {
            if refresh_changed {
                let handle = app.clone();
                std::thread::spawn(move || {
                    let _ = refresh_catalog();
                    let _ = handle.emit("state", snapshot_from_profile());
                });
            }
            snapshot_from_profile()
        }
        Err(error) => json!({ "error": error }),
    }
}

#[tauri::command]
fn games_toggle(slug: String) -> Value {
    let mut settings = load_settings();
    let mut games = settings_games();
    if let Some(position) = games.iter().position(|item| item == &slug) {
        games.remove(position);
    } else {
        games.push(slug);
    }
    settings["games"] = json!(games);
    let _ = save_settings(&settings);
    snapshot_from_profile()
}

#[tauri::command]
fn games_pin(slug: String) -> Value {
    let mut settings = load_settings();
    let mut pinned: Vec<String> = settings
        .get("pinned")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|value| text_of(Some(value))).collect())
        .unwrap_or_default();
    if let Some(position) = pinned.iter().position(|item| item == &slug) {
        pinned.remove(position);
    } else {
        pinned.push(slug);
    }
    settings["pinned"] = json!(pinned);
    let _ = save_settings(&settings);
    snapshot_from_profile()
}

#[tauri::command]
fn games_select_only(slugs: Vec<String>) -> Value {
    let mut settings = load_settings();
    settings["games"] = json!(slugs);
    let _ = save_settings(&settings);
    snapshot_from_profile()
}

#[tauri::command]
fn farm_game(slug: String) -> Value {
    let mut settings = load_settings();
    let games = settings_games();
    if !games.iter().any(|item| item == &slug) {
        settings["games"] = json!([slug]);
        let _ = save_settings(&settings);
    }
    *STOPPING.lock().unwrap() = false;
    *GAMES.lock().unwrap() = vec![slug.clone()];
    *GAME_INDEX.lock().unwrap() = 0;
    let _ = start_miner(None, &slug);
    snapshot_from_profile()
}

#[tauri::command]
fn farm_clear() -> Value {
    let mut settings = load_settings();
    settings["games"] = json!([]);
    let _ = save_settings(&settings);
    *GAMES.lock().unwrap() = Vec::new();
    *GAME_INDEX.lock().unwrap() = 0;
    let _ = stop_miner();
    snapshot_from_profile()
}

#[tauri::command]
fn farm_skip() -> Value {
    advance_game(None);
    snapshot_from_profile()
}

#[tauri::command]
fn auth_login_browser() -> Result<Value, String> {
    Err("В Tauri-сборке используйте вход по коду устройства".into())
}

#[tauri::command]
fn app_init() -> Value {
    snapshot_from_profile()
}

/// Проверить канал: существует ли он и есть ли у него сейчас локальные дропсы.
#[tauri::command]
fn local_check_channel(login: String) -> Value {
    let login = login.trim().trim_start_matches('@').to_lowercase();
    if login.is_empty() {
        return json!({ "error": "Укажите логин канала" });
    }
    if channel_id_by_login(&login).is_none() {
        return json!({ "error": format!("Канал {login} не найден") });
    }
    let mut settings = load_settings();
    settings["localChannel"] = json!(login);
    let _ = save_settings(&settings);
    let token = load_token().unwrap_or_default();
    let found = if token.is_empty() {
        0
    } else {
        refresh_local_into_cache(&token)
    };
    settings = load_settings();
    settings["localCheck"] = json!({
        "login": login,
        "at": chrono::Utc::now().to_rfc3339(),
        "found": found,
    });
    let _ = save_settings(&settings);
    snapshot_from_profile()
}

#[tauri::command]
fn local_clear_channel() -> Value {
    let mut settings = load_settings();
    settings["localChannel"] = json!("");
    settings["localCheck"] = Value::Null;
    let _ = save_settings(&settings);
    if let Some(token) = load_token() {
        refresh_local_into_cache(&token);
    }
    snapshot_from_profile()
}

const HASH_DETAILS: &str = "039277bf98f3130929262cc7c6efd9c141ca3749cb6dca442fc8ead9a53f77c1";
const HASH_CLAIM: &str = "a455deea71bdc9015b78eb49f4acfbce8baa7ccbedd28e549bb025bd0f751930";

/// Забирает все готовые награды: идёт по кампаниям из инвентаря и их деталям.
fn claim_all() -> Value {
    let Some(token) = load_token() else {
        return json!({ "error": "нужен вход в Twitch" });
    };

    let mut claimed = 0usize;
    let mut failed = 0usize;
    let mut seen = 0usize;

    for campaign in inventory_campaigns(&token) {
        for drop in campaign
            .get("timeBasedDrops")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let self_state = drop.get("self").cloned().unwrap_or(Value::Null);
            let instance = text_of(self_state.get("dropInstanceID"));
            if instance.is_empty() {
                continue;
            }
            if self_state.get("isClaimed").and_then(Value::as_bool).unwrap_or(false) {
                continue;
            }
            let required = drop.get("requiredMinutesWatched").and_then(Value::as_f64).unwrap_or(0.0);
            let watched = self_state.get("currentMinutesWatched").and_then(Value::as_f64).unwrap_or(0.0);
            if watched < required || required <= 0.0 {
                continue;
            }
            seen += 1;
            let response = gql_value(
                &token,
                "DropsPage_ClaimDropRewards",
                HASH_CLAIM,
                json!({ "input": { "dropInstanceID": instance } }),
            );
            let status = response
                .and_then(|data| data.get("claimDropRewards").cloned())
                .and_then(|value| value.get("status").cloned())
                .map(|value| text_of(Some(&value)))
                .unwrap_or_default();
            if status == "ELIGIBLE_FOR_ALL" || status == "DROP_INSTANCE_ALREADY_CLAIMED" {
                claimed += 1;
            } else {
                failed += 1;
            }
        }
    }

    json!({ "ok": true, "claimable": seen, "claimed": claimed, "failed": failed })
}
#[tauri::command]
fn drops_progress() -> Value {
    match refresh_progress() {
        Ok(payload) => {
            let drops = payload.get("drops").and_then(Value::as_object).map(|map| map.len()).unwrap_or(0);
            push_log("info", format!("Прогресс обновлён: {drops} дропов"));
        }
        Err(error) => push_log("warn", format!("Прогресс недоступен: {error}")),
    }
    snapshot_from_profile()
}

#[tauri::command]
fn drops_claim_all(app: tauri::AppHandle) -> Value {
    let result = claim_all();
    push_log("info", format!("Клейм: {result}"));
    let snapshot = snapshot_from_profile();
    let _ = app.emit("state", snapshot.clone());
    snapshot
}


#[tauri::command]
fn farm_start(app: tauri::AppHandle) -> Value {
    let settings = read_json(profile_dir().join("settings.json")).unwrap_or(Value::Null);
    let games: Vec<String> = settings
        .get("games")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|value| text_of(Some(value))).collect())
        .unwrap_or_default();
    let slug = games.first().cloned().unwrap_or_default();
    if slug.is_empty() {
        push_log("warn", "Очередь пуста: добавьте игру кнопкой «+» на карточке".to_string());
        *STATUS.lock().unwrap() = json!({
            "state": "idle",
            "running": false,
            "message": "Очередь пуста: добавьте игру кнопкой «+»",
            "channel": Value::Null,
            "game": Value::Null,
        });
        let snapshot = snapshot_from_profile();
        let _ = app.emit("state", snapshot.clone());
        return snapshot;
    }
    *STOPPING.lock().unwrap() = false;
    *GAMES.lock().unwrap() = games;
    *GAME_INDEX.lock().unwrap() = 0;
    let result = start_miner(Some(app.clone()), &slug);
    if let Some(error) = result.get("error") {
        push_log("warn", format!("Автофарм не запущен: {error}"));
    }
    let snapshot = snapshot_from_profile();
    let _ = app.emit("state", snapshot.clone());
    snapshot
}

#[tauri::command]
fn farm_stop(app: tauri::AppHandle) -> Value {
    let _ = stop_miner();
    let snapshot = snapshot_from_profile();
    let _ = app.emit("state", snapshot.clone());
    snapshot
}

#[tauri::command]
async fn auth_login_device(app: tauri::AppHandle) -> Result<Value, String> {
    let started = device_start()?;
    *DEVICE.lock().unwrap() = json!({ "userCode": started.get("userCode"), "uri": started.get("uri") });
    open_external(text_of(started.get("uri")));
    let _ = app.emit("device-code", started.clone());
    let _ = app.emit("state", snapshot_from_profile());
    let token = tauri::async_runtime::spawn_blocking(move || device_poll(started))
        .await
        .map_err(|error| format!("поток входа: {error}"))??;
    *DEVICE.lock().unwrap() = Value::Null;
    // Полученный токен нужно сохранить, иначе вход «успешен», а профиль пуст.
    match save_token(token.trim()) {
        Some(_) => Ok(snapshot_from_profile()),
        None => Err("Twitch не принял токен устройства".into()),
    }
}

#[tauri::command]
fn auth_login_token(token: String) -> Result<Value, String> {
    match save_token(token.trim()) {
        Some(_) => Ok(snapshot_from_profile()),
        None => Err("Токен не принят Twitch".into()),
    }
}

fn exports_path() -> PathBuf {
    profile_dir().join("exports.json")
}

fn register_export(path: &std::path::Path) {
    let mut list = read_json(exports_path())
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    list.push(json!(path.display().to_string()));
    let _ = std::fs::write(exports_path(), serde_json::to_string(&Value::Array(list)).unwrap_or_default());
}

/// Удаляем отчёты, которые приложение сохраняло в «Загрузки».
fn cleanup_exports() {
    if let Some(items) = read_json(exports_path()).and_then(|value| value.as_array().cloned()) {
        for item in items {
            let path = std::path::PathBuf::from(text_of(Some(&item)));
            if path.is_file() {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    // отчёты старых версий, созданные без реестра
    if let Some(home) = std::env::var_os("USERPROFILE") {
        let downloads = std::path::PathBuf::from(home).join("Downloads");
        if let Ok(entries) = std::fs::read_dir(&downloads) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("DropForge-report-") && name.ends_with(".txt") {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }
}

/// Полный сброс профиля: как после чистой установки.
fn reset_profile() {
    cleanup_exports();
    let profile = profile_dir();
    for name in [
        "tauri-auth.json",
        "auth.json",
        "progress.json",
        "catalog-cache.json",
        "settings.json",
        "farm-state.json",
        "snapshot.json",
        "exports.json",
    ] {
        let _ = std::fs::remove_file(profile.join(name));
    }
    let diagnostics = profile.join("diagnostics");
    if diagnostics.is_dir() {
        let _ = std::fs::remove_dir_all(&diagnostics);
    }
    let _ = std::fs::create_dir_all(&profile);
}

#[tauri::command]
fn auth_logout(app: tauri::AppHandle) -> Value {
    // останавливаем всё, что связано с фармом
    stop_miner();
    if let Some(state) = app.try_state::<MinerState>() {
        if let Some(mut child) = state.pubsub.lock().unwrap().take() {
            let _ = child.kill();
        }
    }
    *GAMES.lock().unwrap() = Vec::new();
    *GAME_INDEX.lock().unwrap() = 0;
    *STOPPING.lock().unwrap() = false;
    *STATUS.lock().unwrap() = json!({ "state": "idle", "running": false, "message": "Готов к работе", "channel": null, "game": null });
    LOGS.lock().unwrap().clear();
    set_auth_state(Value::Null);
    // удаляем данные пользователя: токен, прогресс, кэш, настройки
    reset_profile();
    push_log("info", "Выход: данные профиля очищены".to_string());
    let _ = app.emit("state", snapshot_from_profile());
    snapshot_from_profile()
}

#[tauri::command]
fn inventory_refresh(app: tauri::AppHandle) -> Value {
    let handle = app.clone();
    std::thread::spawn(move || {
        let _ = handle.emit("refresh", json!({ "active": true }));
        if let Err(error) = refresh_catalog() {
            eprintln!("refresh error: {error}");
        }
        let snapshot = snapshot_from_profile();
        let _ = handle.emit("refresh", json!({ "active": false }));
        let _ = handle.emit("state", snapshot);
    });
    snapshot_from_profile()
}

static LAST_PROGRESS_REFRESH: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

fn progress_refresh_due() -> bool {
    let mut last = LAST_PROGRESS_REFRESH.lock().unwrap();
    let due = last.map(|at| at.elapsed() >= std::time::Duration::from_secs(300)).unwrap_or(true);
    if due {
        *last = Some(std::time::Instant::now());
    }
    due
}

#[tauri::command]
fn app_snapshot_stamp() -> Value {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    for name in ["catalog-cache.json", "progress.json", "settings.json", "auth.json"] {
        if let Ok(meta) = std::fs::metadata(profile_dir().join(name)) {
            meta.len().hash(&mut hasher);
            if let Ok(modified) = meta.modified() {
                if let Ok(span) = modified.duration_since(std::time::UNIX_EPOCH) {
                    span.as_nanos().hash(&mut hasher);
                }
            }
        }
    }
    STATUS.lock().unwrap().to_string().hash(&mut hasher);
    CHILD.lock().unwrap().as_ref().map(|child| child.id()).hash(&mut hasher);
    json!({ "stamp": format!("{:x}", hasher.finish()) })
}

/// Отпечаток каталога: пока он не меняется, не гоняем 250 КБ кампаний в UI.
fn catalog_stamp() -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    if let Ok(meta) = std::fs::metadata(profile_dir().join("catalog-cache.json")) {
        meta.len().hash(&mut hasher);
        if let Ok(modified) = meta.modified() {
            if let Ok(span) = modified.duration_since(std::time::UNIX_EPOCH) {
                span.as_nanos().hash(&mut hasher);
            }
        }
    }
    format!("{:x}", hasher.finish())
}

fn snapshot_for_ui(since: Option<String>) -> Value {
    let mut snapshot = snapshot_from_profile();
    let stamp = catalog_stamp();
    let same_catalog = since.as_deref() == Some(stamp.as_str());
    if let Some(object) = snapshot.as_object_mut() {
        object.insert("catalogStamp".into(), json!(stamp));
        if same_catalog {
            if let Some(progress) = read_json(progress_path()) {
                if let Some(drops) = progress.get("drops") {
                    object.insert("progressDrops".into(), drops.clone());
                }
            }
            object.insert("campaigns".into(), json!([]));
        }
    }
    snapshot
}

#[tauri::command]
fn app_snapshot(since: Option<String>) -> Value {
    snapshot_for_ui(since)
}

#[tauri::command]
fn app_version() -> Value {
    json!({ "app": env!("CARGO_PKG_VERSION"), "electron": null })
}

#[tauri::command]
fn app_data_dir() -> Value {
    json!(profile_dir().to_string_lossy().to_string())
}

/// Установленная копия (её кладёт NSIS).
fn install_dir() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    let dir = std::path::PathBuf::from(base).join("DropForge");
    if dir.is_dir() { Some(dir) } else { None }
}

/// Текущая копия не совпадает с установленной — значит запущена старая.
fn legacy_state() -> Value {
    let current = std::env::current_exe().ok();
    let install = install_dir().and_then(|dir| {
        let exe = dir.join("dropforge-tauri.exe");
        if exe.is_file() { Some(exe) } else { None }
    });
    let current_text = current.as_ref().map(|path| path.display().to_string()).unwrap_or_default();
    let install_text = install.as_ref().map(|path| path.display().to_string()).unwrap_or_default();
    let stale = !install_text.is_empty() && current_text != install_text;
    json!({
        "currentExe": current_text,
        "installExe": install_text,
        "stale": stale,
    })
}

/// Запустить установленную копию и закрыть текущую.
#[tauri::command]
fn legacy_launch() -> Value {
    let Some(install) = install_dir().map(|dir| dir.join("dropforge-tauri.exe")) else {
        return json!({ "error": "установленная копия не найдена" });
    };
    if !install.is_file() {
        return json!({ "error": "установленная копия не найдена" });
    }
    let exe: Vec<u16> = format!("{}\0", install.display()).encode_utf16().collect();
    let dir = install.parent().map(|path| path.display().to_string()).unwrap_or_default();
    let workdir: Vec<u16> = format!("{dir}\0").encode_utf16().collect();
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let operation: Vec<u16> = "open\0".encode_utf16().collect();
        unsafe {
            ShellExecuteW(std::ptr::null_mut(), operation.as_ptr(), exe.as_ptr(), std::ptr::null(), workdir.as_ptr(), SW_SHOWNORMAL);
        }
        std::process::exit(0);
    }
    #[cfg(not(windows))]
    {
        json!({ "error": "только для Windows" })
    }
}

/// Убрать старые файлы рядом с запущенной копией и вторую установку в Programs.
#[cfg(windows)]
fn is_reparse_point(path: &std::path::Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(meta) => meta.file_attributes() & 0x400 != 0,
        Err(_) => true,
    }
}

#[cfg(not(windows))]
fn is_reparse_point(_path: &std::path::Path) -> bool {
    false
}

fn legacy_cleanup() {
    let current = std::env::current_exe().ok();
    let install = install_dir();
    if let (Some(current), Some(install)) = (current.as_ref(), install.as_ref()) {
        let running_installed = current.starts_with(install);
        if running_installed {
            if let Some(base) = std::env::var_os("LOCALAPPDATA") {
                let duplicate = std::path::PathBuf::from(base).join("Programs").join("DropForge");
                if duplicate.is_dir() && !duplicate.starts_with(install) && !is_reparse_point(&duplicate) {
                    let _ = std::fs::remove_dir_all(&duplicate);
                }
            }
        }
        if running_installed {
            let leftovers = install.join("_up_");
            if leftovers.is_dir() && !is_reparse_point(&leftovers) {
                let _ = std::fs::remove_dir_all(&leftovers);
            }
        }
        if let Some(dir) = current.parent() {
            let legacy = dir.join("DropForge.exe");
            if legacy.is_file() && legacy != *current {
                let _ = std::fs::remove_file(&legacy);
            }
        }
    }
}

/// Кнопки, которым нужен движок: честно сообщаем, что ещё не подключено.
/// Открывает ссылку в браузере без командной оболочки (никаких cmd/start).
fn open_url(url: &str) -> bool {
    if !url.starts_with("https://") {
        return false;
    }
    // управляющие символы и пробелы в URL не допускаем
    if url.chars().any(|ch| ch.is_control() || ch == ' ' || ch == '"') {
        return false;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::Shell::ShellExecuteW;
        use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let operation: Vec<u16> = "open\0".encode_utf16().collect();
        let target: Vec<u16> = format!("{url}\0").encode_utf16().collect();
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            )
        };
        (result as isize) > 32
    }
    #[cfg(not(windows))]
    {
        false
    }
}

fn open_external(url: String) {
    let _ = open_url(&url);
}
#[tauri::command]
fn shell_open(url: String) -> Value {
    let ok = open_url(&url);
    json!({ "ok": ok })
}

#[tauri::command]
fn clipboard_copy_text(text: String) -> Value {
    json!({ "ok": clipboard_copy(&text) })
}

#[tauri::command]
fn app_quit(app: tauri::AppHandle) {
    app.exit(0);
}
#[tauri::command]
fn not_wired() -> Value {
    json!({ "error": "ещё не подключено в Tauri-сборке" })
}

fn main() {
    if single_instance() {
        focus_existing_window();
        return;
    }

    // Отладка: сохранить токен через DPAPI перед запуском
    if let Some(token) = debug_env("DROPFORGE_TOKEN") {
        match save_token(token.trim()) {
            Some((login, user_id)) => println!("token saved for {login} ({user_id})"),
            None => println!("token rejected"),
        }
    }

    if cfg!(debug_assertions) && std::env::var("DROPFORGE_CMDS").is_ok() {
        println!("toggle: {:?}", games_toggle("escape-from-tarkov".into()).get("stats"));
        println!("toggle2: {:?}", games_toggle("albion-online".into()).get("stats"));
        println!("pin: {:?}", games_pin("albion-online".into()).get("stats"));
        println!("settings: {}", load_settings());
        println!("select_only: {:?}", games_select_only(vec!["albion-online".into()]).get("stats"));
        println!("settings after: {}", load_settings());
        return;
    }

    if cfg!(debug_assertions) && std::env::var("DROPFORGE_LOCAL_CHECK").is_ok() {
        let login = std::env::var("DROPFORGE_LOCAL_CHECK").unwrap_or_default();
        let snapshot = local_check_channel(login);
        let check = snapshot
            .get("settings")
            .and_then(|settings| settings.get("localCheck"))
            .cloned()
            .unwrap_or(Value::Null);
        println!("local-check: {check}");
        return;
    }

    if cfg!(debug_assertions) && std::env::var("DROPFORGE_DEVICE").is_ok() {
        match device_start() {
            Ok(value) => println!("device: {}", value),
            Err(error) => println!("device error: {error}"),
        }
        return;
    }

    if cfg!(debug_assertions) && std::env::var("DROPFORGE_CLAIM").is_ok() {
        println!("claim: {}", claim_all());
        return;
    }

    if cfg!(debug_assertions) && std::env::var("DROPFORGE_FARM").is_ok() {
        let list = std::env::var("DROPFORGE_FARM").unwrap_or_default();
        let games: Vec<String> = list.split(',').map(|item| item.trim().to_string()).filter(|item| !item.is_empty()).collect();
        let slug = games.first().cloned().unwrap_or_default();
        *STOPPING.lock().unwrap() = false;
        *GAMES.lock().unwrap() = games;
        *GAME_INDEX.lock().unwrap() = 0;
        let result = start_miner(None, &slug);
        println!("farm: {result}");
        std::thread::sleep(std::time::Duration::from_secs(150));
        stop_miner();
        return;
    }

    if cfg!(debug_assertions) && std::env::var("DROPFORGE_REFRESH").is_ok() {
        match refresh_catalog() {
            Ok(cache) => {
                let count = cache
                    .get("campaigns")
                    .and_then(Value::as_array)
                    .map(|items| items.len())
                    .unwrap_or(0);
                println!("refreshed: {count}");
            }
            Err(error) => println!("refresh error: {error}"),
        }
        return;
    }

    if cfg!(debug_assertions) && std::env::var("DROPFORGE_DUMP").is_ok() {
        let snapshot = snapshot_from_profile();
        let target = portable_root().join("snapshot.json");
        let _ = std::fs::write(&target, serde_json::to_string_pretty(&snapshot).unwrap_or_default());
        println!("{}", target.display());
        return;
    }

    tauri::Builder::default()
        .manage(MinerState { child: Mutex::new(None), pubsub: Mutex::new(None), status: Mutex::new(Value::Null) })
        .setup(|app| {
            // Поддержка: DropForge.exe --diagnostics печатает отчёт и выходит.
            if std::env::args().any(|arg| arg == "--diagnostics") {
                let report = diagnostics_report(app.handle().clone());
                if let Some(text) = report.get("text").and_then(Value::as_str) {
                    println!("{text}");
                }
                std::process::exit(0);
            }
            legacy_cleanup();
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let auth = read_json(auth_path()).unwrap_or(Value::Null);
                    let has_avatar = !text_of(auth.get("avatarUrl")).is_empty();
                    if has_avatar {
                        return;
                    }
                    if let Some(token) = load_token() {
                        let login = text_of(auth.get("login"));
                        if let Some(avatar) = fetch_avatar(&token, &login) {
                            store_avatar(&avatar);
                            if let Some(payload) = read_json(auth_path()) {
                                set_auth_state(payload);
                            }
                            let _ = handle.emit("state", snapshot_from_profile());
                        }
                    }
                });
            }
            // мусор от прошлых обновлений: скрипт и архив больше не нужны
            let temp = std::env::temp_dir();
            let _ = std::fs::remove_file(temp.join("dropforge-update.ps1"));
            let _ = std::fs::remove_file(temp.join("dropforge-update.zip"));
            let _ = std::fs::remove_file(temp.join("dropforge-clip.txt"));
            if let Ok(exe) = std::env::current_exe() {
                let name = exe.file_name().and_then(|value| value.to_str()).unwrap_or_default().to_string();
                if name != "DropForge.exe" {
                    if let Some(dir) = exe.parent() {
                        let duplicate = dir.join("DropForge.exe");
                        if duplicate.is_file() {
                            let _ = std::fs::remove_file(&duplicate);
                        }
                    }
                }
            }
            if std::env::var("DROPFORGE_DEVICE_UI").is_ok() {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    if let Ok(started) = device_start() {
                        *DEVICE.lock().unwrap() = json!({ "userCode": started.get("userCode"), "uri": started.get("uri") });
                        open_external(text_of(started.get("uri")));
                        let _ = handle.emit("state", snapshot_from_profile());
                    }
                });
            }
            if std::env::var("DROPFORGE_AUTOFARM").is_ok() {
                let games = settings_games();
                if let Some(slug) = games.first().cloned() {
                    *GAMES.lock().unwrap() = games;
                    *GAME_INDEX.lock().unwrap() = 0;
                    *STOPPING.lock().unwrap() = false;
                    let handle = app.handle().clone();
                    std::thread::spawn(move || {
                        let _ = start_miner(Some(handle), &slug);
                    });
                }
            }
            // автовозобновление фарма после перезапуска
            {
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let state = read_json(farm_state_path()).unwrap_or(Value::Null);
                    if !state.get("running").and_then(Value::as_bool).unwrap_or(false) {
                        return;
                    }
                    let game = text_of(state.get("game"));
                    if game.is_empty() {
                        return;
                    }
                    let at = state
                        .get("at")
                        .and_then(Value::as_str)
                        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                        .map(|value| value.with_timezone(&chrono::Utc));
                    let elapsed = at.map(|value| (chrono::Utc::now() - value).num_seconds()).unwrap_or(i64::MAX);
                    let window_minutes = load_settings()
                        .get("resumeMinutes")
                        .and_then(Value::as_u64)
                        .unwrap_or(15);
                    if window_minutes == 0 {
                        save_farm_state(false, "", "");
                        return;
                    }
                    let mut resume = elapsed <= (window_minutes as i64) * 60;
                    if !resume {
                        // прошло больше 15 минут: сначала обновляем каталог
                        let _ = refresh_catalog();
                        let cache = read_json(profile_dir().join("catalog-cache.json")).unwrap_or(Value::Null);
                        let now = chrono::Utc::now();
                        resume = cache
                            .get("campaigns")
                            .and_then(Value::as_array)
                            .map(|items| {
                                items.iter().any(|campaign| {
                                    text_of(campaign.get("game").and_then(|game| game.get("slug"))) == game
                                        && campaign
                                            .get("endsAt")
                                            .and_then(Value::as_str)
                                            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                                            .map(|value| value.with_timezone(&chrono::Utc) > now)
                                            .unwrap_or(false)
                                })
                            })
                            .unwrap_or(false);
                    }
                    if !resume {
                        save_farm_state(false, "", "");
                        return;
                    }
                    let queue = settings_games();
                    let mut games = queue;
                    if !games.iter().any(|slug| slug == &game) {
                        games.insert(0, game.clone());
                    }
                    let first = games.first().cloned().unwrap_or(game);
                    *GAMES.lock().unwrap() = games;
                    *GAME_INDEX.lock().unwrap() = 0;
                    *STOPPING.lock().unwrap() = false;
                    push_log("info", format!("Автовозобновление фарма: {first}"));
                    let _ = start_miner(Some(handle.clone()), &first);
                    let _ = handle.emit("state", snapshot_from_profile());
                });
            }

            if let Some(token) = load_token() {
                let user_id = read_json(auth_path())
                    .and_then(|value| value.get("userId").cloned())
                    .map(|value| text_of(Some(&value)))
                    .unwrap_or_default();
                start_pubsub(app.handle().clone(), &app.state::<MinerState>(), token, user_id);
                start_background(app.handle().clone());
                start_watchdog(app.handle().clone());
                start_pubsub_watchdog(app.handle().clone());
                start_tray_updater(app.handle().clone());
            }
            // Иконка в трее: свернуть в трей и выйти
            let show = MenuItem::with_id(app, "show", "Открыть DropForge", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("window icon".into()))?;

            let tray = TrayIconBuilder::new()
                .icon(icon)
                .tooltip("DropForge — автофарм Twitch Drops")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            app.manage(TrayHolder(Mutex::new(Some(tray))));
            Ok(())
        })
        .on_window_event(|window, event| {
            // закрытие окна сворачивает в трей, как в Electron-версии
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            settings_save,
            games_toggle,
            games_pin,
            games_select_only,
            farm_game,
            farm_clear,
            farm_skip,
            auth_login_browser,
            app_init,
            app_snapshot,
            app_snapshot_stamp,
            inventory_refresh,
            auth_login_device,
            auth_login_token,
            auth_logout,
            local_check_channel,
            local_clear_channel,
            drops_progress,
            drops_claim_all,
            farm_start,
            farm_stop,
            app_version,
            app_data_dir,
            shell_open,
            diagnostics_report,
            diagnostics_open,
            legacy_launch,
            report_summary,
            report_issue,
            update_check,
            update_install,
            app_quit,
            clipboard_copy_text,
            not_wired
        ])
        .build(tauri::generate_context!())
        .expect("не удалось собрать окно DropForge")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                let status = STATUS.lock().unwrap().clone();
                let running = status.get("running").and_then(Value::as_bool).unwrap_or(false);
                if running {
                    let game = text_of(status.get("game"));
                    let channel = text_of(status.get("channel").and_then(|value| value.get("login")));
                    save_farm_state(true, &game, &channel);
                }
                // убираем иконку из трея до выхода, иначе Windows показывает её до наведения
                if let Some(holder) = handle.try_state::<TrayHolder>() {
                    let _ = holder.0.lock().unwrap().take();
                }
                let miner = CHILD.lock().unwrap().take();
                let pubsub = handle
                    .try_state::<MinerState>()
                    .and_then(|state| state.pubsub.lock().unwrap().take());
                if let Some(mut child) = miner {
                    let _ = child.kill();
                }
                if let Some(mut child) = pubsub {
                    let _ = child.kill();
                }
            }
        });
}
