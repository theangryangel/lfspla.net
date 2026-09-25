//! Vehicle metadata stored in SPR headers.

use std::fmt;

/// The vehicle display name written into an SPR header by LFS.
///
/// Vehicle mods conventionally append a three-digit revision as `~NNN`.
/// Standard vehicle names and malformed suffixes are retained unchanged.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SprVehicle(String);

impl SprVehicle {
    /// Borrows the complete display name exactly as it appeared in the header.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Splits the display name into its vehicle name and optional mod revision.
    ///
    /// A suffix is recognised only when it is exactly `~NNN`. If the suffix is
    /// absent or malformed, the complete original name is returned with no
    /// revision.
    pub fn into_parts(&self) -> (&str, Option<u16>) {
        let Some((name, revision)) = self.0.rsplit_once('~') else {
            return (self.as_str(), None);
        };
        let name = name.trim();
        if name.is_empty()
            || revision.len() != 3
            || !revision.bytes().all(|byte| byte.is_ascii_digit())
        {
            return (self.as_str(), None);
        }
        let Some(revision) = revision.parse().ok() else {
            return (self.as_str(), None);
        };
        (name, Some(revision))
    }
}

impl From<String> for SprVehicle {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SprVehicle {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl AsRef<str> for SprVehicle {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SprVehicle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_a_mod_name_and_revision() {
        assert_eq!(
            SprVehicle::from("PIRAN FIREFLY 200~007").into_parts(),
            ("PIRAN FIREFLY 200", Some(7))
        );
    }

    #[test]
    fn preserves_standard_names_and_malformed_revisions() {
        assert_eq!(SprVehicle::from("LX4").into_parts(), ("LX4", None));
        assert_eq!(
            SprVehicle::from("broken~7").into_parts(),
            ("broken~7", None)
        );
    }
}
