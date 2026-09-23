use std::time::{Duration, Instant};

// Only these two presets are exposed by the settings UI.
const SHOULDERS: u32 = 0x300;
const X: u32 = 0x4000;
const Y: u32 = 0x8000;
const HOLD: Duration = Duration::from_millis(500);

pub(crate) fn parse(value: &str) -> Result<u32, String> {
    match value {
        "" => Ok(0),
        "LB+RB+X" => Ok(SHOULDERS | X),
        "LB+RB+Y" => Ok(SHOULDERS | Y),
        _ => Err("controller_shortcut_invalid".into()),
    }
}
fn valid(mask: u32) -> bool {
    mask == SHOULDERS | X || mask == SHOULDERS | Y
}
pub(crate) fn normalize(value: &str) -> Result<String, String> {
    parse(value).map(|_| value.to_owned())
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
    fn validates_only_fixed_combinations() {
        assert_eq!(normalize("LB+RB+Y").unwrap(), "LB+RB+Y");
        for invalid in [
            "A",
            "A+B",
            "LB+LB",
            "Guide+A",
            "LB+RB",
            "LB+X",
            "LB+RB+A",
            "LB+RB+X+Y",
            "X+LB+RB",
        ] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
        assert!(validate("LB+RB+X", "LB+RB+X").is_err());
        assert!(validate("", "").is_ok());
        assert!(parse("LB+RB+Y").is_ok());
    }
    #[test]
    fn hold_release_and_reconnect_boundaries() {
        let mut chord = Chord::default();
        let now = Instant::now();
        let mask = parse("LB+RB+X").unwrap();
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
        second.update(Some(X), now);
        assert_eq!(first.update(Some(0x100), now + HOLD), None);
        assert_eq!(second.update(Some(X), now + HOLD), None);
        first.update(Some(SHOULDERS), now + HOLD);
        assert_eq!(first.update(Some(SHOULDERS | X), now + HOLD), None);
        assert_eq!(
            first.update(Some(SHOULDERS | X), now + HOLD * 2),
            Some(SHOULDERS | X)
        );
    }
}
