//! Turns an account `status` value into a login decision.
//!
//! This is the original server's `Users_Master.Status` logic (`FUN_00406e50` in `ps_login.exe`,
//! documented in `docs/original/ps_login.md` section 12). The value mixes account state and staff
//! level: 0 is a normal account, 16/32/48/64/80 are staff levels 1 to 5 (1 has full operator powers),
//! other values block or restrict the account.

use shaiya_proto::login::{LoginStatus, PRIVILEGE_PLAYER};

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    /// Login allowed; `privilege` is the byte sent to the client (11 player, 1 to 5 staff).
    Allowed {
        privilege: u8,
    },
    Refused(LoginStatus),
}

pub fn decide(status: i16) -> Decision {
    use Decision::Refused;
    use LoginStatus as L;

    match status {
        -3 => Refused(L::NoSuchAccount),
        -2 => Refused(L::CannotConnect),
        -1 => Refused(L::BadCredentials),
        1 => Refused(L::UserStop),
        2 => Refused(L::NotAllowed),
        6 => Refused(L::NotAllowedWeb),
        0x11 => Refused(L::Blocked),
        0x21 => Refused(L::Restricted),
        0x31 => Refused(L::Restricted2),
        0x41 => Refused(L::Restricted3),
        0x51 => Refused(L::Restricted4),
        0x80 => Refused(L::Withdrawal),
        0x100 => Refused(L::Withdrawal3Days),
        0x200 => Refused(L::Withdrawal15Days),
        _ => {
            // The original tests the raw 16 bits of the value.
            let bits = status as u16;
            if bits & 2 != 0 {
                Refused(L::Blocked)
            } else if bits & 1 != 0 {
                Refused(L::Restricted)
            } else if (bits >> 7) & 7 == 0 {
                let level = ((bits as u8) >> 4) & 7;
                Decision::Allowed { privilege: if level == 0 { PRIVILEGE_PLAYER } else { level } }
            } else {
                Refused(L::Withdrawal)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Decision::{Allowed, Refused};
    use LoginStatus as L;

    #[test]
    fn every_documented_status_maps_as_in_the_original() {
        let cases: &[(i16, Decision)] = &[
            (-3, Refused(L::NoSuchAccount)),
            (-2, Refused(L::CannotConnect)),
            (-1, Refused(L::BadCredentials)),
            (0, Allowed { privilege: 11 }),
            (1, Refused(L::UserStop)),
            (2, Refused(L::NotAllowed)),
            (6, Refused(L::NotAllowedWeb)),
            (0x11, Refused(L::Blocked)),
            (0x21, Refused(L::Restricted)),
            (0x31, Refused(L::Restricted2)),
            (0x41, Refused(L::Restricted3)),
            (0x51, Refused(L::Restricted4)),
            (0x80, Refused(L::Withdrawal)),
            (0x100, Refused(L::Withdrawal3Days)),
            (0x200, Refused(L::Withdrawal15Days)),
        ];
        for (status, expected) in cases {
            assert_eq!(&decide(*status), expected, "status {status}");
        }
    }

    #[test]
    fn staff_levels_are_status_divided_by_sixteen() {
        for (status, level) in [(16, 1), (32, 2), (48, 3), (64, 4), (80, 5)] {
            assert_eq!(decide(status), Allowed { privilege: level }, "status {status}");
        }
    }

    #[test]
    fn web_and_character_blocks_do_not_stop_the_login() {
        // The guide lists 4 (web) and 8 (character) as blocks enforced elsewhere; the login server lets them in.
        assert_eq!(decide(4), Allowed { privilege: 11 });
        assert_eq!(decide(8), Allowed { privilege: 11 });
    }

    #[test]
    fn nine_is_restricted_because_bit_zero_is_set() {
        assert_eq!(decide(9), Refused(L::Restricted));
    }

    #[test]
    fn unlisted_values_follow_the_bit_rules() {
        assert_eq!(decide(3), Refused(L::Blocked), "bit 1 wins over bit 0");
        assert_eq!(decide(5), Refused(L::Restricted));
        assert_eq!(decide(0x90), Refused(L::Withdrawal), "a withdrawal bit with a staff bit");
    }

    /// The original files' `Docs/` (the configuration guide) lists the messages players see for negative statuses. The bit rules decoded
    /// from the binary agree with it for these values (blocked / restricted / termination in progress).
    #[test]
    fn negative_statuses_agree_with_the_guide() {
        let blocked = [-5, -6, -9, -13, -14];
        let restricted = [-7, -11, -15];
        let termination = [-4, -12, -16];
        for status in blocked {
            assert_eq!(decide(status), Refused(L::Blocked), "status {status}");
        }
        for status in restricted {
            assert_eq!(decide(status), Refused(L::Restricted), "status {status}");
        }
        for status in termination {
            assert_eq!(decide(status), Refused(L::Withdrawal), "status {status}");
        }
        // -8 and -10 are the two where the guide's (machine-translated) text differs from what the code does.
    }
}
