//! Execution and result persistence for one live event.

use std::{future::Future, time::Duration};

use crate::models::event::{game::GameSource as EventMode, results};
use crate::settings::GameServerSettings;
use game::{LayoutSpec, LeaderboardResult, LeaderboardStore};
use insim::{core::vehicle::Vehicle, insim::IsiFlags};
use lfsplanet_bomb as bomb;
use lfsplanet_game as game;
use lfsplanet_land_speed_record as land_speed_record;
use lfsplanet_metronome as metronome;
use lfsplanet_shortcut as shortcut;
use object_store::ObjectStoreExt;

use tokio_util::sync::CancellationToken;

const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

enum Game {
    LandSpeedRecord {
        config: land_speed_record::LandSpeedRecordConfig,
        leaderboard: land_speed_record::LandSpeedRecordLeaderboard,
    },
    Bomb {
        config: bomb::BombConfig,
        leaderboard: bomb::BombLeaderboard,
    },
    Shortcut {
        config: shortcut::ShortcutConfig,
        leaderboard: shortcut::ShortcutLeaderboard,
    },
    Metronome {
        config: metronome::MetronomeConfig,
        leaderboard: metronome::MetronomeLeaderboard,
    },
}

impl Game {
    async fn run_epoch(
        &self,
        connection: &GameServerSettings,
        cancel: CancellationToken,
    ) -> (bool, anyhow::Result<()>) {
        let mut builder = insim::tcp(connection.insim_address)
            .isi_prefix('!')
            .isi_admin_password(connection.admin_password.clone());
        builder = match self {
            Self::LandSpeedRecord { .. } => builder
                .isi_iname("land speed record".to_string())
                .isi_flags(IsiFlags::MCI)
                .isi_interval(Duration::from_millis(100)),
            Self::Bomb { .. } => builder
                .isi_iname("bomb".to_string())
                .isi_flags(IsiFlags::AXM_LOAD),
            Self::Shortcut { .. } => builder.isi_iname("shortcut".to_string()),
            Self::Metronome { .. } => builder.isi_iname("metronome".to_string()),
        };
        let framed = tokio::select! {
            () = cancel.cancelled() => return (false, Ok(())),
            result = builder.connect_async() => match result {
                Ok(framed) => framed,
                Err(error) => return (false, Err(error.into())),
            },
        };
        let result = match self {
            Self::LandSpeedRecord {
                config,
                leaderboard,
            } => {
                land_speed_record::run_land_speed_record_connection(
                    framed,
                    config.clone(),
                    leaderboard.clone(),
                    cancel,
                )
                .await
            }
            Self::Bomb {
                config,
                leaderboard,
            } => {
                bomb::run_bomb_connection(framed, config.clone(), leaderboard.clone(), cancel).await
            }
            Self::Shortcut {
                config,
                leaderboard,
            } => {
                shortcut::run_shortcut_connection(
                    framed,
                    config.clone(),
                    leaderboard.clone(),
                    cancel,
                )
                .await
            }
            Self::Metronome {
                config,
                leaderboard,
            } => {
                metronome::run_metronome_connection(
                    framed,
                    config.clone(),
                    leaderboard.clone(),
                    cancel,
                )
                .await
            }
        };
        (true, result.map_err(Into::into))
    }
}

/// A prepared event runtime, ready to run until cancelled.
pub(crate) struct EventRuntime {
    event_id: Option<i64>,
    connection: GameServerSettings,
    game: Game,
}

impl EventRuntime {
    /// Prepares the existing mode with catalogue assets and persistent standings.
    pub(crate) async fn new(
        pool: sqlx::PgPool,
        database: &sea_orm::DatabaseConnection,
        event: crate::models::event::Model,
        connection: GameServerSettings,
        store: &dyn object_store::ObjectStore,
    ) -> anyhow::Result<Self> {
        let configuration = event
            .game()?
            .ok_or_else(|| anyhow::anyhow!("not a game event"))?;
        let track = configuration.track.parse()?;
        let attachment = crate::models::attachment::Model::required(
            database,
            event
                .layout_attachment_id
                .ok_or_else(|| anyhow::anyhow!("missing layout"))?,
        )
        .await?;
        let bytes = store
            .get(&object_store::path::Path::from(attachment.object_key))
            .await?
            .bytes()
            .await?;
        let layout = Some(LayoutSpec::Objects(
            insim_lyt::Lyt::read(bytes.as_ref())?.objects,
        ));
        let event_id = event.id;
        let vehicle_rules = configuration
            .vehicles
            .iter()
            .map(|rule| {
                Ok(game::VehicleRule {
                    vehicle: rule.vehicle.parse::<Vehicle>()?,
                    added_mass_kg: u8::try_from(rule.added_mass_kg)?,
                    intake_restriction: rule.intake_restriction,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let game = match &configuration.source {
            EventMode::LandSpeedRecord => Game::LandSpeedRecord {
                config: land_speed_record::LandSpeedRecordConfig {
                    track,
                    layout: layout.clone(),
                    vehicle_rules: vehicle_rules.clone(),
                },
                leaderboard: std::sync::Arc::new(PostgresLandSpeedRecordLeaderboard {
                    pool: pool.clone(),
                    event_id,
                }),
            },
            EventMode::Bomb {
                checkpoint_timeout_secs,
                checkpoint_penalty_ms,
                collision_max_penalty_ms,
            } => Game::Bomb {
                config: bomb::BombConfig {
                    checkpoint_timeout: Duration::from_secs(*checkpoint_timeout_secs),
                    checkpoint_penalty: Duration::from_millis(*checkpoint_penalty_ms),
                    collision_max_penalty: Duration::from_millis(*collision_max_penalty_ms),
                    track,
                    layout: layout.clone(),
                    vehicle_rules: vehicle_rules.clone(),
                },
                leaderboard: std::sync::Arc::new(PostgresBombLeaderboard {
                    pool: pool.clone(),
                    event_id,
                }),
            },
            EventMode::Shortcut => Game::Shortcut {
                config: shortcut::ShortcutConfig {
                    track,
                    layout: layout.clone(),
                    vehicle_rules: vehicle_rules.clone(),
                },
                leaderboard: std::sync::Arc::new(PostgresShortcutLeaderboard {
                    pool: pool.clone(),
                    event_id,
                }),
            },
            EventMode::Metronome { target_ms } => Game::Metronome {
                config: metronome::MetronomeConfig {
                    target: Duration::from_millis(*target_ms),
                    track,
                    layout,
                    vehicle_rules,
                },
                leaderboard: std::sync::Arc::new(PostgresMetronomeLeaderboard {
                    pool: pool.clone(),
                    event_id,
                }),
            },
        };
        Ok(Self {
            event_id: Some(event_id),
            connection,
            game,
        })
    }

    /// Runs the event, reconnecting after failures until cancellation.
    pub(crate) async fn run(self, cancel: CancellationToken) {
        let Self {
            event_id,
            connection,
            game,
        } = self;
        let connection_id = connection.insim_address;
        let mut retry_delay = Duration::from_secs(1);
        loop {
            let (connected, result) = game.run_epoch(&connection, cancel.clone()).await;
            if cancel.is_cancelled() {
                tracing::info!(?event_id, %connection_id, "connection runtime stopped");
                break;
            }
            if connected {
                retry_delay = Duration::from_secs(1);
            }
            match result {
                Ok(()) => tracing::warn!(?event_id, %connection_id, "game ended unexpectedly"),
                Err(error) => {
                    tracing::warn!(?event_id, %connection_id, %error, ?retry_delay, "game disconnected; retrying")
                }
            }
            tokio::select! {
                () = cancel.cancelled() => break,
                () = tokio::time::sleep(retry_delay) => {},
            }
            retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
        }
    }
}

#[derive(Debug)]
struct PostgresBombLeaderboard {
    pool: sqlx::PgPool,
    event_id: i64,
}

impl LeaderboardStore<bomb::BombStanding, bomb::BombResult> for PostgresBombLeaderboard {
    fn standings(
        &self,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = LeaderboardResult<Vec<bomb::BombStanding>>> + Send + '_>,
    > {
        Box::pin(async move {
            let rows = results::standings(
                &self.pool,
                self.event_id,
                crate::models::event::game::EventKind::Bomb,
                10,
                0,
            )
            .await?
            .0;
            Ok(rows
                .into_iter()
                .map(|row| bomb::BombStanding {
                    uname: row.username,
                    pname: row.display_name,
                    checkpoints: row.checkpoint_count.unwrap_or_default(),
                    survival_ms: row.survival_ms.unwrap_or_default(),
                })
                .collect())
        })
    }

    fn record(
        &self,
        result: bomb::BombResult,
    ) -> std::pin::Pin<Box<dyn Future<Output = LeaderboardResult<()>> + Send + '_>> {
        Box::pin(async move {
            results::record(&self.pool, self.event_id, results::Submission::Bomb(result)).await?;
            Ok(())
        })
    }
}

#[derive(Debug)]
struct PostgresShortcutLeaderboard {
    pool: sqlx::PgPool,
    event_id: i64,
}

impl LeaderboardStore<shortcut::ShortcutStanding, shortcut::ShortcutResult>
    for PostgresShortcutLeaderboard
{
    fn standings(
        &self,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = LeaderboardResult<Vec<shortcut::ShortcutStanding>>> + Send + '_>,
    > {
        Box::pin(async move {
            let rows = results::standings(
                &self.pool,
                self.event_id,
                crate::models::event::game::EventKind::Shortcut,
                100,
                0,
            )
            .await?
            .0;
            Ok(rows
                .into_iter()
                .map(|row| shortcut::ShortcutStanding {
                    uname: row.username,
                    pname: row.display_name,
                    time_ms: row.time_ms.unwrap_or_default(),
                })
                .collect())
        })
    }

    fn record(
        &self,
        result: shortcut::ShortcutResult,
    ) -> std::pin::Pin<Box<dyn Future<Output = LeaderboardResult<()>> + Send + '_>> {
        Box::pin(async move {
            results::record(
                &self.pool,
                self.event_id,
                results::Submission::Shortcut(result),
            )
            .await?;
            Ok(())
        })
    }
}

#[derive(Debug)]
struct PostgresMetronomeLeaderboard {
    pool: sqlx::PgPool,
    event_id: i64,
}

impl LeaderboardStore<metronome::MetronomeStanding, metronome::MetronomeResult>
    for PostgresMetronomeLeaderboard
{
    fn standings(
        &self,
    ) -> std::pin::Pin<
        Box<dyn Future<Output = LeaderboardResult<Vec<metronome::MetronomeStanding>>> + Send + '_>,
    > {
        Box::pin(async move {
            let rows = results::standings(
                &self.pool,
                self.event_id,
                crate::models::event::game::EventKind::Metronome,
                100,
                0,
            )
            .await?
            .0;
            Ok(rows
                .into_iter()
                .map(|row| metronome::MetronomeStanding {
                    uname: row.username,
                    pname: row.display_name,
                    delta_ms: row.delta_ms.unwrap_or_default(),
                })
                .collect())
        })
    }

    fn record(
        &self,
        result: metronome::MetronomeResult,
    ) -> std::pin::Pin<Box<dyn Future<Output = LeaderboardResult<()>> + Send + '_>> {
        Box::pin(async move {
            results::record(
                &self.pool,
                self.event_id,
                results::Submission::Metronome(result),
            )
            .await?;
            Ok(())
        })
    }
}

struct PostgresLandSpeedRecordLeaderboard {
    pool: sqlx::PgPool,
    event_id: i64,
}
impl
    LeaderboardStore<
        land_speed_record::LandSpeedRecordStanding,
        land_speed_record::LandSpeedRecordResult,
    > for PostgresLandSpeedRecordLeaderboard
{
    fn standings(
        &self,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = LeaderboardResult<Vec<land_speed_record::LandSpeedRecordStanding>>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let rows = results::standings(
                &self.pool,
                self.event_id,
                crate::models::event::game::EventKind::LandSpeedRecord,
                100,
                0,
            )
            .await?
            .0;
            Ok(rows
                .into_iter()
                .map(|row| land_speed_record::LandSpeedRecordStanding {
                    uname: row.username,
                    pname: row.display_name,
                    speed_kph: row.speed_kph.unwrap_or_default(),
                })
                .collect())
        })
    }
    fn record(
        &self,
        result: land_speed_record::LandSpeedRecordResult,
    ) -> std::pin::Pin<Box<dyn Future<Output = LeaderboardResult<()>> + Send + '_>> {
        Box::pin(async move {
            results::record(
                &self.pool,
                self.event_id,
                results::Submission::LandSpeedRecord(result),
            )
            .await?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn runtime_reconnects_after_disconnect_and_cancels_promptly() -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let runtime = EventRuntime {
            event_id: Some(1),
            connection: GameServerSettings {
                insim_address: listener.local_addr()?,
                admin_password: None,
                poll_interval_seconds: std::num::NonZeroU64::new(2).unwrap(),
            },
            game: Game::Shortcut {
                config: shortcut::ShortcutConfig {
                    track: "BL1".parse()?,
                    layout: None,
                    vehicle_rules: Vec::new(),
                },
                leaderboard: shortcut::in_memory_leaderboard(),
            },
        };
        let cancel = CancellationToken::new();
        let task = tokio::spawn(runtime.run(cancel.clone()));
        let reconnect = tokio::time::timeout(Duration::from_secs(8), async {
            // Close two independent TCP connections: the runtime must recover
            // without needing a fresh reconciliation task or an LFS fixture.
            for _ in 0..2 {
                let (socket, _) = listener.accept().await?;
                drop(socket);
            }
            Ok::<_, std::io::Error>(())
        })
        .await;
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(2), task).await??;
        reconnect??;
        Ok(())
    }
}
