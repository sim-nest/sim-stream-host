//! Host stream configuration records.

use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, ClockDomain, StreamMedia, StreamMetadata};

use crate::model::HostDirection;

/// Request supplied to a host backend when opening a stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostStreamConfigRequest {
    backend: Symbol,
    device: Symbol,
    media: StreamMedia,
    direction: HostDirection,
    buffer: BufferPolicy,
    clock: Symbol,
    reconnect: HostReconnectPolicy,
}

/// Accepted stream configuration returned by a host backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostStreamConfig {
    backend: Symbol,
    device: Symbol,
    media: StreamMedia,
    direction: HostDirection,
    buffer: BufferPolicy,
    clock: HostClockInfo,
    latency: HostLatencyInfo,
    reconnect: HostReconnectPolicy,
}

/// Latency estimate for an opened host stream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostLatencyInfo {
    input_frames: u32,
    output_frames: u32,
}

/// Clock metadata for an opened host stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostClockInfo {
    clock: Symbol,
    sample_rate_hz: Option<u32>,
    stable: bool,
}

/// Reconnect behavior accepted by a host backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostReconnectPolicy {
    enabled: bool,
    max_attempts: u32,
    backoff_ms: u32,
}

impl HostStreamConfigRequest {
    /// Builds an open request for `device` on `backend` with a server-frame
    /// clock and reconnect disabled.
    pub fn new(
        backend: Symbol,
        device: Symbol,
        media: StreamMedia,
        direction: HostDirection,
        buffer: BufferPolicy,
    ) -> Self {
        Self {
            backend,
            device,
            media,
            direction,
            buffer,
            clock: ClockDomain::ServerFrame.symbol(),
            reconnect: HostReconnectPolicy::disabled(),
        }
    }

    /// Overrides the requested clock-domain symbol.
    pub fn with_clock(mut self, clock: Symbol) -> Self {
        self.clock = clock;
        self
    }

    /// Overrides the requested reconnect policy.
    pub fn with_reconnect(mut self, reconnect: HostReconnectPolicy) -> Self {
        self.reconnect = reconnect;
        self
    }

    /// Returns the target backend symbol.
    pub fn backend(&self) -> &Symbol {
        &self.backend
    }

    /// Returns the target device symbol.
    pub fn device(&self) -> &Symbol {
        &self.device
    }

    /// Returns the requested stream media.
    pub fn media(&self) -> StreamMedia {
        self.media
    }

    /// Returns the requested stream direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the requested buffer policy.
    pub fn buffer(&self) -> &BufferPolicy {
        &self.buffer
    }

    /// Returns the requested clock-domain symbol.
    pub fn clock(&self) -> &Symbol {
        &self.clock
    }

    /// Returns the requested reconnect policy.
    pub fn reconnect(&self) -> &HostReconnectPolicy {
        &self.reconnect
    }
}

impl HostStreamConfig {
    /// Builds an accepted configuration from a request plus the backend's
    /// resolved latency and clock metadata.
    pub fn from_request(
        request: HostStreamConfigRequest,
        latency: HostLatencyInfo,
        clock: HostClockInfo,
    ) -> Self {
        Self {
            backend: request.backend,
            device: request.device,
            media: request.media,
            direction: request.direction,
            buffer: request.buffer,
            clock,
            latency,
            reconnect: request.reconnect,
        }
    }

    /// Returns the backend that accepted the stream.
    pub fn backend(&self) -> &Symbol {
        &self.backend
    }

    /// Returns the opened device symbol.
    pub fn device(&self) -> &Symbol {
        &self.device
    }

    /// Returns the accepted stream media.
    pub fn media(&self) -> StreamMedia {
        self.media
    }

    /// Returns the accepted stream direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the accepted buffer policy.
    pub fn buffer(&self) -> &BufferPolicy {
        &self.buffer
    }

    /// Returns the resolved clock metadata.
    pub fn clock(&self) -> &HostClockInfo {
        &self.clock
    }

    /// Returns the resolved latency estimate.
    pub fn latency(&self) -> HostLatencyInfo {
        self.latency
    }

    /// Returns the accepted reconnect policy.
    pub fn reconnect(&self) -> &HostReconnectPolicy {
        &self.reconnect
    }

    /// Builds the [`StreamMetadata`] describing the opened stream.
    pub fn metadata(&self) -> StreamMetadata {
        StreamMetadata::new(
            self.device.clone(),
            self.media,
            self.direction.stream_direction(),
            self.clock.clock.clone(),
            self.buffer.clone(),
        )
    }

    /// Checks that this configuration is a valid realtime local audio stream.
    ///
    /// Requires PCM media, the sample clock domain, and a bounded buffer;
    /// returns an evaluation error otherwise.
    pub fn validate_realtime_local_audio(&self) -> Result<()> {
        if self.media != StreamMedia::Pcm {
            return Err(Error::Eval(
                "realtime local audio host streams require PCM media".to_owned(),
            ));
        }
        if ClockDomain::from_symbol(self.clock.clock())? != ClockDomain::Sample {
            return Err(Error::Eval(
                "realtime local audio host streams require the sample clock domain".to_owned(),
            ));
        }
        if self.buffer.capacity() == 0 {
            return Err(Error::Eval(
                "realtime local audio host streams require a bounded buffer".to_owned(),
            ));
        }
        Ok(())
    }

    /// Checks that this configuration is a valid LAN MIDI/control stream.
    ///
    /// Requires MIDI media, a MIDI-tick or control clock domain, and a bounded
    /// buffer; returns an evaluation error otherwise.
    pub fn validate_lan_midi_control(&self) -> Result<()> {
        if self.media != StreamMedia::Midi {
            return Err(Error::Eval(
                "LAN MIDI/control host streams require MIDI media".to_owned(),
            ));
        }
        match ClockDomain::from_symbol(self.clock.clock())? {
            ClockDomain::MidiTick | ClockDomain::Control => {}
            _ => {
                return Err(Error::Eval(
                    "LAN MIDI/control host streams require a MIDI tick or control clock domain"
                        .to_owned(),
                ));
            }
        }
        if self.buffer.capacity() == 0 {
            return Err(Error::Eval(
                "LAN MIDI/control host streams require a bounded buffer".to_owned(),
            ));
        }
        Ok(())
    }

    /// Checks that this configuration is a valid LAN buffered audio preview
    /// stream.
    ///
    /// Requires PCM media and a bounded buffer; returns an evaluation error
    /// otherwise.
    pub fn validate_lan_buffered_audio_preview(&self) -> Result<()> {
        if self.media != StreamMedia::Pcm {
            return Err(Error::Eval(
                "LAN buffered audio preview host streams require PCM media".to_owned(),
            ));
        }
        if self.buffer.capacity() == 0 {
            return Err(Error::Eval(
                "LAN buffered audio preview host streams require a bounded buffer".to_owned(),
            ));
        }
        Ok(())
    }
}

impl HostLatencyInfo {
    /// Builds a latency estimate from input and output frame counts.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_host::HostLatencyInfo;
    ///
    /// let latency = HostLatencyInfo::new(64, 128);
    /// assert_eq!(latency.input_frames(), 64);
    /// assert_eq!(latency.output_frames(), 128);
    /// ```
    pub fn new(input_frames: u32, output_frames: u32) -> Self {
        Self {
            input_frames,
            output_frames,
        }
    }

    /// Returns the estimated input latency in frames.
    pub fn input_frames(self) -> u32 {
        self.input_frames
    }

    /// Returns the estimated output latency in frames.
    pub fn output_frames(self) -> u32 {
        self.output_frames
    }
}

impl HostClockInfo {
    /// Builds clock metadata from a clock-domain symbol, optional sample rate,
    /// and a stability flag.
    pub fn new(clock: Symbol, sample_rate_hz: Option<u32>, stable: bool) -> Self {
        Self {
            clock,
            sample_rate_hz,
            stable,
        }
    }

    /// Returns the clock-domain symbol.
    pub fn clock(&self) -> &Symbol {
        &self.clock
    }

    /// Returns the sample rate in hertz when known.
    pub fn sample_rate_hz(&self) -> Option<u32> {
        self.sample_rate_hz
    }

    /// Returns whether the clock is reported as stable.
    pub fn stable(&self) -> bool {
        self.stable
    }
}

impl HostReconnectPolicy {
    /// Returns a policy that never reconnects.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_attempts: 0,
            backoff_ms: 0,
        }
    }

    /// Returns a policy that reconnects up to `max_attempts` times with a fixed
    /// `backoff_ms` delay between attempts.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_host::HostReconnectPolicy;
    ///
    /// let policy = HostReconnectPolicy::bounded(3, 250);
    /// assert!(policy.enabled());
    /// assert_eq!(policy.max_attempts(), 3);
    /// assert_eq!(policy.backoff_ms(), 250);
    /// ```
    pub fn bounded(max_attempts: u32, backoff_ms: u32) -> Self {
        Self {
            enabled: true,
            max_attempts,
            backoff_ms,
        }
    }

    /// Returns whether reconnection is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the maximum number of reconnect attempts.
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Returns the backoff delay between attempts in milliseconds.
    pub fn backoff_ms(&self) -> u32 {
        self.backoff_ms
    }
}
