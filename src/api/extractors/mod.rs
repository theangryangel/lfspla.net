//! Request extractors for resolved API resources and caller credentials.

mod credentials;
mod era;
mod player;

pub(crate) use credentials::AuthenticatedPlayer;
pub(crate) use credentials::BrowserAuthenticatedPlayer;
pub(crate) use credentials::browser_player;
pub(crate) use credentials::csrf_token;
pub(crate) use credentials::login;
pub(crate) use credentials::reset_csrf_token;
pub(crate) use credentials::secrets_match;
pub(crate) use credentials::verify_csrf;
pub(crate) use era::Era;
pub(crate) use era::resolve_era;
pub(crate) use player::Player;
