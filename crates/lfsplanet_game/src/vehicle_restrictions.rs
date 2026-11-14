use std::{collections::HashMap, future::Future};

use insim::{
    Packet,
    core::vehicle::Vehicle,
    insim::{PlayerHandicap, Plh},
};
use kitcar::{AppError, Dispatch, ExtractCx, Handler, mtc};

/// A vehicle allowed in a session and the exact handicap it requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VehicleRule {
    pub vehicle: Vehicle,
    pub added_mass_kg: u8,
    pub intake_restriction: u8,
}

/// Applies configured handicaps on selection and verifies them on spawn.
pub struct VehicleRestrictions {
    rules: HashMap<Vehicle, VehicleRule>,
}

impl VehicleRestrictions {
    pub fn new(rules: Vec<VehicleRule>) -> Self {
        Self {
            rules: rules.into_iter().map(|rule| (rule.vehicle, rule)).collect(),
        }
    }

    fn packet(rule: VehicleRule, plid: insim::identifiers::PlayerId) -> Plh {
        // insim.rs does not currently re-export PlayerHandicapFlags, so let
        // serde infer the public field's type from its stable bitflags names.
        let flags = serde_json::from_value(serde_json::json!(["MASS", "TRES", "SILENT"]))
            .expect("static player handicap flags are valid");
        Plh {
            hcaps: vec![PlayerHandicap {
                plid,
                flags,
                h_mass: rule.added_mass_kg,
                h_tres: rule.intake_restriction,
            }],
            ..Default::default()
        }
    }
}

impl<S: Send + Sync + 'static, V: kitcar::ui::View + 'static> Handler<(), S, V>
    for VehicleRestrictions
{
    fn call(
        &mut self,
        cx: &ExtractCx<'_, S, V>,
    ) -> impl Future<Output = Result<(), AppError>> + Send {
        let result = (|| -> Result<(), AppError> {
            match cx.dispatch {
                Dispatch::Packet(Packet::Slc(slc)) => {
                    if let Some(rule) = self.rules.get(&slc.cname).copied() {
                        for player in cx
                            .world
                            .players()
                            .into_iter()
                            .filter(|player| player.ucid == slc.ucid)
                        {
                            cx.sender.packet(Self::packet(rule, player.plid))?;
                        }
                    }
                    Ok(())
                }
                Dispatch::Packet(Packet::Npl(npl)) if !self.rules.is_empty() => {
                    let Some(rule) = self.rules.get(&npl.cname).copied() else {
                        if let Some(connection) = cx.world.connection(npl.ucid) {
                            cx.sender.packet(connection.spec())?;
                            cx.sender.packets(mtc(
                                "^1That vehicle is not allowed for this event.",
                                Some(npl.ucid),
                            ))?;
                        }
                        return Ok(());
                    };
                    if npl.h_mass != rule.added_mass_kg || npl.h_tres != rule.intake_restriction {
                        // PLH needs the live PLID, so it must be queued before /spec.
                        cx.sender.packet(Self::packet(rule, npl.plid))?;
                        if let Some(connection) = cx.world.connection(npl.ucid) {
                            cx.sender.packet(connection.spec())?;
                        }
                        cx.sender.packets(mtc(
                            format!(
                                "^3Vehicle handicap set: ^7{} kg, {}% intake. ^3Please rejoin.",
                                rule.added_mass_kg, rule.intake_restriction
                            ),
                            Some(npl.ucid),
                        ))?;
                    }
                    Ok(())
                }
                _ => Ok(()),
            }
        })();
        std::future::ready(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_handicap_packet_sets_both_required_values() {
        let packet = VehicleRestrictions::packet(
            VehicleRule {
                vehicle: Vehicle::Xrt,
                added_mass_kg: 25,
                intake_restriction: 20,
            },
            insim::identifiers::PlayerId(7),
        );
        assert_eq!(packet.hcaps.len(), 1);
        assert_eq!(packet.hcaps[0].h_mass, 25);
        assert_eq!(packet.hcaps[0].h_tres, 20);
    }
}
