//! Owner-editable team profile and its normalization rules.

use sea_orm::ActiveValue::Set;
use validator::{Validate, ValidationError, ValidationErrors};

use super::{ActiveModel, Tag, Tags};

/// Transport-independent input for creating or editing a team.
#[derive(Debug, Validate)]
pub struct TeamDetails {
    #[validate(
        length(
            min = 1,
            max = 100,
            message = "Enter a team name of 1 to 100 characters"
        ),
        non_control_character(message = "The team name cannot contain control characters")
    )]
    pub name: String,
    #[validate(
        length(max = 32, message = "The team tag must be at most 32 characters"),
        non_control_character(message = "The team tag cannot contain control characters")
    )]
    pub tag: Option<String>,
    #[validate(length(
        max = 5000,
        message = "The description must be at most 5,000 characters"
    ))]
    pub description: Option<String>,
    #[validate(
        length(max = 2048, message = "The website URL is too long"),
        custom(
            function = "validate_website",
            message = "Enter an HTTP or HTTPS website without login details"
        )
    )]
    pub website_url: Option<String>,
    pub tags: Vec<Tag>,
    pub accepts_applications: bool,
}

impl TeamDetails {
    /// Trims text, drops blank optional fields, validates, then canonicalizes
    /// the website and removes repeated tags.
    pub(super) fn normalized(mut self) -> Result<Self, ValidationErrors> {
        self.name = self.name.trim().to_owned();
        self.tag = non_blank(self.tag);
        self.description = non_blank(self.description);
        self.website_url = non_blank(self.website_url);
        self.validate()?;
        self.website_url = self
            .website_url
            .as_deref()
            .and_then(website)
            .map(|url| url.to_string());
        let mut tags = Vec::with_capacity(self.tags.len());
        for tag in self.tags {
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        self.tags = tags;
        Ok(self)
    }

    pub(super) fn apply(self, active: &mut ActiveModel) {
        active.name = Set(self.name);
        active.tag = Set(self.tag);
        active.description = Set(self.description);
        active.website_url = Set(self.website_url);
        active.tags = Set(Tags(self.tags));
        active.accepts_applications = Set(self.accepts_applications);
        active.updated_at = Set(time::OffsetDateTime::now_utc());
    }
}

fn non_blank(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Accepts bare hosts such as `example.com` by assuming HTTPS.
fn website(value: &str) -> Option<url::Url> {
    let url = if value.contains("://") {
        url::Url::parse(value)
    } else {
        url::Url::parse(&format!("https://{value}"))
    }
    .ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}

fn validate_website(value: &str) -> Result<(), ValidationError> {
    website(value)
        .map(|_| ())
        .ok_or_else(|| ValidationError::new("website"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn details(name: &str) -> TeamDetails {
        TeamDetails {
            name: name.to_owned(),
            tag: None,
            description: None,
            website_url: None,
            tags: Vec::new(),
            accepts_applications: true,
        }
    }

    #[test]
    fn names_are_trimmed_and_counted_in_characters() {
        assert_eq!(details("  Team  ").normalized().unwrap().name, "Team");
        assert!(details(" \t").normalized().is_err());
        assert!(details("bad\nname").normalized().is_err());
        assert!(details(&"é".repeat(100)).normalized().is_ok());
        assert!(details(&"é".repeat(101)).normalized().is_err());
    }

    #[test]
    fn blank_optional_fields_are_dropped_and_tags_deduplicated() {
        let mut input = details("Team");
        input.tag = Some("  ".into());
        input.description = Some(" About us ".into());
        input.tags = vec![Tag::Race, Tag::Drift, Tag::Race];
        let output = input.normalized().unwrap();
        assert_eq!(output.tag, None);
        assert_eq!(output.description.as_deref(), Some("About us"));
        assert_eq!(output.tags, vec![Tag::Race, Tag::Drift]);
    }

    #[test]
    fn websites_default_to_https_and_reject_credentials_or_other_schemes() {
        let mut input = details("Team");
        input.website_url = Some("example.com".into());
        assert_eq!(
            input.normalized().unwrap().website_url.as_deref(),
            Some("https://example.com/")
        );
        for rejected in [
            "ftp://example.com",
            "https://user:pass@example.com",
            "javascript:alert(1)",
            "https://",
        ] {
            let mut input = details("Team");
            input.website_url = Some(rejected.into());
            assert!(input.normalized().is_err(), "{rejected}");
        }
    }
}
