use std::future::Future;

use insim::{
    core::{object::ObjectInfo, track::Track, vehicle::Vehicle},
    identifiers::ConnectionId,
    insim::{Axm, PmoAction, PmoFlags, RaceLaps},
};
use kitcar::{
    AppError, Dispatch, ExtractCx, Handler, LobbyReady, Sender, TrackChanged, World, mtc,
};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum GamePhase {
    #[default]
    Waiting,
    SettingUp,
    Racing,
}

impl std::fmt::Display for GamePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Waiting => "waiting for players",
            Self::SettingUp => "setting up track",
            Self::Racing => "racing",
        })
    }
}

#[derive(Clone, Debug)]
pub enum LayoutSpec {
    Named(String),
    Objects(Vec<ObjectInfo>),
}

impl From<String> for LayoutSpec {
    fn from(layout: String) -> Self {
        Self::Named(layout)
    }
}

impl From<Vec<ObjectInfo>> for LayoutSpec {
    fn from(objects: Vec<ObjectInfo>) -> Self {
        Self::Objects(objects)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SetupStarted;

#[derive(Clone, Copy, Debug)]
pub struct SetupReady;

#[derive(Clone, Debug)]
pub struct SessionSetup {
    pub track: Track,
    pub laps: RaceLaps,
    pub wind: u8,
    pub layout: Option<LayoutSpec>,
    pub allowed_vehicles: Vec<Vehicle>,
    configured: bool,
    ending_session: bool,
}

impl SessionSetup {
    pub fn new(track: Track, laps: RaceLaps, wind: u8, layout: Option<LayoutSpec>) -> Self {
        Self {
            track,
            laps,
            wind,
            layout,
            allowed_vehicles: Vec::new(),
            configured: false,
            ending_session: false,
        }
    }

    pub fn untimed(track: Track, layout: Option<LayoutSpec>) -> Self {
        Self::new(track, RaceLaps::Untimed, 0, layout)
    }

    pub fn with_allowed_vehicles(mut self, vehicles: Vec<Vehicle>) -> Self {
        self.allowed_vehicles = vehicles;
        self
    }

    pub fn with_vehicle_rules(mut self, rules: &[crate::VehicleRule]) -> Self {
        self.allowed_vehicles = rules.iter().map(|rule| rule.vehicle).collect();
        self
    }

    fn configure(&self, world: &World, sender: &Sender) -> Result<bool, AppError> {
        let track_differs = world.track() != Some(self.track);
        if track_differs {
            sender.packet(world.clear())?;
            sender.packet(world.change_track(self.track))?;
            // Changing track is asynchronous in LFS. Do not queue any setup
            // packets behind `/track`: they can be processed before the new
            // track has finished loading. The resulting TrackChanged event
            // will reconcile the remainder of the setup.
            return Ok(false);
        }
        sender.packet(world.change_laps(self.laps))?;
        sender.packet(world.change_wind(self.wind))?;
        sender.packets(world.restrict_vehicles(&self.allowed_vehicles))?;
        match &self.layout {
            None => {
                sender.packet(world.ax_clear())?;
            }
            Some(LayoutSpec::Named(layout)) => {
                sender.packet(world.ax_clear())?;
                let _ = sender.packets(mtc(
                    "^3Setting up - loading layout, please wait...",
                    Some(ConnectionId::ALL),
                ));
                sender.packet(world.ax_load(layout.clone()))?;
            }
            Some(LayoutSpec::Objects(objects)) => {
                sender.packet(Axm {
                    ucid: ConnectionId::LOCAL,
                    action: PmoAction::ClearAll,
                    ..Default::default()
                })?;
                let chunks = objects.chunks(60);
                let last = chunks.len().saturating_sub(1);
                for (index, chunk) in chunks.enumerate() {
                    sender.packet(Axm {
                        ucid: ConnectionId::LOCAL,
                        flags: if index == last {
                            PmoFlags::FILE_END
                        } else {
                            PmoFlags::empty()
                        },
                        action: PmoAction::AddObjects(chunk.to_vec()),
                        ..Default::default()
                    })?;
                }
            }
        }
        Ok(true)
    }
}

impl<S: Send + Sync + 'static, V: kitcar::ui::View + 'static> Handler<(), S, V> for SessionSetup {
    fn call(
        &mut self,
        cx: &ExtractCx<'_, S, V>,
    ) -> impl Future<Output = Result<(), AppError>> + Send {
        let lobby_ready_event = matches!(
            cx.dispatch,
            Dispatch::Synthetic(event) if event.is::<LobbyReady>()
        );
        let track_changed_event = matches!(
            cx.dispatch,
            Dispatch::Synthetic(event) if event.is::<TrackChanged>()
        );
        // NCN responses can arrive before the startup STA response. Until STA
        // has supplied a track, `session() == None` means "unknown", not that
        // LFS is definitely in the lobby. Reconcile from the settled World
        // state rather than trusting an early LobbyReady event on its own.
        let lobby_is_actionable = cx.world.track().is_some()
            && cx.world.session().is_none()
            && cx.world.client_count() > 0;
        let reconcile_lobby = lobby_ready_event || (!self.configured && track_changed_event);
        let result = if lobby_is_actionable && reconcile_lobby {
            let sender = &cx.sender;
            self.ending_session = false;
            let _ = sender.event(SetupStarted);
            if self.configured {
                let _ = sender.packets(mtc(
                    "^2Setup complete - type ^3/ready^2 to start!",
                    Some(ConnectionId::ALL),
                ));
                let _ = sender.event(SetupReady);
                Ok(())
            } else {
                self.configure(cx.world, sender).map(|configured| {
                    if configured {
                        self.configured = true;
                        let _ = sender.packets(mtc(
                            "^2Setup complete - type ^3/ready^2 to start!",
                            Some(ConnectionId::ALL),
                        ));
                        let _ = sender.event(SetupReady);
                    }
                })
            }
        } else if !self.configured
            && !self.ending_session
            && cx.world.session().is_some()
            && cx.world.client_count() > 0
        {
            // A newly installed game can take over a connection while the
            // previous game is still in an active session. LobbyReady cannot
            // be emitted until that session ends, so initiate the transition
            // once and let the resulting lobby event drive configuration.
            let sender = &cx.sender;
            let _ = sender.event(SetupStarted);
            sender.packet(cx.world.end()).map(|()| {
                self.ending_session = true;
            })
        } else {
            Ok(())
        };
        async move { result }
    }
}
