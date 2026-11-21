//! The Rust port of the pure selectors of `src/core/watcher.js`.

#![forbid(unsafe_code)]

use serde_json::{Map, Value};

fn prop<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    match value {
        Value::Object(map) => map.get(key),
        _ => None,
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
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

fn number_value(value: f64) -> Value {
    if !value.is_finite() {
        return Value::Null;
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return Value::from(value as i64);
    }
    serde_json::Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn to_number(value: Option<&Value>) -> f64 {
    match value {
        None | Some(Value::Null) => 0.0,
        Some(Value::Bool(b)) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        Some(Value::String(s)) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().unwrap_or(f64::NAN)
            }
        }
        Some(_) => f64::NAN,
    }
}

fn urgency_of(campaign: &Value) -> f64 {
    // JS: `campaign.totalRequiredMinutes || 1`
    let total = to_number(prop(campaign, "totalRequiredMinutes"));
    let minutes = if total == 0.0 || total.is_nan() {
        1.0
    } else {
        total
    };
    let left_raw = prop(campaign, "minutesLeft");
    let left = match left_raw {
        Some(value) if value.is_f64() && truthy(value) => to_number(Some(value)),
        Some(Value::Number(number)) if number.as_f64().map(|v| v.is_finite()).unwrap_or(false) => {
            to_number(Some(&Value::Number(number.clone())))
        }
        _ => 100000.0,
    };
    left / minutes
}

pub fn urgency(campaign: Value) -> Result<Value, String> {
    Ok(number_value(urgency_of(&campaign)))
}

fn accepts_channel(campaign: &Value, channel: &Value) -> bool {
    match prop(campaign, "allowedChannels") {
        None | Some(Value::Null) => true,
        Some(Value::Array(items)) => {
            let id = match prop(channel, "id") {
                Some(Value::String(text)) => text.clone(),
                Some(other) => js_string(other),
                None => String::new(),
            };
            items
                .iter()
                .any(|item| prop(item, "id").map(js_string).as_deref() == Some(id.as_str()))
        }
        Some(_) => true,
    }
}

pub fn pick_campaign(campaigns: Value, options: Option<Value>) -> Result<Value, String> {
    let targets: Vec<String> = options
        .as_ref()
        .and_then(|value| prop(value, "targets"))
        .and_then(|value| value.as_array())
        .map(|items| items.iter().map(js_string).collect())
        .unwrap_or_default();
    let skipped: Vec<String> = options
        .as_ref()
        .and_then(|value| prop(value, "skipped"))
        .and_then(|value| value.as_array())
        .map(|items| items.iter().map(js_string).collect())
        .unwrap_or_default();

    let list = match campaigns {
        Value::Array(items) => items,
        _ => return Ok(Value::Null),
    };

    let mut candidates: Vec<Value> = list
        .into_iter()
        .filter(|campaign| {
            let slug = prop(campaign, "game")
                .and_then(|game| prop(game, "slug"))
                .map(js_string);
            let in_targets = slug
                .as_deref()
                .map(|slug| targets.iter().any(|t| t == slug))
                .unwrap_or(false);
            let can_earn = prop(campaign, "canEarn").map(truthy).unwrap_or(false);
            let id = prop(campaign, "id").map(js_string).unwrap_or_default();
            let not_skipped = !skipped.contains(&id);
            in_targets && can_earn && not_skipped
        })
        .collect();

    if candidates.is_empty() {
        return Ok(Value::Null);
    }
    candidates.sort_by(|a, b| {
        urgency_of(a)
            .partial_cmp(&urgency_of(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(candidates.remove(0))
}

pub fn pick_channel_from_streams(streams: Value, campaign: Value) -> Result<Value, String> {
    let list = match streams {
        Value::Array(items) => items,
        _ => return Ok(Value::Null),
    };

    let mut usable: Vec<Value> = Vec::new();
    for stream in &list {
        let broadcaster = match prop(stream, "broadcaster") {
            Some(value @ Value::Object(_)) => value,
            _ => continue,
        };
        let login = match prop(broadcaster, "login") {
            Some(Value::String(text)) if !text.is_empty() => text.clone(),
            _ => continue,
        };
        let mut channel = Map::new();
        let id = prop(broadcaster, "id")
            .map(js_string)
            .unwrap_or_else(|| "null".into());
        channel.insert("id".into(), Value::String(id));
        channel.insert("login".into(), Value::String(login.clone()));
        let display = prop(broadcaster, "displayName")
            .filter(|value| truthy(value))
            .map(js_string)
            .unwrap_or(login);
        channel.insert("displayName".into(), Value::String(display));
        channel.insert(
            "viewers".into(),
            number_value(to_number(prop(stream, "viewersCount"))),
        );
        let title = prop(stream, "title")
            .filter(|value| truthy(value))
            .map(js_string)
            .unwrap_or_default();
        channel.insert("title".into(), Value::String(title));
        let channel = Value::Object(channel);
        if accepts_channel(&campaign, &channel) {
            usable.push(channel);
        }
    }

    if usable.is_empty() {
        return Ok(Value::Null);
    }
    usable.sort_by(|a, b| {
        to_number(prop(a, "viewers"))
            .partial_cmp(&to_number(prop(b, "viewers")))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(usable.remove(0))
}
