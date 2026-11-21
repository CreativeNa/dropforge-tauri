//! The Rust port of the pure half of `src/core/gql.js`.

#![forbid(unsafe_code)]

use serde_json::{Map, Number, Value};

/* --------------------------------------------------------------- helpers */

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0 && !f.is_nan()).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

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
        Value::Null => "undefined".into(),
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
    Number::from_f64(value)
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

fn default_ops() -> Value {
    Value::Object(Map::new())
}

/// `{ ...defaults, ...variables }`
fn merge_variables(defaults: Option<&Value>, variables: Option<&Value>) -> Value {
    let mut out = Map::new();
    if let Some(Value::Object(map)) = defaults {
        for (key, value) in map {
            out.insert(key.clone(), value.clone());
        }
    }
    if let Some(Value::Object(map)) = variables {
        for (key, value) in map {
            out.insert(key.clone(), value.clone());
        }
    }
    Value::Object(out)
}

/* ------------------------------------------------------------- operations */

pub fn operation_body(
    name: Value,
    variables: Option<Value>,
    ops: Option<Value>,
) -> Result<Value, String> {
    let ops = ops.unwrap_or_else(default_ops);
    let key = js_string(&name);
    let op = prop(&ops, &key).cloned().unwrap_or(Value::Null);
    if !truthy(&op) {
        // GqlError: `Неизвестная операция ${name}`
        return Err(format!("Неизвестная операция {}", key));
    }

    let operation_name = prop(&op, "operationName").cloned().unwrap_or(Value::Null);
    let hash = prop(&op, "hash").cloned().unwrap_or(Value::Null);
    let defaults = prop(&op, "defaults");
    let variables = merge_variables(defaults, variables.as_ref());

    let mut persisted = Map::new();
    persisted.insert("version".into(), Value::from(1));
    persisted.insert("sha256Hash".into(), hash);
    let mut extensions = Map::new();
    extensions.insert("persistedQuery".into(), Value::Object(persisted));

    let mut out = Map::new();
    out.insert("operationName".into(), operation_name);
    out.insert("extensions".into(), Value::Object(extensions));
    out.insert("variables".into(), variables);
    Ok(Value::Object(out))
}

pub fn build_batch_body(operations: Value, ops: Option<Value>) -> Result<Value, String> {
    let list = match &operations {
        Value::Array(items) => items.clone(),
        Value::Null => return Err("operations.map is not a function".into()),
        _ => return Err("operations.map is not a function".into()),
    };

    let mut out = Vec::with_capacity(list.len());
    for op in &list {
        if let Some(query) = prop(op, "query").cloned().filter(truthy) {
            let name = prop(op, "name").cloned().unwrap_or(Value::Null);
            let mut body = Map::new();
            body.insert("operationName".into(), name);
            body.insert("query".into(), query);
            body.insert(
                "variables".into(),
                prop(op, "variables")
                    .cloned()
                    .unwrap_or_else(|| Value::Object(Map::new())),
            );
            out.push(Value::Object(body));
            continue;
        }
        let name = prop(op, "name").cloned().unwrap_or(Value::Null);
        let variables = prop(op, "variables").cloned();
        out.push(operation_body(name, variables, ops.clone())?);
    }
    Ok(Value::Array(out))
}

/* -------------------------------------------------------------- payloads */

fn messages_of(item: &Value) -> Result<Vec<String>, String> {
    let errors = match prop(item, "errors") {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(items)) => items.clone(),
        Some(_) => return Err("item.errors.map is not a function".into()),
    };
    Ok(errors
        .iter()
        .filter_map(|error| prop(error, "message").map(js_string))
        .filter(|message| !message.is_empty())
        .collect())
}

fn operation_name(body: &Value, operations: &Value, index: usize) -> Option<String> {
    let from_body = body
        .as_array()
        .and_then(|items| items.get(index))
        .and_then(|item| prop(item, "operationName"))
        .map(js_string);
    from_body.or_else(|| {
        operations
            .as_array()
            .and_then(|items| items.get(index))
            .and_then(|item| prop(item, "name"))
            .map(js_string)
    })
}

pub fn interpret_batch_payload(
    payload: Value,
    body: Value,
    operations: Value,
    options: Option<Value>,
) -> Result<Value, String> {
    let attempt = to_number(options.as_ref().and_then(|value| prop(value, "attempt")));
    let retries = options
        .as_ref()
        .and_then(|value| prop(value, "retries"))
        .map(|value| to_number(Some(value)))
        .unwrap_or(3.0);
    let delay = options
        .as_ref()
        .and_then(|value| prop(value, "delay"))
        .map(|value| to_number(Some(value)))
        .unwrap_or(1500.0);

    let list = match &payload {
        Value::Array(items) => items.clone(),
        other => vec![other.clone()],
    };
    let mut results: Vec<Value> = Vec::new();

    for (index, raw_item) in list.iter().enumerate() {
        let item = match raw_item {
            Value::Null => Value::Object(Map::new()),
            other => other.clone(),
        };
        if body.is_null() {
            // JS evaluates `body[index]` first and throws on null
            return Err("Cannot read properties of null (reading '0')".into());
        }
        let name = operation_name(&body, &operations, index);
        let messages = messages_of(&item)?;

        if !messages.is_empty() {
            if messages
                .iter()
                .any(|message| message == "PersistedQueryNotFound")
            {
                if attempt < retries {
                    return Ok(serde_json::json!({
                        "retry": true,
                        "delay": 5000,
                        "results": results,
                    }));
                }
                let name = name.unwrap_or_else(|| "undefined".into());
                return Err(format!(
                    "Twitch обновил запрос «{}». Нужно обновить хэш операции.",
                    name
                ));
            }

            let recoverable = messages.iter().any(|message| {
                matches!(
                    message.as_str(),
                    "service error"
                        | "service timeout"
                        | "request cancelled"
                        | "service unavailable"
                )
            });
            if recoverable && attempt < retries {
                return Ok(serde_json::json!({
                    "retry": true,
                    "delay": number_value((delay * 2.0).min(15000.0)),
                    "results": results,
                }));
            }

            if messages.len() == 1 && messages[0] == "server error" {
                results.push(Value::Null);
                continue;
            }

            return Err(format!("GraphQL: {}", messages.join("; ")));
        }

        if let Some(error) = prop(&item, "error").cloned().filter(truthy) {
            let message = prop(&item, "message").cloned();
            let suffix = match message {
                Some(Value::Null) | None => String::new(),
                Some(value) => format!(" {}", js_string(&value)),
            };
            return Err(format!("GraphQL: {}{}", js_string(&error), suffix)
                .trim_end()
                .to_string());
        }

        let data = prop(&item, "data").cloned().unwrap_or(Value::Null);
        results.push(if data.is_null() { Value::Null } else { data });
    }

    Ok(serde_json::json!({
        "retry": false,
        "delay": number_value(delay),
        "results": results,
    }))
}
