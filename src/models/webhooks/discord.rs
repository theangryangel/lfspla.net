//! Discord message rendering and validation of credential-bearing webhook URLs.

use super::WebhookEvent;
use serde::Serialize;
use url::Url;

#[derive(Serialize)]
pub struct DiscordMessage {
    embeds: Vec<Embed>,
    allowed_mentions: AllowedMentions,
}
#[derive(Serialize)]
struct AllowedMentions {
    parse: Vec<String>,
}
#[derive(Serialize)]
struct Embed {
    title: String,
    url: String,
    fields: Vec<Field>,
}
#[derive(Serialize)]
struct Field {
    name: &'static str,
    value: String,
    inline: bool,
}

pub fn render(event: &WebhookEvent, public_base_url: &Url) -> DiscordMessage {
    let (title, driver, era, track, vehicle, lap_time_ms, rank) = match event {
        WebhookEvent::HotlapValidated {
            driver,
            era,
            track,
            vehicle,
            lap_time_ms,
            rank,
            ..
        } => (
            "New Hotlap!",
            driver,
            era,
            track,
            vehicle,
            lap_time_ms,
            rank,
        ),
        WebhookEvent::WorldRecordSet {
            driver,
            era,
            track,
            vehicle,
            lap_time_ms,
            rank,
            ..
        } => (
            "New World Record!",
            driver,
            era,
            track,
            vehicle,
            lap_time_ms,
            rank,
        ),
    };
    let mut chart = public_base_url.clone();
    chart.set_query(None);
    chart.set_fragment(None);
    chart
        .path_segments_mut()
        .expect("public base URL has a host")
        .clear()
        .extend(["hotlaps", era, "charts", track, vehicle]);
    DiscordMessage {
        embeds: vec![Embed {
            title: title.into(),
            url: chart.into(),
            fields: vec![
                Field {
                    name: "Driver",
                    value: escape(driver),
                    inline: true,
                },
                Field {
                    name: "Track",
                    value: escape(track),
                    inline: true,
                },
                Field {
                    name: "Vehicle",
                    value: escape(vehicle),
                    inline: true,
                },
                Field {
                    name: "Lap time",
                    value: format!(
                        "{}:{:02}.{:03}",
                        lap_time_ms / 60_000,
                        lap_time_ms / 1_000 % 60,
                        lap_time_ms % 1_000
                    ),
                    inline: true,
                },
                Field {
                    name: "Rank",
                    value: rank.map_or_else(|| "Unranked".into(), |rank| format!("#{rank}")),
                    inline: true,
                },
            ],
        }],
        allowed_mentions: AllowedMentions { parse: Vec::new() },
    }
}

fn escape(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars().take(200) {
        if "\\`*_{}[]()<>~|".contains(ch) {
            result.push('\\');
        }
        if !ch.is_control() {
            result.push(ch);
        }
    }
    result
}

/// Restrict this format to Discord's HTTPS execution endpoint, preventing
/// arbitrary outbound requests. Canonicalize aliases and discard query options.
pub fn webhook_url(value: &str) -> Result<Url, &'static str> {
    let invalid = "Enter a Discord webhook URL: https://discord.com/api/webhooks/ID/TOKEN";
    let mut url = Url::parse(value.trim()).map_err(|_| invalid)?;
    if url.scheme() != "https"
        || !matches!(url.host_str(), Some("discord.com" | "discordapp.com"))
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || value.len() > 2048
    {
        return Err(invalid);
    }
    let parts: Vec<_> = url.path_segments().ok_or(invalid)?.collect();
    let (id, token) = match parts.as_slice() {
        ["api", "webhooks", id, token] | ["api", "v10", "webhooks", id, token] => (*id, *token),
        _ => return Err(invalid),
    };
    if id.is_empty()
        || !id.bytes().all(|c| c.is_ascii_digit())
        || token.is_empty()
        || !token
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
    {
        return Err(invalid);
    }
    let path = format!("/api/webhooks/{id}/{token}");
    url.set_host(Some("discord.com")).map_err(|_| invalid)?;
    url.set_path(&path);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_reject_other_hosts_credentials_and_endpoint_suffixes() {
        for url in [
            "http://discord.com/api/webhooks/123/abc",
            "https://localhost/api/webhooks/123/abc",
            "https://discord.com.evil.test/api/webhooks/123/abc",
            "https://user@discord.com/api/webhooks/123/abc",
            "https://discord.com/api/webhooks/123/abc/messages",
            "https://discord.com/api/webhooks/123/abc?thread_id=1",
            "https://discord.com/api/webhooks/123/a%2fb",
        ] {
            assert!(webhook_url(url).is_err(), "{url}");
        }
        assert_eq!(
            webhook_url("https://discordapp.com/api/v10/webhooks/123/abc_-123")
                .unwrap()
                .as_str(),
            "https://discord.com/api/webhooks/123/abc_-123"
        );
    }

    #[test]
    fn render_snapshot_includes_rank_and_omits_replay_download() {
        let event = WebhookEvent::HotlapValidated {
            hotlap_id: 42,
            driver: "@everyone **driver**".into(),
            era: "current".into(),
            track: "BL1".into(),
            vehicle: "XFG".into(),
            lap_time_ms: 91234,
            rank: None,
        };
        let public_base_url = Url::parse("https://lfspla.net").unwrap();
        let message = serde_json::to_value(render(&event, &public_base_url)).unwrap();
        assert_eq!(message["allowed_mentions"]["parse"], serde_json::json!([]));
        assert_eq!(message["embeds"][0]["title"], "New Hotlap!");
        assert_eq!(message["embeds"][0]["fields"][3]["value"], "1:31.234");
        assert_eq!(message["embeds"][0]["fields"][4]["name"], "Rank");
        assert_eq!(message["embeds"][0]["fields"][4]["value"], "Unranked");
        assert_eq!(message["embeds"][0]["fields"].as_array().unwrap().len(), 5);
        assert_eq!(
            message["embeds"][0]["url"],
            "https://lfspla.net/hotlaps/current/charts/BL1/XFG"
        );
        assert_eq!(
            serde_json::from_value::<WebhookEvent>(serde_json::to_value(&event).unwrap()).unwrap(),
            event
        );
        assert_eq!(
            super::super::WebhookEventKind::from(&event),
            super::super::WebhookEventKind::HotlapValidated
        );

        let record = WebhookEvent::WorldRecordSet {
            hotlap_id: 42,
            driver: "driver".into(),
            era: "current".into(),
            track: "BL1".into(),
            vehicle: "XFG".into(),
            lap_time_ms: 91234,
            rank: Some(1),
        };
        let rendered = serde_json::to_value(render(&record, &public_base_url)).unwrap();
        assert_eq!(rendered["embeds"][0]["title"], "New World Record!");
        assert_eq!(rendered["embeds"][0]["fields"][4]["value"], "#1");
        assert_eq!(
            super::super::WebhookEventKind::from(&record),
            super::super::WebhookEventKind::WorldRecordSet
        );
    }

    #[test]
    fn old_event_snapshots_default_to_unranked() {
        let event: WebhookEvent = serde_json::from_value(serde_json::json!({
            "type": "hotlap_validated",
            "hotlap_id": 42,
            "driver": "driver",
            "era": "current",
            "track": "BL1",
            "vehicle": "XFG",
            "lap_time_ms": 91234
        }))
        .unwrap();
        assert!(matches!(
            event,
            WebhookEvent::HotlapValidated { rank: None, .. }
        ));
    }
}
