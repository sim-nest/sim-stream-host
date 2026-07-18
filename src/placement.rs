//! Host stream placement vocabulary and LAN peer placement policy.

use sim_kernel::{CapabilityName, Error, Expr, Result, Symbol};
use sim_lib_stream_core::{
    BridgeLatency, ClockDomain, DomainBridgeDescriptor, LatencyClass, PlacedFragment,
    StreamCapability, StreamEnvelope, TransportProfile,
};

use crate::model::{
    HostOpenPlan, stream_host_capability, stream_host_device_read_effect_kind,
    stream_host_device_write_effect_kind,
};

const DEFAULT_BEATS_PER_BAR: u32 = 4;

/// Stable runtime key for an audio evaluation site.
///
/// The key is plain data so device catalogs can store and compare audio sites
/// without carrying platform handles.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AudioSiteKey(pub Symbol);

impl AudioSiteKey {
    /// Builds an audio site key from a stable symbolic name.
    pub fn new(name: &str) -> Self {
        Self(Symbol::new(name))
    }
}

/// Export-record-style descriptor for an audio device.
///
/// The card carries only stable metadata and never owns platform handles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDeviceCard {
    /// Stable site key used for registration and lookup.
    pub key: AudioSiteKey,
    /// Human-facing device name.
    pub display_name: String,
    /// Number of output channels supported by this site.
    pub channels_out: u16,
    /// Number of input channels supported by this site.
    pub channels_in: u16,
    /// Advertised sample rates in hertz.
    pub sample_rates: Vec<u32>,
    /// Whether opening this site requires real host hardware.
    pub hardware_required: bool,
}

impl AudioDeviceCard {
    /// Builds a deterministic modeled stereo device card for validation.
    pub fn modeled(key: AudioSiteKey, name: impl Into<String>) -> Self {
        Self {
            key,
            display_name: name.into(),
            channels_out: 2,
            channels_in: 2,
            sample_rates: vec![44_100, 48_000],
            hardware_required: false,
        }
    }
}

/// Caller-side request to place an audio graph on a registered audio site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioPlacementRequest {
    /// Target site key selected by placement.
    pub site_key: AudioSiteKey,
    /// Host stream request forwarded to the selected backend.
    pub stream_request: crate::HostStreamConfigRequest,
}

/// Placement tier for a host-visible stream device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    /// Deterministic in-process fixture that needs no host hardware.
    Modeled,
    /// Real host hardware behind the named transport.
    Hardware {
        /// Transport responsible for opening the hardware device.
        transport: Symbol,
    },
}

/// Stream-device media family surfaced by the shared device catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    /// Audio PCM device.
    Audio,
    /// MIDI event device.
    Midi,
}

/// Direction supported by a cataloged stream device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceDirection {
    /// Device produces stream input.
    Input,
    /// Device consumes stream output.
    Output,
    /// Device supports input and output.
    Duplex,
}

impl DeviceDirection {
    /// Returns the device effects performed when this catalog direction opens.
    pub fn effect_kinds(self) -> Vec<Symbol> {
        match self {
            Self::Input => vec![stream_host_device_read_effect_kind()],
            Self::Output => vec![stream_host_device_write_effect_kind()],
            Self::Duplex => vec![
                stream_host_device_read_effect_kind(),
                stream_host_device_write_effect_kind(),
            ],
        }
    }
}

/// Catalog row for a host-visible stream device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRecord {
    /// Stable catalog identifier.
    pub id: Symbol,
    /// Human-facing device label.
    pub display_name: String,
    /// Device media family.
    pub kind: DeviceKind,
    /// Device stream direction.
    pub direction: DeviceDirection,
    /// Placement tier for the device.
    pub placement: Placement,
}

impl DeviceRecord {
    /// Builds a modeled MIDI input record.
    pub fn modeled_midi_input(id: &str, name: impl Into<String>) -> Self {
        Self::modeled(id, name, DeviceKind::Midi, DeviceDirection::Input)
    }

    /// Builds a modeled MIDI output record.
    pub fn modeled_midi_output(id: &str, name: impl Into<String>) -> Self {
        Self::modeled(id, name, DeviceKind::Midi, DeviceDirection::Output)
    }

    /// Builds a modeled audio output record.
    pub fn modeled_audio_output(id: &str, name: impl Into<String>) -> Self {
        Self::modeled(id, name, DeviceKind::Audio, DeviceDirection::Output)
    }

    /// Builds a modeled audio record from an audio device card.
    pub fn modeled_audio_from_card(card: &AudioDeviceCard) -> Self {
        Self::audio_from_card(card, Placement::Modeled)
    }

    /// Builds an audio record from an audio device card and explicit placement.
    pub fn audio_from_card(card: &AudioDeviceCard, placement: Placement) -> Self {
        let direction = match (card.channels_in > 0, card.channels_out > 0) {
            (true, true) => DeviceDirection::Duplex,
            (true, false) => DeviceDirection::Input,
            (false, true) | (false, false) => DeviceDirection::Output,
        };
        Self {
            id: card.key.0.clone(),
            display_name: card.display_name.clone(),
            kind: DeviceKind::Audio,
            direction,
            placement,
        }
    }

    /// Builds the host-open authority plan for this cataloged device.
    pub fn open_plan(&self) -> HostOpenPlan {
        HostOpenPlan::new(
            self.plan_backend_symbol(),
            self.id.clone(),
            self.direction.effect_kinds(),
            vec![stream_host_capability()],
        )
    }

    fn modeled(
        id: &str,
        name: impl Into<String>,
        kind: DeviceKind,
        direction: DeviceDirection,
    ) -> Self {
        Self {
            id: Symbol::new(id),
            display_name: name.into(),
            kind,
            direction,
            placement: Placement::Modeled,
        }
    }

    fn plan_backend_symbol(&self) -> Symbol {
        match &self.placement {
            Placement::Modeled => Symbol::qualified("stream/host", "modeled-catalog"),
            Placement::Hardware { transport } => transport.clone(),
        }
    }
}

/// Stable site name for a non-real-time node hosted by a LAN peer.
pub fn lan_peer_site_symbol() -> Symbol {
    Symbol::qualified("stream/site", "lan-peer")
}

/// Stable mode name for jitter-buffered LAN placement.
pub fn lan_jitter_buffered_mode_symbol() -> Symbol {
    Symbol::qualified("stream/lan-mode", "jitter-buffered")
}

/// Stable mode name for musically aligned bar-delay collaboration.
pub fn lan_bar_delay_mode_symbol() -> Symbol {
    Symbol::qualified("stream/lan-mode", "collab-bardelay")
}

/// Diagnostic emitted when a pinned sample-domain node is refused across LAN.
pub fn lan_pinned_sample_refusal_diagnostic() -> Symbol {
    Symbol::qualified("stream/lan-diagnostic", "pinned-sample-remote-refused")
}

/// Diagnostic emitted when experimental pinned sample-domain LAN placement is used.
pub fn lan_pinned_sample_experimental_diagnostic() -> Symbol {
    Symbol::qualified("stream/lan-diagnostic", "pinned-sample-remote-experimental")
}

/// Capability required to try pinned sample-domain placement across LAN.
pub fn lan_experimental_remote_sample_capability() -> CapabilityName {
    CapabilityName::new("stream.lan.experimental-remote-sample")
}

/// Placement mode for a stream fragment hosted on a LAN peer.
///
/// Selects how packets crossing the LAN are buffered and time-aligned: a plain
/// jitter buffer for buffered preview, or a musically aligned bar delay for
/// collaborative play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LanPlacementMode {
    /// Jitter-buffered preview placement.
    JitterBuffered {
        /// Packets retained in the jitter buffer.
        jitter_packets: u32,
        /// Latency-compensation delay applied, in frames.
        latency_comp_frames: u64,
    },
    /// Musically aligned collaborative placement delayed by whole bars.
    BarDelay {
        /// Number of bars of alignment delay.
        bars: u32,
        /// Beats per bar used to size the bar delay.
        beats_per_bar: u32,
        /// Tempo in beats per minute used to size the bar delay.
        tempo_bpm: u32,
        /// Packets retained in the jitter buffer.
        jitter_packets: u32,
        /// Latency-compensation delay applied, in frames.
        latency_comp_frames: u64,
    },
}

impl LanPlacementMode {
    /// Builds a jitter-buffered mode retaining `jitter_packets` packets.
    ///
    /// Returns an evaluation error when `jitter_packets` is zero.
    ///
    /// # Examples
    ///
    /// ```
    /// use sim_lib_stream_host::LanPlacementMode;
    ///
    /// let mode = LanPlacementMode::jitter_buffered(4, 128).unwrap();
    /// assert_eq!(mode.jitter_packets(), 4);
    /// assert_eq!(mode.latency_comp_frames(), 128);
    /// assert!(mode.bar_delay_millis().is_none());
    /// ```
    pub fn jitter_buffered(jitter_packets: u32, latency_comp_frames: u64) -> Result<Self> {
        if jitter_packets == 0 {
            return Err(Error::Eval(
                "LAN jitter buffer must retain at least one packet".to_owned(),
            ));
        }
        Ok(Self::JitterBuffered {
            jitter_packets,
            latency_comp_frames,
        })
    }

    /// Builds a bar-delay mode delaying `bars` bars at `tempo_bpm`.
    ///
    /// Uses a default of four beats per bar. Returns an evaluation error when
    /// `bars`, `tempo_bpm`, or `jitter_packets` is zero.
    pub fn bar_delay(
        bars: u32,
        tempo_bpm: u32,
        jitter_packets: u32,
        latency_comp_frames: u64,
    ) -> Result<Self> {
        if bars == 0 {
            return Err(Error::Eval(
                "LAN bar-delay mode must delay at least one bar".to_owned(),
            ));
        }
        if tempo_bpm == 0 {
            return Err(Error::Eval(
                "LAN bar-delay mode tempo must be greater than zero".to_owned(),
            ));
        }
        if jitter_packets == 0 {
            return Err(Error::Eval(
                "LAN jitter buffer must retain at least one packet".to_owned(),
            ));
        }
        Ok(Self::BarDelay {
            bars,
            beats_per_bar: DEFAULT_BEATS_PER_BAR,
            tempo_bpm,
            jitter_packets,
            latency_comp_frames,
        })
    }

    /// Returns the stable mode symbol.
    pub fn symbol(self) -> Symbol {
        match self {
            Self::JitterBuffered { .. } => lan_jitter_buffered_mode_symbol(),
            Self::BarDelay { .. } => lan_bar_delay_mode_symbol(),
        }
    }

    /// Returns the latency class this mode places the fragment into.
    pub fn latency_class(self) -> LatencyClass {
        match self {
            Self::JitterBuffered { .. } => LatencyClass::BufferedPreview,
            Self::BarDelay { .. } => LatencyClass::CollabBarDelay,
        }
    }

    /// Returns the number of packets retained in the jitter buffer.
    pub fn jitter_packets(self) -> u32 {
        match self {
            Self::JitterBuffered { jitter_packets, .. } | Self::BarDelay { jitter_packets, .. } => {
                jitter_packets
            }
        }
    }

    /// Returns the latency-compensation delay in frames.
    pub fn latency_comp_frames(self) -> u64 {
        match self {
            Self::JitterBuffered {
                latency_comp_frames,
                ..
            }
            | Self::BarDelay {
                latency_comp_frames,
                ..
            } => latency_comp_frames,
        }
    }

    /// Returns the bar-delay length in milliseconds, or `None` for
    /// jitter-buffered placement.
    pub fn bar_delay_millis(self) -> Option<u64> {
        match self {
            Self::JitterBuffered { .. } => None,
            Self::BarDelay {
                bars,
                beats_per_bar,
                tempo_bpm,
                ..
            } => Some(
                u64::from(bars)
                    .saturating_mul(u64::from(beats_per_bar))
                    .saturating_mul(60_000)
                    / u64::from(tempo_bpm),
            ),
        }
    }

    /// Returns the transport profile advertised for this mode.
    pub fn transport_profile(self) -> Result<TransportProfile> {
        match self {
            Self::JitterBuffered { .. } => Ok(TransportProfile::lan_buffered_audio_preview()),
            Self::BarDelay { .. } => TransportProfile::new(
                Symbol::qualified("stream/profile", "lan-collab-bardelay"),
                LatencyClass::CollabBarDelay,
                vec![
                    StreamCapability::Remote,
                    StreamCapability::Bounded,
                    StreamCapability::Preview,
                    StreamCapability::Lossy,
                ],
            ),
        }
    }

    fn bridges(self) -> Vec<DomainBridgeDescriptor> {
        vec![
            DomainBridgeDescriptor::jitter_buffer(self.jitter_packets()),
            DomainBridgeDescriptor::latency_comp_delay(self.latency_comp_frames()),
        ]
    }
}

/// Request to place a stream fragment on a LAN peer under a chosen mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanPlacementRequest {
    fragment: PlacedFragment,
    mode: LanPlacementMode,
    realtime_pinned: bool,
    capabilities: Vec<CapabilityName>,
}

impl LanPlacementRequest {
    /// Builds a request to place `fragment` using `mode`, unpinned and with no
    /// extra capabilities.
    pub fn new(fragment: PlacedFragment, mode: LanPlacementMode) -> Self {
        Self {
            fragment,
            mode,
            realtime_pinned: false,
            capabilities: Vec::new(),
        }
    }

    /// Marks whether the fragment is pinned to realtime (sample-locked) play.
    pub fn with_realtime_pin(mut self, realtime_pinned: bool) -> Self {
        self.realtime_pinned = realtime_pinned;
        self
    }

    /// Grants an additional capability to the request.
    pub fn with_capability(mut self, capability: CapabilityName) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Plans the placement, returning a report or an evaluation error.
    ///
    /// Refuses a realtime-pinned sample-domain fragment across the LAN unless
    /// the experimental remote-sample capability is granted, in which case it
    /// proceeds and records an experimental diagnostic.
    pub fn plan(&self) -> Result<LanPlacementReport> {
        let experimental = self
            .capabilities
            .contains(&lan_experimental_remote_sample_capability());
        if self.realtime_pinned && self.fragment_has_sample_domain() && !experimental {
            let diagnostic = lan_pinned_sample_refusal_diagnostic();
            return Err(Error::Eval(format!(
                "{}: pinned sample-domain nodes cannot be sample-locked across LAN",
                diagnostic.as_qualified_str()
            )));
        }

        let mut diagnostics = Vec::new();
        if self.realtime_pinned && self.fragment_has_sample_domain() {
            diagnostics.push(lan_pinned_sample_experimental_diagnostic());
        }

        let profile = self.mode.transport_profile()?;
        let output_envelopes =
            remote_output_envelopes(&self.fragment.output_envelopes(), &profile, &diagnostics)?;
        Ok(LanPlacementReport {
            fragment_id: self.fragment.id().clone(),
            site: lan_peer_site_symbol(),
            mode: self.mode,
            bridges: self.mode.bridges(),
            output_envelopes,
            diagnostics,
        })
    }

    fn fragment_has_sample_domain(&self) -> bool {
        self.fragment
            .input_edges()
            .iter()
            .chain(self.fragment.output_edges())
            .any(|edge| edge.rate_contract().clock_domain() == ClockDomain::Sample)
    }
}

/// Outcome of planning a LAN fragment placement.
///
/// Records the placement site, mode, the domain bridges inserted, the rewritten
/// output envelopes carrying the remote transport profile, and any diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanPlacementReport {
    fragment_id: Symbol,
    site: Symbol,
    mode: LanPlacementMode,
    bridges: Vec<DomainBridgeDescriptor>,
    output_envelopes: Vec<StreamEnvelope>,
    diagnostics: Vec<Symbol>,
}

impl LanPlacementReport {
    /// Returns the placed fragment identifier.
    pub fn fragment_id(&self) -> &Symbol {
        &self.fragment_id
    }

    /// Returns the placement site symbol.
    pub fn site(&self) -> &Symbol {
        &self.site
    }

    /// Returns the placement mode.
    pub fn mode(&self) -> LanPlacementMode {
        self.mode
    }

    /// Returns the latency class of the placement.
    pub fn latency_class(&self) -> LatencyClass {
        self.mode.latency_class()
    }

    /// Returns the domain bridges inserted by the placement.
    pub fn bridges(&self) -> &[DomainBridgeDescriptor] {
        &self.bridges
    }

    /// Returns the rewritten output envelopes.
    pub fn output_envelopes(&self) -> &[StreamEnvelope] {
        &self.output_envelopes
    }

    /// Returns the diagnostics recorded during planning.
    pub fn diagnostics(&self) -> &[Symbol] {
        &self.diagnostics
    }

    /// Returns the total latency added by the inserted bridges.
    pub fn added_bridge_latency(&self) -> BridgeLatency {
        self.bridges
            .iter()
            .fold(BridgeLatency::zero(), |latency, bridge| {
                latency.plus(bridge.latency())
            })
    }

    /// Returns the bar-delay length in milliseconds when the mode uses one.
    pub fn bar_delay_millis(&self) -> Option<u64> {
        self.mode.bar_delay_millis()
    }

    /// Builds a browse/inspection expression summarizing the placement.
    pub fn to_expr(&self) -> Expr {
        let latency = self.added_bridge_latency();
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("fragment")),
                Expr::Symbol(self.fragment_id.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("site")),
                Expr::Symbol(self.site.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("mode")),
                Expr::Symbol(self.mode.symbol()),
            ),
            (
                Expr::Symbol(Symbol::new("latency-class")),
                Expr::Symbol(self.latency_class().symbol()),
            ),
            (
                Expr::Symbol(Symbol::new("bar-delay-ms")),
                Expr::String(self.bar_delay_millis().unwrap_or(0).to_string()),
            ),
            (
                Expr::Symbol(Symbol::new("bridge-latency-frames")),
                Expr::String(latency.frame_count().to_string()),
            ),
            (
                Expr::Symbol(Symbol::new("bridge-latency-packets")),
                Expr::String(latency.packet_count().to_string()),
            ),
            (
                Expr::Symbol(Symbol::new("bridges")),
                Expr::List(
                    self.bridges
                        .iter()
                        .map(|bridge| Expr::Symbol(bridge.kind().symbol()))
                        .collect(),
                ),
            ),
            (
                Expr::Symbol(Symbol::new("diagnostics")),
                Expr::List(self.diagnostics.iter().cloned().map(Expr::Symbol).collect()),
            ),
            (
                Expr::Symbol(Symbol::new("output-profiles")),
                Expr::List(
                    self.output_envelopes
                        .iter()
                        .map(|envelope| Expr::Symbol(envelope.profile().name().clone()))
                        .collect(),
                ),
            ),
        ])
    }
}

fn remote_output_envelopes(
    envelopes: &[StreamEnvelope],
    profile: &TransportProfile,
    diagnostics: &[Symbol],
) -> Result<Vec<StreamEnvelope>> {
    envelopes
        .iter()
        .map(|envelope| {
            let mut envelope_diagnostics = envelope.diagnostics().to_vec();
            envelope_diagnostics.extend(diagnostics.iter().cloned());
            StreamEnvelope::new_with_clock_domains(
                envelope.stream_id().clone(),
                envelope.packet_id().clone(),
                envelope.media(),
                envelope.direction(),
                envelope.sequence(),
                envelope.ticks().to_vec(),
                envelope.clock_domain(),
                envelope.clock_domains().to_vec(),
                profile.clone(),
                envelope_diagnostics,
                envelope.packet().clone(),
            )
        })
        .collect()
}
