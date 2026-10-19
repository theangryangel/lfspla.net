//! Shared input checks.

use validator::ValidationError;

pub(crate) fn validate_non_blank(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new("blank"));
    }
    Ok(())
}

pub(crate) fn validate_badge_slug(value: &str) -> Result<(), ValidationError> {
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
    fn non_blank_values_may_have_surrounding_whitespace() {
        assert!(validate_non_blank(" value ").is_ok());
        assert!(validate_non_blank(" \t").is_err());
    }
}
