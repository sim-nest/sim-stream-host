//! Host backend capability metadata and browse card helpers.

use sim_kernel::{CapabilityName, Expr, Symbol};

/// Capability helpers for device-local sensors and actuators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceCapability {
    /// Pose or spatial tracking samples.
    Pose,
    /// Camera frames or still captures.
    Camera,
    /// Health or biometric samples.
    Health,
    /// Location samples.
    Location,
    /// Microphone input samples.
    Mic,
    /// Vendor diagnostic reports.
    VendorReport,
}

impl DeviceCapability {
    /// All baseline device capabilities.
    pub const ALL: [Self; 6] = [
        Self::Pose,
        Self::Camera,
        Self::Health,
        Self::Location,
        Self::Mic,
        Self::VendorReport,
    ];

    /// Stable capability name, suitable for `Cx::require`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pose => "device/pose",
            Self::Camera => "device/camera",
            Self::Health => "device/health",
            Self::Location => "device/location",
            Self::Mic => "device/mic",
            Self::VendorReport => "device/vendor-report",
        }
    }

    /// Returns the kernel capability name.
    pub fn capability_name(self) -> CapabilityName {
        CapabilityName::new(self.as_str())
    }

    /// Returns the visible grant symbol used by consent receipts.
    pub fn grant_symbol(self) -> Symbol {
        Symbol::qualified("device", self.local_name())
    }

    /// Resolves a baseline helper from a capability name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_str() == name)
    }

    fn local_name(self) -> &'static str {
        self.as_str()
            .strip_prefix("device/")
            .expect("device capability names keep the device/ prefix")
    }
}

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
