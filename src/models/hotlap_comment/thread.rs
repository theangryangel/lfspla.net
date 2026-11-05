//! Thread loading and assembly, independent of HTTP responses.
use super::{self as comment, Column, CommentFilter, Entity, Model};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbErr, EntityTrait, ExprTrait, FromQueryResult,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Select,
};

#[derive(FromQueryResult)]
pub(crate) struct ThreadEntry {
    #[sea_orm(nested)]
    pub(crate) comment: comment::Model,
    pub(crate) lfs_username: String,
    pub(crate) display_name: String,
    pub(crate) country_code: Option<String>,
    pub(crate) flag_code: Option<String>,
    pub(crate) reply_count: i64,
}

pub(crate) struct CommentThread {
    pub(crate) entry: ThreadEntry,
    pub(crate) replies: Vec<CommentThread>,
}

impl Model {
    pub(crate) async fn list(
        database: &impl ConnectionTrait,
        query: Select<Entity>,
        offset: u64,
        limit: u64,
    ) -> Result<(Vec<CommentThread>, u64), DbErr> {
        let total = query.clone().count(database).await?;
        let ids = query
            .order_by_asc(Column::Id)
            .offset(offset)
            .limit(limit)
            .select_only()
            .column(Column::Id)
            .into_tuple::<i64>()
            .all(database)
            .await?;
        Ok((Self::threads(database, ids).await?, total))
    }

    /// Zero-based position of a comment's root within the filtered root order.
    pub(crate) async fn root_position(
        database: &impl ConnectionTrait,
        query: Select<Entity>,
        hotlap_id: i64,
        comment_id: i64,
    ) -> Result<Option<u64>, DbErr> {
        let target_root = Entity::find()
            .select_only()
            .column_as(
                sea_orm::sea_query::Expr::cust("COALESCE(root_id, id)"),
                "root",
            )
            .on_hotlap(hotlap_id)
            .filter(Column::Id.eq(comment_id))
            .into_query();
        let Some(target) = query
            .clone()
            .filter(Column::Id.in_subquery(target_root))
            .one(database)
            .await?
        else {
            return Ok(None);
        };
        let mut precedes = Condition::any().add(
            Condition::all()
                .add(Self::open_report_priority().eq(target.is_open_report()))
                .add(Column::Id.lt(target.id)),
        );
        if !target.is_open_report() {
            precedes = precedes.add(Self::open_report_priority());
        }
        Ok(Some(query.filter(precedes).count(database).await?))
    }

    pub(crate) async fn threads(
        database: &impl ConnectionTrait,
        selected_ids: Vec<i64>,
    ) -> Result<Vec<CommentThread>, DbErr> {
        if selected_ids.is_empty() {
            return Ok(Vec::new());
        }
        // Only SQL placeholders are interpolated; all IDs remain bound parameters.
        let placeholders = (1..=selected_ids.len())
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>()
            .join(", ");
        // Follow actual parent links, so sibling branches cannot consume the
        // descendant budget when loading a comment or a page of direct replies.
        // UNION also terminates traversal if malformed stored data contains a cycle.
        let entries = ThreadEntry::find_by_statement(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Postgres,
            include_str!("thread.sql").replace("{selected_ids}", &placeholders),
            selected_ids.iter().copied().map(sea_orm::Value::from),
        ))
        .all(database)
        .await?;
        let mut child_ids: std::collections::BTreeMap<i64, Vec<i64>> =
            std::collections::BTreeMap::new();
        for entry in &entries {
            if let Some(parent) = entry.comment.parent_id {
                child_ids.entry(parent).or_default().push(entry.comment.id);
            }
        }
        let mut nodes = entries
            .into_iter()
            .map(|entry| {
                (
                    entry.comment.id,
                    CommentThread {
                        entry,
                        replies: Vec::new(),
                    },
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        // Assemble the fetched subtree in memory, preserving selected-entry order.
        fn nest(
            id: i64,
            nodes: &mut std::collections::BTreeMap<i64, CommentThread>,
            child_ids: &std::collections::BTreeMap<i64, Vec<i64>>,
        ) -> Option<CommentThread> {
            let mut row = nodes.remove(&id)?;
            row.replies = child_ids
                .get(&id)
                .into_iter()
                .flatten()
                .filter_map(|child| nest(*child, nodes, child_ids))
                .collect();
            Some(row)
        }
        Ok(selected_ids
            .into_iter()
            .filter_map(|id| nest(id, &mut nodes, &child_ids))
            .collect())
    }
    pub(crate) async fn thread(
        database: &impl ConnectionTrait,
        id: i64,
    ) -> Result<Option<CommentThread>, DbErr> {
        Ok(Self::threads(database, vec![id]).await?.pop())
    }
}
