//! Shareable invitations. Only the hash of the secret is persisted.

use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveValue::Set,
    DatabaseTransaction, DeleteMany, QueryOrder, Select, TransactionTrait, UpdateMany,
    sea_query::{Expr, ExprTrait},
};
use sha2::{Digest, Sha256};
use validator::Validate;

use super::{Status, TeamError, member};

const SECRET_BYTES: usize = 32;
const SECRET_HEX_LEN: usize = SECRET_BYTES * 2;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "team_invite")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub team_id: i64,
    pub created_by: i64,
    pub token_hash: Vec<u8>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub expires_at: Option<TimeDateTimeWithTimeZone>,
    pub max_uses: Option<i64>,
    pub uses: i64,
    pub revoked_at: Option<TimeDateTimeWithTimeZone>,
}

impl ActiveModelBehavior for ActiveModel {}

/// Transport-independent limits for a new invitation. `None` means unlimited.
#[derive(Debug, Validate)]
pub(crate) struct CreateTeamInvite {
    #[validate(range(min = 1, max = 365, message = "Choose an expiry of 1 to 365 days"))]
    pub expires_in_days: Option<i64>,
    #[validate(range(min = 1, max = 10_000, message = "Choose a usage limit of 1 to 10,000"))]
    pub max_uses: Option<i64>,
}

/// A newly issued invitation, paired with the only copy of its secret.
pub(crate) struct IssuedTeamInvite {
    pub token: String,
    pub record: Model,
}

impl Model {
    pub(crate) fn usable_at(&self, now: time::OffsetDateTime) -> bool {
        self.revoked_at.is_none()
            && self.expires_at.is_none_or(|expiry| expiry > now)
            && self.max_uses.is_none_or(|limit| self.uses < limit)
    }

    /// Every invitation the team has issued, newest first. Owners only.
    pub(crate) async fn list(
        database: &DatabaseConnection,
        team_id: i64,
        actor_id: i64,
    ) -> Result<Vec<Self>, TeamError> {
        member::Model::require_owner(database, team_id, actor_id).await?;
        Ok(Entity::find()
            .of_team(team_id)
            .order_by_desc(Column::CreatedAt)
            .order_by_desc(Column::Id)
            .all(database)
            .await?)
    }

    pub(crate) async fn issue(
        database: &DatabaseConnection,
        team_id: i64,
        actor_id: i64,
        request: CreateTeamInvite,
    ) -> Result<IssuedTeamInvite, TeamError> {
        request.validate()?;
        let transaction = database.begin().await?;
        let team = super::Model::lock(&transaction, team_id).await?;
        member::Model::require_owner(&transaction, team_id, actor_id).await?;
        if team.status != Status::Active {
            return Err(TeamError::Disbanded);
        }
        let token = hex::encode(rand::random::<[u8; SECRET_BYTES]>());
        let now = time::OffsetDateTime::now_utc();
        let record = ActiveModel {
            team_id: Set(team_id),
            created_by: Set(actor_id),
            token_hash: Set(secret_hash(&token)),
            expires_at: Set(request
                .expires_in_days
                .map(|days| now + time::Duration::days(days))),
            max_uses: Set(request.max_uses),
            ..Default::default()
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        Ok(IssuedTeamInvite { token, record })
    }

    /// Revoking twice succeeds and keeps the original revocation time.
    pub(crate) async fn revoke(
        database: &DatabaseConnection,
        team_id: i64,
        actor_id: i64,
        invite_id: i64,
    ) -> Result<(), TeamError> {
        let transaction = database.begin().await?;
        super::Model::lock(&transaction, team_id).await?;
        member::Model::require_owner(&transaction, team_id, actor_id).await?;
        let exists = Entity::find()
            .of_team(team_id)
            .with_id(invite_id)
            .one(&transaction)
            .await?
            .is_some();
        if !exists {
            return Err(TeamError::InviteNotFound);
        }
        Entity::update_many()
            .col_expr(
                Column::RevokedAt,
                Expr::value(time::OffsetDateTime::now_utc()),
            )
            .of_team(team_id)
            .with_id(invite_id)
            .live()
            .exec(&transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Revokes every live invitation. Callers must hold the team lock.
    pub(crate) async fn revoke_all(
        transaction: &DatabaseTransaction,
        team_id: i64,
    ) -> Result<(), DbErr> {
        Entity::update_many()
            .col_expr(
                Column::RevokedAt,
                Expr::value(time::OffsetDateTime::now_utc()),
            )
            .of_team(team_id)
            .live()
            .exec(transaction)
            .await?;
        Ok(())
    }

    /// The team a usable invitation would join. Does not consume a use.
    pub(crate) async fn preview(
        database: &DatabaseConnection,
        token: &str,
    ) -> Result<super::Model, TeamError> {
        let invite = Self::find_by_secret(database, token).await?;
        if !invite.usable_at(time::OffsetDateTime::now_utc()) {
            return Err(TeamError::InviteUnavailable);
        }
        super::Entity::find_by_id(invite.team_id)
            .filter(super::Column::Status.eq(Status::Active))
            .one(database)
            .await?
            .ok_or(TeamError::InviteUnavailable)
    }

    /// Joins as a member. Existing members keep their role and use no invitation.
    pub(crate) async fn accept(
        database: &DatabaseConnection,
        token: &str,
        player_id: i64,
    ) -> Result<super::Model, TeamError> {
        let transaction = database.begin().await?;
        let invite = Self::find_by_secret(&transaction, token).await?;
        let team = super::Model::lock(&transaction, invite.team_id)
            .await
            .map_err(|_| TeamError::InviteUnavailable)?;
        // Re-read after locking the team: revocation or another redemption may have won the lock.
        let invite = Self::find_by_secret(&transaction, token).await?;
        if team.status != Status::Active || !invite.usable_at(time::OffsetDateTime::now_utc()) {
            return Err(TeamError::InviteUnavailable);
        }
        let existing = member::Entity::find_by_id((team.id, player_id))
            .one(&transaction)
            .await?;
        if existing.is_none() {
            member::ActiveModel {
                team_id: Set(team.id),
                player_id: Set(player_id),
                role: Set(member::Role::Member),
            }
            .insert(&transaction)
            .await?;
            Entity::update_many()
                .col_expr(Column::Uses, Expr::col(Column::Uses).add(1))
                .with_id(invite.id)
                .exec(&transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(team)
    }

    async fn find_by_secret(
        database: &impl ConnectionTrait,
        token: &str,
    ) -> Result<Self, TeamError> {
        if !valid_secret_shape(token) {
            return Err(TeamError::InviteUnavailable);
        }
        Entity::find()
            .filter(Column::TokenHash.eq(secret_hash(token)))
            .one(database)
            .await?
            .ok_or(TeamError::InviteUnavailable)
    }
}

fn valid_secret_shape(token: &str) -> bool {
    token.len() == SECRET_HEX_LEN && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn secret_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Database filters for invitation queries.
pub(crate) trait TeamInviteFilter: QueryFilter + Sized {
    fn of_team(self, team_id: i64) -> Self {
        self.filter(Column::TeamId.eq(team_id))
    }

    fn with_id(self, invite_id: i64) -> Self {
        self.filter(Column::Id.eq(invite_id))
    }

    /// Not revoked. Says nothing about expiry or remaining uses.
    fn live(self) -> Self {
        self.filter(Column::RevokedAt.is_null())
    }
}

impl TeamInviteFilter for Select<Entity> {}
impl TeamInviteFilter for UpdateMany<Entity> {}
impl TeamInviteFilter for DeleteMany<Entity> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_must_be_full_length_hex() {
        assert!(valid_secret_shape(&"a1".repeat(32)));
        assert!(!valid_secret_shape(&"a1".repeat(31)));
        assert!(!valid_secret_shape(&"zz".repeat(32)));
    }

    #[test]
    fn invitation_limits_are_optional_but_bounded() {
        let limits = |expires_in_days, max_uses| CreateTeamInvite {
            expires_in_days,
            max_uses,
        };
        assert!(limits(None, None).validate().is_ok());
        assert!(limits(Some(365), Some(10_000)).validate().is_ok());
        assert!(limits(Some(0), None).validate().is_err());
        assert!(limits(None, Some(10_001)).validate().is_err());
    }
}
