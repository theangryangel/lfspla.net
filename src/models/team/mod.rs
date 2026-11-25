//! Racing teams, their membership roles, invitations and recruitment policy.
//!
//! Every change to a team locks its row first, so role checks and membership
//! changes for one team are serialized.

use std::collections::HashMap;

use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveValue::Set,
    Condition, DatabaseTransaction, FromQueryResult, IntoActiveModel, Order, QueryOrder,
    QuerySelect, QueryTrait, Select, TransactionTrait,
    sea_query::{Expr, Func, extension::postgres::PgExpr},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::ValidationErrors;

use crate::db::escaped_like::EscapedLike;

mod details;
pub(crate) mod invite;
pub(crate) mod member;

pub(crate) use details::TeamDetails;
use member::TeamMemberFilter;

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "team")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub lfsworld_id: Option<i64>,
    pub name: String,
    pub tag: Option<String>,
    pub website_url: Option<String>,
    pub description: Option<String>,
    #[sea_orm(column_type = "JsonBinary")]
    pub tags: Tags,
    pub accepts_applications: bool,
    pub status: Status,
    /// Unknown when the source export has a zero timestamp.
    pub lfsworld_updated_at: Option<TimeDateTimeWithTimeZone>,
    pub created_at: TimeDateTimeWithTimeZone,
    pub updated_at: TimeDateTimeWithTimeZone,
    #[sea_orm(has_many)]
    pub members: HasMany<member::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tag {
    Race,
    Drift,
    Drag,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
#[serde(transparent)]
pub struct Tags(pub Vec<Tag>);

#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "disbanded")]
    Disbanded,
}

/// Why a team workflow was refused. Transport layers choose the wording.
#[derive(Debug, thiserror::Error)]
pub(crate) enum TeamError {
    #[error("team not found")]
    NotFound,
    #[error("team member not found")]
    MemberNotFound,
    #[error("team invitation not found")]
    InviteNotFound,
    /// Expired, revoked, exhausted, or for a team that is no longer active.
    #[error("team invitation unavailable")]
    InviteUnavailable,
    #[error("only team owners can do this")]
    OwnerRequired,
    #[error("a team must keep at least one owner")]
    LastOwner,
    #[error("ownership must go to another member")]
    TransferToSelf,
    #[error("the team is disbanded")]
    Disbanded,
    #[error(transparent)]
    Validation(#[from] ValidationErrors),
    #[error(transparent)]
    Database(#[from] DbErr),
}

impl Model {
    /// Creates a team with its creator as the only owner.
    pub(crate) async fn create(
        database: &DatabaseConnection,
        owner_id: i64,
        details: TeamDetails,
    ) -> Result<Self, TeamError> {
        let details = details.normalized()?;
        let transaction = database.begin().await?;
        let mut active = ActiveModel {
            status: Set(Status::Active),
            ..Default::default()
        };
        details.apply(&mut active);
        let team = active.insert(&transaction).await?;
        member::ActiveModel {
            team_id: Set(team.id),
            player_id: Set(owner_id),
            role: Set(member::Role::Owner),
        }
        .insert(&transaction)
        .await?;
        transaction.commit().await?;
        Ok(team)
    }

    pub(crate) async fn update(
        database: &DatabaseConnection,
        id: i64,
        actor_id: i64,
        details: TeamDetails,
    ) -> Result<(), TeamError> {
        let details = details.normalized()?;
        let transaction = database.begin().await?;
        let team = Self::lock(&transaction, id).await?;
        member::Model::require_owner(&transaction, id, actor_id).await?;
        if team.status == Status::Disbanded && details.accepts_applications {
            return Err(TeamError::Disbanded);
        }
        let mut active = team.into_active_model();
        details.apply(&mut active);
        active.update(&transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Disbanding also closes applications and revokes every invitation.
    pub(crate) async fn set_status(
        database: &DatabaseConnection,
        id: i64,
        actor_id: i64,
        status: Status,
    ) -> Result<(), TeamError> {
        let transaction = database.begin().await?;
        let team = Self::lock(&transaction, id).await?;
        member::Model::require_owner(&transaction, id, actor_id).await?;
        let mut active = team.into_active_model();
        active.status = Set(status);
        active.updated_at = Set(time::OffsetDateTime::now_utc());
        if status == Status::Disbanded {
            active.accepts_applications = Set(false);
            invite::Model::revoke_all(&transaction, id).await?;
        }
        active.update(&transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Hands ownership to another member. The previous owner stays on as a
    /// member, and invitations they may have shared are revoked.
    pub(crate) async fn transfer_ownership(
        database: &DatabaseConnection,
        id: i64,
        actor_id: i64,
        new_owner_id: i64,
    ) -> Result<(), TeamError> {
        if actor_id == new_owner_id {
            return Err(TeamError::TransferToSelf);
        }
        let transaction = database.begin().await?;
        Self::lock(&transaction, id).await?;
        member::Model::require_owner(&transaction, id, actor_id).await?;
        let target = member::Model::find(&transaction, id, new_owner_id).await?;
        let mut target = target.into_active_model();
        target.role = Set(member::Role::Owner);
        target.update(&transaction).await?;
        member::Entity::update_many()
            .col_expr(member::Column::Role, Expr::value(member::Role::Member))
            .of_team(id)
            .of_player(actor_id)
            .exec(&transaction)
            .await?;
        invite::Model::revoke_all(&transaction, id).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Locks the team row for the rest of the transaction.
    pub(crate) async fn lock(
        transaction: &DatabaseTransaction,
        id: i64,
    ) -> Result<Self, TeamError> {
        Entity::find_by_id(id)
            .lock_exclusive()
            .one(transaction)
            .await?
            .ok_or(TeamError::NotFound)
    }

    /// Roster sizes keyed by team. Teams without members are absent.
    pub(crate) async fn member_counts(
        database: &impl ConnectionTrait,
        ids: impl IntoIterator<Item = i64>,
    ) -> Result<HashMap<i64, i64>, DbErr> {
        #[derive(FromQueryResult)]
        struct MemberCount {
            team_id: i64,
            member_count: i64,
        }
        let counts = member::Entity::find()
            .select_only()
            .column(member::Column::TeamId)
            .column_as(member::Column::PlayerId.count(), "member_count")
            .filter(member::Column::TeamId.is_in(ids))
            .group_by(member::Column::TeamId)
            .into_model::<MemberCount>()
            .all(database)
            .await?;
        Ok(counts
            .into_iter()
            .map(|row| (row.team_id, row.member_count))
            .collect())
    }

    pub(crate) async fn member_count(&self, database: &impl ConnectionTrait) -> Result<i64, DbErr> {
        Ok(Self::member_counts(database, [self.id])
            .await?
            .get(&self.id)
            .copied()
            .unwrap_or(0))
    }
}

/// Database filters for team queries.
pub(crate) trait TeamFilter: QueryFilter + Sized {
    /// Matches names and tags literally, ignoring case. Blank searches match all teams.
    fn matching(self, query: &str) -> Self {
        let query = query.trim();
        if query.is_empty() {
            return self;
        }

        let pattern = EscapedLike::contains(query);
        self.filter(
            Condition::any()
                .add(Expr::col(Column::Name).ilike(pattern.clone()))
                .add(Expr::col(Column::Tag).ilike(pattern)),
        )
    }

    fn with_tag(self, tag: Tag) -> Self {
        self.filter(Expr::col(Column::Tags).contains(serde_json::json!([tag])))
    }

    fn accepting_applications(self, accepts: bool) -> Self {
        self.filter(Column::AcceptsApplications.eq(accepts))
    }

    /// Teams the player belongs to in any role.
    fn with_member(self, player_id: i64) -> Self {
        let memberships = member::Entity::find()
            .select_only()
            .column(member::Column::TeamId)
            .of_player(player_id)
            .into_query();
        self.filter(Column::Id.in_subquery(memberships))
    }
}

impl TeamFilter for Select<Entity> {}

/// The order team listings are shown in.
pub(crate) trait TeamOrder: QueryOrder + Sized {
    /// Case-insensitive name order, with the identity as a stable tie-break.
    fn in_name_order(self) -> Self {
        self.order_by(Func::lower(Expr::col(Column::Name)), Order::Asc)
            .order_by_asc(Column::Id)
    }
}

impl TeamOrder for Select<Entity> {}

#[cfg(test)]
mod database_tests {
    use super::*;

    #[sqlx::test(migrations = false)]
    #[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
    async fn snapshot_preserves_profiles_and_imports_membership_roles(
        pool: sqlx::PgPool,
    ) -> anyhow::Result<()> {
        let before = sqlx::migrate::Migrator::with_migrations(
            sqlx::migrate!()
                .iter()
                .filter(|migration| migration.version < 20261125120000)
                .cloned()
                .collect(),
        );
        before.run(&pool).await?;
        sqlx::query(
            "INSERT INTO player (lfs_username, display_name, deny_auth)
             VALUES ('orion', 'Existing display name', true)",
        )
        .execute(&pool)
        .await?;
        sqlx::migrate!().run(&pool).await?;
        // Re-running migrations must leave the imported snapshot alone.
        sqlx::migrate!().run(&pool).await?;

        let counts: (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM team), (SELECT count(*) FROM player),
             (SELECT count(*) FROM team_member),
             (SELECT count(*) FROM team_member WHERE role = 'owner')",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(counts, (1388, 8053, 8686, 1388));
        let existing: (String, bool, String) = sqlx::query_as(
            "SELECT display_name, deny_auth, role FROM player
             JOIN team_member ON player_id = player.id
             JOIN team ON team.id = team_id
             WHERE lfs_username = 'orion' AND team.lfsworld_id = 1",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(
            existing,
            ("Existing display name".into(), true, "owner".into())
        );

        let missing_owner: String = sqlx::query_scalar(
            "SELECT role FROM team_member JOIN player ON player.id = player_id
             JOIN team ON team.id = team_id
             WHERE team.name = 'Norwegian smoke drifters' AND lfs_username = 'aleks1985'",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(missing_owner, "owner");
        let memberships: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM team_member JOIN player ON player.id = player_id
             WHERE lfs_username = 'Locovich'",
        )
        .fetch_one(&pool)
        .await?;
        assert_eq!(memberships, 5);

        let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool.clone());
        let team = Entity::find()
            .filter(Column::LfsworldId.eq(1_i64))
            .one(&database)
            .await?
            .unwrap();
        assert_eq!(team.tag.as_deref(), Some("[ZT]"));
        assert_eq!(team.tags, Tags(vec![Tag::Race]));
        assert_eq!(
            team.description.as_deref(),
            Some("Do you feel the SPEEEEED?!")
        );
        assert!(team.accepts_applications);
        assert_eq!(team.status, Status::Active);
        assert_eq!(team.lfsworld_updated_at, None);
        assert!(
            sqlx::query("UPDATE team_member SET role = 'admin' WHERE team_id = $1")
                .bind(team.id)
                .execute(&pool)
                .await
                .is_err()
        );
        Ok(())
    }
}
