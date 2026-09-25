//! Shared input checks.

use validator::ValidationError;

pub(crate) fn validate_non_blank(value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        return Err(ValidationError::new("blank"));
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
