//! Media players through MPRIS (D-Bus session bus): Spotify, browsers, VLC…
//!
//! Commands go to the player that is playing, else to a paused one, else to
//! the first one. Without any player, the caller falls back to the media keys.

use zbus::blocking::fdo::DBusProxy;
use zbus::blocking::{Connection, Proxy};

use crate::{ActionError, MediaCommand};

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PATH: &str = "/org/mpris/MediaPlayer2";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    Stopped,
}

impl Status {
    fn parse(s: &str) -> Self {
        match s {
            "Playing" => Self::Playing,
            "Paused" => Self::Paused,
            _ => Self::Stopped,
        }
    }
}

/// The player commands go to.
#[must_use]
pub fn choose(players: &[(String, Status)]) -> Option<&str> {
    [Status::Playing, Status::Paused, Status::Stopped]
        .iter()
        .find_map(|wanted| players.iter().find(|(_, s)| s == wanted))
        .map(|(name, _)| name.as_str())
}

fn failed(e: zbus::Error) -> ActionError {
    ActionError::Failed(format!("media player (MPRIS): {e}"))
}

/// Players on the session bus and their status.
fn players(conn: &Connection) -> Result<Vec<(String, Status)>, ActionError> {
    let names = DBusProxy::new(conn)
        .and_then(|bus| bus.list_names().map_err(Into::into))
        .map_err(failed)?;
    Ok(names
        .into_iter()
        .map(|n| n.to_string())
        .filter(|n| n.starts_with(PREFIX))
        .map(|name| {
            let status = player(conn, &name)
                .and_then(|p| p.get_property::<String>("PlaybackStatus"))
                .map_or(Status::Stopped, |s| Status::parse(&s));
            (name, status)
        })
        .collect())
}

fn player<'a>(conn: &Connection, name: &'a str) -> zbus::Result<Proxy<'a>> {
    Proxy::new(conn, name, PATH, PLAYER)
}

/// Sends `command` to the chosen player. `Ok(false)` when there is no player
/// (the caller then presses the media key).
///
/// # Errors
/// If the session bus is unreachable or the player refuses the call.
pub fn command(command: MediaCommand) -> Result<bool, ActionError> {
    let conn = Connection::session().map_err(failed)?;
    let players = players(&conn)?;
    let Some(name) = choose(&players) else {
        return Ok(false);
    };
    let method = match command {
        MediaCommand::PlayPause => "PlayPause",
        MediaCommand::Next => "Next",
        MediaCommand::Previous => "Previous",
        MediaCommand::Stop => "Stop",
    };
    tracing::debug!(player = name, method, "MPRIS command");
    player(&conn, name)
        .and_then(|p| p.call_method(method, &()).map(drop))
        .map_err(failed)?;
    Ok(true)
}

/// Whether some player is playing (`None`: no player at all).
///
/// # Errors
/// If the session bus is unreachable.
pub fn playing() -> Result<Option<bool>, ActionError> {
    let conn = Connection::session().map_err(failed)?;
    let players = players(&conn)?;
    Ok((!players.is_empty()).then(|| players.iter().any(|(_, s)| *s == Status::Playing)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_playing_player_wins_then_a_paused_one() {
        let p = |list: &[(&str, Status)]| -> Vec<(String, Status)> {
            list.iter().map(|(n, s)| ((*n).to_owned(), *s)).collect()
        };
        let list = p(&[
            ("a", Status::Stopped),
            ("b", Status::Paused),
            ("c", Status::Playing),
        ]);
        assert_eq!(choose(&list), Some("c"));
        let list = p(&[("a", Status::Stopped), ("b", Status::Paused)]);
        assert_eq!(choose(&list), Some("b"));
        let list = p(&[("a", Status::Stopped)]);
        assert_eq!(choose(&list), Some("a"));
        assert_eq!(choose(&[]), None);
    }
}
