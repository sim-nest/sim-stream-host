//! Hardware bring-up evidence for watch provider routes.

use sim_kernel::Expr;
use sim_lib_stream_host::{DeviceError, DeviceResult};
use sim_value::build::{list, map, qsym, text};

use crate::WatchRoute;

/// The committed bring-up ledger fixture path.
pub const BRINGUP_LEDGER_FIXTURE_NAME: &str = "bringup/ledger";

/// The committed bring-up ledger fixture text.
pub const BRINGUP_LEDGER_FIXTURE: &str = include_str!("../bringup/ledger");

/// A route that can carry bring-up proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BringupRoute {
    /// Standard BLE GATT route through BlueZ.
    Ble,
    /// Phone relay notification route.
    Relay,
    /// Zepp Mini Program bridge route.
    Zepp,
    /// Local file import route.
    Import,
    /// Watch-local Wi-Fi LAN route, enabled only by explicit device proof.
    WifiLan,
}

impl BringupRoute {
    /// Returns the stable ledger key for the route.
    pub const fn key(self) -> &'static str {
        match self {
            Self::Ble => "ble",
            Self::Relay => "relay",
            Self::Zepp => "zepp",
            Self::Import => "import",
            Self::WifiLan => "wifi-lan",
        }
    }

    /// Returns the proof expected for the route.
    pub const fn proof_expectation(self) -> &'static str {
        match self {
            Self::Ble => "BlueZ HR/Battery/DeviceInfo/CurrentTime proof",
            Self::Relay => "phone relay notification lane proof",
            Self::Zepp => "Zepp Mini Program bridge proof",
            Self::Import => "GPX/FIT/TCX/CSV import proof",
            Self::WifiLan => "Zepp OS API and device policy Wi-Fi LAN proof",
        }
    }
}

/// Human-audited proof for enabling one watch route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteProof {
    /// Whether a human has verified the route proof.
    pub verified: bool,
    /// Firmware, adapter, policy, or log evidence for the route.
    pub proof: Option<String>,
}

impl RouteProof {
    /// Builds an unverified proof slot.
    pub const fn unverified() -> Self {
        Self {
            verified: false,
            proof: None,
        }
    }

    /// Builds a verified proof slot with an evidence note.
    pub fn verified(proof: impl Into<String>) -> Self {
        Self {
            verified: true,
            proof: Some(proof.into()),
        }
    }

    /// Returns this proof as an expression fixture entry.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("verified", Expr::Bool(self.verified)),
            (
                "proof",
                self.proof
                    .as_ref()
                    .map_or(Expr::Nil, |proof| text(proof.as_str())),
            ),
        ])
    }
}

impl Default for RouteProof {
    fn default() -> Self {
        Self::unverified()
    }
}

/// Per-route hardware bring-up ledger for watch provider routes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BringupLedger {
    /// BLE route proof.
    pub ble: RouteProof,
    /// Phone relay route proof.
    pub relay: RouteProof,
    /// Zepp Mini Program bridge route proof.
    pub zepp: RouteProof,
    /// File import route proof.
    pub import: RouteProof,
    /// Watch-local Wi-Fi LAN route proof.
    pub wifi_lan: RouteProof,
}

impl BringupLedger {
    /// Returns the proof entry for a route.
    pub const fn entry(&self, route: BringupRoute) -> &RouteProof {
        match route {
            BringupRoute::Ble => &self.ble,
            BringupRoute::Relay => &self.relay,
            BringupRoute::Zepp => &self.zepp,
            BringupRoute::Import => &self.import,
            BringupRoute::WifiLan => &self.wifi_lan,
        }
    }

    /// Returns the mutable proof entry for a route.
    pub fn entry_mut(&mut self, route: BringupRoute) -> &mut RouteProof {
        match route {
            BringupRoute::Ble => &mut self.ble,
            BringupRoute::Relay => &mut self.relay,
            BringupRoute::Zepp => &mut self.zepp,
            BringupRoute::Import => &mut self.import,
            BringupRoute::WifiLan => &mut self.wifi_lan,
        }
    }

    /// Marks a route proof as verified with the supplied evidence note.
    pub fn verify(&mut self, route: BringupRoute, proof: impl Into<String>) {
        *self.entry_mut(route) = RouteProof::verified(proof);
    }

    /// Returns a sample ledger that freezes the synthetic import lane proof.
    pub fn import_fixture() -> Self {
        let mut ledger = Self::default();
        ledger.verify(
            BringupRoute::Import,
            "synthetic GPX/FIT/TCX/CSV fixtures exercised by CI",
        );
        ledger
    }

    /// Returns this ledger as a stable expression fixture.
    pub fn to_expr(&self) -> Expr {
        map(vec![
            ("kind", qsym("stream/wristbridge", "bringup-ledger")),
            ("fixture", text(BRINGUP_LEDGER_FIXTURE_NAME)),
            ("ble", self.ble.to_expr()),
            ("relay", self.relay.to_expr()),
            ("zepp", self.zepp.to_expr()),
            ("import", self.import.to_expr()),
            ("wifi-lan", self.wifi_lan.to_expr()),
            (
                "notes",
                list(vec![
                    text(BringupRoute::Ble.proof_expectation()),
                    text(BringupRoute::Relay.proof_expectation()),
                    text(BringupRoute::Zepp.proof_expectation()),
                    text(BringupRoute::Import.proof_expectation()),
                    text(BringupRoute::WifiLan.proof_expectation()),
                ]),
            ),
        ])
    }
}

/// Fail-closed guard for enabling hardware watch routes.
#[derive(Debug, Default, Clone, Copy)]
pub struct RouteEnableGuard;

impl RouteEnableGuard {
    /// Allows stub and import routes, and requires verified proof for hardware routes.
    pub fn enable(route: &WatchRoute, ledger: &BringupLedger) -> DeviceResult<()> {
        match route {
            WatchRoute::Stub | WatchRoute::Import(_) => Ok(()),
            WatchRoute::Ble(_) => require_verified(BringupRoute::Ble, ledger),
            WatchRoute::Relay(_) => require_verified(BringupRoute::Relay, ledger),
            WatchRoute::ZeppBridge(_) => require_verified(BringupRoute::Zepp, ledger),
        }
    }
}

fn require_verified(route: BringupRoute, ledger: &BringupLedger) -> DeviceResult<()> {
    let proof = ledger.entry(route);
    if proof.verified {
        Ok(())
    } else {
        Err(DeviceError::Host(format!(
            "watch route {} requires verified bring-up proof: {}",
            route.key(),
            route.proof_expectation()
        )))
    }
}
