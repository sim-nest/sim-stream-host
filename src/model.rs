//! Host backend, device, direction, and open-plan model records.

use sim_kernel::{
    CapabilityName, Expr, Symbol,
    effect::{effect_device_read_kind, effect_device_write_kind},
};
use sim_lib_stream_core::{BufferPolicy, StreamDirection, StreamMedia, StreamMetadata};

use crate::capability::HostBackendCapability;

/// Returns the capability name gating host stream device access.
pub fn stream_host_capability() -> CapabilityName {
    CapabilityName::new("stream.host")
}

/// Stable metadata describing a host backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostBackendInfo {
    id: Symbol,
    transport: Symbol,
    media: StreamMedia,
    hardware_required: bool,
    callbacks_bounded: bool,
    capabilities: Vec<HostBackendCapability>,
}

/// Specification of an enumerated host device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostDeviceSpec {
    id: Symbol,
    backend: Symbol,
    media: StreamMedia,
    direction: HostDirection,
    clock: Symbol,
    buffer: BufferPolicy,
}

/// Direction of a host stream relative to the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostDirection {
    /// Device delivers data into the runtime (a source).
    Input,
    /// Device receives data from the runtime (a sink).
    Output,
    /// Device both delivers and receives data.
    Duplex,
}

/// Resolved plan describing how a device would be opened.
///
/// Names the backend, device, the effect kind the open performs, and the
/// capabilities the open requires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostOpenPlan {
    backend: Symbol,
    device: Symbol,
    effect_kind: Symbol,
    requires: Vec<CapabilityName>,
}

impl HostBackendInfo {
    /// Builds backend metadata with bounded callbacks and no capabilities.
    pub fn new(id: Symbol, transport: Symbol, media: StreamMedia, hardware_required: bool) -> Self {
        Self {
            id,
            transport,
            media,
            hardware_required,
            callbacks_bounded: true,
            capabilities: Vec::new(),
        }
    }

    /// Replaces the advertised capabilities.
    pub fn with_capabilities(mut self, capabilities: Vec<HostBackendCapability>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// Returns the backend identifier symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the transport symbol.
    pub fn transport(&self) -> &Symbol {
        &self.transport
    }

    /// Returns the backend media.
    pub fn media(&self) -> StreamMedia {
        self.media
    }

    /// Returns whether opening a stream requires hardware.
    pub fn hardware_required(&self) -> bool {
        self.hardware_required
    }

    /// Returns whether host callbacks are bounded (non-blocking, fixed queue).
    pub fn callbacks_bounded(&self) -> bool {
        self.callbacks_bounded
    }

    /// Returns the advertised capabilities.
    pub fn capabilities(&self) -> &[HostBackendCapability] {
        &self.capabilities
    }

    /// Builds the browse card expression for this backend.
    pub fn card_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("subject")),
                Expr::Symbol(self.id.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("kind")),
                Expr::Symbol(Symbol::qualified("stream", "host-backend")),
            ),
            (
                Expr::Symbol(Symbol::new("transport")),
                Expr::Symbol(self.transport.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("media")),
                Expr::Symbol(self.media.symbol()),
            ),
            (
                Expr::Symbol(Symbol::new("hardware-required")),
                Expr::Bool(self.hardware_required),
            ),
            (
                Expr::Symbol(Symbol::new("callbacks-bounded")),
                Expr::Bool(self.callbacks_bounded),
            ),
            (
                Expr::Symbol(Symbol::new("capabilities")),
                Expr::List(
                    self.capabilities
                        .iter()
                        .map(|capability| Expr::Symbol(capability.symbol()))
                        .collect(),
                ),
            ),
        ])
    }
}

impl HostDeviceSpec {
    /// Builds a device spec from its identity, media, direction, clock, and
    /// buffer policy.
    pub fn new(
        id: Symbol,
        backend: Symbol,
        media: StreamMedia,
        direction: HostDirection,
        clock: Symbol,
        buffer: BufferPolicy,
    ) -> Self {
        Self {
            id,
            backend,
            media,
            direction,
            clock,
            buffer,
        }
    }

    /// Returns the device identifier symbol.
    pub fn id(&self) -> &Symbol {
        &self.id
    }

    /// Returns the owning backend symbol.
    pub fn backend(&self) -> &Symbol {
        &self.backend
    }

    /// Returns the device media.
    pub fn media(&self) -> StreamMedia {
        self.media
    }

    /// Returns the device direction.
    pub fn direction(&self) -> HostDirection {
        self.direction
    }

    /// Returns the device clock-domain symbol.
    pub fn clock(&self) -> &Symbol {
        &self.clock
    }

    /// Returns the device buffer policy.
    pub fn buffer(&self) -> &BufferPolicy {
        &self.buffer
    }

    /// Builds the [`StreamMetadata`] for a stream opened on this device.
    pub fn metadata(&self) -> StreamMetadata {
        StreamMetadata::new(
            self.id.clone(),
            self.media,
            self.direction.stream_direction(),
            self.clock.clone(),
            self.buffer.clone(),
        )
    }

    /// Builds the [`HostOpenPlan`] for opening this device.
    pub fn open_plan(&self) -> HostOpenPlan {
        HostOpenPlan {
            backend: self.backend.clone(),
            device: self.id.clone(),
            effect_kind: self.direction.effect_kind(),
            requires: vec![stream_host_capability()],
        }
    }

    /// Builds the browse card expression for this device.
    pub fn card_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                Expr::Symbol(Symbol::new("subject")),
                Expr::Symbol(self.id.clone()),
            ),
            (
                Expr::Symbol(Symbol::new("kind")),
                Expr::Symbol(Symbol::qualified("stream", "host-device")),
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
            (
                Expr::Symbol(Symbol::new("clock")),
                Expr::Symbol(self.clock.clone()),
            ),
            (Expr::Symbol(Symbol::new("buffer")), self.buffer.to_expr()),
        ])
    }
}

impl HostDirection {
    /// Maps this host direction to the core [`StreamDirection`].
    pub fn stream_direction(self) -> StreamDirection {
        match self {
            Self::Input => StreamDirection::Source,
            Self::Output => StreamDirection::Sink,
            Self::Duplex => StreamDirection::Duplex,
        }
    }

    /// Returns the stable qualified symbol naming this direction.
    pub fn symbol(self) -> Symbol {
        match self {
            Self::Input => Symbol::qualified("stream/host-direction", "input"),
            Self::Output => Symbol::qualified("stream/host-direction", "output"),
            Self::Duplex => Symbol::qualified("stream/host-direction", "duplex"),
        }
    }

    /// Returns the effect kind an open in this direction performs (device read
    /// for input, device write for output or duplex).
    pub fn effect_kind(self) -> Symbol {
        match self {
            Self::Input => effect_device_read_kind(),
            Self::Output | Self::Duplex => effect_device_write_kind(),
        }
    }
}

impl HostOpenPlan {
    /// Returns the backend that would perform the open.
    pub fn backend(&self) -> &Symbol {
        &self.backend
    }

    /// Returns the device that would be opened.
    pub fn device(&self) -> &Symbol {
        &self.device
    }

    /// Returns the effect kind the open performs.
    pub fn effect_kind(&self) -> &Symbol {
        &self.effect_kind
    }

    /// Returns the capabilities the open requires.
    pub fn requires(&self) -> &[CapabilityName] {
        &self.requires
    }
}
