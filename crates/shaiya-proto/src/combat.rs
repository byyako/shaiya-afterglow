//! Packets of players fighting monsters and each other: attacks, death, experience and level-up (`docs/original/combat.md`), the
//! kill and death counters and their reward panels (`docs/original/pvp.md`).

use crate::frame::encode;

/// Server: the player starts fighting (no payload, `CUser::SendAttackStart`).
pub const OP_ATTACK_START: u16 = 0x0212;
/// Server: experience gained (`u32 gain, u32 bonus`).
pub const OP_EXP: u16 = 0x0207;
/// Client: attack a player (`u32 player id`). Server: a player's attack on a player, or the result of a refused attack
/// (`TP_PC_ATTACK`).
pub const OP_ATTACK_USER: u16 = 0x0502;
/// Server: a blow on a player came back to its attacker (`TP_PC_SKILL_MIRROR`: `u32 reflector, u32 attacker, u16 HP, SP, MP`).
pub const OP_MIRROR_USER: u16 = 0x0515;
/// Server: one of the player's counters (`TP_USER_KILLCOUNT`: `u8 index` 0 kills, 1 deaths, `u32 value`).
pub const OP_KILL_COUNT: u16 = 0x020E;
/// Server: a player's kills, to its view (`TP_USER_KILLINFO`: `u32 id, u32 kills`).
pub const OP_KILL_INFO: u16 = 0x0522;
/// Client: the reward panel's steps (no payload). Server: `TP_STATUS_RESULTINFO` (`u8 next kill step, u8 next death step`).
pub const OP_STATUS_INFO: u16 = 0x0218;
/// Client: take the next kill reward (no payload). Server: `TP_STATUS_RESULTKILL` (`u8 ok, u8 stat points`).
pub const OP_STATUS_KILL: u16 = 0x0219;
/// Client: take the next death reward (no payload). Server: `TP_STATUS_RESULTDEATH` (`u8 ok, u32 gold`).
pub const OP_STATUS_DEATH: u16 = 0x021A;
/// `0x020E` indexes.
pub const COUNT_KILLS: u8 = 0;
pub const COUNT_DEATHS: u8 = 1;
/// Client: attack a monster (`u32 monster id`). Server: an attack on a monster (`TP_PC_ATTACK`).
pub const OP_ATTACK_MOB: u16 = 0x0503;
/// Server: HP, MP and SP gained (`TP_USER_RECOVER`, signed).
pub const OP_RECOVER: u16 = 0x0505;
/// Server: a level-up without the effect, and with it (`TP_USER_LEVELUP`).
pub const OP_LEVEL_SILENT: u16 = 0x0508;
pub const OP_LEVEL_UP: u16 = 0x051E;
/// Server: a player's three maxima.
pub const OP_MAX_ALL: u16 = 0x051F;
/// Server: a monster died (`u32 monster id, u8 killer kind, u32 killer id`).
pub const OP_MOB_DEATH: u16 = 0x0606;
/// Killer kind of `0x0606` and `0x0504`: a player, a monster, a guard (`CUser::DeathByNPC`, `CMob::Death(3, guard)`).
pub const KILLER_PLAYER: u8 = 1;
pub const KILLER_MOB: u8 = 2;
pub const KILLER_NPC: u8 = 3;
/// Server: a player died (`TP_DEATH`: `u32 id, u8 killer kind, u32 killer id`).
pub const OP_DEATH: u16 = 0x0504;
/// Server: the player's own HP, MP and SP (`TP_USER_RECOVERSET`, absolute).
pub const OP_RECOVER_SET: u16 = 0x0521;
/// Server: a player came back to life (`TP_DEAD_REBIRTH`).
pub const OP_REBIRTH: u16 = 0x0551;
/// Client: back to life (`u8 kind`: 2 at the bind position, 3 the same with a party, 4 and 5 with an item).
pub const OP_REBIRTH_REQUEST: u16 = 0x0553;
/// Server: a monster's normal attack on a player (`TP_MOB_ATTACKPC`).
pub const OP_MOB_ATTACK: u16 = 0x0605;
/// Server: a monster's speeds (`TP_SET_SPEED`: `u32 id, u8 attack speed, u8 move speed`).
pub const OP_MOB_SPEED: u16 = 0x0609;
/// Server: HP a monster regained (`TP_MOB_RECOVER`: `u32 id, i32 HP`).
pub const OP_MOB_RECOVER: u16 = 0x0610;
/// `0x0551` kind of a resurrection item used where the player lies (`CUser::KillSoulByItemNoMove`).
pub const REBIRTH_HERE: u8 = 1;
/// `0x0551` kind of a return to the bind position (`CUser::KillSoul`).
pub const REBIRTH_BIND: u8 = 4;
/// Next to the party's leader (`CUser::KillSoulParty`).
pub const REBIRTH_PARTY: u8 = 5;

/// `0x0503` from the client: `u32 monster id`.
pub fn parse_attack(payload: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(payload.try_into().ok()?))
}

/// `TP_PC_ATTACK`: `u8 result, u32 attacker, u32 target, u16 HP, SP, MP damage`.
fn attack(opcode: u16, result: u8, attacker: u32, target: u32, damage: [u16; 3]) -> Vec<u8> {
    let mut p = vec![result];
    p.extend_from_slice(&attacker.to_le_bytes());
    p.extend_from_slice(&target.to_le_bytes());
    for d in damage {
        p.extend_from_slice(&d.to_le_bytes());
    }
    encode(opcode, &p)
}

/// `0x0503`: a player's attack on a monster.
pub fn attack_mob(result: u8, attacker: u32, mob: u32, hp_damage: u16) -> Vec<u8> {
    attack(OP_ATTACK_MOB, result, attacker, mob, [hp_damage, 0, 0])
}

/// `0x0502`: a player's blow on a player.
pub fn attack_user(result: u8, attacker: u32, target: u32, damage: [u16; 3]) -> Vec<u8> {
    attack(OP_ATTACK_USER, result, attacker, target, damage)
}

/// `0x0515`: the damage a reflector sent back to its attacker.
pub fn mirror_user(reflector: u32, attacker: u32, damage: [u16; 3]) -> Vec<u8> {
    let mut p = reflector.to_le_bytes().to_vec();
    p.extend_from_slice(&attacker.to_le_bytes());
    for d in damage {
        p.extend_from_slice(&d.to_le_bytes());
    }
    encode(OP_MIRROR_USER, &p)
}

/// `0x020E`.
pub fn kill_count(index: u8, value: u32) -> Vec<u8> {
    let mut p = vec![index];
    p.extend_from_slice(&value.to_le_bytes());
    encode(OP_KILL_COUNT, &p)
}

/// `0x0522`.
pub fn kill_info(id: u32, kills: u32) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.extend_from_slice(&kills.to_le_bytes());
    encode(OP_KILL_INFO, &p)
}

/// `0x0218`.
pub fn status_info(kill_step: u8, death_step: u8) -> Vec<u8> {
    encode(OP_STATUS_INFO, &[kill_step, death_step])
}

/// `0x0219`.
pub fn status_kill(ok: bool, stat_points: u8) -> Vec<u8> {
    encode(OP_STATUS_KILL, &[u8::from(ok), stat_points])
}

/// `0x021A`.
pub fn status_death(ok: bool, gold: u32) -> Vec<u8> {
    let mut p = vec![u8::from(ok)];
    p.extend_from_slice(&gold.to_le_bytes());
    encode(OP_STATUS_DEATH, &p)
}

/// `0x0502` to the player alone when an attack cannot go on (target 0, no damage).
pub fn attack_refused(result: u8, attacker: u32) -> Vec<u8> {
    attack(OP_ATTACK_USER, result, attacker, 0, [0; 3])
}

pub fn attack_start() -> Vec<u8> {
    encode(OP_ATTACK_START, &[])
}

/// `0x0606`.
pub fn mob_death(mob: u32, killer_kind: u8, killer: u32) -> Vec<u8> {
    let mut p = mob.to_le_bytes().to_vec();
    p.push(killer_kind);
    p.extend_from_slice(&killer.to_le_bytes());
    encode(OP_MOB_DEATH, &p)
}

/// `0x0207`.
pub fn exp_gain(gain: u32, bonus: u32) -> Vec<u8> {
    let mut p = gain.to_le_bytes().to_vec();
    p.extend_from_slice(&bonus.to_le_bytes());
    encode(OP_EXP, &p)
}

/// `0x0508` (`effect` false) or `0x051E`: `u32 id, u16 level, u16 stat points, u16 skill points, u32 previous total, u32 next
/// total`.
pub fn level_up(effect: bool, id: u32, level: u16, stat_point: u16, skill_point: u16, exp_bounds: (u32, u32)) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    for v in [level, stat_point, skill_point] {
        p.extend_from_slice(&v.to_le_bytes());
    }
    p.extend_from_slice(&exp_bounds.0.to_le_bytes());
    p.extend_from_slice(&exp_bounds.1.to_le_bytes());
    encode(if effect { OP_LEVEL_UP } else { OP_LEVEL_SILENT }, &p)
}

/// `0x051F`: `u32 id, u16 max HP, max MP, max SP`.
pub fn max_all(id: u32, max: [u16; 3]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    for v in max {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_MAX_ALL, &p)
}

/// `0x0505`: `u32 id, i16 HP, MP, SP` added.
pub fn recover(id: u32, gained: [i16; 3]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    for v in gained {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_RECOVER, &p)
}

/// `0x0605`: `u8 result, u32 monster, u32 player, u16 HP, SP, MP damage`.
pub fn mob_attack(result: u8, mob: u32, player: u32, hp_damage: u16) -> Vec<u8> {
    mob_attack_values(result, mob, player, [hp_damage, 0, 0])
}

/// `0x0605` with SP and MP damage too (a give-up buff moves HP damage onto them).
pub fn mob_attack_values(result: u8, mob: u32, player: u32, damage: [u16; 3]) -> Vec<u8> {
    attack(OP_MOB_ATTACK, result, mob, player, damage)
}

/// Server: HP damage absorbed (`u16`, "<n> absorption damage").
pub const OP_ABSORBED: u16 = 0x0525;

/// `0x0525`: the HP damage absorption took (`docs/original/lapis.md` section 2).
pub fn absorbed(points: u16) -> Vec<u8> {
    encode(OP_ABSORBED, &points.to_le_bytes())
}

/// `0x0504`.
pub fn death(id: u32, killer_kind: u8, killer: u32) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(killer_kind);
    p.extend_from_slice(&killer.to_le_bytes());
    encode(OP_DEATH, &p)
}

/// `0x0521`: `u16 HP, MP, SP`.
pub fn recover_set(health: [u16; 3]) -> Vec<u8> {
    encode(OP_RECOVER_SET, &health.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>())
}

/// `0x0551`: `u32 id, u8 kind, u32 experience, f32 x, y, z`.
pub fn rebirth(id: u32, kind: u8, exp: u32, position: [f32; 3]) -> Vec<u8> {
    let mut p = id.to_le_bytes().to_vec();
    p.push(kind);
    p.extend_from_slice(&exp.to_le_bytes());
    for v in position {
        p.extend_from_slice(&v.to_le_bytes());
    }
    encode(OP_REBIRTH, &p)
}

/// `0x0553` from the client: `u8 kind`.
pub fn parse_rebirth_request(payload: &[u8]) -> Option<u8> {
    match payload {
        [kind] => Some(*kind),
        _ => None,
    }
}

/// `0x0609`.
pub fn mob_speed(mob: u32, attack_speed: u8, move_speed: u8) -> Vec<u8> {
    let mut p = mob.to_le_bytes().to_vec();
    p.extend_from_slice(&[attack_speed, move_speed]);
    encode(OP_MOB_SPEED, &p)
}

/// `0x0610`.
pub fn mob_recover(mob: u32, gained: i32) -> Vec<u8> {
    let mut p = mob.to_le_bytes().to_vec();
    p.extend_from_slice(&gained.to_le_bytes());
    encode(OP_MOB_RECOVER, &p)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::payload;

    #[test]
    fn packets_have_the_original_sizes() {
        // Sizes as the original counts them: opcode and payload.
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&attack_mob(1, 7, 9, 300)), 17);
        assert_eq!(payload(&attack_mob(1, 7, 9, 300)), [1, 7, 0, 0, 0, 9, 0, 0, 0, 0x2c, 1, 0, 0, 0, 0]);
        assert_eq!(len(&attack_refused(7, 7)), 17);
        assert_eq!(len(&attack_start()), 2);
        assert_eq!(len(&mob_death(9, KILLER_PLAYER, 7)), 11);
        assert_eq!(len(&exp_gain(5, 0)), 10);
        assert_eq!(len(&level_up(true, 7, 11, 5, 3, (100, 200))), 20);
        assert_eq!(&level_up(false, 7, 11, 5, 3, (100, 200))[2..4], OP_LEVEL_SILENT.to_le_bytes());
        assert_eq!(len(&max_all(7, [1, 2, 3])), 12);
        assert_eq!(payload(&recover(7, [-1, 2, 3])), [7, 0, 0, 0, 0xff, 0xff, 2, 0, 3, 0]);
        assert_eq!(parse_attack(&[9, 0, 0, 0]), Some(9));
        assert_eq!(parse_attack(&[9, 0, 0]), None);
        assert_eq!(len(&mob_attack(1, 9, 7, 30)), 17);
        assert_eq!(payload(&mob_attack(1, 9, 7, 30))[..9], [1, 9, 0, 0, 0, 7, 0, 0, 0]);
        assert_eq!(len(&death(7, KILLER_MOB, 9)), 11);
        assert_eq!(payload(&absorbed(12)), [12, 0]);
        assert_eq!(len(&recover_set([1, 2, 3])), 8);
        assert_eq!(len(&rebirth(7, REBIRTH_BIND, 100, [1.0, 2.0, 3.0])), 23);
        assert_eq!(parse_rebirth_request(&[2]), Some(2));
        assert_eq!(parse_rebirth_request(&[]), None);
        assert_eq!(len(&mob_speed(9, 3, 2)), 8);
        assert_eq!(len(&mob_recover(9, 15)), 10);
    }

    #[test]
    fn player_kill_packets_have_the_original_sizes() {
        let len = |p: &[u8]| p.len() - 2;
        assert_eq!(len(&attack_user(1, 7, 8, [300, 0, 0])), 17);
        assert_eq!(payload(&attack_user(0, 7, 8, [1, 2, 3]))[9..], [1, 0, 2, 0, 3, 0]);
        assert_eq!(len(&mirror_user(8, 7, [5, 0, 0])), 16);
        assert_eq!(payload(&kill_count(COUNT_DEATHS, 3)), [1, 3, 0, 0, 0]);
        assert_eq!(len(&kill_count(COUNT_KILLS, 3)), 7);
        assert_eq!(len(&kill_info(7, 3)), 10);
        assert_eq!(len(&status_info(1, 1)), 4);
        assert_eq!(payload(&status_kill(true, 3)), [1, 3]);
        assert_eq!(len(&status_death(true, 100)), 7);
    }
}
