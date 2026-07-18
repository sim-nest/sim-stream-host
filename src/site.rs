//! Audio site abstraction for placement-routed host streams.

use std::sync::Arc;

use sim_kernel::Result;

use crate::placement::{AudioDeviceCard, AudioSiteKey};
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
