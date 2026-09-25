//! LFS-backed browser authentication.
use sea_orm::{
    ActiveValue::{NotSet, Set},
    DatabaseConnection, EntityTrait,
    sea_query::{Expr, Func, OnConflict},
};

use crate::models::players::{PlayerColumn, PlayerEntity, PlayerModel, PlayerMutation};
use lfsplanet_lfs_api::{Error as LfsError, OAuthProvider};

/// Errors produced while exchanging an LFS OAuth code and recording the player.
#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    /// The exchange with LFS failed, or returned something unusable.
    #[error(transparent)]
    Lfs(#[from] LfsError),
    /// Player persistence failed.
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

/// Exchanges an LFS OAuth code and records the authenticated player.
pub(crate) async fn authenticate(
    database: &DatabaseConnection,
    oauth: &OAuthProvider,
    code: &str,
) -> Result<Option<PlayerModel>, AuthenticationError> {
    let account = oauth.authenticate(code).await?;

    // Keep the player's chosen country and original creation date on login.
    let player = PlayerEntity::insert(PlayerMutation {
        id: NotSet,
        lfs_username: Set(account.username.clone()),
        display_name: Set(account.display_name.clone()),
        deny_auth: NotSet,
        deny_uploads: NotSet,
        country_code: NotSet,
        created_at: NotSet,
        last_authenticated_at: Set(Some(time::OffsetDateTime::now_utc())),
        lfsworld_id: Set(Some(account.lfsworld_id)),
    })
    .on_conflict(
        OnConflict::new()
            .expr(Func::lower(Expr::col(PlayerColumn::LfsUsername)))
            .update_columns([
                PlayerColumn::DisplayName,
                PlayerColumn::LastAuthenticatedAt,
                PlayerColumn::LfsworldId,
            ])
            .to_owned(),
    )
    .exec_with_returning(database)
    .await?;

    Ok((!player.deny_auth).then_some(player))
}
