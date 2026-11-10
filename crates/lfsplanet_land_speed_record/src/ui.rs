use super::state::LandSpeedRecordGlobal;
use kitcar::ui::{self, Component, Ui, View};
use lfsplanet_game::{hud_active, hud_muted, hud_text, hud_title};

#[derive(Clone, Default, Debug)]
pub(super) struct LandSpeedRecordConnectionProps {
    pub(super) uname: String,
}

pub(super) struct LandSpeedRecordView;
impl Component for LandSpeedRecordView {
    type Message = ();
    type Props<'a> = (
        &'a LandSpeedRecordGlobal,
        &'a LandSpeedRecordConnectionProps,
    );
    fn render(&self, (global, player): Self::Props<'_>) -> ui::Node<Self::Message> {
        let status = global
            .leaderboard
            .iter()
            .find(|entry| entry.uname.eq_ignore_ascii_case(&player.uname))
            .map_or_else(
                || "Cross checkpoint 1 at speed".to_string(),
                |entry| format!("PB: {:.2} km/h", entry.speed_kph),
            );
        let rows: Vec<ui::Node<()>> = global
            .leaderboard
            .iter()
            .take(8)
            .enumerate()
            .map(|(i, standing)| {
                let style = if standing.uname.eq_ignore_ascii_case(&player.uname) {
                    hud_active()
                } else {
                    hud_text()
                };
                ui::row([
                    ui::text(format!("#{}", i + 1), style).w(5.0).h(5.0),
                    ui::text(standing.pname.as_str(), style.align_left())
                        .w(20.0)
                        .h(5.0),
                    ui::text(
                        format!("{:.2} km/h", standing.speed_kph),
                        style.align_right(),
                    )
                    .w(20.0)
                    .h(5.0),
                ])
            })
            .collect();
        ui::col([
            ui::row([
                ui::text(format!("Land Speed Record - {}", global.phase), hud_title())
                    .w(55.0)
                    .h(5.0),
                ui::text(status, hud_muted()).w(45.0).h(5.0),
            ])
            .justify_center(),
            ui::col([ui::text("Fastest runs", hud_title()).w(45.0).h(5.0)])
                .pl(5.0)
                .w(200.0)
                .mt(10.0)
                .items_start()
                .with_children(rows),
        ])
    }
}
impl View for LandSpeedRecordView {
    type Global = LandSpeedRecordGlobal;
    type Connection = LandSpeedRecordConnectionProps;
    fn mount(_: insim::identifiers::ConnectionId, _: ui::ViewHandle<Self::Message>) -> Self {
        Self
    }
    fn props<'a>(global: &'a Self::Global, connection: &'a Self::Connection) -> Self::Props<'a> {
        (global, connection)
    }
}
pub(super) type LandSpeedRecordUi = Ui<LandSpeedRecordView>;
