//! Host backend capability metadata and browse card helpers.

use sim_kernel::{Expr, Symbol};

/// Capability advertised by a host backend.
///
/// Each variant names one host-integration feature a backend may support, such
/// as a media direction, hotplug/reconnect handling, or the deterministic fake
/// transport used during validation.
///
/// # Examples
///
/// ```
/// use sim_lib_stream_host::HostBackendCapability;
///
/// let symbol = HostBackendCapability::Duplex.symbol();
/// assert_eq!(symbol.as_qualified_str(), "stream/host-capability/duplex");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostBackendCapability {
    /// Backend can capture audio from a host input device.
    AudioInput,
    /// Backend can render audio to a host output device.
    AudioOutput,
    /// Backend can receive MIDI from a host input port.
    MidiInput,
    /// Backend can send MIDI to a host output port.
    MidiOutput,
    /// Backend can drive a single full-duplex (input and output) device.
    Duplex,
    /// Backend reports device arrival and removal at runtime.
    Hotplug,
    /// Backend can re-establish a dropped device or peer connection.
    Reconnect,
    /// Backend can enumerate and plan without opening hardware streams.
    Offline,
    /// Backend is a deterministic fake used for validation rather than hardware.
    Fake,
}

impl HostBackendCapability {
    /// Returns the stable qualified symbol naming this capability.
    pub fn symbol(self) -> Symbol {
        match self {
            Self::AudioInput => Symbol::qualified("stream/host-capability", "audio-input"),
            Self::AudioOutput => Symbol::qualified("stream/host-capability", "audio-output"),
            Self::MidiInput => Symbol::qualified("stream/host-capability", "midi-input"),
            Self::MidiOutput => Symbol::qualified("stream/host-capability", "midi-output"),
            Self::Duplex => Symbol::qualified("stream/host-capability", "duplex"),
            Self::Hotplug => Symbol::qualified("stream/host-capability", "hotplug"),
            Self::Reconnect => Symbol::qualified("stream/host-capability", "reconnect"),
            Self::Offline => Symbol::qualified("stream/host-capability", "offline"),
            Self::Fake => Symbol::qualified("stream/host-capability", "fake"),
        }
    }
}

/// Emits a browse card for a missing backend capability.
pub fn missing_capability_card_expr(backend: &Symbol, capability: HostBackendCapability) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("subject")),
            Expr::Symbol(backend.clone()),
        ),
        (
            Expr::Symbol(Symbol::new("kind")),
            Expr::Symbol(Symbol::qualified("stream", "host-missing-capability")),
        ),
        (
            Expr::Symbol(Symbol::new("capability")),
            Expr::Symbol(capability.symbol()),
        ),
    ])
}
