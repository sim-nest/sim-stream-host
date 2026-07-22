//! Host stream placement vocabulary and LAN peer placement policy.

use std::fmt;

mod audio;
mod device;
mod lan;

use crate::site::DeviceSite;

pub use audio::{AudioDeviceCard, AudioPlacementRequest, AudioSiteKey};
pub use device::{DeviceDirection, DeviceKind, DeviceRecord, Placement};
pub use lan::{
    LanPlacementMode, LanPlacementReport, LanPlacementRequest, lan_bar_delay_mode_symbol,
    lan_experimental_remote_sample_capability, lan_jitter_buffered_mode_symbol,
    lan_peer_site_symbol, lan_pinned_sample_experimental_diagnostic,
    lan_pinned_sample_refusal_diagnostic,
};

/// Placement plan for a device surface encoder and adapter pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DevicePlacement {
    /// Site that encodes samples and commands for the selected surface codec.
    pub encoder: DeviceSite,
    /// Latency-critical adapter site placed at the device edge.
    pub adapter: DeviceSite,
}

impl DevicePlacement {
    /// Builds a device placement plan from an encoder and adapter site.
    pub fn new(encoder: DeviceSite, adapter: DeviceSite) -> Self {
        Self { encoder, adapter }
    }

    /// Validates placement invariants for live device operation.
    pub fn validate(&self) -> std::result::Result<(), PlacementError> {
        if self.adapter.is_edge_local() {
            Ok(())
        } else {
            Err(PlacementError::AdapterMustBeEdgeLocal)
        }
    }
}

/// Device placement validation error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlacementError {
    /// The latency-critical adapter is not device or edge local.
    AdapterMustBeEdgeLocal,
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdapterMustBeEdgeLocal => f.write_str("device adapter must be edge-local"),
        }
    }
}

impl std::error::Error for PlacementError {}
