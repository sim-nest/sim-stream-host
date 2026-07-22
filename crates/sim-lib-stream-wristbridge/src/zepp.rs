//! Zepp Mini Program and watchface bridge route.

use sim_kernel::Expr;
use sim_lib_stream_host::DeviceResult;
use sim_value::build;

use crate::{WatchCommandPacket, WatchRouteKind, WornEvent, encode_watch_command};

/// Local Zepp companion bridge configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct ZeppBridgeLink {
    companion_id: String,
    events: Vec<WornEvent>,
}

impl ZeppBridgeLink {
    /// Builds an empty Zepp bridge route.
    pub fn new(companion_id: impl Into<String>) -> Self {
        Self {
            companion_id: companion_id.into(),
            events: Vec::new(),
        }
    }

    /// Builds a Zepp bridge route with synthetic events.
    pub fn with_scripted_events(companion_id: impl Into<String>, events: Vec<WornEvent>) -> Self {
        Self {
            companion_id: companion_id.into(),
            events,
        }
    }

    /// Local companion bridge id.
    pub fn companion_id(&self) -> &str {
        &self.companion_id
    }

    /// Synthetic events supplied to sessions opened on this route.
    pub fn scripted_events(&self) -> Vec<WornEvent> {
        self.events.clone()
    }

    /// Validates and serializes a command into the Zepp bridge envelope.
    pub fn serialize_command(&self, command: &Expr) -> DeviceResult<Expr> {
        let packet = encode_watch_command(WatchRouteKind::ZeppBridge, command)?;
        Ok(build::map(vec![
            (
                "kind",
                build::qsym("stream/wristbridge", "zepp-bridge-command"),
            ),
            ("companion", build::text(self.companion_id.clone())),
            ("packet", packet.to_expr()),
        ]))
    }

    /// Validates and returns the route-neutral packet.
    pub fn command_packet(&self, command: &Expr) -> DeviceResult<WatchCommandPacket> {
        encode_watch_command(WatchRouteKind::ZeppBridge, command)
    }
}
