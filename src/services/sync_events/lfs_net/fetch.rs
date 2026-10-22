//! Bounded, sequential requests to public league pages.

use anyhow::{Context, Result, ensure};
use std::time::Duration;

const MAX_REQUESTS: usize = 128;
const MAX_BYTES: usize = 2 * 1024 * 1024;

pub(crate) struct Fetcher {
    client: reqwest::Client,
    requests: usize,
}

impl Fetcher {
    pub(crate) fn new() -> Result<Self> {
        Self::build(false)
    }

    pub(crate) fn new_logos() -> Result<Self> {
        Self::build(true)
    }

    fn build(logos: bool) -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                "lfsplanet/",
                env!("CARGO_PKG_VERSION"),
                " (+https://lfspla.net)"
            ))
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 {
                    return attempt.error("too many redirects");
                }
                let valid = if logos {
                    logo_url(attempt.url().as_str())
                } else {
                    super::parse::source_url(attempt.url().as_str())
                };
                if valid.is_err() {
                    return attempt.error("redirect outside permitted LFS league paths");
                }
                attempt.follow()
            }))
            .build()?;
        Ok(Self {
            client,
            requests: 0,
        })
    }

    pub(crate) async fn get(&mut self, path: &str) -> Result<String> {
        let url = super::parse::source_url(path)?;
        let response = self.request(url).await?;
        ensure!(
            response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value
                    .split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/html"))),
            "expected an HTML response"
        );
        String::from_utf8(Self::body(response).await?).context("source page is not UTF-8")
    }

    pub(crate) async fn logo(&mut self, path: &str) -> Result<crate::models::league::Logo> {
        let response = self.request(logo_url(path)?).await?;
        let media_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .context("missing logo content type")?
            .split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let bytes = Self::body(response).await?;
        decode_logo(bytes, &media_type)
    }

    async fn request(&mut self, url: url::Url) -> Result<reqwest::Response> {
        ensure!(
            self.requests < MAX_REQUESTS,
            "event scrape request budget exhausted"
        );
        if self.requests > 0 {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        self.requests += 1;
        Ok(self.client.get(url).send().await?.error_for_status()?)
    }

    async fn body(mut response: reqwest::Response) -> Result<Vec<u8>> {
        ensure!(
            response
                .content_length()
                .is_none_or(|size| size <= MAX_BYTES as u64),
            "source page too large"
        );
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            ensure!(
                bytes.len() + chunk.len() <= MAX_BYTES,
                "source page too large"
            );
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

/// Images are restricted to the dedicated LFS logo directory, including redirects.
pub(super) fn logo_url(path: &str) -> Result<url::Url> {
    let base = url::Url::parse("https://www.lfs.net/")?;
    let url = base.join(path)?;
    ensure!(
        url.origin() == base.origin()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "unexpected league logo URL"
    );
    let name = url
        .path()
        .strip_prefix("/dynamic/leaguelogos/")
        .context("unexpected logo path")?;
    ensure!(
        !name.is_empty()
            && name != "."
            && name != ".."
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')),
        "unexpected logo filename"
    );
    Ok(url)
}

pub(super) fn decode_logo(bytes: Vec<u8>, media_type: &str) -> Result<crate::models::league::Logo> {
    let extension = match media_type {
        "image/png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => "png",
        "image/jpeg" if bytes.starts_with(&[0xff, 0xd8, 0xff]) => "jpg",
        "image/gif" if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") => "gif",
        "image/webp" if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") => "webp",
        _ => anyhow::bail!("unsupported or mismatched league logo image type: {media_type}"),
    };
    let media_type = match extension {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "gif" => "image/gif",
        _ => "image/webp",
    };
    Ok(crate::models::league::Logo {
        bytes,
        extension,
        media_type,
    })
}
