//! Team membership also determines a player's permissions within that team.

use sea_orm::entity::prelude::*;
use sea_orm::{
    ActiveValue::Set, DatabaseTransaction, DeleteMany, IntoActiveModel, Select, TransactionTrait,
    UpdateMany,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use super::{TeamError, invite};
use crate::models::{Player, player};

#[sea_orm::model]
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "team_member")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub team_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub player_id: i64,
    pub role: Role,
    #[sea_orm(belongs_to, from = "team_id", to = "id", on_delete = "Cascade")]
    pub team: BelongsTo<super::Entity>,
    #[sea_orm(belongs_to, from = "player_id", to = "id")]
    pub player: BelongsTo<player::Entity>,
}

impl ActiveModelBehavior for ActiveModel {}

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
pub enum Role {
    #[sea_orm(string_value = "owner")]
    Owner,
    #[default]
    #[sea_orm(string_value = "member")]
    Member,
}

impl Model {
    pub async fn find(
        database: &impl ConnectionTrait,
        team_id: i64,
        player_id: i64,
    ) -> Result<Self, TeamError> {
        Entity::find_by_id((team_id, player_id))
            .one(database)
            .await?
            .ok_or(TeamError::MemberNotFound)
    }

    pub async fn require_owner(
        database: &impl ConnectionTrait,
        team_id: i64,
        player_id: i64,
    ) -> Result<(), TeamError> {
        let owner = Entity::find()
            .of_team(team_id)
            .of_player(player_id)
            .owners()
            .one(database)
            .await?;
        owner.map(|_| ()).ok_or(TeamError::OwnerRequired)
    }

    async fn require_another_owner(
        transaction: &DatabaseTransaction,
        team_id: i64,
    ) -> Result<(), TeamError> {
        let owners = Entity::find()
            .of_team(team_id)
            .owners()
            .count(transaction)
            .await?;
        if owners <= 1 {
            return Err(TeamError::LastOwner);
        }
        Ok(())
    }

    /// Players may leave on their own; removing someone else needs an owner.
    pub async fn remove(
        database: &DatabaseConnection,
        team_id: i64,
        actor_id: i64,
        player_id: i64,
    ) -> Result<(), TeamError> {
        let transaction = database.begin().await?;
        super::Model::lock(&transaction, team_id).await?;
        if actor_id != player_id {
            Self::require_owner(&transaction, team_id, actor_id).await?;
        }
        let member = Self::find(&transaction, team_id, player_id).await?;
        if member.role == Role::Owner {
            Self::require_another_owner(&transaction, team_id).await?;
        }
        member.delete(&transaction).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Teams may have several owners, but never none. Demoting an owner
    /// revokes invitations they may have shared.
    pub async fn set_role(
        database: &DatabaseConnection,
        team_id: i64,
        actor_id: i64,
        player_id: i64,
        role: Role,
    ) -> Result<(), TeamError> {
        let transaction = database.begin().await?;
        super::Model::lock(&transaction, team_id).await?;
        Self::require_owner(&transaction, team_id, actor_id).await?;
        let member = Self::find(&transaction, team_id, player_id).await?;
        if member.role != role {
            if member.role == Role::Owner {
                Self::require_another_owner(&transaction, team_id).await?;
                invite::Model::revoke_all(&transaction, team_id).await?;
            }
            let mut active = member.into_active_model();
            active.role = Set(role);
            active.update(&transaction).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Members with their players, owners first, then by username.
    pub async fn roster(
        database: &impl ConnectionTrait,
        team_id: i64,
    ) -> Result<Vec<(Role, Player)>, DbErr> {
        let mut roster = Entity::find()
            .of_team(team_id)
            .find_also_related(player::Entity)
            .all(database)
            .await?
            .into_iter()
            .filter_map(|(member, player)| Some((member.role, player?)))
            .collect::<Vec<_>>();
        roster.sort_by_key(|(role, player)| {
            (
                *role != Role::Owner,
                player.lfs_username.to_ascii_lowercase(),
            )
        });
        Ok(roster)
    }
}

/// Database filters for membership queries.
pub trait TeamMemberFilter: QueryFilter + Sized {
    fn of_team(self, team_id: i64) -> Self {
        self.filter(Column::TeamId.eq(team_id))
    }

    fn of_player(self, player_id: i64) -> Self {
        self.filter(Column::PlayerId.eq(player_id))
    }

    fn owners(self) -> Self {
        self.filter(Column::Role.eq(Role::Owner))
    }
}

impl TeamMemberFilter for Select<Entity> {}
impl TeamMemberFilter for UpdateMany<Entity> {}
impl TeamMemberFilter for DeleteMany<Entity> {}
