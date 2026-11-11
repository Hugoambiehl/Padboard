//! Volume arithmetic shared by the platform audio backends. Volumes are
//! fractions of the nominal level (`1.0` = 100 %), per channel.

use crate::{DEFAULT_VOLUME_STEP, VolumeCommand};

/// Volume of a device: the loudest channel, as most mixers show it.
#[must_use]
pub fn level(channels: &[f64]) -> f64 {
    channels.iter().copied().fold(0.0, f64::max)
}

/// Percent shown to the user (0–100, rounded).
#[must_use]
pub fn percent(channels: &[f64]) -> u32 {
    // At most 100 % (clamped), so the cast cannot truncate.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let p = (level(channels).clamp(0.0, 1.0) * 100.0).round() as u32;
    p
}

/// New channel volumes for an `up` / `down` / `set` command. The balance
/// between channels is kept (each one is scaled); the result never goes
/// above 100 %. `None` for `toggle_mute`, which does not change the volume.
#[must_use]
pub fn adjusted(channels: &[f64], command: VolumeCommand, value: Option<u32>) -> Option<Vec<f64>> {
    let current = level(channels);
    let step = f64::from(value.unwrap_or(DEFAULT_VOLUME_STEP)) / 100.0;
    let target = match command {
        VolumeCommand::Up => current + step,
        VolumeCommand::Down => current - step,
        VolumeCommand::Set => f64::from(value.unwrap_or(0)) / 100.0,
        VolumeCommand::ToggleMute => return None,
    }
    .clamp(0.0, 1.0);
    // Rounded to whole percents, so repeated steps land on 5 %, 10 %…
    let target = (target * 100.0).round() / 100.0;
    Some(if current <= f64::EPSILON {
        vec![target; channels.len().max(1)]
    } else {
        channels.iter().map(|c| c / current * target).collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
    }

    #[test]
    fn steps_are_bounded_and_rounded() {
        let up = adjusted(&[0.5, 0.5], VolumeCommand::Up, None).unwrap();
        assert!(close(&up, &[0.55, 0.55]), "{up:?}");
        let up = adjusted(&[0.98], VolumeCommand::Up, Some(10)).unwrap();
        assert!(close(&up, &[1.0]), "{up:?}");
        let down = adjusted(&[0.03], VolumeCommand::Down, None).unwrap();
        assert!(close(&down, &[0.0]), "{down:?}");
        let set = adjusted(&[0.2, 0.2], VolumeCommand::Set, Some(64)).unwrap();
        assert!(close(&set, &[0.64, 0.64]), "{set:?}");
        assert_eq!(adjusted(&[0.2], VolumeCommand::ToggleMute, None), None);
    }

    #[test]
    fn the_balance_is_kept() {
        let up = adjusted(&[0.4, 0.2], VolumeCommand::Up, Some(10)).unwrap();
        assert!(close(&up, &[0.5, 0.25]), "{up:?}");
        // From silence, every channel gets the same level.
        let up = adjusted(&[0.0, 0.0], VolumeCommand::Up, None).unwrap();
        assert!(close(&up, &[0.05, 0.05]), "{up:?}");
    }

    #[test]
    fn percent_is_the_loudest_channel() {
        assert_eq!(percent(&[0.64, 0.5]), 64);
        assert_eq!(percent(&[1.4]), 100);
    }
}
