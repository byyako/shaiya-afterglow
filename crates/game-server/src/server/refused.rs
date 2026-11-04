//! Telling a player why a rule refused it (`docs/plans/ext-2-rules.md` decision 1 and section 5.3): a whisper from `Server`, the rule's
//! own message or the point's default text, at most once per rule per player every 10 seconds; a silent rule says nothing.

use super::*;
use crate::rules::Refusal;

/// How long a rule keeps quiet toward a player it told.
const TELL_EVERY: Duration = Duration::from_secs(10);
/// How many rules a player's list remembers.
const TOLD_MAX: usize = 8;

/// The text to whisper for `refusal` now, or `None` (a silent rule, or the player was told by that rule in the last 10 seconds). Notes
/// the telling in `told`.
fn due(told: &mut Vec<(u32, Instant)>, refusal: &Refusal, now: Instant) -> Option<Arc<str>> {
    let text = refusal.text.clone()?;
    if told.iter().any(|&(rule, at)| rule == refusal.rule && now.saturating_duration_since(at) < TELL_EVERY) {
        return None;
    }
    told.retain(|&(rule, _)| rule != refusal.rule);
    if told.len() >= TOLD_MAX {
        told.remove(0);
    }
    told.push((refusal.rule, now));
    Some(text)
}

/// Whispers the refusal of a request the player made, within the limit above.
pub(super) async fn tell_refused(conn: &mut Connection, state: &mut WorldState, refusal: &Refusal) -> Result<()> {
    if let Some(text) = due(&mut state.told, refusal, Instant::now()) {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        conn.send(chat::whisper(false, SERVER_NAME, &bytes)).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refusal(rule: u32, text: Option<&str>) -> Refusal {
        Refusal { rule, name: Arc::from("x"), text: text.map(Arc::from) }
    }

    #[test]
    fn a_rule_whispers_once_in_ten_seconds() {
        let mut told = Vec::new();
        let start = Instant::now();
        let gap = refusal(1, Some("Too far apart."));
        assert_eq!(due(&mut told, &gap, start).as_deref(), Some("Too far apart."));
        assert!(due(&mut told, &gap, start + Duration::from_secs(1)).is_none(), "the second within 10 s says nothing");
        assert!(due(&mut told, &gap, start + Duration::from_millis(9_999)).is_none());
        assert_eq!(due(&mut told, &gap, start + Duration::from_secs(10)).as_deref(), Some("Too far apart."), "after 10 s it may again");
        assert!(due(&mut told, &gap, start + Duration::from_secs(11)).is_none(), "the limit runs from the last telling");
        assert_eq!(told.len(), 1);
    }

    #[test]
    fn another_rule_whispers_and_a_silent_one_does_not() {
        let mut told = Vec::new();
        let start = Instant::now();
        assert!(due(&mut told, &refusal(1, Some("a")), start).is_some());
        assert_eq!(due(&mut told, &refusal(2, Some("b")), start).as_deref(), Some("b"), "each rule has its own limit");
        assert!(due(&mut told, &refusal(3, None), start).is_none(), "message \"\" is silent");
        assert_eq!(told.len(), 2, "a silent rule is not remembered");
    }

    #[test]
    fn the_list_keeps_eight_and_drops_the_oldest() {
        let mut told = Vec::new();
        let start = Instant::now();
        for rule in 1..=9 {
            assert!(due(&mut told, &refusal(rule, Some("x")), start + Duration::from_millis(u64::from(rule))).is_some());
        }
        assert_eq!(told.len(), 8);
        assert!(!told.iter().any(|&(rule, _)| rule == 1), "the oldest was dropped");
        assert!(due(&mut told, &refusal(1, Some("x")), start + Duration::from_millis(20)).is_some(), "so rule 1 may whisper again at once");
        assert!(due(&mut told, &refusal(9, Some("x")), start + Duration::from_millis(21)).is_none());
    }
}
