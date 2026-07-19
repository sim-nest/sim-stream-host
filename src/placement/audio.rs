//! Audio placement site keys, cards, and requests.

use sim_kernel::Symbol;

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
