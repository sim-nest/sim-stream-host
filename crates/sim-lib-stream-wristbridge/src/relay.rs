//! Phone relay route over a local network link.

use sim_kernel::Expr;
use sim_lib_stream_host::DeviceResult;
use sim_value::build;

use crate::{WatchCommandPacket, WatchRouteKind, WornEvent, encode_watch_command};

/// Local phone relay configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct RelayLink {
    endpoint: String,
    events: Vec<WornEvent>,
}

impl RelayLink {
    /// Builds an empty relay route.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            events: Vec::new(),
        }
    }

    /// Builds a relay route with synthetic events.
    pub fn with_scripted_events(endpoint: impl Into<String>, events: Vec<WornEvent>) -> Self {
        Self {
            endpoint: endpoint.into(),
            events,
        }
    }

    /// Local relay endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Synthetic events supplied to sessions opened on this route.
    pub fn scripted_events(&self) -> Vec<WornEvent> {
        self.events.clone()
    }

    /// Validates and serializes a command into the relay wire envelope.
    pub fn serialize_command(&self, command: &Expr) -> DeviceResult<Expr> {
        let packet = encode_watch_command(WatchRouteKind::Relay, command)?;
        Ok(build::map(vec![
            ("kind", build::qsym("stream/wristbridge", "relay-command")),
            ("endpoint", build::text(self.endpoint.clone())),
            ("packet", packet.to_expr()),
        ]))
    }

    /// Validates and returns the route-neutral packet.
    pub fn command_packet(&self, command: &Expr) -> DeviceResult<WatchCommandPacket> {
        encode_watch_command(WatchRouteKind::Relay, command)
    }
}
