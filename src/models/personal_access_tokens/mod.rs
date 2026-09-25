//! Persistence and verification for user-managed personal access tokens.

mod entity;
mod filter;

pub(crate) use filter::PersonalAccessTokenFilter;

pub(crate) use entity::*;
pub use entity::{
    ActiveModel as PersonalAccessTokenMutation, Column as PersonalAccessTokenColumn,
    Entity as PersonalAccessTokenEntity, Model as PersonalAccessTokenModel,
};

use sea_orm::{
    ActiveValue::{NotSet, Set},
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter,
    sea_query::Expr,
};
use sha2::{Digest, Sha256};
use validator::{Validate, ValidationErrors};

use super::players::{PlayerEntity, PlayerModel};

pub(crate) const TOKEN_PREFIX: &str = "lfspla_";
pub(crate) const DEFAULT_EXPIRY_DAYS: u16 = 90;
pub(crate) const MAX_EXPIRY_DAYS: u16 = 365;
const TOKEN_SECRET_BYTES: usize = 32;
const TOKEN_HEX_BYTES: usize = TOKEN_SECRET_BYTES * 2;
const LAST_USED_WRITE_INTERVAL: time::Duration = time::Duration::hours(1);

/// A newly issued credential, paired with the only copy of its secret.
pub(crate) struct CreatedPersonalAccessToken {
    pub token: String,
    pub record: PersonalAccessTokenModel,
}

/// Transport-independent input for issuing a personal access token.
#[derive(Debug, Validate)]
pub(crate) struct CreatePersonalAccessToken {
    #[validate(
        length(
            max = 100,
            message = "The token name must be no longer than 100 characters"
        ),
        custom(
            function = "crate::validate::validate_non_blank",
            message = "Give the token a name describing where it will be used"
        ),
        non_control_character(message = "The token name cannot contain control characters")
    )]
    pub name: String,
    #[validate(range(
        min = 1,
        max = "MAX_EXPIRY_DAYS",
        message = "Choose an expiry from 1 to 365 days"
    ))]
    pub expires_in_days: Option<u16>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CreateError {
    #[error(transparent)]
    Validation(#[from] ValidationErrors),
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
}

/// Validates and issues a token. HTTP callers share this complete workflow.
pub(crate) async fn issue(
    database: &DatabaseConnection,
    player_id: i64,
    request: CreatePersonalAccessToken,
) -> Result<CreatedPersonalAccessToken, CreateError> {
    request.validate()?;
    let expiry_days = request.expires_in_days.unwrap_or(DEFAULT_EXPIRY_DAYS);
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(i64::from(expiry_days));
    Ok(create(database, player_id, request.name.trim(), expires_at).await?)
}

pub(crate) async fn revoke(
    database: &DatabaseConnection,
    player_id: i64,
    token_id: i64,
) -> Result<bool, sea_orm::DbErr> {
    let result = PersonalAccessTokenEntity::update_many()
        .col_expr(
            PersonalAccessTokenColumn::RevokedAt,
            Expr::value(time::OffsetDateTime::now_utc()),
        )
        .with_id(token_id)
        .owned_by(player_id)
        .live()
        .exec(database)
        .await?;
    Ok(result.rows_affected == 1)
}

pub(crate) async fn create(
    database: &DatabaseConnection,
    player_id: i64,
    name: &str,
    expires_at: time::OffsetDateTime,
) -> Result<CreatedPersonalAccessToken, sea_orm::DbErr> {
    let token = generate_token();
    let token_hash = token_hash(&token);
    let token_hint = token_hint(&token);
    let model = PersonalAccessTokenEntity::insert(PersonalAccessTokenMutation {
        id: NotSet,
        player_id: Set(player_id),
        name: Set(name.to_owned()),
        token_hash: Set(token_hash),
        token_hint: Set(token_hint),
        created_at: NotSet,
        expires_at: Set(expires_at),
        last_used_at: Set(None),
        revoked_at: Set(None),
    })
    .exec_with_returning(database)
    .await?;

    Ok(CreatedPersonalAccessToken {
        token,
        record: model,
    })
}

/// Deletes tokens expired or revoked before the cutoff.
pub(crate) async fn delete_stale(
    database: &DatabaseConnection,
    cutoff: time::OffsetDateTime,
) -> Result<u64, sea_orm::DbErr> {
    let result = PersonalAccessTokenEntity::delete_many()
        .filter(
            Condition::any()
                .add(PersonalAccessTokenColumn::RevokedAt.lt(cutoff))
                .add(PersonalAccessTokenColumn::ExpiresAt.lt(cutoff)),
        )
        .exec(database)
        .await?;
    Ok(result.rows_affected)
}

pub(crate) async fn authenticate(
    database: &DatabaseConnection,
    token: &str,
) -> Result<Option<PlayerModel>, sea_orm::DbErr> {
    if !valid_token_shape(token) {
        return Ok(None);
    }

    let now = time::OffsetDateTime::now_utc();
    let Some((credential, player)) = PersonalAccessTokenEntity::find()
        .filter(PersonalAccessTokenColumn::TokenHash.eq(token_hash(token)))
        .usable_at(now)
        .find_also_related(PlayerEntity)
        .one(database)
        .await?
    else {
        return Ok(None);
    };
    let Some(player) = player else {
        return Ok(None);
    };
    if player.deny_auth {
        return Ok(None);
    }

    PersonalAccessTokenEntity::update_many()
        .col_expr(
            PersonalAccessTokenColumn::LastUsedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(PersonalAccessTokenColumn::Id.eq(credential.id))
        .filter(
            Condition::any()
                .add(PersonalAccessTokenColumn::LastUsedAt.is_null())
                .add(PersonalAccessTokenColumn::LastUsedAt.lt(now - LAST_USED_WRITE_INTERVAL)),
        )
        .exec(database)
        .await?;

    Ok(Some(player))
}

fn generate_token() -> String {
    let secret: [u8; TOKEN_SECRET_BYTES] = rand::random();
    format!("{TOKEN_PREFIX}{}", hex::encode(secret))
}

fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Number of trailing characters shown so an owner can recognise a token.
const TOKEN_HINT_SUFFIX: usize = 8;

fn token_hint(token: &str) -> String {
    // `get` rather than a slice: a short token is a caller error, not a panic
    // in the middle of issuing credentials.
    let suffix = token
        .len()
        .checked_sub(TOKEN_HINT_SUFFIX)
        .and_then(|start| token.get(start..))
        .unwrap_or(token);
    format!("{TOKEN_PREFIX}...{suffix}")
}

fn valid_token_shape(token: &str) -> bool {
    token.len() == TOKEN_PREFIX.len() + TOKEN_HEX_BYTES
        && token.starts_with(TOKEN_PREFIX)
        && token[TOKEN_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_have_the_public_prefix_and_full_entropy() {
        let token = generate_token();
        assert!(valid_token_shape(&token));
        assert_eq!(token.len(), TOKEN_PREFIX.len() + TOKEN_HEX_BYTES);
        assert_eq!(token_hash(&token).len(), 32);
        assert!(token_hint(&token).starts_with("lfspla_..."));
        assert!(token.ends_with(&token_hint(&token)["lfspla_...".len()..]));
    }

    #[test]
    fn token_shape_is_strict() {
        assert!(!valid_token_shape("lfspla_short"));
        assert!(!valid_token_shape(&format!("wrong_{}", "a".repeat(64))));
        assert!(!valid_token_shape(&format!("lfspla_{}", "A".repeat(64))));
        assert!(!valid_token_shape(&format!("lfspla_{}", "z".repeat(64))));
    }
}
