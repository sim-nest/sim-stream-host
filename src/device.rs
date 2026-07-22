//! Session-oriented stream-device provider surface.

use std::fmt;

use sim_kernel::{Expr, Symbol};

/// Result type returned by device providers and sessions.
pub type DeviceResult<T> = std::result::Result<T, DeviceError>;

/// Error returned by stream-device providers and sessions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceError {
    /// The selected provider does not support opening or using a device.
    Unsupported,
    /// A sample expression was malformed for the requested sample kind.
    Sample(String),
    /// A provider or session failed for a host-specific reason.
    Host(String),
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str("device provider is unsupported"),
            Self::Sample(message) => write!(f, "device sample error: {message}"),
            Self::Host(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for DeviceError {}

impl From<DeviceError> for sim_kernel::Error {
    fn from(error: DeviceError) -> Self {
        match error {
            DeviceError::Unsupported => {
                Self::HostError("device provider is unsupported".to_owned())
            }
            DeviceError::Sample(message) => Self::Eval(format!("device sample error: {message}")),
            DeviceError::Host(message) => Self::HostError(message),
        }
    }
}

/// Stream-facing profile advertised by a concrete device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Stable device identity.
    pub device: Symbol,
    /// Sample streams this device can emit.
    pub streams: Vec<Symbol>,
    /// Input controls accepted by the device.
    pub inputs: Vec<Symbol>,
    /// Output actuators exposed by the device.
    pub outputs: Vec<Symbol>,
    /// Sample kinds the provider may return from [`DeviceSession::poll`].
    pub sample_kinds: Vec<Symbol>,
}

impl DeviceProfile {
    /// Builds a device profile from stable stream-facing metadata.
    pub fn new(
        device: Symbol,
        streams: Vec<Symbol>,
        inputs: Vec<Symbol>,
        outputs: Vec<Symbol>,
        sample_kinds: Vec<Symbol>,
    ) -> Self {
        Self {
            device,
            streams,
            inputs,
            outputs,
            sample_kinds,
        }
    }

    /// Builds the deterministic modeled edge profile used by tests and docs.
    pub fn modeled_edge() -> Self {
        Self::new(
            Symbol::qualified("device", "modeled-edge"),
            vec![
                Symbol::qualified("device/stream", "battery"),
                Symbol::qualified("device/stream", "motion"),
            ],
            vec![Symbol::qualified("device/input", "button")],
            vec![
                Symbol::qualified("device/output", "screen"),
                Symbol::qualified("device/output", "haptic"),
            ],
            vec![device_sample_kind_symbol("device-caps")],
        )
    }

    /// Returns whether this profile advertises `sample_kind`.
    pub fn supports_sample_kind(&self, sample_kind: &Symbol) -> bool {
        self.sample_kinds.contains(sample_kind)
    }
}

/// Device sample expression contract used by session polling helpers.
pub trait DeviceSample: Sized {
    /// Stable bare sample kind, such as `device-caps`.
    fn sample_kind() -> &'static str;

    /// Encodes the sample as a self-describing expression.
    fn to_expr(&self) -> Expr;

    /// Decodes the sample from its expression form.
    fn from_expr(expr: &Expr) -> DeviceResult<Self>;
}

/// Returns the qualified sample-kind symbol for `kind`.
pub fn device_sample_kind_symbol(kind: &str) -> Symbol {
    Symbol::qualified("stream/device-sample", kind)
}

/// Provider that opens one stream-device session.
pub trait DeviceProvider: Send {
    /// Opens a provider-owned device session.
    fn open(&self) -> DeviceResult<Box<dyn DeviceSession>>;
}

/// Open stream-device session.
pub trait DeviceSession: Send {
    /// Returns the profile for this session.
    fn profile(&self) -> &DeviceProfile;

    /// Starts sample or command processing.
    fn start(&mut self) -> DeviceResult<()>;

    /// Polls one sample expression for `kind`.
    fn poll(&mut self, kind: &str) -> DeviceResult<Option<Expr>>;

    /// Sends an actuator command expression to the device.
    fn send(&mut self, command: &Expr) -> DeviceResult<()>;

    /// Stops sample or command processing and releases session resources.
    fn stop(&mut self) -> DeviceResult<()>;
}

/// Polls and decodes a typed device sample from a session.
pub fn poll_device_sample<S>(session: &mut dyn DeviceSession) -> DeviceResult<Option<S>>
where
    S: DeviceSample,
{
    session
        .poll(S::sample_kind())?
        .map(|expr| S::from_expr(&expr))
        .transpose()
}

/// Hardware-free provider used when no concrete device provider is installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubProvider {
    profile: DeviceProfile,
}

impl StubProvider {
    /// Builds a stub provider for the supplied profile.
    pub fn new(profile: DeviceProfile) -> Self {
        Self { profile }
    }

    /// Returns the profile this stub advertises for browse and placement.
    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    /// Builds an unopened stub session for provider-surface validation.
    pub fn session(&self) -> StubSession {
        StubSession::new(self.profile.clone())
    }
}

impl DeviceProvider for StubProvider {
    fn open(&self) -> DeviceResult<Box<dyn DeviceSession>> {
        Err(DeviceError::Unsupported)
    }
}

/// Hardware-free session that refuses all live device operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubSession {
    profile: DeviceProfile,
}

impl StubSession {
    /// Builds a stub session for the supplied profile.
    pub fn new(profile: DeviceProfile) -> Self {
        Self { profile }
    }
}

impl DeviceSession for StubSession {
    fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    fn start(&mut self) -> DeviceResult<()> {
        Err(DeviceError::Unsupported)
    }

    fn poll(&mut self, _kind: &str) -> DeviceResult<Option<Expr>> {
        Err(DeviceError::Unsupported)
    }

    fn send(&mut self, _command: &Expr) -> DeviceResult<()> {
        Err(DeviceError::Unsupported)
    }

    fn stop(&mut self) -> DeviceResult<()> {
        Ok(())
    }
}
