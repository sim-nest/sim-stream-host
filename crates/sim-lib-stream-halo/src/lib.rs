#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Local Halo glasses provider and byte-budgeted Lua glance renderer.
//!
//! The crate adapts direct BLE, Web Bluetooth, and phone-relay routes to the
//! shared stream-device session surface. Sensor input stays in the shared XR
//! sample model, camera capture is an explicit consent-gated one-shot pull, and
//! `scene/glance` output is reduced to changed Lua cells under a per-tick byte
//! budget.

pub mod ble;
pub mod camera;
pub mod frame_budget;
pub mod lua_render;
pub mod provider;
pub mod relay;
pub mod sample;
pub mod stub;

pub use ble::{BlueZLink, WebBluetoothLink};
pub use camera::{
    HALO_CAMERA_SIZE_PX, HaloCameraPull, halo_camera_store_key, pull_halo_camera_once,
};
pub use frame_budget::{FrameDiff, LuaFrameBudget, LuaFrameScheduler, LuaFrameState, diff_glance};
pub use lua_render::{LuaCell, LuaCellPriority, LuaRegion, encode_lua_cells};
pub use provider::{
    HALO_BUTTON_SAMPLE_KIND, HALO_CAMERA_SAMPLE_KIND, HALO_MIC_SAMPLE_KIND,
    HALO_MOTION_SAMPLE_KIND, HALO_TAP_SAMPLE_KIND, HaloFramePacket, HaloProvider, HaloRoute,
    HaloRouteKind, HaloSample, HaloSession, halo_device_profile,
};
pub use relay::RelayLink;
pub use sample::{HaloButton, HaloButtonSample, halo_button_sample_kind_symbol};
pub use stub::halo_stub_provider;

#[cfg(test)]
mod tests;
