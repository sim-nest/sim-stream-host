//! Local watch provider routes for SIM worn streams.
//!
//! The crate adapts watch-specific BLE, phone relay, Zepp bridge, and import
//! sources into the shared stream-host device provider surface.

#![deny(missing_docs)]

pub mod ble;
pub mod bringup;
#[cfg(test)]
mod bringup_tests;
mod command;
mod event;
pub mod import;
pub mod relay;
mod session;
pub mod stub;
#[cfg(test)]
mod tests;
pub mod zepp;

pub use bringup::{
    BRINGUP_LEDGER_FIXTURE, BRINGUP_LEDGER_FIXTURE_NAME, BringupLedger, BringupRoute,
    RouteEnableGuard, RouteProof,
};
pub use command::{WatchCommandKind, WatchCommandPacket, encode_watch_command};
pub use event::{WORN_EVENT_SAMPLE_KIND, WornEvent, worn_event_sample_kind};
pub use import::{ImportFormat, ImportSource};
pub use session::{WatchProvider, WatchRoute, WatchRouteKind, WatchSession, watch_device_profile};
pub use stub::watch_stub_provider;
