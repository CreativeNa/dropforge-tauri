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
                    json!({
                        "id": benefit.get("id"),
                        "name": benefit.get("name"),
                        "type": benefit.get("distributionType"),
                        "imageUrl": benefit.get("imageAssetURL"),
                        "badge": benefit.get("badge").cloned().unwrap_or(json!(false)),
                    })
                })
                .collect();
            json!({
                "id": drop.get("id"),
                "name": drop.get("name"),
                "startsAt": drop.get("startAt"),
                "endsAt": drop.get("endAt"),
                "requiredMinutes": required,
                "requiredSubs": drop.get("requiredSubs").cloned().unwrap_or(json!(0)),
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
        Ok(response) => format!("OK {}", response.status().as_u16()),
        Err(ureq::Error::StatusCode(code)) => format!("HTTP {code}"),
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
    for name in ["DropForge.exe", "dropforge-tauri.exe", "watcher-svc\\dropforge-watcher-svc.exe", "pubsub-svc\\dropforge-pubsub-svc.exe"] {
        let path = exe_dir.join(name);
        if let Ok(meta) = std::fs::metadata(&path) {
            lines.push(format!("file: {name} — {} байт, изменён {}", meta.len(), chrono::DateTime::<chrono::Utc>::from(meta.modified().unwrap_or(std::time::SystemTime::now())).to_rfc3339()));
        } else {
            lines.push(format!("file: {name} — ОТСУТСТВУЕТ"));
        }
    }
    let settings = load_settings();
    lines.push(format!("settings.games: {}", text_of(settings.get("games"))));
    lines.push(format!("settings.flags: autoClaim={} spade={} tray={}",
        settings.get("autoClaim").and_then(Value::as_bool).unwrap_or(false),
        settings.get("sendSpadeEvents").and_then(Value::as_bool).unwrap_or(false),
        settings.get("minimizeToTray").and_then(Value::as_bool).unwrap_or(false)));
    lines.push(format!("auth: {}", if load_token().is_some() { "токен есть" } else { "токена нет" }));
    let status = STATUS.lock().unwrap().clone();
    lines.push(format!("status: {status}"));
    let child = CHILD.lock().unwrap().as_ref().and_then(|child| child.id().into());
    lines.push(format!("watcher pid: {}", child.map(|pid| pid.to_string()).unwrap_or_else(|| "нет".into())));
    let progress = read_json(progress_path()).unwrap_or(Value::Null);
    lines.push(format!("progress.at: {}", text_of(progress.get("at"))));
    lines.push(format!("progress.drops: {}", progress.get("drops").and_then(Value::as_object).map(|map| map.len()).unwrap_or(0)));
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
    for asset in release.get("assets").and_then(Value::as_array).cloned().unwrap_or_default() {
        let name = text_of(asset.get("name"));
        if name.ends_with(".zip") {
            asset_url = text_of(asset.get("browser_download_url"));
            asset_name = name;
            asset_size = asset.get("size").and_then(Value::as_u64).unwrap_or(0);
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
        "url": release.get("html_url").cloned().unwrap_or(Value::Null),
    })
}

fn updater_script() -> String {
    [
        "param([int]$AppPid, [string]$Zip, [string]$Dst, [string]$ExeName)",
        "try { Wait-Process -Id $AppPid -Timeout 90 -ErrorAction SilentlyContinue } catch {}",
        "Start-Sleep -Milliseconds 700",
        "$tmp = Join-Path $env:TEMP (\"df-\" + [guid]::NewGuid().ToString(\"n\"))",
        "Expand-Archive -LiteralPath $Zip -DestinationPath $tmp -Force",
        "$src = Join-Path $tmp \"DropForge\"",
        "if (-not (Test-Path $src)) { $src = $tmp }",
        "Get-ChildItem $src -Force | ForEach-Object { Copy-Item $_.FullName -Destination $Dst -Recurse -Force }",
        "if ($ExeName -ne \"DropForge.exe\") { Copy-Item (Join-Path $Dst \"DropForge.exe\") (Join-Path $Dst $ExeName) -Force }",
        "Start-Process -FilePath (Join-Path $Dst $ExeName)",
        "Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue",
        "Remove-Item $Zip -Force -ErrorAction SilentlyContinue",
    ].join("\n")
}

#[tauri::command]
fn update_install(app: tauri::AppHandle, url: String) -> Value {
    if !url.starts_with("https://github.com/") && !url.starts_with("https://objects.githubusercontent.com/") {
        return json!({ "started": false, "error": "обновления берём только с GitHub" });
    }
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => return json!({ "started": false, "error": error.to_string() }),
    };
    let exe_name = exe.file_name().and_then(|name| name.to_str()).unwrap_or("DropForge.exe").to_string();
    let dir = exe.parent().map(|path| path.to_path_buf()).unwrap_or_default();
    let zip_path = std::env::temp_dir().join("dropforge-update.zip");
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
    if let Err(error) = std::fs::write(&zip_path, &bytes) {
        return json!({ "started": false, "error": error.to_string() });
    }
    let script = std::env::temp_dir().join("dropforge-update.ps1");
    if let Err(error) = std::fs::write(&script, updater_script()) {
        return json!({ "started": false, "error": error.to_string() });
    }
    let mut command = Command::new("powershell");
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
        .arg(&exe_name);
    hide_console(&mut command);
    if let Err(error) = command.spawn() {
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
    let category = groups
        .first()
        .and_then(|group| group.get("categories"))
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned()
        .unwrap_or(Value::Null);

    let mut drops = Vec::new();
    for group in &groups {
        let minutes = group
            .get("unlockRequirements")
            .and_then(|req| req.get("minuteWatchedGoal"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if minutes == 0.0 {
            continue;
        }
        for reward in group.get("rewards").and_then(Value::as_array).cloned().unwrap_or_default() {
            let image = reward
                .get("thumbnailImage")
                .and_then(|image| image.get("image1xURL"))
                .cloned()
                .unwrap_or(Value::Null);
            // бейджи приходят и как BADGE, и как QUEST_REWARD: отличаем по адресу ассета и ссылке
            let image_text = text_of(Some(&image));
            let redemption = text_of(reward.get("redemptionURL")).to_lowercase();
            let is_badge = image_text.contains("/badges/") || redemption.contains("badge");
            drops.push(json!({
                "id": text_or_null(reward.get("id")),
                "name": reward.get("name").cloned().unwrap_or(json!("Награда")),
                "startAt": text_or_null(campaign.get("startsAt")),
                "endAt": reward.get("earnableUntil").or_else(|| campaign.get("endsAt")).cloned().unwrap_or(Value::Null),
                "requiredMinutesWatched": minutes,
                "preconditionDrops": [],
                "benefitEdges": [{
                    "benefit": {
                        "id": text_or_null(reward.get("id")),
                        "name": reward.get("name").cloned().unwrap_or(json!("Награда")),
                        "distributionType": "QUEST_REWARD",
                        "imageAssetURL": image,
                        "badge": is_badge,
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
            "boxArtURL": Value::Null,
        },
        "allow": { "isEnabled": false, "channels": [] },
        "timeBasedDrops": drops,
        "rewardCampaign": true,
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
    let payload = json!({
        "at": chrono::Utc::now().to_rfc3339(),
        "campaigns": campaigns_seen,
        "drops": drops,
        "claimedBenefits": claimed,
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
    let drops = http_json(CATALOG_URL).ok_or("каталог недоступен")?;
    let rewards = http_json(REWARDS_URL).unwrap_or(Value::Null);

    let mut campaigns = Vec::new();
    for group in drops.get("games").and_then(Value::as_array).cloned().unwrap_or_default() {
        for campaign in group.get("rewards").and_then(Value::as_array).cloned().unwrap_or_default() {
            if campaign.get("status").and_then(Value::as_str) == Some("EXPIRED") {
                continue;
            }
            campaigns.push(raw_from_catalog(&group, &campaign));
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
            match campaigns.iter().position(|item| text_of(item.get("id")) == id) {
                Some(position) => campaigns[position] = campaign,
                None => campaigns.push(campaign),
            }
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

fn save_token(token: &str) -> Option<(String, String)> {
    let (login, user_id, client_id) = validate_token(token)?;
    let protected = dpapi(token.as_bytes(), true)?;
    let payload = json!({
        "token": format!("enc:{}", base64_encode(&protected)),
        "login": login,
        "userId": user_id,
        "clientId": client_id,
        "savedAt": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|span| span.as_secs()).unwrap_or(0),
    });
    std::fs::create_dir_all(profile_dir()).ok();
    std::fs::write(auth_path(), serde_json::to_string_pretty(&payload).ok()?).ok()?;
    // Кэш авторизации мог запомнить пустое состояние до входа — обновляем его.
    set_auth_state(payload.clone());
    Some((text_of(payload.get("login")), text_of(payload.get("userId"))))
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
        login.to_lowercase()
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
                    json!({
                        "id": benefit.get("id"),
                        "name": benefit.get("name"),
                        "type": benefit.get("distributionType"),
                        "imageUrl": benefit.get("imageAssetURL"),
                        "badge": benefit.get("badge").cloned().unwrap_or(json!(false)),
                    })
                })
                .collect();
            json!({
                "id": drop.get("id"),
                "name": drop.get("name"),
                "startsAt": drop.get("startAt"),
                "endsAt": drop.get("endAt"),
                "requiredMinutes": required,
                "requiredSubs": drop.get("requiredSubs").cloned().unwrap_or(json!(0)),
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

static GAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static GAME_INDEX: Mutex<usize> = Mutex::new(0);
static STOPPING: Mutex<bool> = Mutex::new(false);
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
    candidates.into_iter().find(|candidate| candidate.is_file())
}

/// PubSub в отдельном процессе: события уходят в UI как есть.
/// Первый бесплатный неполученный бейдж, который можно фармить.
fn first_badge_target() -> Option<(String, String)> {
    let snapshot = snapshot_from_profile();
    let badges = snapshot.get("badges")?.as_array()?.clone();
    for badge in badges {
        let paid = badge.get("paid").and_then(Value::as_bool).unwrap_or(true);
        let claimed = badge.get("claimed").and_then(Value::as_bool).unwrap_or(true);
        let minutes = badge.get("requiredMinutes").and_then(Value::as_f64).unwrap_or(0.0);
        let local = badge.get("local").and_then(Value::as_bool).unwrap_or(false);
        if paid || claimed || minutes <= 0.0 || local {
            continue;
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

fn start_background(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(600));
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
        // если ничего не фармится — берём бесплатный неполученный бейдж
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
        if auto_badges && !running {
            if let Some((slug, name)) = first_badge_target() {
                push_log("info", format!("Беру бесплатный бейдж: {name}"));
                let _ = start_miner(Some(app.clone()), &slug);
            }
        }
        let _ = app.emit("state", snapshot_from_profile());
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
    candidates.into_iter().find(|candidate| candidate.is_file())
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
fn allowed_channel_for(slug: &str) -> Option<(String, String)> {
    let cache = read_json(profile_dir().join("catalog-cache.json"))?;
    let campaigns = cache.get("campaigns").and_then(Value::as_array)?;
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
            if !id.is_empty() && !name.is_empty() {
                return Some((id, name));
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
    // кампания, ограниченная списком каналов: берём первый из разрешённых
    if let Some(found) = allowed_channel_for(slug) {
        return Some(found);
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
        return json!({ "error": format!("нет живых каналов с Drops для {slug}") });
    };

    let mut guard = CHILD.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
    }
    let minutes = std::env::var("DROPFORGE_WATCH_MINUTES").unwrap_or_else(|_| "1440".to_string());
    let interval = std::env::var("DROPFORGE_TICK_INTERVAL").unwrap_or_else(|_| "60".to_string());
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

    match command.spawn() {
        Ok(mut child) => {
            let pid = child.id();
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
                    let tick_limit = std::env::var("DROPFORGE_TICKS").ok().and_then(|value| value.parse::<usize>().ok());
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
                        let _ = refresh_progress();
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
                            let message = match minutes {
                                Some(value) => format!("Смотрю {login_for_thread} · {value:.0} мин"),
                                None => format!("Смотрю {login_for_thread}"),
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
                    if !stopping {
                        if let Some(handle) = &handle {
                            let _ = handle.emit("log", json!({ "at": "", "level": "info", "message": "Стрим закончился — беру следующую игру" }));
                        }
                        push_log("info", "Стрим закончился — беру следующую игру".to_string());
                        advance_game(handle.clone());
                    }
                });
            }
            *guard = Some(child);
            let message = format!("Смотрю {login}");
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
    let mut name: Vec<u16> = "Global\\DropForgeTauriSingleInstance".encode_utf16().collect();
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
    let mut badges: Vec<Value> = Vec::new();
    for campaign in &campaigns {
        let campaign_name = text_of(campaign.get("name"));
        let campaign_id = text_of(campaign.get("id"));
        let game_slug = text_of(campaign.get("game").and_then(|game| game.get("slug")));
        let game_name = text_of(campaign.get("game").and_then(|game| game.get("name")));
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
                let claimed = claimed_ids.contains(&benefit_id);
                let paid = subs > 0.0 || minutes <= 0.0;
                badges.push(json!({
                    "id": benefit_id,
                    "name": benefit.get("name"),
                    "imageUrl": benefit.get("imageUrl"),
                    "campaignId": campaign_id,
                    "campaign": campaign_name,
                    "gameSlug": game_slug,
                    "game": game_name,
                    "requiredMinutes": minutes,
                    "requiredSubs": subs,
                    "paid": paid,
                    "claimed": claimed,
                    "endsAt": campaign.get("endsAt"),
                    "local": campaign.get("local").and_then(Value::as_bool).unwrap_or(false),
                }));
            }
        }
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
                    "displayName": login
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
        "badges": badges,
        "device": DEVICE.lock().unwrap().clone(),
        "catalog": Value::Null,
        "demo": false,
        "platform": std::env::consts::OS,
    })
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
            "priorityMode": "ending-soonest"
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

fn settings_games() -> Vec<String> {
    load_settings()
        .get("games")
        .and_then(Value::as_array)
        .map(|items| items.iter().map(|value| text_of(Some(value))).collect())
        .unwrap_or_default()
}

#[tauri::command]
fn settings_save(patch: Value) -> Value {
    let mut settings = load_settings();
    if let (Some(target), Some(source)) = (settings.as_object_mut(), patch.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
    match save_settings(&settings) {
        Ok(()) => snapshot_from_profile(),
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

#[tauri::command]
fn auth_logout() -> Value {
    let _ = std::fs::remove_file(auth_path());
    set_auth_state(Value::Null);
    snapshot_from_profile()
}

#[tauri::command]
fn inventory_refresh(app: tauri::AppHandle) -> Value {
    let _ = app.emit("refresh", json!({ "active": true }));
    let result = refresh_catalog();
    let snapshot = snapshot_from_profile();
    if let Err(error) = result {
        eprintln!("refresh error: {error}");
    }
    let _ = app.emit("refresh", json!({ "active": false }));
    let _ = app.emit("state", snapshot.clone());
    snapshot
}

#[tauri::command]
fn app_snapshot() -> Value {
    snapshot_from_profile()
}

#[tauri::command]
fn app_version() -> Value {
    json!({ "app": env!("CARGO_PKG_VERSION"), "electron": null })
}

#[tauri::command]
fn app_data_dir() -> Value {
    json!(profile_dir().to_string_lossy().to_string())
}

/// Кнопки, которым нужен движок: честно сообщаем, что ещё не подключено.
/// Открывает ссылку в браузере без командной оболочки (никаких cmd/start).
fn open_url(url: &str) -> bool {
    if !url.starts_with("https://") && !url.starts_with("http://") {
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

    if std::env::var("DROPFORGE_CMDS").is_ok() {
        println!("toggle: {:?}", games_toggle("escape-from-tarkov".into()).get("stats"));
        println!("toggle2: {:?}", games_toggle("albion-online".into()).get("stats"));
        println!("pin: {:?}", games_pin("albion-online".into()).get("stats"));
        println!("settings: {}", load_settings());
        println!("select_only: {:?}", games_select_only(vec!["albion-online".into()]).get("stats"));
        println!("settings after: {}", load_settings());
        return;
    }

    if let Ok(login) = std::env::var("DROPFORGE_LOCAL_CHECK") {
        let snapshot = local_check_channel(login);
        let check = snapshot
            .get("settings")
            .and_then(|settings| settings.get("localCheck"))
            .cloned()
            .unwrap_or(Value::Null);
        println!("local-check: {check}");
        return;
    }

    if std::env::var("DROPFORGE_DEVICE").is_ok() {
        match device_start() {
            Ok(value) => println!("device: {}", value),
            Err(error) => println!("device error: {error}"),
        }
        return;
    }

    if std::env::var("DROPFORGE_CLAIM").is_ok() {
        println!("claim: {}", claim_all());
        return;
    }

    if let Ok(list) = std::env::var("DROPFORGE_FARM") {
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

    if std::env::var("DROPFORGE_REFRESH").is_ok() {
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

    if std::env::var("DROPFORGE_DUMP").is_ok() {
        let snapshot = snapshot_from_profile();
        let target = portable_root().join("snapshot.json");
        let _ = std::fs::write(&target, serde_json::to_string_pretty(&snapshot).unwrap_or_default());
        println!("{}", target.display());
        return;
    }

    tauri::Builder::default()
        .manage(MinerState { child: Mutex::new(None), pubsub: Mutex::new(None), status: Mutex::new(Value::Null) })
        .setup(|app| {
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
            if let Some(token) = load_token() {
                let user_id = read_json(auth_path())
                    .and_then(|value| value.get("userId").cloned())
                    .map(|value| text_of(Some(&value)))
                    .unwrap_or_default();
                start_pubsub(app.handle().clone(), &app.state::<MinerState>(), token, user_id);
                start_background(app.handle().clone());
            }
            // Иконка в трее: свернуть в трей и выйти
            let show = MenuItem::with_id(app, "show", "Открыть DropForge", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let icon = app
                .default_window_icon()
                .cloned()
                .ok_or_else(|| tauri::Error::AssetNotFound("window icon".into()))?;

            let _tray = TrayIconBuilder::new()
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
            update_check,
            update_install,
            app_quit,
            not_wired
        ])
        .build(tauri::generate_context!())
        .expect("не удалось собрать окно DropForge")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
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
