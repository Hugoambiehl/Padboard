//! Output volume and microphone through the PulseAudio native protocol.
//!
//! PipeWire (Ubuntu's default) serves the same protocol through
//! `pipewire-pulse`, so this works on both. The `pulseaudio` crate speaks it
//! in pure Rust: nothing to install. Calls are blocking, one short-lived
//! connection each (a local socket: a few milliseconds), so a restarted
//! sound server never leaves a stale connection behind.

use std::ffi::{CStr, CString};
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use pulseaudio::protocol::{
    self, AuthParams, AuthReply, ChannelVolume, Command, GetSinkInfo, GetSourceInfo, Prop, Props,
    ServerInfo, SetClientNameReply, SetDeviceMuteParams, SetDeviceVolumeParams, SinkInfo,
    SourceInfo, SubscriptionMask, Volume,
};

use crate::ActionError;

const CLIENT_NAME: &CStr = c"Padboard";
const TIMEOUT: Duration = Duration::from_secs(2);
/// `PA_VOLUME_NORM`: 100 %.
const NORM: f64 = 65536.0;

pub use crate::state::AudioState;

pub struct Pulse {
    stream: BufReader<UnixStream>,
    version: u16,
    seq: u32,
}

fn unavailable(e: impl std::fmt::Display) -> ActionError {
    ActionError::Unavailable(format!(
        "sound server (PulseAudio / PipeWire) unreachable: {e}"
    ))
}

fn failed(e: impl std::fmt::Display) -> ActionError {
    ActionError::Failed(format!("sound server: {e}"))
}

impl Pulse {
    /// Connects and authenticates to the user's sound server.
    ///
    /// # Errors
    /// [`ActionError::Unavailable`] if no server answers.
    pub fn connect() -> Result<Self, ActionError> {
        let path = pulseaudio::socket_path_from_env().ok_or_else(|| unavailable("no socket"))?;
        let socket = UnixStream::connect(&path).map_err(unavailable)?;
        socket
            .set_read_timeout(Some(TIMEOUT))
            .map_err(unavailable)?;
        socket
            .set_write_timeout(Some(TIMEOUT))
            .map_err(unavailable)?;
        let mut stream = BufReader::new(socket);

        let cookie = pulseaudio::cookie_path_from_env()
            .and_then(|path| std::fs::read(path).ok())
            .unwrap_or_default();
        let auth = Command::Auth(AuthParams {
            version: protocol::MAX_VERSION,
            supports_shm: false,
            supports_memfd: false,
            cookie,
        });
        protocol::write_command_message(stream.get_mut(), 0, &auth, protocol::MAX_VERSION)
            .map_err(unavailable)?;
        let (_, reply) =
            protocol::read_reply_message::<AuthReply>(&mut stream, protocol::MAX_VERSION)
                .map_err(unavailable)?;
        let version = protocol::MAX_VERSION.min(reply.version);

        let mut pulse = Self {
            stream,
            version,
            seq: 1,
        };
        let mut props = Props::new();
        props.set(Prop::ApplicationName, CLIENT_NAME);
        pulse.request::<SetClientNameReply>(&Command::SetClientName(props))?;
        Ok(pulse)
    }

    fn send(&mut self, command: &Command) -> Result<(), ActionError> {
        self.seq = self.seq.wrapping_add(1);
        protocol::write_command_message(self.stream.get_mut(), self.seq, command, self.version)
            .map_err(failed)
    }

    fn request<R: protocol::CommandReply>(&mut self, command: &Command) -> Result<R, ActionError> {
        self.send(command)?;
        let (_, reply) =
            protocol::read_reply_message::<R>(&mut self.stream, self.version).map_err(failed)?;
        Ok(reply)
    }

    fn ack(&mut self, command: &Command) -> Result<(), ActionError> {
        self.send(command)?;
        protocol::read_ack_message(&mut self.stream).map_err(failed)?;
        Ok(())
    }

    fn server(&mut self) -> Result<ServerInfo, ActionError> {
        self.request(&Command::GetServerInfo)
    }

    /// The default output device.
    ///
    /// # Errors
    /// [`ActionError::Unavailable`] if there is none.
    pub fn default_sink(&mut self) -> Result<SinkInfo, ActionError> {
        let name = self
            .server()?
            .default_sink_name
            .ok_or_else(|| ActionError::Unavailable("no output device".into()))?;
        self.request(&Command::GetSinkInfo(GetSinkInfo {
            index: None,
            name: Some(name),
        }))
    }

    /// The default input device (microphone).
    ///
    /// # Errors
    /// [`ActionError::Unavailable`] if there is none.
    pub fn default_source(&mut self) -> Result<SourceInfo, ActionError> {
        let name = self
            .server()?
            .default_source_name
            .ok_or_else(|| ActionError::Unavailable("no microphone".into()))?;
        let source: SourceInfo = self.request(&Command::GetSourceInfo(GetSourceInfo {
            index: None,
            name: Some(name),
        }))?;
        // With no microphone, the default "source" can be a speaker monitor.
        if source.monitor_of_sink_index.is_some() {
            return Err(ActionError::Unavailable("no microphone".into()));
        }
        Ok(source)
    }

    pub fn set_sink_mute(&mut self, name: CString, mute: bool) -> Result<(), ActionError> {
        self.ack(&Command::SetSinkMute(SetDeviceMuteParams {
            device_index: None,
            device_name: Some(name),
            mute,
        }))
    }

    pub fn set_source_mute(&mut self, name: CString, mute: bool) -> Result<(), ActionError> {
        self.ack(&Command::SetSourceMute(SetDeviceMuteParams {
            device_index: None,
            device_name: Some(name),
            mute,
        }))
    }

    pub fn set_sink_volume(&mut self, name: CString, channels: &[f64]) -> Result<(), ActionError> {
        let mut volume = ChannelVolume::empty();
        for c in channels {
            // Clamped to 0–100 % by `audio::adjusted`.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            volume.push(Volume::from_u32_clamped((c * NORM).round() as u32));
        }
        self.ack(&Command::SetSinkVolume(SetDeviceVolumeParams {
            device_index: None,
            device_name: Some(name),
            volume,
        }))
    }

    /// Output volume and mute, microphone mute.
    ///
    /// # Errors
    /// [`ActionError::Unavailable`] without an output device.
    pub fn state(&mut self) -> Result<AudioState, ActionError> {
        let sink = self.default_sink()?;
        let input_muted = match self.default_source() {
            Ok(source) => Some(source.muted),
            Err(ActionError::Unavailable(_)) => None,
            Err(e) => return Err(e),
        };
        Ok(AudioState {
            output_percent: crate::audio::percent(&channels(&sink.cvolume)),
            output_muted: sink.muted,
            input_muted,
        })
    }

    /// Turns this connection into a change feed: [`Self::next_change`] then
    /// returns on each change of a device or of the defaults.
    ///
    /// # Errors
    /// If the subscription is refused.
    pub fn subscribe(mut self) -> Result<Subscription, ActionError> {
        self.ack(&Command::Subscribe(
            SubscriptionMask::SINK | SubscriptionMask::SOURCE | SubscriptionMask::SERVER,
        ))?;
        // Changes come when they come: no read timeout any more.
        self.stream
            .get_ref()
            .set_read_timeout(None)
            .map_err(failed)?;
        Ok(Subscription(self))
    }
}

/// The exact raw state of the default devices, to put back after a test.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RawSnapshot {
    pub sink: String,
    pub sink_volumes: Vec<u32>,
    pub sink_muted: bool,
    pub source: Option<(String, Vec<u32>, bool)>,
}

impl Pulse {
    pub fn snapshot(&mut self) -> Result<RawSnapshot, ActionError> {
        let sink = self.default_sink()?;
        let source = match self.default_source() {
            Ok(s) => Some((
                s.name.to_string_lossy().into_owned(),
                s.cvolume.channels().iter().map(Volume::as_u32).collect(),
                s.muted,
            )),
            Err(ActionError::Unavailable(_)) => None,
            Err(e) => return Err(e),
        };
        Ok(RawSnapshot {
            sink: sink.name.to_string_lossy().into_owned(),
            sink_volumes: sink.cvolume.channels().iter().map(Volume::as_u32).collect(),
            sink_muted: sink.muted,
            source,
        })
    }

    pub fn restore(&mut self, snapshot: &RawSnapshot) -> Result<(), ActionError> {
        let name = |n: &str| CString::new(n).map_err(failed);
        let raw = |values: &[u32]| {
            let mut volume = ChannelVolume::empty();
            for v in values {
                volume.push(Volume::from_u32_clamped(*v));
            }
            volume
        };
        self.ack(&Command::SetSinkVolume(SetDeviceVolumeParams {
            device_index: None,
            device_name: Some(name(&snapshot.sink)?),
            volume: raw(&snapshot.sink_volumes),
        }))?;
        self.set_sink_mute(name(&snapshot.sink)?, snapshot.sink_muted)?;
        if let Some((source, volumes, muted)) = &snapshot.source {
            self.ack(&Command::SetSourceVolume(SetDeviceVolumeParams {
                device_index: None,
                device_name: Some(name(source)?),
                volume: raw(volumes),
            }))?;
            self.set_source_mute(name(source)?, *muted)?;
        }
        Ok(())
    }
}

/// Per-channel volumes as fractions of 100 %.
#[must_use]
pub fn channels(volume: &ChannelVolume) -> Vec<f64> {
    volume
        .channels()
        .iter()
        .map(|v| f64::from(v.as_u32()) / NORM)
        .collect()
}

/// A connection subscribed to device changes.
pub struct Subscription(Pulse);

impl Subscription {
    /// Blocks until something changes (volume, mute, default device…).
    ///
    /// # Errors
    /// When the sound server goes away: subscribe again later.
    pub fn next_change(&mut self) -> Result<(), ActionError> {
        loop {
            let (_, command) = protocol::read_command_message(&mut self.0.stream, self.0.version)
                .map_err(failed)?;
            if matches!(command, Command::SubscribeEvent(_)) {
                return Ok(());
            }
        }
    }
}
