#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Host-device stream backend substrate.
//!
//! The crate currently implements the selected RTP-MIDI subset without opening
//! sockets or platform devices during normal validation. Host callbacks only
//! receive a cloneable bounded queue handle and may enqueue packets through
//! non-blocking calls. Device smoke tests are ignored by default; run them
//! manually only when a matching external peer or platform device is available.

mod audio_provider;
mod backend;
mod capability;
mod cassette;
mod catalog;
mod config;
mod config_probe;
pub mod cookbook;
mod device;
mod eval_site;
mod fake;
mod inventory;
mod midi_live_eval_site;
mod midi_provider;
mod model;
mod placement;
mod provider;
mod queue;
mod registry;
mod ring;
mod router;
#[cfg(feature = "rtp-midi")]
mod rtp_midi;
mod site;

pub use audio_provider::ModeledAudioProvider;
pub use backend::{HostBackend, HostOpenStream, HostStreamDriver};
pub use capability::{HostBackendCapability, missing_capability_card_expr};
pub use cassette::{HostCallbackCassette, HostCallbackReplayReport};
pub use catalog::{DeviceCatalog, audio_device_export_symbol, audio_site_export_symbol};
pub use config::{
    HostClockInfo, HostLatencyInfo, HostReconnectPolicy, HostStreamConfig, HostStreamConfigRequest,
};
pub use config_probe::{
    HostStreamConfigProbe, hardware_inventory_probe_capability_symbol,
    host_stream_config_probe_symbol, stream_host_config_lib_symbol,
};
pub use cookbook::fake_backend_demo;
pub use device::{
    DeviceError, DeviceProfile, DeviceProvider, DeviceResult, DeviceSample, DeviceSession,
    StubProvider, StubSession, device_sample_kind_symbol, poll_device_sample,
};
pub use eval_site::{DeviceProvider as CatalogDeviceProvider, StreamEvalSite};
pub use fake::{FakeBackend, fake_backend_symbol};
pub use inventory::{HostDeviceInventory, HostPortSpec};
pub use midi_live_eval_site::MidiLiveEvalSite;
pub use midi_provider::ModeledMidiProvider;
pub use model::{
    HostBackendInfo, HostDeviceSpec, HostDirection, HostOpenPlan, stream_host_capability,
    stream_host_device_read_effect_kind, stream_host_device_write_effect_kind,
};
pub use placement::{
    AudioDeviceCard, AudioPlacementRequest, AudioSiteKey, DeviceDirection, DeviceKind,
    DevicePlacement, DeviceRecord, LanPlacementMode, LanPlacementReport, LanPlacementRequest,
    Placement, PlacementError, lan_bar_delay_mode_symbol,
    lan_experimental_remote_sample_capability, lan_jitter_buffered_mode_symbol,
    lan_peer_site_symbol, lan_pinned_sample_experimental_diagnostic,
    lan_pinned_sample_refusal_diagnostic,
};
pub use provider::{
    AUDIO_PROVIDER_ABI_VERSION, AUDIO_PROVIDER_ENTRY_V1, AudioProviderEntry, AudioProviderHost,
    AudioProviderProofEntries, AudioProviderRegistrar, RouterAudioProviderRegistrar,
    native_audio_provider_capability,
};
pub use queue::HostCallbackQueue;
pub use registry::HostBackendRegistry;
pub use ring::{ProcessRingPush, ProcessRingSnapshot, ProcessSharedRing};
pub use router::{AudioRouter, RegisteredAudioSite};
#[cfg(feature = "rtp-midi")]
pub use rtp_midi::{RtpMidiBackend, RtpMidiPort, rtp_midi_backend_symbol};
pub use site::{AudioSite, DeviceSite, DeviceSiteLocality, ModeledAudioSite};

#[cfg(test)]
mod catalog_tests;
#[cfg(test)]
mod config_probe_tests;
#[cfg(test)]
mod device_tests;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod provider_tests;
#[cfg(test)]
mod ring_tests;
#[cfg(test)]
mod tests;
