//! Country flag choices from FlagCDN. See README.md for source and policy.

#[cfg(feature = "sea-orm")]
mod sea_orm;

use celes::Country;
use serde::Serialize;

include!(concat!(env!("OUT_DIR"), "/catalogue.rs"));

fn catalogue_entry(code: &str) -> Option<&'static (&'static str, &'static str)> {
    CATALOGUE
        .binary_search_by_key(&code, |&(code, _)| code)
        .ok()
        .map(|index| &CATALOGUE[index])
}

/// A valid, lowercase FlagCDN code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct FlagCode(&'static str);

impl FlagCode {
    /// Trims whitespace and accepts either case.
    pub fn parse(code: &str) -> Option<Self> {
        catalogue_entry(code.trim().to_ascii_lowercase().as_str()).map(|&(code, _)| Self(code))
    }

    pub fn as_str(self) -> &'static str {
        self.0
    }

    /// English name from FlagCDN.
    pub fn name(self) -> &'static str {
        catalogue_entry(self.0).expect("validated flag code").1
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Flag {
    pub code: FlagCode,
    pub name: &'static str,
}

/// Allowed flags for a country.
pub trait CountryFlagsExt {
    /// National flag first.
    fn flags(&self) -> Vec<Flag>;
    fn allows_flag(&self, flag: FlagCode) -> bool;
}

fn country_flags(country: &Country) -> &'static [Flag] {
    COUNTRY_FLAGS
        .binary_search_by_key(&country.alpha2, |&(code, _)| code)
        .ok()
        .map_or(&[], |index| COUNTRY_FLAGS[index].1)
}

impl CountryFlagsExt for Country {
    fn flags(&self) -> Vec<Flag> {
        country_flags(self).to_vec()
    }

    fn allows_flag(&self, flag: FlagCode) -> bool {
        country_flags(self).iter().any(|choice| choice.code == flag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn national_flag_first_and_un_available() {
        for country in Country::get_countries() {
            let flags = country.flags();
            assert!(flags[0].code.as_str().eq_ignore_ascii_case(country.alpha2));
            assert!(flags.iter().any(|flag| flag.code.as_str() == "un"));
        }
    }

    #[test]
    fn flags_respect_country_and_region() {
        let flags = Country::from_alpha2("GB").unwrap().flags();
        for code in ["gb-eng", "gb-nir", "gb-sct", "gb-wls"] {
            assert!(flags.iter().any(|flag| flag.code.as_str() == code));
        }
        assert!(!flags.iter().any(|flag| flag.code.as_str() == "eu"));
        let us = Country::from_alpha2("US").unwrap();
        assert!(us.allows_flag(FlagCode::parse("US-CA").unwrap()));
        assert!(!us.allows_flag(FlagCode::parse("eu").unwrap()));
        assert!(!us.allows_flag(FlagCode::parse("gb-sct").unwrap()));
        assert!(
            Country::from_alpha2("NO")
                .unwrap()
                .allows_flag(FlagCode::parse("eu").unwrap())
        );
        assert!(FlagCode::parse("made-up").is_none());
    }
}
