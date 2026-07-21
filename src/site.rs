//! Audio site abstraction for placement-routed host streams.

use std::sync::Arc;

use sim_kernel::{Result, Symbol};

use crate::placement::{AudioDeviceCard, AudioSiteKey};
use crate::{DeviceProfile, DeviceResult};
use crate::{HostBackend, HostOpenStream, HostStreamConfigRequest};

/// Runtime-openable audio placement target.
///
/// An audio site owns stable descriptive metadata and delegates stream opening
/// to the backend implementation selected for that site.
pub trait AudioSite: Send + Sync {
    /// Returns the stable key used to register and route this site.
    fn key(&self) -> &AudioSiteKey;

    /// Returns the browseable device card for this site.
    fn card(&self) -> &AudioDeviceCard;

    /// Opens a host stream for this site using the supplied stream request.
    ///
    /// This is site-level dispatch for an already checked placement. Public
    /// placement opens should use
    /// [`AudioRouter::open_placement_checked`](crate::AudioRouter::open_placement_checked).
    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream>;
}

/// Modeled audio site backed by an existing host backend.
pub struct ModeledAudioSite {
    card: AudioDeviceCard,
    backend: Arc<dyn HostBackend>,
}

impl ModeledAudioSite {
    /// Builds a modeled site from a device card and host backend.
    pub fn new(card: AudioDeviceCard, backend: Arc<dyn HostBackend>) -> Self {
        Self { card, backend }
    }
}

impl AudioSite for ModeledAudioSite {
    fn key(&self) -> &AudioSiteKey {
        &self.card.key
    }

    fn card(&self) -> &AudioDeviceCard {
        &self.card
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        self.backend.open(request)
    }
}

/// Placement locality advertised by a device site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceSiteLocality {
    /// Device adapter runs at the device or edge boundary.
    EdgeLocal,
    /// Site runs on the host but not at the device edge.
    HostLocal,
    /// Site crosses a remote transport boundary.
    Remote,
}

/// Export-record-style descriptor for a stream device site.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceSite {
    /// Stable site symbol exported by the provider.
    pub symbol: Symbol,
    /// Device profile carried by this site export.
    pub profile: DeviceProfile,
    /// Surface codec used to encode device samples and commands.
    pub surface_codec_id: Symbol,
    /// Locality used by placement validation.
    pub locality: DeviceSiteLocality,
}

impl DeviceSite {
    /// Builds a device site descriptor.
    pub fn new(
        symbol: Symbol,
        profile: DeviceProfile,
        surface_codec_id: Symbol,
        locality: DeviceSiteLocality,
    ) -> Self {
        Self {
            symbol,
            profile,
            surface_codec_id,
            locality,
        }
    }

    /// Builds a device or edge-local site descriptor.
    pub fn edge_local(symbol: Symbol, profile: DeviceProfile, surface_codec_id: Symbol) -> Self {
        Self::new(
            symbol,
            profile,
            surface_codec_id,
            DeviceSiteLocality::EdgeLocal,
        )
    }

    /// Builds a host-local site descriptor.
    pub fn host_local(symbol: Symbol, profile: DeviceProfile, surface_codec_id: Symbol) -> Self {
        Self::new(
            symbol,
            profile,
            surface_codec_id,
            DeviceSiteLocality::HostLocal,
        )
    }

    /// Builds a remote site descriptor.
    pub fn remote(symbol: Symbol, profile: DeviceProfile, surface_codec_id: Symbol) -> Self {
        Self::new(
            symbol,
            profile,
            surface_codec_id,
            DeviceSiteLocality::Remote,
        )
    }

    /// Returns whether this site is local enough for a latency-critical adapter.
    pub fn is_edge_local(&self) -> bool {
        self.locality == DeviceSiteLocality::EdgeLocal
    }

    /// Checks that this site is local enough for a latency-critical adapter.
    pub fn require_edge_local(&self) -> DeviceResult<()> {
        if self.is_edge_local() {
            Ok(())
        } else {
            Err(crate::DeviceError::Host(format!(
                "device adapter site {} must be edge-local",
                self.symbol
            )))
        }
    }
}
