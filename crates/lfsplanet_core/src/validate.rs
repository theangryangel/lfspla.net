//! Shared input checks.

use anyhow::ensure;
use validator::ValidationError;

/// Requires 1 to `max` characters that are not all whitespace.
pub fn bounded_text(value: &str, name: &str, max: usize) -> anyhow::Result<()> {
    ensure!(
        !value.trim().is_empty() && value.chars().count() <= max,
        "{name} must contain 1 to {max} characters"
    );
    Ok(())
}

pub fn validate_non_blank(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new("blank"));
    }
    Ok(())
}

pub fn validate_badge_slug(value: &str) -> Result<(), ValidationError> {
    if value.is_empty()
        || value.len() > 80
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(ValidationError::new("badge_slug"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_slugs_start_alphanumeric_and_use_lowercase_digits_or_hyphens() {
        for valid in ["a", "0", "season-2026", "top-10-", &"a".repeat(80)] {
            assert!(validate_badge_slug(valid).is_ok(), "{valid}");
        }
        for invalid in [
            "",
            "-leading",
            "Upper",
            "under_score",
            "space d",
            "café",
            &"a".repeat(81),
        ] {
            assert!(validate_badge_slug(invalid).is_err(), "{invalid}");
        }
    }
}
