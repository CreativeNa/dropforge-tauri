//! Первый шаг к Rust-рантайму: сервис сам ходит в сеть, без Node и Electron.
//!
//!   dropforge-twitch-svc                          # сводка по каталогу
//!   dropforge-twitch-svc refresh --token <oauth>  # каталог + награды + Inventory + свежие кампании
//!   SVC_DUMP=1 … refresh                          # подробный список кампаний

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

const DROPS: &str = "https://raw.githubusercontent.com/SunkwiBOT/twitch-drops-api/main/drops.json";
const REWARDS: &str = "https://raw.githubusercontent.com/SunkwiBOT/twitch-drops-api/main/rewards.json";
const GQL: &str = "https://gql.twitch.tv/gql";
const CLIENT_ID: &str = "kimne78kx3ncx6brgo4mv6wki5h1ko";

const HASH_INVENTORY: &str = "8337eb8541b314040b0edde0c09c5c7a2783ba1960aa9edfbf3bac16d0fec404";
const HASH_DASHBOARD: &str = "c16bb890cc8ce7647a96ee69cd313d423a378a3dedadf630a1017cde18975feb";
const HASH_DETAILS: &str = "039277bf98f3130929262cc7c6efd9c141ca3749cb6dca442fc8ead9a53f77c1";
const HASH_DIRECTORY: &str = "86bcceb4e8b1a51256ff8eed8bd8aae4acacf80d737efe904f84f3aeadf8cafd";
const HASH_AVAILABLE: &str = "782dad0f032942260171d2d80a654f88bdd0c5a9dddc392e9bc92218a0f42d20";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(25)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) DropForge/1.0")
        .build()
        .new_agent()
}

fn get_json(agent: &ureq::Agent, url: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(agent.get(url).call()?.body_mut().read_json()?)
}

fn gql(
    agent: &ureq::Agent,
    token: &str,
    operation: &str,
    hash: &str,
    variables: Value,
) -> Result<Value, Box<dyn std::error::Error>> {
    let body = json!([{
        "operationName": operation,
        "extensions": { "persistedQuery": { "version": 1, "sha256Hash": hash } },
        "variables": variables
    }]);
    let response: Value = agent
        .post(GQL)
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "text/plain;charset=UTF-8")
        .header("Origin", "https://www.twitch.tv")
        .header("Authorization", &format!("OAuth {token}"))
        .send(Value::Array(body.as_array().cloned().unwrap_or_default()).to_string())?
        .body_mut()
        .read_json()?;
    Ok(response
        .get(0)
        .and_then(|item| item.get("data"))
        .cloned()
        .unwrap_or(Value::Null))
}

fn text(value: Option<&Value>) -> String {
    value.and_then(Value::as_str).unwrap_or("").to_string()
}

fn iso_ms(value: Option<&Value>) -> Option<i64> {
    let raw = value?.as_str()?;
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|date| date.with_timezone(&Utc).timestamp_millis())
}

/// Кампания в сводке: id, слаг игры, число дропов, статус и конец.
struct Summary {
    id: String,
    slug: String,
    drops: usize,
    expired: bool,
    ends: Option<String>,
}

fn summary_of(campaign: &Value) -> Option<Summary> {
    let id = campaign.get("id").and_then(Value::as_str)?;
    Some(Summary {
        id: id.to_string(),
        slug: text(campaign.get("game").and_then(|game| game.get("slug"))),
        drops: campaign
            .get("timeBasedDrops")
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0),
        expired: campaign.get("status").and_then(Value::as_str) == Some("EXPIRED"),
        ends: campaign
            .get("endAt")
            .or_else(|| campaign.get("endsAt"))
            .and_then(Value::as_str)
            .map(|value| value.to_string()),
    })
}

fn from_catalog(snapshot: &Value) -> Vec<Summary> {
    let mut out = Vec::new();
    let games = snapshot
        .get("games")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for game in games {
        let rewards = game
            .get("rewards")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        out.extend(rewards.iter().filter_map(summary_of));
    }
    out
}

fn from_rewards(snapshot: &Value) -> Vec<Summary> {
    let mut out = Vec::new();
    let campaigns = snapshot
        .get("campaigns")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for campaign in campaigns {
        let Some(id) = campaign.get("id").and_then(Value::as_str) else {
            continue;
        };
        let groups = campaign
            .get("rewardGroups")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let slug = groups
            .first()
            .and_then(|group| group.get("categories"))
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .and_then(|category| category.get("slug"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        let mut drops = 0;
        for group in &groups {
            let minutes = group
                .get("unlockRequirements")
                .and_then(|req| req.get("minuteWatchedGoal"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            if minutes == 0.0 {
                continue;
            }
            drops += group
                .get("rewards")
                .and_then(Value::as_array)
                .map(|items| items.len())
                .unwrap_or(0);
        }

        out.push(Summary {
            id: id.to_string(),
            slug,
            drops,
            expired: campaign.get("status").and_then(Value::as_str) == Some("EXPIRED"),
            ends: campaign
                .get("endsAt")
                .and_then(Value::as_str)
                .map(|value| value.to_string()),
        });
    }
    out
}

fn from_inventory(inventory: &Value) -> Vec<Summary> {
    inventory
        .get("currentUser")
        .and_then(|user| user.get("inventory"))
        .and_then(|inv| inv.get("dropCampaignsInProgress"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(summary_of)
        .collect()
}

fn from_dashboard(dashboard: &Value) -> Vec<Summary> {
    dashboard
        .get("currentUser")
        .and_then(|user| user.get("dropCampaigns"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(summary_of)
        .collect()
}

fn validate_login(agent: &ureq::Agent, token: &str) -> Result<String, Box<dyn std::error::Error>> {
    let response: Value = agent
        .get("https://id.twitch.tv/oauth2/validate")
        .header("Authorization", &format!("OAuth {token}"))
        .call()?
        .body_mut()
        .read_json()?;
    Ok(text(response.get("login")))
}

/// Детали кампаний: пачками по 10, как в приложении.
fn fetch_details(
    agent: &ureq::Agent,
    token: &str,
    login: &str,
    ids: &[String],
) -> Result<BTreeMap<String, Summary>, Box<dyn std::error::Error>> {
    let mut out = BTreeMap::new();
    for chunk in ids.chunks(10) {
        let body: Vec<Value> = chunk
            .iter()
            .map(|id| {
                json!({
                    "operationName": "DropCampaignDetails",
                    "extensions": { "persistedQuery": { "version": 1, "sha256Hash": HASH_DETAILS } },
                    "variables": { "channelLogin": login, "dropID": id }
                })
            })
            .collect();
        let response: Value = agent
            .post(GQL)
            .header("Client-Id", CLIENT_ID)
            .header("Content-Type", "text/plain;charset=UTF-8")
            .header("Origin", "https://www.twitch.tv")
            .header("Authorization", &format!("OAuth {token}"))
            .send(Value::Array(body).to_string())?
            .body_mut()
            .read_json()?;

        let Some(list) = response.as_array() else {
            continue;
        };
        for item in list {
            let Some(campaign) = item
                .get("data")
                .and_then(|data| data.get("user"))
                .and_then(|user| user.get("dropCampaign"))
            else {
                continue;
            };
            if let Some(summary) = summary_of(campaign) {
                out.insert(summary.id.clone(), summary);
            }
        }
    }
    Ok(out)
}

/// Игры, у которых всё закончилось: у Twitch спрашиваем актуальные кампании.
const RAW_DASHBOARD: &str = "query ViewerDropsDashboard { currentUser { id dropCampaigns { id name status startAt endAt accountLinkURL imageURL game { id name slug displayName boxArtURL } allow { isEnabled channels { id name } } timeBasedDrops { id name startAt endAt requiredMinutesWatched preconditionDrops { id } benefitEdges { benefit { id name distributionType imageAssetURL } } self { currentMinutesWatched dropInstanceID isClaimed } } } } }";

fn raw_dashboard(
    agent: &ureq::Agent,
    token: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    let body = json!([{
        "operationName": "ViewerDropsDashboard",
        "query": RAW_DASHBOARD,
        "variables": { "fetchRewardCampaigns": false }
    }]);
    let response: Value = agent
        .post(GQL)
        .header("Client-Id", CLIENT_ID)
        .header("Content-Type", "text/plain;charset=UTF-8")
        .header("Origin", "https://www.twitch.tv")
        .header("Authorization", &format!("OAuth {token}"))
        .send(Value::Array(body.as_array().cloned().unwrap_or_default()).to_string())?
        .body_mut()
        .read_json()?;
    Ok(response
        .get(0)
        .and_then(|item| item.get("data"))
        .cloned()
        .unwrap_or(Value::Null))
}

fn probe_stale(
    agent: &ureq::Agent,
    token: &str,
    merged: &BTreeMap<String, Summary>,
    focus: &[String],
) -> Result<Vec<Summary>, Box<dyn std::error::Error>> {
    let now = Utc::now().timestamp_millis();
    let mut latest: BTreeMap<String, i64> = BTreeMap::new();
    for item in merged.values() {
        if item.slug.is_empty() {
            continue;
        }
        let end = item
            .ends
            .as_deref()
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|date| date.with_timezone(&Utc).timestamp_millis())
            .unwrap_or(i64::MIN);
        let entry = latest.entry(item.slug.clone()).or_insert(i64::MIN);
        if end > *entry {
            *entry = end;
        }
    }

    // игры из очереди проверяем всегда, как это делает приложение
    let mut stale: Vec<String> = focus.to_vec();
    for (slug, end) in &latest {
        if *end < now && !stale.contains(slug) {
            stale.push(slug.clone());
        }
    }
    stale.truncate(6);

    let mut found = Vec::new();
    for slug in stale {
        // сначала каналы с включёнными Drops, иначе — любые живые: у кампаний
        // с наградами за просмотр категории часы идут и без Drops
        let mut channel_id: Option<String> = None;
        for drops_only in [true, false] {
            if channel_id.is_some() {
                break;
            }
            let filters = if drops_only {
                json!(["DROPS_ENABLED"])
            } else {
                json!([])
            };
            let directory = gql(
                agent,
                token,
                "DirectoryPage_Game",
                HASH_DIRECTORY,
                json!({
                    "limit": 30,
                    "slug": slug,
                    "imageWidth": 50,
                    "includeCostreaming": false,
                    "options": {
                        "broadcasterLanguages": [],
                        "freeformTags": null,
                        "includeRestricted": ["SUB_ONLY_LIVE"],
                        "recommendationsContext": { "platform": "web" },
                        "sort": "RELEVANCE",
                        "systemFilters": filters,
                        "tags": []
                    },
                    "sortTypeIsRecency": false
                }),
            )?;
            // как в приложении: чем меньше зрителей, тем «дешевле» канал
            let mut streams: Vec<(i64, String)> = directory
                .get("game")
                .and_then(|game| game.get("streams"))
                .and_then(|streams| streams.get("edges"))
                .and_then(Value::as_array)
                .map(|edges| {
                    edges
                        .iter()
                        .filter_map(|edge| {
                            let node = edge.get("node")?;
                            let id = node
                                .get("broadcaster")
                                .and_then(|broadcaster| broadcaster.get("id"))
                                .and_then(Value::as_str)?;
                            let viewers = node
                                .get("viewersCount")
                                .and_then(Value::as_i64)
                                .unwrap_or(0);
                            Some((viewers, id.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            streams.sort_by_key(|(viewers, _)| *viewers);
            channel_id = streams.first().map(|(_, id)| id.clone());
        }

        let Some(channel_id) = channel_id else {
            continue;
        };

        let available = gql(
            agent,
            token,
            "DropsHighlightService_AvailableDrops",
            HASH_AVAILABLE,
            json!({ "channelID": channel_id.clone() }),
        )?;
        if let Some(list) = available
            .get("channel")
            .and_then(|channel| channel.get("viewerDropCampaigns"))
            .and_then(Value::as_array)
        {
            found.extend(list.iter().filter_map(summary_of));
        }
    }
    Ok(found)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().cloned().unwrap_or_default();
    let mut token = String::new();
    let mut focus: Vec<String> = Vec::new();
    if let Some(index) = args.iter().position(|arg| arg == "--focus") {
        args.remove(index);
        if index < args.len() {
            let raw = args.remove(index);
            focus = raw
                .split(',')
                .map(|part| part.trim().to_string())
                .filter(|part| !part.is_empty())
                .collect();
        }
    }
    if let Some(index) = args.iter().position(|arg| arg == "--token") {
        args.remove(index);
        if index < args.len() {
            token = args.remove(index);
        }
    }

    let agent = agent();

    if mode == "refresh" {
        if token.is_empty() {
            return Err("refresh требует --token <oauth>".into());
        }
        let drops_snapshot = get_json(&agent, DROPS)?;
        let rewards_snapshot = get_json(&agent, REWARDS)?;
        let inventory = gql(&agent, &token, "Inventory", HASH_INVENTORY, json!({ "fetchRewardCampaigns": false }))?;
        let mut dashboard = gql(&agent, &token, "ViewerDropsDashboard", HASH_DASHBOARD, json!({ "fetchRewardCampaigns": false }))?;
        let mut dashboard_items = from_dashboard(&dashboard);
        if dashboard_items.is_empty() {
            // Twitch отдаёт пустой список по persisted-запросу, но тот же набор
            // полей работает обычным GraphQL-запросом — как в приложении
            dashboard = raw_dashboard(&agent, &token)?;
            dashboard_items = from_dashboard(&dashboard);
        }

        let mut merged: BTreeMap<String, Summary> = BTreeMap::new();
        for item in from_catalog(&drops_snapshot)
            .into_iter()
            .chain(from_rewards(&rewards_snapshot))
            .chain(dashboard_items)
        {
            if item.expired {
                continue;
            }
            merged.insert(item.id.clone(), item);
        }

        let in_progress = from_inventory(&inventory);
        let login = validate_login(&agent, &token)?;
        let ids: Vec<String> = in_progress.iter().map(|item| item.id.clone()).collect();
        let details = fetch_details(&agent, &token, &login, &ids)?;

        for item in &in_progress {
            // движок отбрасывает кампанию только по статусу из Inventory
            if item.expired {
                continue;
            }
            let resolved = details.get(&item.id);
            merged.insert(
                item.id.clone(),
                Summary {
                    id: item.id.clone(),
                    slug: resolved
                        .map(|entry| entry.slug.clone())
                        .filter(|slug| !slug.is_empty())
                        .unwrap_or_else(|| item.slug.clone()),
                    drops: resolved.map(|entry| entry.drops).unwrap_or(item.drops),
                    expired: false,
                    ends: resolved
                        .and_then(|entry| entry.ends.clone())
                        .or_else(|| item.ends.clone()),
                },
            );
        }

        // то же, что делает приложение: добираем кампании у игр, где всё закончилось
        for item in probe_stale(&agent, &token, &merged, &focus)? {
            if item.expired {
                continue;
            }
            merged.insert(item.id.clone(), item);
        }

        if std::env::var("SVC_DUMP").is_ok() {
            let dump: Vec<Value> = merged
                .values()
                .map(|item| {
                    json!({
                        "id": item.id,
                        "slug": item.slug,
                        "drops": item.drops,
                        "expired": item.expired,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string(&dump)?);
            return Ok(());
        }

        let live: Vec<&Summary> = merged.values().filter(|item| !item.expired).collect();
        let games: BTreeSet<&str> = live
            .iter()
            .map(|item| item.slug.as_str())
            .filter(|slug| !slug.is_empty())
            .collect();
        let drops: usize = live.iter().map(|item| item.drops).sum();

        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "campaigns": live.len(),
                "games": games.len(),
                "drops": drops,
                "inventoryCampaigns": in_progress.len(),
            }))?
        );
        return Ok(());
    }

    let snapshot = get_json(&agent, DROPS)?;
    let catalog = from_catalog(&snapshot);
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "games": snapshot.get("games").and_then(Value::as_array).map(|items| items.len()).unwrap_or(0),
            "campaigns": catalog.iter().filter(|item| !item.expired).count(),
            "drops": catalog.iter().filter(|item| !item.expired).map(|item| item.drops).sum::<usize>(),
        }))?
    );
    Ok(())
}
