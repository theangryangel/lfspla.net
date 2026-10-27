//! Optional public links, edited by the account owner.
use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(
    Clone, Debug, Default, PartialEq, Serialize, Deserialize, ToSchema, FromJsonQueryResult,
)]
#[serde(default)]
pub struct ProfileLinks {
    #[schema(required)]
    pub twitch_channel: Option<String>,
    #[schema(required)]
    pub youtube_channel: Option<String>,
    #[schema(required)]
    pub youtube_video_id: Option<String>,
    #[schema(required)]
    pub website_url: Option<String>,
}

impl ProfileLinks {
    pub(crate) fn validate(mut self) -> Result<Self, &'static str> {
        for field in [
            &mut self.twitch_channel,
            &mut self.youtube_channel,
            &mut self.youtube_video_id,
            &mut self.website_url,
        ] {
            *field = field
                .take()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty());
        }
        if let Some(channel) = &mut self.twitch_channel {
            if channel.len() > 25
                || !channel
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
            {
                return Err("Enter a Twitch channel name using letters, numbers, or underscores");
            }
            channel.make_ascii_lowercase();
        }
        if let Some(video) = &self.youtube_video_id
            && (video.len() != 11
                || !video
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-'))
        {
            return Err("Enter a valid YouTube video link");
        }
        for (value, youtube) in [
            (&mut self.youtube_channel, true),
            (&mut self.website_url, false),
        ] {
            if let Some(value) = value {
                let url = url::Url::parse(value).map_err(|_| "Enter a valid HTTPS link")?;
                if value.len() > 2048
                    || url.scheme() != "https"
                    || url.host_str().is_none()
                    || !url.username().is_empty()
                    || url.password().is_some()
                {
                    return Err("Enter a valid HTTPS link without login details");
                }
                if youtube
                    && (!matches!(url.host_str(), Some("youtube.com" | "www.youtube.com"))
                        || !(url.path().starts_with("/@")
                            || url.path().starts_with("/channel/")
                            || url.path().starts_with("/c/")
                            || url.path().starts_with("/user/")))
                {
                    return Err(
                        "Enter a YouTube channel link, such as https://www.youtube.com/@yourchannel",
                    );
                }
                *value = url.to_string();
            }
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_links_and_clears_blank_fields() {
        let links = ProfileLinks {
            twitch_channel: Some("  Driver_1 ".into()),
            website_url: Some("  ".into()),
            ..Default::default()
        }
        .validate()
        .unwrap();
        assert_eq!(links.twitch_channel.as_deref(), Some("driver_1"));
        assert_eq!(links.website_url, None);
        assert_eq!(
            serde_json::from_str::<ProfileLinks>("{}").unwrap(),
            ProfileLinks::default()
        );
    }

    #[test]
    fn rejects_unsafe_urls_and_invalid_platform_identifiers() {
        for website in [
            "javascript:alert(1)",
            "http://example.com",
            "https://user:pass@example.com",
        ] {
            assert!(
                ProfileLinks {
                    website_url: Some(website.into()),
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        for channel in [
            "https://youtube.com.evil.test/@driver",
            "https://www.youtube.com/watch?v=abcdefghijk",
        ] {
            assert!(
                ProfileLinks {
                    youtube_channel: Some(channel.into()),
                    ..Default::default()
                }
                .validate()
                .is_err()
            );
        }
        assert!(
            ProfileLinks {
                twitch_channel: Some("driver?parent=evil.test".into()),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            ProfileLinks {
                youtube_video_id: Some("../bad".into()),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
