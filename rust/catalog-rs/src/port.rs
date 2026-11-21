//! The Rust port of `src/core/catalog.js` (the pure mapping half).

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
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().unwrap_or(f64::NAN)
            }
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
    Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn set(target: &mut Map<String, Value>, key: &str, value: Value) {
    target.insert(key.to_string(), value);
}

/// `String(url).replace(/-\d+x\d+(?=\.\w+$)/, '-285x380')`
fn normalize_box_art(url: Option<&Value>) -> Value {
    let Some(value) = url else { return Value::Null };
    if !truthy(value) {
        return Value::Null;
    }
    let text = js_string(value);
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    let mut dot = None;
    for i in (0..len).rev() {
        if chars[i] == '.' {
            if i + 1 < len
                && chars[i + 1..]
                    .iter()
                    .all(|c| c.is_alphanumeric() || *c == '_')
            {
                dot = Some(i);
            }
            break;
        }
    }
    let Some(dot) = dot else {
        return Value::String(text);
    };

    let mut j = dot;
    let mut after = 0;
    while j > 0 && chars[j - 1].is_ascii_digit() {
        j -= 1;
        after += 1;
    }
    if after == 0 || j == 0 || chars[j - 1] != 'x' {
        return Value::String(text);
    }
    j -= 1;
    let mut before = 0;
    while j > 0 && chars[j - 1].is_ascii_digit() {
        j -= 1;
        before += 1;
    }
    if before == 0 || j == 0 || chars[j - 1] != '-' {
        return Value::String(text);
    }
    j -= 1;

    let mut out: String = chars[..j].iter().collect();
    out.push_str("-285x380");
    out.extend(chars[dot..].iter());
    Value::String(out)
}

/* ----------------------------------------------------------- drop payload */

fn channel_value(channel: &Value) -> Value {
    let mut out = Map::new();
    set(
        &mut out,
        "id",
        prop(channel, "id").cloned().unwrap_or(Value::Null),
    );
    let name = prop(channel, "displayName").or_else(|| prop(channel, "name"));
    set(&mut out, "name", name.cloned().unwrap_or(Value::Null));
    Value::Object(out)
}

fn benefit_value(edge: &Value) -> Value {
    let benefit = prop(edge, "benefit");
    let mut out = Map::new();
    set(
        &mut out,
        "id",
        benefit
            .and_then(|b| prop(b, "id"))
            .cloned()
            .unwrap_or(Value::Null),
    );
    set(
        &mut out,
        "name",
        benefit
            .and_then(|b| prop(b, "name"))
            .cloned()
            .unwrap_or_else(|| Value::String("Награда".into())),
    );
    set(
        &mut out,
        "distributionType",
        benefit
            .and_then(|b| prop(b, "distributionType"))
            .cloned()
            .unwrap_or_else(|| Value::String("DIRECT_ENTITLEMENT".into())),
    );
    set(
        &mut out,
        "imageAssetURL",
        benefit
            .and_then(|b| prop(b, "imageAssetURL"))
            .cloned()
            .unwrap_or(Value::Null),
    );
    Value::Object(out)
}

fn as_array(value: Option<&Value>, message: &str) -> Result<Vec<Value>, String> {
    match value {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => Ok(items.clone()),
        Some(_) => Err(message.to_string()),
    }
}

fn raw_campaign(group: &Value, campaign: &Value) -> Value {
    let game = prop(campaign, "game").cloned().unwrap_or(Value::Null);
    let allow = prop(campaign, "allow").cloned().unwrap_or(Value::Null);

    let game_id = prop(&game, "id")
        .or_else(|| prop(group, "gameId"))
        .cloned()
        .unwrap_or(Value::Null);
    let game_name = prop(&game, "displayName")
        .or_else(|| prop(&game, "name"))
        .or_else(|| prop(group, "gameDisplayName"))
        .cloned()
        .unwrap_or_else(|| Value::String("Неизвестная игра".into()));
    let game_slug = prop(&game, "slug")
        .cloned()
        .unwrap_or_else(|| Value::String(String::new()));

    let mut game_out = Map::new();
    set(&mut game_out, "id", game_id);
    set(&mut game_out, "name", game_name);
    set(&mut game_out, "slug", game_slug);
    set(
        &mut game_out,
        "boxArtURL",
        normalize_box_art(prop(group, "gameBoxArtURL")),
    );

    let allow_out = {
        let mut out = Map::new();
        set(
            &mut out,
            "isEnabled",
            Value::Bool(prop(&allow, "isEnabled") == Some(&Value::Bool(true))),
        );
        let channels = as_array(prop(&allow, "channels"), "allow.channels is not iterable")
            .unwrap_or_default()
            .iter()
            .map(channel_value)
            .collect();
        set(&mut out, "channels", Value::Array(channels));
        Value::Object(out)
    };

    let drops = as_array(
        prop(campaign, "timeBasedDrops"),
        "timeBasedDrops is not iterable",
    )
    .unwrap_or_default()
    .iter()
    .enumerate()
    .map(|(index, drop)| {
        let mut out = Map::new();
        set(
            &mut out,
            "id",
            prop(drop, "id").cloned().unwrap_or(Value::Null),
        );
        set(
            &mut out,
            "name",
            prop(drop, "name")
                .cloned()
                .unwrap_or_else(|| Value::String(format!("Награда {}", index + 1))),
        );
        set(
            &mut out,
            "startAt",
            prop(drop, "startAt").cloned().unwrap_or(Value::Null),
        );
        set(
            &mut out,
            "endAt",
            prop(drop, "endAt").cloned().unwrap_or(Value::Null),
        );
        set(
            &mut out,
            "requiredMinutesWatched",
            prop(drop, "requiredMinutesWatched")
                .cloned()
                .unwrap_or(Value::Null),
        );
        set(&mut out, "preconditionDrops", Value::Array(Vec::new()));
        let edges = as_array(prop(drop, "benefitEdges"), "benefitEdges is not iterable")
            .unwrap_or_default()
            .iter()
            .map(|edge| {
                let mut edge_out = Map::new();
                set(&mut edge_out, "benefit", benefit_value(edge));
                Value::Object(edge_out)
            })
            .collect();
        set(&mut out, "benefitEdges", Value::Array(edges));
        Value::Object(out)
    })
    .collect();

    let mut out = Map::new();
    set(
        &mut out,
        "id",
        prop(campaign, "id").cloned().unwrap_or(Value::Null),
    );
    set(
        &mut out,
        "name",
        prop(campaign, "name").cloned().unwrap_or(Value::Null),
    );
    set(
        &mut out,
        "status",
        prop(campaign, "status")
            .cloned()
            .unwrap_or_else(|| Value::String("ACTIVE".into())),
    );
    set(
        &mut out,
        "startAt",
        prop(campaign, "startAt").cloned().unwrap_or(Value::Null),
    );
    set(
        &mut out,
        "endAt",
        prop(campaign, "endAt").cloned().unwrap_or(Value::Null),
    );
    set(
        &mut out,
        "accountLinkURL",
        prop(campaign, "accountLinkURL")
            .cloned()
            .unwrap_or(Value::Null),
    );
    set(&mut out, "game", Value::Object(game_out));
    set(&mut out, "allow", allow_out);
    set(&mut out, "timeBasedDrops", Value::Array(drops));
    set(&mut out, "suggestedChannel", Value::Null);
    set(&mut out, "source", Value::String("catalog".into()));
    Value::Object(out)
}

pub fn parse_drop_payload(payload: Value) -> Result<Value, String> {
    let games = prop(&payload, "games").cloned().unwrap_or(Value::Null);
    let games = match games {
        Value::Array(items) => items,
        _ => return Err("неожиданный формат ответа".into()),
    };

    let mut campaigns = Vec::new();
    for group in &games {
        let rewards = as_array(prop(group, "rewards"), "group.rewards is not iterable")?;
        for campaign in &rewards {
            let id = prop(campaign, "id").cloned().unwrap_or(Value::Null);
            if !truthy(&id) {
                continue;
            }
            if prop(campaign, "status") == Some(&Value::String("EXPIRED".into())) {
                continue;
            }
            campaigns.push(raw_campaign(group, campaign));
        }
    }
    Ok(Value::Array(campaigns))
}

/* --------------------------------------------------------- reward payload */

const REWARDS_PAGE: &str = "https://www.twitch.tv/drops/rewards";

fn reward_campaign(campaign: &Value) -> Value {
    let groups = as_array(
        prop(campaign, "rewardGroups"),
        "rewardGroups is not iterable",
    )
    .unwrap_or_default();
    let category = groups
        .first()
        .and_then(|group| prop(group, "categories"))
        .and_then(|value| value.as_array())
        .and_then(|items| items.first())
        .cloned()
        .or_else(|| prop(campaign, "game").cloned())
        .unwrap_or(Value::Object(Map::new()));

    let mut drops = Vec::new();
    for group in &groups {
        let minutes = to_number(
            prop(group, "unlockRequirements").and_then(|value| prop(value, "minuteWatchedGoal")),
        );
        if minutes == 0.0 {
            continue;
        }
        let rewards =
            as_array(prop(group, "rewards"), "group.rewards is not iterable").unwrap_or_default();
        for reward in &rewards {
            let id = prop(reward, "id").cloned().unwrap_or(Value::Null);
            if !truthy(&id) {
                continue;
            }
            let mut drop = Map::new();
            set(&mut drop, "id", id.clone());
            set(
                &mut drop,
                "name",
                prop(reward, "name")
                    .cloned()
                    .unwrap_or_else(|| Value::String("Награда".into())),
            );
            set(
                &mut drop,
                "startAt",
                prop(campaign, "startsAt").cloned().unwrap_or(Value::Null),
            );
            set(
                &mut drop,
                "endAt",
                prop(reward, "earnableUntil")
                    .cloned()
                    .or_else(|| prop(campaign, "endsAt").cloned())
                    .unwrap_or(Value::Null),
            );
            set(&mut drop, "requiredMinutesWatched", number_value(minutes));
            set(&mut drop, "preconditionDrops", Value::Array(Vec::new()));
            let image = prop(reward, "thumbnailImage")
                .and_then(|value| prop(value, "image1xURL"))
                .cloned()
                .unwrap_or(Value::Null);
            let mut benefit = Map::new();
            set(
                &mut benefit,
                "id",
                prop(reward, "id").cloned().unwrap_or(Value::Null),
            );
            set(
                &mut benefit,
                "name",
                prop(reward, "name")
                    .cloned()
                    .unwrap_or_else(|| Value::String("Награда".into())),
            );
            set(
                &mut benefit,
                "distributionType",
                Value::String("QUEST_REWARD".into()),
            );
            set(&mut benefit, "imageAssetURL", image);
            let mut edge = Map::new();
            set(&mut edge, "benefit", Value::Object(benefit));
            set(
                &mut drop,
                "benefitEdges",
                Value::Array(vec![Value::Object(edge)]),
            );
            drops.push(Value::Object(drop));
        }
    }

    let category_id = prop(&category, "id").cloned().unwrap_or(Value::Null);
    let box_art = match &category_id {
        Value::Null => Value::Null,
        other => Value::String(format!(
            "https://static-cdn.jtvnw.net/ttv-boxart/{}-285x380.jpg",
            js_string(other)
        )),
    };

    let mut game = Map::new();
    set(&mut game, "id", category_id);
    set(
        &mut game,
        "name",
        prop(&category, "displayName")
            .or_else(|| prop(&category, "name"))
            .cloned()
            .unwrap_or_else(|| Value::String("Twitch".into())),
    );
    set(
        &mut game,
        "slug",
        prop(&category, "slug")
            .cloned()
            .unwrap_or_else(|| Value::String(String::new())),
    );
    set(&mut game, "boxArtURL", box_art);

    let mut allow = Map::new();
    set(&mut allow, "isEnabled", Value::Bool(false));
    set(&mut allow, "channels", Value::Array(Vec::new()));

    let mut out = Map::new();
    set(
        &mut out,
        "id",
        prop(campaign, "id").cloned().unwrap_or(Value::Null),
    );
    set(
        &mut out,
        "name",
        prop(campaign, "name").cloned().unwrap_or(Value::Null),
    );
    set(
        &mut out,
        "status",
        prop(campaign, "status")
            .cloned()
            .unwrap_or_else(|| Value::String("ACTIVE".into())),
    );
    set(
        &mut out,
        "startAt",
        prop(campaign, "startsAt").cloned().unwrap_or(Value::Null),
    );
    set(
        &mut out,
        "endAt",
        prop(campaign, "endsAt").cloned().unwrap_or(Value::Null),
    );
    set(&mut out, "accountLinkURL", Value::Null);
    set(&mut out, "game", Value::Object(game));
    set(&mut out, "allow", Value::Object(allow));
    set(&mut out, "timeBasedDrops", Value::Array(drops));
    set(&mut out, "rewardCampaign", Value::Bool(true));
    set(
        &mut out,
        "webUrl",
        prop(campaign, "webUrl")
            .cloned()
            .unwrap_or_else(|| Value::String(REWARDS_PAGE.into())),
    );
    set(&mut out, "suggestedChannel", Value::Null);
    set(&mut out, "source", Value::String("reward".into()));
    Value::Object(out)
}

pub fn parse_reward_payload(payload: Value) -> Result<Value, String> {
    let campaigns = prop(&payload, "campaigns").cloned().unwrap_or(Value::Null);
    let campaigns = match campaigns {
        Value::Array(items) => items,
        _ => return Err("неожиданный формат ответа".into()),
    };

    let mapped: Vec<Value> = campaigns
        .iter()
        .filter(|campaign| {
            let id = prop(campaign, "id").cloned().unwrap_or(Value::Null);
            truthy(&id) && prop(campaign, "status") != Some(&Value::String("EXPIRED".into()))
        })
        .map(reward_campaign)
        .collect();

    let filtered: Vec<Value> = mapped
        .into_iter()
        .filter(|campaign| {
            let slug = prop(campaign, "game")
                .and_then(|game| prop(game, "slug"))
                .map(truthy)
                .unwrap_or(false);
            let drops = prop(campaign, "timeBasedDrops")
                .and_then(|value| value.as_array())
                .map(|items| !items.is_empty())
                .unwrap_or(false);
            slug && drops
        })
        .collect();

    Ok(Value::Array(filtered))
}
