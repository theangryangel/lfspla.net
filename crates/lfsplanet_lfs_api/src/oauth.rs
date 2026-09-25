//! The browser OAuth flow that signs a player in with their LFS account.

use oauth2::{
    AuthUrl, ClientId, CsrfToken, EndpointNotSet, EndpointSet, RedirectUrl, Scope,
    basic::BasicClient, url::Url,
};
use serde::Deserialize;

use crate::{Error, LfsClient, endpoints};

/// OAuth client type with only the authorization endpoint configured.
///
/// The token exchange is done against [`LfsClient`] rather than through this
/// client, so the remaining endpoints stay unset.
type AuthorizationClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointNotSet>;

/// The LFS account behind an authorization code.
#[derive(Debug)]
pub struct LfsAccount {
    pub lfsworld_id: i64,
    pub username: String,
    pub display_name: String,
}

/// The browser sign-in flow against LFS.
#[derive(Clone)]
pub struct OAuthProvider {
    client: LfsClient,
    authorization_client: AuthorizationClient,
    redirect_uri: String,
}

impl OAuthProvider {
    /// Builds the provider for a service that LFS redirects back to at
    /// `redirect_uri`.
    pub fn new(client: LfsClient, redirect_uri: String) -> anyhow::Result<Self> {
        let authorization_client = BasicClient::new(ClientId::new(client.client_id.clone()))
            .set_auth_uri(AuthUrl::new(endpoints::AUTHORIZE.to_owned())?)
            .set_redirect_uri(RedirectUrl::new(redirect_uri.clone())?);
        Ok(Self {
            client,
            authorization_client,
            redirect_uri,
        })
    }

    /// The URL to send the browser to, with the one-time state to store
    /// against the session and check on the way back.
    pub fn authorize_url(&self) -> (Url, CsrfToken) {
        self.authorization_client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new("openid".to_owned()))
            .add_scope(Scope::new("profile".to_owned()))
            .url()
    }

    /// Exchanges a browser authorization code for the player's LFS account.
    ///
    /// The access token is used once, here, and then dropped: it belongs to
    /// the player rather than to this service, so unlike
    /// [`LfsClient::service_token`] it is never cached.
    pub async fn authenticate(&self, code: &str) -> Result<LfsAccount, Error> {
        let token = self
            .client
            .token(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", self.redirect_uri.as_str()),
                ("client_id", self.client.client_id.as_str()),
                ("client_secret", self.client.client_secret.as_str()),
            ])
            .await?;

        let body = self
            .client
            .http
            .get(endpoints::USERINFO)
            .bearer_auth(&token.access_token)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let envelope: UserInfoEnvelope =
            serde_json::from_slice(&body).map_err(Error::InvalidUserInfo)?;
        envelope.data.into_account()
    }
}

#[derive(Debug, Deserialize)]
struct UserInfoEnvelope {
    data: UserInfo,
}

#[derive(Debug, Deserialize)]
struct UserInfo {
    sub: i64,
    preferred_username: Option<String>,
    username: Option<String>,
    name: Option<String>,
}

impl UserInfo {
    /// LFS names the account under `preferred_username`, and older responses
    /// under `username`. `name` is the display name when the player has set
    /// one, and the account name stands in when they have not.
    fn into_account(self) -> Result<LfsAccount, Error> {
        let username = self
            .preferred_username
            .or(self.username)
            .ok_or(Error::MissingUsername)?;
        Ok(LfsAccount {
            lfsworld_id: self.sub,
            display_name: self.name.unwrap_or_else(|| username.clone()),
            username,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(value: serde_json::Value) -> Result<LfsAccount, Error> {
        serde_json::from_value::<UserInfoEnvelope>(value)
            .unwrap()
            .data
            .into_account()
    }

    #[test]
    fn userinfo_accepts_lfs_username_fallback() {
        let account = account(serde_json::json!({
            "data": {
                "sub": 6700,
                "username": "example",
                "name": "Example Driver"
            }
        }))
        .unwrap();
        assert_eq!(account.lfsworld_id, 6700);
        assert_eq!(account.username, "example");
        assert_eq!(account.display_name, "Example Driver");
    }

    #[test]
    fn userinfo_prefers_the_preferred_username_and_falls_back_to_it_for_display() {
        let account = account(serde_json::json!({
            "data": {
                "sub": 1,
                "preferred_username": "preferred",
                "username": "legacy"
            }
        }))
        .unwrap();
        assert_eq!(account.username, "preferred");
        assert_eq!(account.display_name, "preferred");
    }

    #[test]
    fn userinfo_without_any_username_is_rejected() {
        assert!(matches!(
            account(serde_json::json!({ "data": { "sub": 1 } })),
            Err(Error::MissingUsername)
        ));
    }
}
