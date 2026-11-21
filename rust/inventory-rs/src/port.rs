//! The Rust port of `selectStaleGames` from `src/core/inventory.js`.

#![forbid(unsafe_code)]

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde_json::Value;

fn prop<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map.get(key),
        _ => None,
    }
}

fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".into(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

/// `new Date(value).getTime()`: NaN для невалидных значений.
fn date_ms(value: Option<&Value>) -> f64 {
    let Some(value) = value else { return f64::NAN };
    match value {
        Value::String(text) => parse_iso(text)
            .map(|date| date.timestamp_millis() as f64)
            .unwrap_or(f64::NAN),
        Value::Number(number) => number.as_f64().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn parse_iso(text: &str) -> Option<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
        return Some(parsed.with_timezone(&Utc));
    }
    NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .and_then(|date| date.and_hms_opt(0, 0, 0))
        .map(|naive| Utc.from_utc_datetime(&naive))
}

pub fn select_stale_games(
    candidates: Value,
    focus: Option<Value>,
    options: Option<Value>,
) -> Result<Value, String> {
    let now = options
        .as_ref()
        .and_then(|value| prop(value, "now"))
        .and_then(|value| value.as_f64())
        .unwrap_or_else(|| Utc::now().timestamp_millis() as f64);
    let limit = options
        .as_ref()
        .and_then(|value| prop(value, "limit"))
        .and_then(|value| value.as_f64())
        .unwrap_or(6.0) as usize;
    let cooldowns = options
        .as_ref()
        .and_then(|value| prop(value, "cooldowns"))
        .cloned()
        .unwrap_or(Value::Null);

    let list = match &candidates {
        Value::Array(items) => items.clone(),
        _ => return Ok(Value::Array(Vec::new())),
    };

    let mut order: Vec<String> = Vec::new();
    let mut latest: Vec<f64> = Vec::new();

    for campaign in &list {
        let slug = match prop(campaign, "game").and_then(|game| prop(game, "slug")) {
            Some(Value::String(text)) if !text.is_empty() => text.clone(),
            _ => continue,
        };
        let end = match prop(campaign, "endAt") {
            None | Some(Value::Null) => f64::INFINITY,
            Some(value) => date_ms(Some(value)),
        };
        match order.iter().position(|existing| existing == &slug) {
            Some(index) => {
                if end > latest[index] {
                    latest[index] = end;
                }
            }
            None => {
                order.push(slug);
                latest.push(end.max(0.0));
            }
        }
    }

    let wanted = match focus {
        Some(Value::Array(items)) => items.iter().map(js_string).collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    let mut selected: Vec<String> = Vec::new();
    for (index, slug) in order.iter().enumerate() {
        let ended = !latest[index].is_finite() || latest[index] < now;
        if ended || wanted.contains(slug) {
            selected.push(slug.clone());
        }
    }
    if selected.is_empty() {
        return Ok(Value::Array(Vec::new()));
    }

    let mut unique: Vec<String> = Vec::new();
    for slug in selected {
        if !unique.contains(&slug) {
            let ready = match prop(&cooldowns, &slug) {
                Some(value) => value.as_f64().unwrap_or(0.0) < now,
                None => 0.0 < now,
            };
            if ready {
                unique.push(slug);
            }
        }
    }
    unique.truncate(limit);

    Ok(Value::Array(
        unique.into_iter().map(Value::String).collect(),
    ))
}
