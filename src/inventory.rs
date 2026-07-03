//! Host device and port inventory records.

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_core::StreamMedia;

use crate::model::{HostDeviceSpec, HostDirection};

/// Devices and ports reported by a backend enumeration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostDeviceInventory {
    backend: Symbol,
    devices: Vec<HostDeviceSpec>,
    ports: Vec<HostPortSpec>,
}

/// Addressable host port owned by an enumerated device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostPortSpec {
    id: Symbol,
    device: Symbol,
    backend: Symbol,
    media: StreamMedia,
    direction: HostDirection,
}

impl HostDeviceInventory {
    /// Creates an empty inventory owned by `backend`.
    pub fn new(backend: Symbol) -> Self {
        Self {
            backend,
            devices: Vec::new(),
            ports: Vec::new(),
        }
    }

    /// Replaces the enumerated devices.
    pub fn with_devices(mut self, devices: Vec<HostDeviceSpec>) -> Self {
        self.devices = devices;
        self
    }

    /// Replaces the enumerated ports.
    pub fn with_ports(mut self, ports: Vec<HostPortSpec>) -> Self {
        self.ports = ports;
        self
    }

    /// Returns the owning backend symbol.
    pub fn backend(&self) -> &Symbol {
        &self.backend
    }

    /// Returns the enumerated devices.
    pub fn devices(&self) -> &[HostDeviceSpec] {
        &self.devices
    }

    /// Returns the enumerated ports.
    pub fn ports(&self) -> &[HostPortSpec] {
        &self.ports
    }

    /// Builds browse card expressions for every device and port.
    pub fn card_exprs(&self) -> Vec<Expr> {
        self.devices
            .iter()
            .map(HostDeviceSpec::card_expr)
            .chain(self.ports.iter().map(HostPortSpec::card_expr))
            .collect()
    }
}

impl HostPortSpec {
    /// Builds a port spec identifying `id` on `device` under `backend`.
    pub fn new(
        id: Symbol,
        device: Symbol,
        backend: Symbol,
        media: StreamMedia,
        direction: HostDirection,
    ) -> Self {
        Self {
            id,
            device,
            backend,
            media,
            direction,
        }
    }

    /// Returns the port identifier symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the owning device symbol.
    pub fn device(&self) -> &Symbol {
        &self.device
    }

    /// Returns the owning backend symbol.
    pub fn backend(&self) -> &Symbol {
        &self.backend
    }

    /// Returns the port media.
    pub fn media(&self) -> StreamMedia {
        self.media
    }

    /// Returns the port direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Builds the browse card expression for this port.
    pub fn card_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("subject")),
                Expr::Symbol(self.id.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("kind")),
                Expr::Symbol(Symbol::qualified("stream", "host-port")),
            ),
            (
                Expr::Symbol(Symbol::new("device")),
                Expr::Symbol(self.device.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("backend")),
                Expr::Symbol(self.backend.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("media")),
                Expr::Symbol(self.media.symbol()),
            ),
            (
                Expr::Symbol(Symbol::new("direction")),
                Expr::Symbol(self.direction.symbol()),
            ),
        ])
    }
}
