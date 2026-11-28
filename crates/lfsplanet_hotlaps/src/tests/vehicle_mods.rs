//! Vehicle mod synchronisation against hotlap-backed author profiles.

use crate::models::player::PlayerHotlapProfile;
use lfsplanet_core::models::vehicle::Entity as VehicleEntity;
use lfsplanet_core::vehicle_id::VehicleId;
use lfsplanet_lfs_net_api::RemoteVehicleMod;
use sea_orm::{EntityTrait, Set};

#[sqlx::test(migrator = "lfsplanet_core::db::MIGRATOR")]
#[cfg_attr(not(feature = "test-database"), ignore = "requires PostgreSQL")]
async fn sync_links_and_creates_ordinary_author_profiles(pool: sqlx::PgPool) -> anyhow::Result<()> {
    use lfsplanet_core::models::player::{ActiveModel as PlayerMutation, Entity as PlayerEntity};
    use sea_orm::{ActiveModelTrait, PaginatorTrait};
    let database = sea_orm::SqlxPostgresConnector::from_sqlx_postgres_pool(pool);
    let original_players = PlayerEntity::find().count(&database).await?;
    let existing = PlayerMutation {
        lfs_username: Set("KnownAuthor".to_owned()),
        display_name: Set("Chosen display name".to_owned()),
        country_code: Set(Some(celes::Country::germany().into())),
        deny_auth: Set(true),
        deny_uploads: Set(true),
        last_authenticated_at: Set(Some(time::OffsetDateTime::UNIX_EPOCH)),
        ..Default::default()
    }
    .insert(&database)
    .await?;
    let entry = |id: &str, author: Option<&str>| RemoteVehicleMod {
        id: id.to_owned(),
        name: id.to_owned(),
        version: 1,
        class: None,
        author_username: author.map(str::to_owned),
        work_in_progress: false,
        is_private: false,
        is_tweak_mod: false,
        published_at: None,
        cover_thumb_url: None,
        metadata: serde_json::json!({}),
    };
    let mut remote = vec![
        entry("AAAA01", Some(" knownAUTHOR ")),
        entry("AAAA02", Some("NewAuthor")),
        entry("AAAA03", Some("newauthor")),
        entry("AAAA04", None),
        entry("AAAA05", Some("   ")),
    ];
    lfsplanet_core::models::Vehicle::refresh(&database, &remote).await?;
    lfsplanet_core::models::Vehicle::refresh(&database, &remote).await?;
    assert_eq!(
        PlayerEntity::find().count(&database).await?,
        original_players + 2
    );
    assert_eq!(
        PlayerEntity::find_by_id(existing.id)
            .one(&database)
            .await?
            .unwrap(),
        existing
    );
    let vehicles = VehicleEntity::find().all(&database).await?;
    let linked = |id: &str| {
        vehicles
            .iter()
            .find(|vehicle| vehicle.id.to_string() == id)
            .unwrap()
            .author_player_id
    };
    assert_eq!(linked("AAAA01"), Some(existing.id));
    let author_id = linked("AAAA02").unwrap();
    assert_eq!(linked("AAAA03"), Some(author_id));
    assert_eq!(linked("AAAA04"), None);
    assert_eq!(linked("AAAA05"), None);
    let author = PlayerEntity::find_by_id(author_id)
        .one(&database)
        .await?
        .unwrap();
    assert_eq!(author.display_name, "NewAuthor");
    assert!(!author.deny_auth);
    assert!(!author.deny_uploads);
    let profile = author.profile(&database).await?;
    assert_eq!(profile.stats.hotlaps, 0);
    let authenticated = lfsplanet_core::models::Player::record_authentication(
        &database,
        &lfsplanet_lfs_net_api::LfsAccount {
            lfsworld_id: 12345,
            username: "NEWAUTHOR".to_owned(),
            display_name: "Author's chosen name".to_owned(),
        },
    )
    .await?;
    assert_eq!(authenticated.id, author_id);
    assert_eq!(authenticated.display_name, "Author's chosen name");
    lfsplanet_core::models::Vehicle::refresh(&database, &remote).await?;
    assert_eq!(
        PlayerEntity::find_by_id(author_id)
            .one(&database)
            .await?
            .unwrap(),
        authenticated
    );
    remote[1].author_username = Some("KnownAuthor".to_owned());
    remote[2].author_username = None;
    lfsplanet_core::models::Vehicle::refresh(&database, &remote).await?;
    assert_eq!(
        VehicleEntity::find_by_id("AAAA02".parse::<VehicleId>()?)
            .one(&database)
            .await?
            .unwrap()
            .author_player_id,
        Some(existing.id)
    );
    assert_eq!(
        VehicleEntity::find_by_id("AAAA03".parse::<VehicleId>()?)
            .one(&database)
            .await?
            .unwrap()
            .author_player_id,
        None
    );
    Ok(())
}
