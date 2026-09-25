//! LFS integration.

pub(crate) mod installations;

use crate::settings::LfsSettings;

use lfsplanet_lfs_api::LfsClient;

/// Builds an LFS API client when the application has configured credentials.
pub(crate) fn api_client(settings: &LfsSettings) -> anyhow::Result<Option<LfsClient>> {
    let Some(oauth) = &settings.oauth else {
        return Ok(None);
    };
    Ok(Some(LfsClient::new(
        lfsplanet_lfs_api::http_client(settings.outbound_http_timeout.duration())?,
        oauth.client_id.as_str().to_owned(),
        oauth.client_secret.as_str().to_owned(),
    )))
}

/// Whether a value can safely identify one installation beneath its root.
pub(crate) fn valid_installation_id(id: &str) -> bool {
    !id.is_empty()
        && id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
        && std::path::Path::new(id).components().count() == 1
        && !matches!(id, "." | "..")
}

#[cfg(test)]
mod installation_id_tests {
    use super::valid_installation_id;

    #[test]
    fn installation_ids_are_safe_single_path_components() {
        for id in ["0.7", "lfs-0.8", "validator_1"] {
            assert!(valid_installation_id(id), "{id}");
        }
        for id in ["", ".", "..", "0.7/other", "LFS 0.7"] {
            assert!(!valid_installation_id(id), "{id}");
        }
    }
}
