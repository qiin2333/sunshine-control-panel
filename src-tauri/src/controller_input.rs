use std::time::{Duration, Instant};

// XInput button bits plus two synthetic bits for the independent triggers.
const BUTTONS: &[(&str, u32)] = &[
    ("Back", 0x20),
    ("Start", 0x10),
    ("LB", 0x100),
    ("RB", 0x200),
    ("LT", 0x10000),
    ("RT", 0x20000),
    ("A", 0x1000),
    ("B", 0x2000),
    ("X", 0x4000),
    ("Y", 0x8000),
    ("L3", 0x40),
    ("R3", 0x80),
    ("Up", 1),
    ("Down", 2),
    ("Left", 4),
    ("Right", 8),
];
const MODIFIERS: u32 = 0x30330;
const HOLD: Duration = Duration::from_millis(500);

pub(crate) fn parse(value: &str) -> Result<u32, String> {
    if value.is_empty() {
        return Ok(0);
    }
    if value.len() > 100 {
        return Err("controller_shortcut_invalid".into());
    }
    let mut mask = 0;
    for part in value.split('+') {
        let bit = BUTTONS
            .iter()
            .find(|(name, _)| *name == part)
            .map(|(_, bit)| *bit)
            .ok_or("controller_shortcut_invalid")?;
        if mask & bit != 0 {
            return Err("controller_shortcut_invalid".into());
        }
        mask |= bit;
    }
    if !valid(mask) {
        return Err("controller_shortcut_invalid".into());
    }
    Ok(mask)
}
fn valid(mask: u32) -> bool {
    mask.count_ones() >= 2 && mask & MODIFIERS != 0 && mask & 3 != 3 && mask & 12 != 12
}
pub(crate) fn label(mask: u32) -> String {
    BUTTONS
        .iter()
        .filter(|(_, bit)| mask & bit != 0)
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join("+")
}
pub(crate) fn normalize(value: &str) -> Result<String, String> {
    parse(value).map(label)
}
pub(crate) fn validate(nr: &str, overlay: &str) -> Result<(), String> {
    let nr = parse(nr)?;
    let overlay = parse(overlay)?;
    if nr != 0 && nr == overlay {
        return Err("controller_shortcut_duplicate".into());
    }
    Ok(())
}

#[derive(Default)]
pub(crate) struct Chord {
    armed: bool,
    candidate: u32,
    since: Option<Instant>,
}
impl Chord {
    // Start/reconnect/configuration changes require a neutral sample first.
    // A held chord fires once and cannot rearm until every button is released.
    pub(crate) fn update(&mut self, mask: Option<u32>, now: Instant) -> Option<u32> {
        let Some(mask) = mask else {
            *self = Self::default();
            return None;
        };
        if mask == 0 {
            *self = Self {
                armed: true,
                ..Self::default()
            };
        } else if self.armed {
            if mask != self.candidate {
                self.candidate = mask;
                self.since = Some(now);
            } else if valid(mask)
                && self
                    .since
                    .is_some_and(|since| now.duration_since(since) >= HOLD)
            {
                self.armed = false;
                return Some(mask);
            }
        }
        None
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn read(index: u32) -> Option<u32> {
    use windows::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};
    let mut state = XINPUT_STATE::default();
    if unsafe { XInputGetState(index, &mut state) } != 0 {
        return None;
    }
    let pad = state.Gamepad;
    Some(
        u32::from(pad.wButtons.0) & 0xf3ff
            | if pad.bLeftTrigger >= 128 { 0x10000 } else { 0 }
            | if pad.bRightTrigger >= 128 { 0x20000 } else { 0 },
    )
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn read(_: u32) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_and_canonicalizes_combinations() {
        assert_eq!(normalize("A+LB").unwrap(), "LB+A");
        for invalid in [
            "A",
            "A+B",
            "LB+LB",
            "Guide+A",
            "LB+Up+Down",
            "LB+Left+Right",
        ] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
        assert!(validate("A+LB", "LB+A").is_err());
        assert!(validate("", "").is_ok());
        assert!(parse("LT+RT").is_ok());
    }
    #[test]
    fn hold_release_and_reconnect_boundaries() {
        let mut chord = Chord::default();
        let now = Instant::now();
        let mask = parse("LB+A").unwrap();
        assert_eq!(chord.update(Some(mask), now), None);
        assert_eq!(chord.update(Some(mask), now + HOLD), None);
        chord.update(Some(0), now);
        chord.update(Some(mask), now);
        assert_eq!(
            chord.update(Some(mask), now + HOLD - Duration::from_millis(1)),
            None
        );
        assert_eq!(chord.update(Some(mask), now + HOLD), Some(mask));
        assert_eq!(chord.update(Some(mask), now + HOLD * 2), None);
        chord.update(Some(0x100), now + HOLD * 2);
        assert_eq!(chord.update(Some(mask), now + HOLD * 3), None);
        chord.update(None, now + HOLD * 3);
        assert_eq!(chord.update(Some(mask), now + HOLD * 4), None);
        chord.update(Some(0), now + HOLD * 4);
        chord.update(Some(mask), now + HOLD * 4);
        assert_eq!(chord.update(Some(mask), now + HOLD * 5), Some(mask));
    }
    #[test]
    fn changed_chords_restart_hold_and_controllers_do_not_combine() {
        let now = Instant::now();
        let mut first = Chord::default();
        let mut second = Chord::default();
        first.update(Some(0), now);
        second.update(Some(0), now);
        first.update(Some(0x100), now);
        second.update(Some(0x1000), now);
        assert_eq!(first.update(Some(0x100), now + HOLD), None);
        assert_eq!(second.update(Some(0x1000), now + HOLD), None);
        assert_eq!(first.update(Some(0x1100), now + HOLD), None);
        assert_eq!(first.update(Some(0x1100), now + HOLD * 2), Some(0x1100));
    }
}
