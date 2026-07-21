//! Standard BLE route modeled through a local BlueZ host.

use sim_kernel::Expr;
use sim_lib_stream_host::DeviceResult;

use crate::{WatchCommandPacket, WatchRouteKind, WornEvent, encode_watch_command};

/// Local BlueZ route configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct BlueZLink {
    adapter: String,
    device: String,
    events: Vec<WornEvent>,
}

impl BlueZLink {
    /// Builds an empty BlueZ route.
    pub fn new(adapter: impl Into<String>, device: impl Into<String>) -> Self {
        Self {
            adapter: adapter.into(),
            device: device.into(),
            events: Vec::new(),
        }
    }

    /// Builds a BlueZ route with synthetic events.
    pub fn with_scripted_events(
        adapter: impl Into<String>,
        device: impl Into<String>,
        events: Vec<WornEvent>,
    ) -> Self {
        Self {
            adapter: adapter.into(),
            device: device.into(),
            events,
        }
    }

    /// Local BlueZ adapter name.
    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    /// Device address or stable host label.
    pub fn device(&self) -> &str {
        &self.device
    }

    /// Synthetic events supplied to sessions opened on this route.
    pub fn scripted_events(&self) -> Vec<WornEvent> {
        self.events.clone()
    }

    /// Validates and serializes a command for the BLE route.
    pub fn serialize_command(&self, command: &Expr) -> DeviceResult<WatchCommandPacket> {
        encode_watch_command(WatchRouteKind::Ble, command)
    }
}
