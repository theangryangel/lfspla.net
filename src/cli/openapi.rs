//! Generate the OpenAPI document describing the public API.

use std::io::Write;

use anyhow::Context;

/// Prints the OpenAPI document to standard output.
pub(crate) fn run() -> anyhow::Result<()> {
    std::io::stdout()
        .write_all(document()?.as_bytes())
        .context("failed to write the OpenAPI document to standard output")
}

fn document() -> anyhow::Result<String> {
    let json = crate::api::openapi()
        .to_pretty_json()
        .context("failed to serialize the OpenAPI document as JSON")?;
    Ok(format!("{json}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_is_valid_json() {
        serde_json::from_str::<serde_json::Value>(&document().unwrap()).unwrap();
    }
}
