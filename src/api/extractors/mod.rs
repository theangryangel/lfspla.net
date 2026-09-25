//! Request extractors for resolved API resources and caller credentials.

mod credentials;
mod era;
mod player;

pub(crate) use credentials::{
    AuthenticatedPlayer, BrowserAuthenticatedPlayer, browser_player, csrf_token, login,
    reset_csrf_token, secrets_match, verify_csrf,
};
pub(crate) use era::{Era, resolve_era};
pub(crate) use player::Player;
