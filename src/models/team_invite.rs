//! Shareable invitations. Only the hash of the secret is persisted.

use sea_orm::entity::prelude::*;

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

impl Model {
    pub(crate) fn usable_at(&self, now: time::OffsetDateTime) -> bool {
        self.revoked_at.is_none()
            && self.expires_at.is_none_or(|expiry| expiry > now)
            && self.max_uses.is_none_or(|limit| self.uses < limit)
    }
}
