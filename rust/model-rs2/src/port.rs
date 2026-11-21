//! The Rust port of `src/core/model.js`.
//!
//! Mirrors the JS semantics: missing properties read as `undefined` (so keys are
//! omitted from JSON), falsy checks follow JS truthiness, dates are `Date`
//! objects serialized as ISO strings, and error paths raise where the source
//! throws.

#![forbid(unsafe_code)]

use chrono::{DateTime, NaiveDate, SecondsFormat, TimeZone, Utc};
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

fn is_object_like(value: &Value) -> bool {
    matches!(value, Value::Object(_) | Value::Array(_))
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
        Value::Null => "null".into(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
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
            if trimmed.is_empty() { 0.0 } else { trimmed.parse::<f64>().unwrap_or(f64::NAN) }
        }
        Some(Value::Array(items)) => match items.len() {
            0 => 0.0,
            1 => to_number(items.first()),
            _ => f64::NAN,
        },
        Some(Value::Object(_)) => f64::NAN,
    }
}

fn number_value(value: f64) -> Value {
    if !value.is_finite() {
        return Value::Null;
    }
    if value.fract() == 0.0 && value.abs() < 1e15 {
        return Value::from(value as i64);
    }
    Number::from_f64(value).map(Value::Number).unwrap_or(Value::Null)
}

fn parse_date(value: Option<&Value>) -> Option<DateTime<Utc>> {
    let value = value?;
    if !truthy(value) {
        return None;
    }
    match value {
        Value::String(text) => {
            if let Ok(parsed) = DateTime::parse_from_rfc3339(text) {
                return Some(parsed.with_timezone(&Utc));
            }
            if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
                return date.and_hms_opt(0, 0, 0).map(|naive| Utc.from_utc_datetime(&naive));
            }
            None
        }
        Value::Number(number) => {
            let millis = number.as_f64()?;
            Utc.timestamp_millis_opt(millis as i64).single()
        }
        // our own output carries Dates as {"$date": iso}
        Value::Object(map) => {
            map.get("$date").and_then(|value| value.as_str()).and_then(|text| DateTime::parse_from_rfc3339(text).ok()).map(|parsed| parsed.with_timezone(&Utc))
        }
        _ => None,
    }
}

fn iso(date: DateTime<Utc>) -> String {
    date.to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn date_value(date: Option<DateTime<Utc>>) -> Value {
    // the wire tags JS Dates as {"$date": iso}; index.cjs turns them back into Date
    match date {
        Some(date) => serde_json::json!({ "$date": iso(date) }),
        None => Value::Null,
    }
}

/// `String.prototype.replace(/-?\d+x\d+(?=\.\w+$)/, '')` from the source.
fn strip_dimensions(url: Option<&Value>) -> Value {
    let Some(value) = url else { return Value::Null };
    if !truthy(value) {
        return Value::Null;
    }
    let text = js_string(value);
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    // suffix after the last dot must be word characters up to the end
    let mut dot = None;
    for i in (0..len).rev() {
        if chars[i] == '.' {
            let suffix_ok = i + 1 < len && chars[i + 1..].iter().all(|c| c.is_alphanumeric() || *c == '_');
            if suffix_ok {
                dot = Some(i);
            }
            break;
        }
    }
    let Some(dot) = dot else {
        return Value::String(text);
    };

    let mut j = dot;
    let mut digits_after = 0;
    while j > 0 && chars[j - 1].is_ascii_digit() {
        j -= 1;
        digits_after += 1;
    }
    if digits_after == 0 || j == 0 || (chars[j - 1] != 'x' && chars[j - 1] != 'X') {
        return Value::String(text);
    }
    j -= 1;
    let mut digits_before = 0;
    while j > 0 && chars[j - 1].is_ascii_digit() {
        j -= 1;
        digits_before += 1;
    }
    if digits_before == 0 {
        return Value::String(text);
    }
    if j > 0 && chars[j - 1] == '-' {
        j -= 1;
    }
    let out: String = chars[..j].iter().chain(chars[dot..].iter()).collect();
    Value::String(out)
}

fn set(target: &mut Map<String, Value>, key: &str, value: Value) {
    target.insert(key.to_string(), value);
}

fn set_opt(target: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(value) = value {
        target.insert(key.to_string(), value);
    }
}

/* ------------------------------------------------------------ mergeData */

fn merge(primary: Value, secondary: Value) -> Value {
    if !truthy(&primary) {
        return secondary;
    }
    if !truthy(&secondary) {
        return primary;
    }
    if !is_object_like(&primary) || !is_object_like(&secondary) {
        return primary;
    }
    if primary.is_array() || secondary.is_array() {
        return primary;
    }

    let primary_map = primary.as_object().expect("object");
    let secondary_map = secondary.as_object().expect("object");
    let mut merged = Map::new();

    for (key, first) in primary_map {
        match secondary_map.get(key) {
            Some(second) => {
                let nested = truthy(first) && truthy(second) && is_object_like(first) && is_object_like(second);
                let value = if nested { merge(first.clone(), second.clone()) } else { first.clone() };
                merged.insert(key.clone(), value);
            }
            None => {
                merged.insert(key.clone(), first.clone());
            }
        }
    }
    for (key, second) in secondary_map {
        if !primary_map.contains_key(key) {
            merged.insert(key.clone(), second.clone());
        }
    }
    Value::Object(merged)
}

/* --------------------------------------------------------------- drops */

fn build_benefit(edge: Option<&Value>) -> Value {
    let mut out = Map::new();
    let benefit = edge.and_then(|edge| prop(edge, "benefit"));
    set(&mut out, "id", benefit.and_then(|b| prop(b, "id")).cloned().unwrap_or(Value::Null));
    set(&mut out, "name", benefit.and_then(|b| prop(b, "name")).cloned().unwrap_or_else(|| Value::String("Награда".into())));
    set(&mut out, "type", benefit.and_then(|b| prop(b, "distributionType")).cloned().unwrap_or_else(|| Value::String("UNKNOWN".into())));
    set(&mut out, "imageUrl", benefit.and_then(|b| prop(b, "imageAssetURL")).cloned().unwrap_or(Value::Null));
    Value::Object(out)
}

struct ClaimedBenefits {
    provided: Option<Value>,
}

impl ClaimedBenefits {
    /// JS calls `claimedBenefits.get(...)`, which only exists on a real `Map`.
    fn lookup(&self, id: &Value) -> Result<Option<DateTime<Utc>>, String> {
        match &self.provided {
            None => Ok(None),
            Some(Value::Object(map)) => Ok(map.get(&js_string(id)).and_then(|value| parse_date(Some(value)))),
            Some(_) => Ok(None),
        }
    }
}

fn build_drop(campaign_id: Option<&Value>, data: &Value, claimed: &ClaimedBenefits) -> Result<Value, String> {
    let edges = match prop(data, "benefitEdges") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.clone(),
        Some(_) => return Err("data.benefitEdges.map is not a function".into()),
    };
    let benefits: Vec<Value> = edges.iter().map(|edge| build_benefit(Some(edge))).collect();
    let self_value = prop(data, "self").cloned().unwrap_or(Value::Null);
    let has_self = truthy(&self_value);
    let start_at = parse_date(prop(data, "startAt"));
    let end_at = parse_date(prop(data, "endAt"));

    let mut claimed_by_benefits = false;
    if !has_self && !benefits.is_empty() {
        let mut all = true;
        for benefit in &benefits {
            let id = prop(benefit, "id").cloned().unwrap_or(Value::Null);
            let awarded = claimed.lookup(&id)?;
            match (awarded, start_at, end_at) {
                (Some(awarded), Some(start), Some(end)) if awarded >= start && awarded < end => {}
                _ => all = false,
            }
            if !all {
                break;
            }
        }
        claimed_by_benefits = all;
    }

    let self_claimed = prop(&self_value, "isClaimed").map(truthy);
    let is_claimed = self_claimed.unwrap_or(claimed_by_benefits);
    let required_minutes = to_number(prop(data, "requiredMinutesWatched"));
    let current_minutes = if is_claimed { required_minutes } else { to_number(prop(&self_value, "currentMinutesWatched")) };

    let mut out = Map::new();
    set(&mut out, "id", prop(data, "id").cloned().unwrap_or_else(crate::js::undefined));
    set(&mut out, "name", prop(data, "name").cloned().unwrap_or_else(crate::js::undefined));
    set(&mut out, "campaignId", campaign_id.cloned().unwrap_or_else(crate::js::undefined));
    set(&mut out, "startsAt", date_value(start_at));
    set(&mut out, "endsAt", date_value(end_at));
    set(&mut out, "requiredMinutes", number_value(required_minutes));
    set(&mut out, "currentMinutes", number_value(current_minutes));
    set(&mut out, "isClaimed", Value::Bool(is_claimed));
    set(&mut out, "claimId", prop(&self_value, "dropInstanceID").cloned().unwrap_or(Value::Null));
    set(&mut out, "benefits", Value::Array(benefits));
    let prerequisites: Vec<Value> = match prop(data, "preconditionDrops") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.iter().map(|item| prop(item, "id").cloned().unwrap_or(Value::Null)).collect(),
        Some(_) => return Err("data.preconditionDrops.map is not a function".into()),
    };
    set(&mut out, "prerequisiteIds", Value::Array(prerequisites));

    let remaining = (required_minutes - current_minutes).max(0.0);
    set(&mut out, "remainingMinutes", number_value(remaining));
    let progress = if is_claimed {
        1.0
    } else if required_minutes == 0.0 {
        0.0
    } else {
        (current_minutes / required_minutes).min(1.0)
    };
    set(&mut out, "progress", number_value(progress));
    set(
        &mut out,
        "canClaim",
        Value::Bool(prop(&self_value, "dropInstanceID").map(truthy).unwrap_or(false) && !is_claimed && current_minutes >= required_minutes),
    );

    Ok(Value::Object(out))
}

fn drop_field<'a>(drop: &'a Value, key: &str) -> Option<&'a Value> {
    prop(drop, key)
}

fn drop_number(drop: &Value, key: &str) -> f64 {
    to_number(drop_field(drop, key))
}

pub fn campaign_data(raw: Value, claimed_benefits: Option<Value>) -> Result<Value, String> {
    if raw.is_null() {
        return Err("Cannot read properties of null (reading 'game')".into());
    }
    let claimed = ClaimedBenefits { provided: claimed_benefits.filter(|value| !value.is_null() && !crate::js::is_undefined(value)) };

    let game = prop(&raw, "game").cloned().unwrap_or(Value::Null);
    let start_at = parse_date(prop(&raw, "startAt"));
    let end_at = parse_date(prop(&raw, "endAt"));

    let drop_items = match prop(&raw, "timeBasedDrops") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items.clone(),
        Some(_) => return Err("raw.timeBasedDrops.map is not a function".into()),
    };
    let campaign_id = prop(&raw, "id").cloned();
    let mut drops = Vec::with_capacity(drop_items.len());
    for item in &drop_items {
        drops.push(build_drop(campaign_id.as_ref(), item, &claimed)?);
    }

    let allow = prop(&raw, "allow").cloned().unwrap_or(Value::Null);
    let allowed_channels = {
        let channels = prop(&allow, "channels");
        let is_enabled = prop(&allow, "isEnabled");
        let list = match channels {
            Some(Value::Array(items)) if !items.is_empty() => items.clone(),
            _ => Vec::new(),
        };
        if !list.is_empty() && is_enabled.map(|value| *value != Value::Bool(false)).unwrap_or(true) {
            let mapped: Vec<Value> = list
                .iter()
                .map(|channel| {
                    let mut out = Map::new();
                    set(&mut out, "id", Value::String(js_string(prop(channel, "id").unwrap_or(&Value::Null))));
                    let login = prop(channel, "name").or_else(|| prop(channel, "login"));
                    set(&mut out, "login", login.cloned().unwrap_or(Value::Null));
                    let display = prop(channel, "displayName").or_else(|| prop(channel, "name")).or_else(|| prop(channel, "login"));
                    set(&mut out, "displayName", display.cloned().unwrap_or(Value::Null));
                    Value::Object(out)
                })
                .collect();
            Value::Array(mapped)
        } else {
            Value::Null
        }
    };

    let game_object = {
        let mut out = Map::new();
        set(&mut out, "id", prop(&game, "id").cloned().unwrap_or(Value::Null));
        let name = prop(&game, "displayName").or_else(|| prop(&game, "name"));
        set(&mut out, "name", name.cloned().unwrap_or_else(|| Value::String("Неизвестная игра".into())));
        let slug = prop(&game, "slug").or_else(|| prop(&game, "name"));
        set(
            &mut out,
            "slug",
            match slug {
                None => Value::String(String::new()),
                Some(value) if value.is_null() => Value::String(String::new()),
                Some(value) => value.clone(),
            },
        );
        set(&mut out, "boxArtUrl", strip_dimensions(prop(&game, "boxArtURL")));
        Value::Object(out)
    };

    let self_value = prop(&raw, "self").cloned().unwrap_or(Value::Null);
    let account_connected = prop(&self_value, "isAccountConnected");
    let has_link = prop(&raw, "accountLinkURL").cloned().unwrap_or(Value::Null);

    let status = prop(&raw, "status").cloned().unwrap_or(Value::Null);
    let status_text = if status.is_null() { "ACTIVE".to_string() } else { js_string(&status) };

    let mut out = Map::new();
    set(&mut out, "id", prop(&raw, "id").cloned().unwrap_or_else(crate::js::undefined));
    set(&mut out, "name", prop(&raw, "name").cloned().unwrap_or_else(crate::js::undefined));
    set(&mut out, "status", Value::String(status_text));
    set(&mut out, "linkUrl", prop(&raw, "accountLinkURL").cloned().unwrap_or(Value::Null));
    set(&mut out, "needsAccountLink", Value::Bool(truthy(&self_value) && account_connected == Some(&Value::Bool(false)) && truthy(&has_link)));
    set(&mut out, "linked", Value::Bool(if truthy(&self_value) { account_connected != Some(&Value::Bool(false)) } else { true }));
    set(&mut out, "startsAt", date_value(start_at));
    set(&mut out, "endsAt", date_value(end_at));
    set(&mut out, "game", game_object);
    set(&mut out, "allowedChannels", allowed_channels);
    set(&mut out, "suggestedChannel", prop(&raw, "suggestedChannel").cloned().unwrap_or(Value::Null));
    set(&mut out, "reward", Value::Bool(prop(&raw, "rewardCampaign") == Some(&Value::Bool(true))));
    let web_url = prop(&raw, "webUrl").cloned().unwrap_or_else(|| {
        if prop(&raw, "rewardCampaign") == Some(&Value::Bool(true)) { Value::String("https://www.twitch.tv/drops/rewards".into()) } else { Value::Null }
    });
    set(&mut out, "webUrl", web_url);
    set(&mut out, "drops", Value::Array(drops));
    Ok(Value::Object(out))
}

/* ---------------------------------------------------------- campaignView */

/// JS rebuilds each drop with real Date objects, which the wire tags as {"$date": …}.
fn drop_with_dates(drop: &Value) -> Value {
    let mut out = match drop {
        Value::Object(map) => map.clone(),
        other => return other.clone(),
    };
    for key in ["startsAt", "endsAt"] {
        if let Some(value) = out.get(key).cloned() {
            out.insert(key.to_string(), date_value(parse_date(Some(&value))));
        }
    }
    Value::Object(out)
}

pub fn campaign_view(data: Value, now: Option<Value>) -> Result<Value, String> {
    let now = now.as_ref().and_then(|value| parse_date(Some(value))).unwrap_or_else(Utc::now);

    let status = prop(&data, "status").map(js_string).unwrap_or_else(|| "ACTIVE".to_string());
    let start_at = parse_date(prop(&data, "startsAt"));
    let end_at = parse_date(prop(&data, "endsAt"));
    let reward = prop(&data, "reward") == Some(&Value::Bool(true));
    let drops: Vec<Value> = match prop(&data, "drops") {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    };

    // JS `&&` returns the falsy operand: no start/end date gives null, not false
    let active = if status == "EXPIRED" {
        Value::Bool(false)
    } else if start_at.is_none() {
        Value::Null
    } else if start_at.map(|value| value > now).unwrap_or(false) {
        Value::Bool(false)
    } else if end_at.is_none() {
        Value::Null
    } else {
        Value::Bool(end_at.map(|value| now < value).unwrap_or(false))
    };
    let upcoming = status == "UPCOMING" || start_at.map(|value| value > now).unwrap_or(false);
    let expired = status == "EXPIRED" || end_at.map(|value| value <= now).unwrap_or(false);

    let claimed_ids: Vec<Value> =
        drops.iter().filter(|drop| prop(drop, "isClaimed").map(truthy).unwrap_or(false)).filter_map(|drop| prop(drop, "id").cloned()).collect();

    let total_required: f64 =
        drops.iter().filter(|drop| !prop(drop, "isClaimed").map(truthy).unwrap_or(false)).map(|drop| drop_number(drop, "remainingMinutes")).sum();
    let earned: f64 = drops
        .iter()
        .map(
            |drop| {
                if prop(drop, "isClaimed").map(truthy).unwrap_or(false) { drop_number(drop, "requiredMinutes") } else { drop_number(drop, "currentMinutes") }
            },
        )
        .sum();
    let total: f64 = drops.iter().map(|drop| drop_number(drop, "requiredMinutes")).sum();

    let farmable: Vec<Value> = drops
        .iter()
        .filter(|drop| {
            if prop(drop, "isClaimed").map(truthy).unwrap_or(false) {
                return false;
            }
            if drop_number(drop, "requiredMinutes") <= 0.0 {
                return false;
            }
            if parse_date(drop_field(drop, "startsAt")).map(|value| value > now).unwrap_or(false) {
                return false;
            }
            if parse_date(drop_field(drop, "endsAt")).map(|value| value <= now).unwrap_or(false) {
                return false;
            }
            match prop(drop, "prerequisiteIds").and_then(|value| value.as_array()) {
                None => true,
                Some(ids) => ids.iter().all(|id| claimed_ids.iter().any(|claimed| claimed == id)),
            }
        })
        .cloned()
        .collect();

    let mut sorted = farmable.clone();
    sorted.sort_by(|a, b| drop_number(a, "remainingMinutes").partial_cmp(&drop_number(b, "remainingMinutes")).unwrap_or(std::cmp::Ordering::Equal));

    let can_earn = if reward { !expired && !farmable.is_empty() } else { !expired && !farmable.is_empty() && end_at.map(|value| value > now).unwrap_or(true) };
    let minutes_left = match end_at {
        Some(value) => crate::js::number((value.timestamp_millis() as f64 - now.timestamp_millis() as f64) / 60000.0),
        None => crate::js::number(f64::INFINITY),
    };

    if data.is_null() {
        return Err("Cannot read properties of null (reading 'startsAt')".into());
    }

    // JS `{...data}` keeps array/string indices as string keys
    let mut out = match data {
        Value::Object(map) => map,
        Value::Array(items) => {
            let mut map = Map::new();
            for (index, item) in items.into_iter().enumerate() {
                map.insert(index.to_string(), item);
            }
            map
        }
        Value::String(text_value) => {
            let mut map = Map::new();
            for (index, ch) in text_value.chars().enumerate() {
                map.insert(index.to_string(), Value::String(ch.to_string()));
            }
            map
        }
        _ => Map::new(),
    };
    set(&mut out, "acceptsChannel", serde_json::json!({ "$function": "acceptsChannel" }));
    set(&mut out, "active", active);
    set(&mut out, "upcoming", Value::Bool(upcoming));
    set(&mut out, "expired", Value::Bool(expired));
    set(&mut out, "minutesLeft", minutes_left);
    set(&mut out, "totalRequiredMinutes", number_value(total_required));
    set(&mut out, "earnedMinutes", number_value(earned));
    set(&mut out, "totalMinutes", number_value(total));
    set(&mut out, "progress", number_value(if total != 0.0 { earned / total } else { 0.0 }));
    set(&mut out, "completed", Value::Bool(!drops.is_empty() && drops.iter().all(|drop| prop(drop, "isClaimed").map(truthy).unwrap_or(false))));
    set(&mut out, "farmableDrops", Value::Array(farmable.iter().map(drop_with_dates).collect()));
    set(&mut out, "canEarn", Value::Bool(can_earn));
    set(&mut out, "nextDrop", sorted.first().map(drop_with_dates).unwrap_or(Value::Null));
    Ok(Value::Object(out))
}

/* ---------------------------------------------------------- groupByGame */

pub fn group_by_game(campaigns: Value) -> Result<Value, String> {
    let items = campaigns.as_array().ok_or_else(|| "campaigns is not iterable".to_string())?;

    let mut order: Vec<String> = Vec::new();
    let mut games: Vec<Map<String, Value>> = Vec::new();

    for campaign in items {
        let game = prop(campaign, "game").ok_or_else(|| "Cannot read properties of undefined (reading 'slug')".to_string())?;
        if !is_object_like(game) {
            return Err("Cannot read properties of undefined (reading 'slug')".into());
        }
        let slug = prop(game, "slug").map(js_string).unwrap_or_default();
        let name = prop(game, "name").map(js_string).unwrap_or_default();
        let key = if !slug.is_empty() { slug } else { name };
        if key.is_empty() {
            continue;
        }
        let index = match order.iter().position(|existing| existing == &key) {
            Some(index) => index,
            None => {
                let mut entry = Map::new();
                set(&mut entry, "slug", Value::String(key.clone()));
                set_opt(&mut entry, "name", prop(game, "name").cloned());
                set_opt(&mut entry, "boxArtUrl", prop(game, "boxArtUrl").cloned());
                set_opt(&mut entry, "gameId", prop(game, "id").cloned());
                set(&mut entry, "campaigns", Value::Array(Vec::new()));
                order.push(key.clone());
                games.push(entry);
                order.len() - 1
            }
        };
        if let Some(list) = games[index].get_mut("campaigns").and_then(|value| value.as_array_mut()) {
            list.push(campaign.clone());
        }
    }

    let mut out = Vec::with_capacity(games.len());
    for entry in games {
        let campaigns = entry.get("campaigns").and_then(|value| value.as_array()).cloned().unwrap_or_default();

        let active: Vec<Value> = campaigns
            .iter()
            .filter(|campaign| truthy(prop(campaign, "active").unwrap_or(&Value::Null)) || truthy(prop(campaign, "upcoming").unwrap_or(&Value::Null)))
            .cloned()
            .collect();

        let mut total_drops = 0.0;
        let mut claimed_drops = 0.0;
        for campaign in &campaigns {
            let drops = prop(campaign, "drops")
                .and_then(|value| value.as_array())
                .ok_or_else(|| "Cannot read properties of undefined (reading 'length')".to_string())?;
            total_drops += drops.len() as f64;
            claimed_drops += drops.iter().filter(|drop| truthy(prop(drop, "isClaimed").unwrap_or(&Value::Null))).count() as f64;
        }

        let ends_at = campaigns.iter().filter_map(|campaign| prop(campaign, "endsAt")).find(|value| truthy(value)).cloned().unwrap_or(Value::Null);

        let can_earn = campaigns.iter().any(|campaign| truthy(prop(campaign, "canEarn").unwrap_or(&Value::Null)));

        let mut game = entry;
        set(&mut game, "activeCampaigns", Value::Array(active));
        set(&mut game, "totalDrops", number_value(total_drops));
        set(&mut game, "claimedDrops", number_value(claimed_drops));
        set(&mut game, "endsAt", ends_at);
        set(&mut game, "canEarn", Value::Bool(can_earn));
        out.push(Value::Object(game));
    }

    Ok(Value::Array(out))
}

/* ------------------------------------------------------------- exports */

pub fn merge_data(primary: Value, secondary: Value) -> Result<Value, String> {
    Ok(merge(primary, secondary))
}
