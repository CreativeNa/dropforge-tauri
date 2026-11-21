//! The Rust port of the pure half of `src/core/http.js`.

#![forbid(unsafe_code)]

use serde_json::Value;

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

fn to_number(value: Option<&Value>) -> f64 {
    match value {
        None => f64::NAN,
        Some(value) => {
            // маркеры $num и обычные числа мост разбирает сам
            if let Some(number) = crate::js::as_number(value) {
                return number;
            }
            match value {
                Value::Null => 0.0,
                Value::Bool(b) => {
                    if *b {
                        1.0
                    } else {
                        0.0
                    }
                }
                Value::String(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        0.0
                    } else {
                        trimmed.parse::<f64>().unwrap_or(f64::NAN)
                    }
                }
                Value::Array(items) => match items.len() {
                    0 => 0.0,
                    1 => to_number(items.first()),
                    _ => f64::NAN,
                },
                _ => f64::NAN,
            }
        }
    }
}

fn safe_parse(text: &Value) -> Value {
    if !truthy(text) {
        return Value::Null;
    }
    serde_json::from_str(&js_string(text)).unwrap_or(Value::Null)
}

pub fn parse_body(text: Value, parse: Option<Value>) -> Result<Value, String> {
    let mode = parse
        .as_ref()
        .map(js_string)
        .unwrap_or_else(|| "auto".into());
    if mode == "json" {
        return Ok(safe_parse(&text));
    }
    if mode == "text" {
        return Ok(text);
    }
    let parsed = safe_parse(&text);
    if parsed.is_null() {
        return Ok(text);
    }
    Ok(parsed)
}

pub fn is_retryable_status(status: Value) -> Result<Value, String> {
    let code = to_number(Some(&status));
    Ok(Value::Bool(code == 429.0 || code >= 500.0))
}

pub fn next_delay(delay: Value, max_backoff_ms: Option<Value>) -> Result<Value, String> {
    let current = to_number(Some(&delay));
    // JS: аргумент undefined включает значение по умолчанию, NaN — нет
    let max = match max_backoff_ms.as_ref() {
        None => 30000.0,
        Some(value) if crate::js::is_undefined(value) => 30000.0,
        Some(value) => to_number(Some(value)),
    };
    let doubled = current * 2.0;
    // Math.min в JS возвращает NaN, если хоть один аргумент NaN
    let result = if doubled.is_nan() || max.is_nan() {
        f64::NAN
    } else {
        doubled.min(max)
    };
    Ok(crate::js::number(result))
}
