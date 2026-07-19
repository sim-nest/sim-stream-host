//! Host stream placement vocabulary and LAN peer placement policy.

mod audio;
mod device;
mod lan;

pub use audio::{AudioDeviceCard, AudioPlacementRequest, AudioSiteKey};
pub use device::{DeviceDirection, DeviceKind, DeviceRecord, Placement};
pub use lan::{
    LanPlacementMode, LanPlacementReport, LanPlacementRequest, lan_bar_delay_mode_symbol,
    lan_experimental_remote_sample_capability, lan_jitter_buffered_mode_symbol,
    lan_peer_site_symbol, lan_pinned_sample_experimental_diagnostic,
    lan_pinned_sample_refusal_diagnostic,
};
