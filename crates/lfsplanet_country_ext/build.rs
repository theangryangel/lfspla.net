use std::{collections::BTreeMap, env, fmt::Write, fs, path::PathBuf};

// EU flag eligibility extends beyond EU members. GB is excluded by choice.
const EUROPE: &[&str] = &[
    "AD", "AL", "AM", "AT", "AX", "AZ", "BA", "BE", "BG", "BY", "CH", "CY", "CZ", "DE", "DK", "EE",
    "ES", "FI", "FO", "FR", "GE", "GG", "GI", "GR", "HR", "HU", "IE", "IM", "IS", "IT", "JE", "KZ",
    "LI", "LT", "LU", "LV", "MC", "MD", "ME", "MK", "MT", "NL", "NO", "PL", "PT", "RO", "RS", "RU",
    "SE", "SI", "SJ", "SK", "SM", "TR", "UA", "VA", "XK",
];

fn main() {
    println!("cargo:rerun-if-changed=codes.json");

    let json = fs::read_to_string("codes.json").expect("read bundled FlagCDN catalogue");
    let catalogue: BTreeMap<String, String> =
        serde_json::from_str(&json).expect("valid bundled FlagCDN catalogue");
    let mut generated = String::from("static CATALOGUE: &[(&str, &str)] = &[\n");
    for (code, name) in &catalogue {
        writeln!(generated, "    ({code:?}, {name:?}),").expect("write catalogue entry");
    }
    generated.push_str("];\n");

    let mut countries = celes::Country::get_countries();
    countries.sort_by_key(|country| country.alpha2);
    for code in ["eu", "un"] {
        assert!(catalogue.contains_key(code), "missing policy flag {code}");
    }
    for code in EUROPE {
        assert!(
            countries.iter().any(|country| country.alpha2 == *code),
            "unknown country in EUROPE policy: {code}"
        );
    }
    for code in catalogue.keys() {
        assert!(
            !code.is_empty() && *code == code.to_ascii_lowercase(),
            "noncanonical flag code: {code}"
        );
        if let Some((parent, _)) = code.split_once('-') {
            assert!(
                countries
                    .iter()
                    .any(|country| country.alpha2.eq_ignore_ascii_case(parent)),
                "unknown subdivision parent for flag {code}"
            );
        }
    }

    generated.push_str("static COUNTRY_FLAGS: &[(&str, &[Flag])] = &[\n");
    for country in countries {
        let national = country.alpha2.to_ascii_lowercase();
        assert!(
            catalogue.contains_key(&national),
            "missing national flag for {}",
            country.alpha2
        );
        let mut flags: Vec<_> = catalogue
            .iter()
            .filter(|(code, _)| {
                *code == &national
                    || code.as_str() == "un"
                    || code
                        .split_once('-')
                        .is_some_and(|(parent, _)| parent == national)
                    || (code.as_str() == "eu" && EUROPE.contains(&country.alpha2))
            })
            .collect();
        flags.sort_by_key(|(code, _)| *code != &national);
        writeln!(generated, "    ({:?}, &[", country.alpha2).expect("write country entry");
        for (code, name) in flags {
            writeln!(
                generated,
                "        Flag {{ code: FlagCode({code:?}), name: {name:?} }},"
            )
            .expect("write country flag");
        }
        generated.push_str("    ]),\n");
    }
    generated.push_str("];\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build output directory"));
    fs::write(output.join("catalogue.rs"), generated).expect("write generated FlagCDN catalogue");
}
