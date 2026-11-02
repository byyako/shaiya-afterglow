//! Mounts in the zone (`docs/original/vehicles.md`): each player's vehicle status, the mount's shape, the two-seat pair and its
//! invitation, and their packets to the players around. Part of the zone: the functions take `maps` themselves or get the map's
//! players from a caller that holds it. The summon's timing and the mount item are the connection's.

use super::*;
use crate::sync::Relock;
use shaiya_proto::vehicle as vp;

/// A mount being ridden: its shape (`m_nVehicleType + m_nVehicleTypeAdd`, section 3) and whether it takes a passenger (type 15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mount {
    pub shape: u8,
    pub two_seat: bool,
}

/// The vehicle status (`m_nVehicleStatus`): 0 on foot, 1 summoning, 2 riding, 3 behind someone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Riding {
    #[default]
    OnFoot,
    Summoning,
    Riding(Mount),
    Behind,
}

/// A player's part in mounts: its status, the other one of its pair (`m_dwVehicle2CharID`: a driver's passenger, a passenger's
/// driver), and an invitation (`m_dwVehicle2ReqID`; the asker keeps the 30 s deadline, `m_dwTimeOverVehicle2`).
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Ride {
    pub(super) status: Riding,
    pub(super) partner: u32,
    pub(super) invite: u32,
    pub(super) invite_until: Option<Instant>,
}

/// How long an invitation to ride behind waits for its answer.
const INVITE_TIME: Duration = Duration::from_secs(30);

type Players = HashMap<u32, Placed>;

impl Placed {
    /// `0x051D` with the shape others see (`CUser::SendShape`): the mount's while riding, otherwise the hiding or transformation.
    pub(super) fn shape_packet(&self) -> Vec<u8> {
        if !self.occupant.visible {
            return sk::shape(self.occupant.id, super::INVISIBLE_SHAPE);
        }
        match self.ride.status {
            Riding::Riding(mount) => sk::shape(self.occupant.id, mount.shape),
            _ => form_packet(self.occupant.id, self.form),
        }
    }

    /// The asker's invitation, while its 30 s last (`CUser::UpdateTimeover` ends it).
    fn asking(&self, now: Instant) -> Option<u32> {
        (self.ride.invite != 0 && self.ride.invite_until.is_none_or(|until| now < until)).then_some(self.ride.invite)
    }
}

/// The shape to the player's view, itself included.
fn show_shape(players: &Players, id: u32) {
    let Some(p) = players.get(&id) else { return };
    let packet = p.shape_packet();
    players.values().filter(|o| in_view(o.cell, p.cell)).for_each(|o| o.send(&packet));
}

/// `0x021C` about a passenger to its view (`CUser::SendShapeVehicle2`): its driver, or 0 once it got off.
fn show_behind(players: &Players, passenger: u32) {
    let Some(p) = players.get(&passenger) else { return };
    let driver = if p.ride.status == Riding::Behind { p.ride.partner } else { 0 };
    let packet = vp::behind(passenger, driver);
    players.values().filter(|o| in_view(o.cell, p.cell)).for_each(|o| o.send(&packet));
}

/// A passenger is put down: on foot, no driver, `0x021C` with 0 to its view.
fn put_down(players: &mut Players, passenger: u32) {
    let Some(p) = players.get_mut(&passenger) else { return };
    if p.ride.status == Riding::Behind {
        p.ride.status = Riding::OnFoot;
    }
    p.ride.partner = 0;
    show_behind(players, passenger);
}

/// `CUser::CancleActionVehicle` (section 4): a driver's passenger is put down and its invitation cleared, silently. The status is
/// the caller's.
fn drop_passenger(players: &mut Players, driver: u32) {
    let Some(p) = players.get_mut(&driver) else { return };
    if !matches!(p.ride.status, Riding::Riding(_)) {
        return;
    }
    let (passenger, asked) = (std::mem::take(&mut p.ride.partner), std::mem::take(&mut p.ride.invite));
    p.ride.invite_until = None;
    if passenger != 0 {
        put_down(players, passenger);
    }
    if let Some(t) = players.get_mut(&asked).filter(|t| t.ride.invite == driver) {
        t.ride.invite = 0;
    }
}

/// `CUser::StatusCastingStop`, its vehicle part (section 2): summoning stops silently; a rider gets off (its shape to the view, no
/// `0x0216`); a passenger gets off and so does its driver.
pub(super) fn casting_stop(players: &mut Players, id: u32) {
    let Some(p) = players.get_mut(&id) else { return };
    match p.ride.status {
        Riding::OnFoot => {}
        Riding::Summoning => p.ride.status = Riding::OnFoot,
        Riding::Riding(_) => {
            drop_passenger(players, id);
            if let Some(p) = players.get_mut(&id) {
                p.ride.status = Riding::OnFoot;
            }
            show_shape(players, id);
        }
        Riding::Behind => {
            let driver = p.ride.partner;
            put_down(players, id);
            if let Some(d) = players.get_mut(&driver) {
                d.ride.partner = 0;
                d.ride.status = Riding::OnFoot;
                show_shape(players, driver);
            }
        }
    }
}

/// A pair splits as the player leaves its place (`CZone::MoveUser`, `CWorld::ZoneLeaveUserMove`, `CUser::LeaveWorld`): the passenger
/// is put down, the driver rides on. The invitations end too (`CUser::LeaveZone`): a target's asker that still rides gets `0x021E` 2.
pub(super) fn split(players: &mut Players, id: u32, leaving: bool, now: Instant) {
    let Some(p) = players.get(&id) else { return };
    let (status, partner) = (p.ride.status, p.ride.partner);
    match status {
        Riding::Riding(_) if partner != 0 => {
            put_down(players, partner);
            if let Some(p) = players.get_mut(&id) {
                p.ride.partner = 0;
            }
        }
        Riding::Behind => {
            if let Some(d) = players.get_mut(&partner) {
                d.ride.partner = 0;
            }
            put_down(players, id);
        }
        _ => {}
    }
    if !leaving {
        return;
    }
    let Some(p) = players.get_mut(&id) else { return };
    let other = std::mem::take(&mut p.ride.invite);
    p.ride.invite_until = None;
    let was_asking = matches!(p.ride.status, Riding::Riding(_));
    if p.ride.status == Riding::Summoning {
        p.ride.status = Riding::OnFoot;
    }
    if other == 0 {
        return;
    }
    if let Some(o) = players.get_mut(&other).filter(|o| o.ride.invite == id) {
        // The one asked left: its asker, still riding and asking, is told.
        if !was_asking && matches!(o.ride.status, Riding::Riding(_)) && o.asking(now).is_some() {
            o.send(&vp::answer(vp::ASK_FAILED));
        }
        o.ride.invite = 0;
        o.ride.invite_until = None;
    }
}

impl Zones {
    /// The player's vehicle status.
    pub fn riding(&self, map: u16, id: u32) -> Riding {
        self.maps.guard().get(&map).and_then(|p| p.get(&id)).map_or(Riding::OnFoot, |p| p.ride.status)
    }

    /// The summoning starts (section 1): an invitation the player received is dropped (its asker gets `0x021E` 2), and `0x0217` goes
    /// to the view; returns it for the player.
    pub fn start_summon(&self, map: u16, id: u32) -> Vec<u8> {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return Vec::new() };
        let Some(p) = players.get_mut(&id) else { return Vec::new() };
        p.ride.status = Riding::Summoning;
        let asker = std::mem::take(&mut p.ride.invite);
        if let Some(a) = players.get_mut(&asker).filter(|a| a.ride.invite == id && matches!(a.ride.status, Riding::Riding(_))) {
            a.ride.invite = 0;
            a.ride.invite_until = None;
            a.send(&vp::answer(vp::ASK_FAILED));
        }
        let packet = vp::ready(id);
        let p = &players[&id];
        for o in players.values().filter(|o| o.occupant.id != id && in_view(o.cell, p.cell)) {
            o.send(&packet);
        }
        packet
    }

    /// A summon stops without a word (a step, a failed completion).
    pub fn stop_summon(&self, map: u16, id: u32) {
        if let Some(p) = self.maps.guard().get_mut(&map).and_then(|p| p.get_mut(&id))
            && p.ride.status == Riding::Summoning
        {
            p.ride.status = Riding::OnFoot;
        }
    }

    /// The summon completes: riding, the mount's shape to the view; returns it for the player. `None` when the player is no longer
    /// summoning.
    pub fn mount(&self, map: u16, id: u32, mount: Mount) -> Option<Vec<u8>> {
        let mut maps = self.maps.guard();
        let players = maps.get_mut(&map)?;
        let p = players.get_mut(&id).filter(|p| p.ride.status == Riding::Summoning)?;
        p.ride.status = Riding::Riding(mount);
        let packet = p.shape_packet();
        let p = &players[&id];
        for o in players.values().filter(|o| o.occupant.id != id && in_view(o.cell, p.cell)) {
            o.send(&packet);
        }
        Some(packet)
    }

    /// Getting off, or a summon stopped, by `0x0216` or a change of the mount slot (sections 1 and 2): a passenger put down, on foot,
    /// the shape to the view; returns it for the player. `None` when neither summoning nor riding.
    pub fn dismount(&self, map: u16, id: u32) -> Option<Vec<u8>> {
        let mut maps = self.maps.guard();
        let players = maps.get_mut(&map)?;
        let status = players.get(&id)?.ride.status;
        if !matches!(status, Riding::Summoning | Riding::Riding(_)) {
            return None;
        }
        drop_passenger(players, id);
        let p = players.get_mut(&id)?;
        p.ride.status = Riding::OnFoot;
        let packet = p.shape_packet();
        let p = &players[&id];
        for o in players.values().filter(|o| o.occupant.id != id && in_view(o.cell, p.cell)) {
            o.send(&packet);
        }
        Some(packet)
    }

    /// `0x021D` (section 4): a driver of a two-seat mount with nobody behind asks an on-foot player of its country, in the zone and not
    /// asked already. Returns `0x021E` 2 for the asker when that fails; otherwise the target gets `0x021D`.
    pub fn ask_to_ride(&self, map: u16, asker: u32, target: u32, now: Instant) -> Option<Vec<u8>> {
        let failed = Some(vp::answer(vp::ASK_FAILED));
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return failed };
        let Some(a) = players.get(&asker) else { return failed };
        let two_seat = matches!(a.ride.status, Riding::Riding(Mount { two_seat: true, .. }));
        let country = a.occupant.shape.country;
        let fits = |t: &Placed| t.ride.invite == 0 && t.ride.status == Riding::OnFoot && t.occupant.shape.country == country;
        if !two_seat || a.ride.partner != 0 || target == asker || !players.get(&target).is_some_and(fits) {
            return failed;
        }
        let a = players.get_mut(&asker)?;
        a.ride.invite = target;
        a.ride.invite_until = Some(now + INVITE_TIME);
        let t = players.get_mut(&target)?;
        t.ride.invite = asker;
        t.send(&vp::ask(asker));
        None
    }

    /// `0x021E` (section 4): the answer of an on-foot player with an invitation goes to its asker as it came; yes (0) puts it behind
    /// the asker (`0x021C` to its view). Both invitations end.
    pub fn answer_ride(&self, map: u16, target: u32, answer: u8, now: Instant) {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(t) = players.get_mut(&target) else { return };
        if t.ride.status != Riding::OnFoot || t.ride.invite == 0 {
            return;
        }
        let asker = std::mem::take(&mut t.ride.invite);
        let Some(a) = players.get_mut(&asker).filter(|a| a.asking(now) == Some(target)) else { return };
        a.send(&vp::answer(answer));
        a.ride.invite = 0;
        a.ride.invite_until = None;
        if answer == vp::ANSWER_YES {
            a.ride.partner = target;
            let t = players.get_mut(&target).expect("found above");
            t.ride.partner = asker;
            t.ride.status = Riding::Behind;
            show_behind(players, target);
        }
    }

    /// `0x021C` from a passenger (section 4): it gets off; its driver rides on.
    pub fn get_off_behind(&self, map: u16, passenger: u32) {
        let mut maps = self.maps.guard();
        let Some(players) = maps.get_mut(&map) else { return };
        let Some(p) = players.get(&passenger).filter(|p| p.ride.status == Riding::Behind) else { return };
        let driver = p.ride.partner;
        if let Some(d) = players.get_mut(&driver) {
            d.ride.partner = 0;
        }
        put_down(players, passenger);
    }
}
