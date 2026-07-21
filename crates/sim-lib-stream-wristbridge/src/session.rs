//! Shared watch provider and session wiring.

use std::collections::VecDeque;

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_host::{
    DeviceError, DeviceProfile, DeviceProvider, DeviceResult, DeviceSample, DeviceSession,
    device_sample_kind_symbol,
};

use crate::ble::BlueZLink;
use crate::bringup::{BringupLedger, RouteEnableGuard};
use crate::command::{WatchCommandPacket, encode_watch_command};
use crate::import::ImportSource;
use crate::relay::RelayLink;
use crate::zepp::ZeppBridgeLink;
use crate::{WORN_EVENT_SAMPLE_KIND, WornEvent};

/// Local route used by a watch provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchRouteKind {
    /// Standard BLE route through a local BlueZ host.
    Ble,
    /// Phone relay route over a local network link.
    Relay,
    /// Zepp companion bridge route.
    ZeppBridge,
    /// File import route for exported watch data.
    Import,
}

impl WatchRouteKind {
    /// Stable route token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Ble => "ble",
            Self::Relay => "relay",
            Self::ZeppBridge => "zepp-bridge",
            Self::Import => "import",
        }
    }
}

/// Configured route for opening a watch provider session.
#[derive(Clone, Debug, PartialEq)]
pub enum WatchRoute {
    /// Standard BLE route through a local BlueZ host.
    Ble(BlueZLink),
    /// Phone relay route carrying worn events and command packets.
    Relay(RelayLink),
    /// Zepp companion bridge route for Mini Program or watchface data.
    ZeppBridge(ZeppBridgeLink),
    /// File import route for watch exports.
    Import(ImportSource),
    /// Hardware-free unsupported route.
    Stub,
}

/// Watch provider backed by one local route.
#[derive(Clone, Debug, PartialEq)]
pub struct WatchProvider {
    route: WatchRoute,
    profile: DeviceProfile,
    bringup: BringupLedger,
}

impl WatchProvider {
    /// Builds a watch provider for `route`.
    pub fn new(route: WatchRoute) -> Self {
        Self {
            route,
            profile: watch_device_profile(),
            bringup: BringupLedger::default(),
        }
    }

    /// Builds a provider with an explicit hardware bring-up ledger.
    pub fn with_bringup_ledger(mut self, bringup: BringupLedger) -> Self {
        self.bringup = bringup;
        self
    }

    /// Builds a provider for a standard BLE route.
    pub fn ble(link: BlueZLink) -> Self {
        Self::new(WatchRoute::Ble(link))
    }

    /// Builds a provider for a phone relay route.
    pub fn relay(link: RelayLink) -> Self {
        Self::new(WatchRoute::Relay(link))
    }

    /// Builds a provider for a Zepp bridge route.
    pub fn zepp_bridge(link: ZeppBridgeLink) -> Self {
        Self::new(WatchRoute::ZeppBridge(link))
    }

    /// Builds a provider for an import route.
    pub fn import(source: ImportSource) -> Self {
        Self::new(WatchRoute::Import(source))
    }

    /// Builds an unsupported provider for hardware-free defaults.
    pub fn stub() -> Self {
        Self::new(WatchRoute::Stub)
    }

    /// Returns the advertised watch profile.
    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    /// Returns the hardware bring-up ledger used by this provider.
    pub const fn bringup_ledger(&self) -> &BringupLedger {
        &self.bringup
    }

    /// Opens a concrete watch session without boxing.
    pub fn open_session(&self) -> DeviceResult<WatchSession> {
        RouteEnableGuard::enable(&self.route, &self.bringup)?;
        match &self.route {
            WatchRoute::Stub => Err(DeviceError::Unsupported),
            WatchRoute::Ble(link) => Ok(WatchSession::new(
                WatchRouteKind::Ble,
                link.scripted_events(),
                self.profile.clone(),
            )),
            WatchRoute::Relay(link) => Ok(WatchSession::new(
                WatchRouteKind::Relay,
                link.scripted_events(),
                self.profile.clone(),
            )),
            WatchRoute::ZeppBridge(link) => Ok(WatchSession::new(
                WatchRouteKind::ZeppBridge,
                link.scripted_events(),
                self.profile.clone(),
            )),
            WatchRoute::Import(source) => Ok(WatchSession::new(
                WatchRouteKind::Import,
                source.events()?,
                self.profile.clone(),
            )),
        }
    }
}

impl DeviceProvider for WatchProvider {
    fn open(&self) -> DeviceResult<Box<dyn DeviceSession>> {
        Ok(Box::new(self.open_session()?))
    }
}

/// Open watch session over one local route.
#[derive(Clone, Debug, PartialEq)]
pub struct WatchSession {
    profile: DeviceProfile,
    route: WatchRouteKind,
    events: VecDeque<Expr>,
    sent: Vec<WatchCommandPacket>,
}

impl WatchSession {
    /// Builds a watch session from already-normalized events.
    pub fn new(route: WatchRouteKind, events: Vec<WornEvent>, profile: DeviceProfile) -> Self {
        Self {
            profile,
            route,
            events: events.into_iter().map(|event| event.to_expr()).collect(),
            sent: Vec::new(),
        }
    }

    /// Active route kind.
    pub fn route(&self) -> WatchRouteKind {
        self.route
    }

    /// Command packets accepted by this session.
    pub fn sent_commands(&self) -> &[WatchCommandPacket] {
        &self.sent
    }
}

impl DeviceSession for WatchSession {
    fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    fn start(&mut self) -> DeviceResult<()> {
        Ok(())
    }

    fn poll(&mut self, kind: &str) -> DeviceResult<Option<Expr>> {
        if kind == WORN_EVENT_SAMPLE_KIND {
            Ok(self.events.pop_front())
        } else {
            Ok(None)
        }
    }

    fn send(&mut self, command: &Expr) -> DeviceResult<()> {
        self.sent.push(encode_watch_command(self.route, command)?);
        Ok(())
    }

    fn stop(&mut self) -> DeviceResult<()> {
        Ok(())
    }
}

/// Returns the watch stream-device profile advertised by route providers.
pub fn watch_device_profile() -> DeviceProfile {
    DeviceProfile::new(
        Symbol::qualified("device", "amazfit-t-rex-3-pro"),
        vec![
            Symbol::qualified("device/stream", "heart-rate"),
            Symbol::qualified("device/stream", "motion"),
            Symbol::qualified("device/stream", "location"),
            Symbol::qualified("device/stream", "battery"),
            Symbol::qualified("device/stream", "connection"),
            Symbol::qualified("device/stream", "mic-audio"),
        ],
        vec![
            Symbol::qualified("device/input", "button"),
            Symbol::qualified("device/input", "touch"),
            Symbol::qualified("device/input", "tap"),
            Symbol::qualified("device/input", "raise"),
        ],
        vec![
            Symbol::qualified("watch/output", "notification"),
            Symbol::qualified("watch/output", "haptic"),
            Symbol::qualified("watch/output", "face-slot"),
            Symbol::qualified("watch/output", "alarm"),
            Symbol::qualified("watch/output", "privacy"),
        ],
        vec![device_sample_kind_symbol(WORN_EVENT_SAMPLE_KIND)],
    )
}
