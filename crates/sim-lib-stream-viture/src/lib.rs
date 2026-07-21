#![forbid(unsafe_code)]
#![deny(missing_docs)]
//! Local VITURE glasses provider for SIM XR stream samples.
//!
//! The crate adapts VITURE headset routes to the shared stream-device provider
//! surface. It emits XR pose samples as ordinary expressions, validates display
//! and IMU-control commands, and returns `Unsupported` cleanly when no SDK-backed
//! device can be opened.

pub mod device_control;
pub mod provider;

pub use device_control::{
    VitureCommandKind, VitureControlPacket, encode_viture_command, viture_command_symbol,
};
pub use provider::{
    DEFAULT_LEGACY_IMU_RATE_HZ, DEFAULT_POSE_STEP_NS, VITURE_POSE_SAMPLE_KIND, VitureProvider,
    VitureRoute, VitureRouteKind, VitureSession, viture_device_profile,
};

#[cfg(test)]
mod tests;
