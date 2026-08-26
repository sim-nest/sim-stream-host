//! Session-oriented stream-device provider surface.

use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    time::Duration,
};

use sim_kernel::{CapabilityName, Expr, Symbol};

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
    /// Provider metadata or an effect request violates the authority contract.
    Contract(String),
}

impl fmt::Display for DeviceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str("device provider is unsupported"),
            Self::Sample(message) => write!(f, "device sample error: {message}"),
            Self::Host(message) => f.write_str(message),
            Self::Contract(message) => write!(f, "device contract error: {message}"),
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
            DeviceError::Contract(message) => {
                Self::Eval(format!("device contract error: {message}"))
            }
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
    /// Sample kinds the provider may return from [`ObservationSession::poll`].
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
    fn open(&self) -> DeviceResult<OpenedSession>;
}

/// Open read-only stream-device session.
///
/// This trait deliberately has no downcast hook and no generic command method.
/// A caller holding this object can only observe and release the device.
///
/// ```compile_fail
/// # use sim_lib_stream_host::{EffectRequest, ObservationSession};
/// fn hostile_caller(session: &mut dyn ObservationSession, request: EffectRequest) {
///     session.invoke(request); // observation authority has no invocation escape
/// }
/// ```
pub trait ObservationSession: Send {
    /// Returns the profile for this session.
    fn profile(&self) -> &DeviceProfile;

    /// Starts sample processing.
    fn start(&mut self) -> DeviceResult<()>;

    /// Polls one sample expression for `kind`.
    fn poll(&mut self, kind: &str) -> DeviceResult<Option<Expr>>;

    /// Stops sample processing and releases session resources.
    fn stop(&mut self) -> DeviceResult<()>;
}

/// Open device session that may invoke registered, named effects.
pub trait EffectSession: ObservationSession {
    /// Invokes one request after matching its registered descriptor.
    fn invoke(&mut self, request: EffectRequest) -> DeviceResult<EffectReceipt>;
}

/// Authority-preserving result of opening a provider.
pub enum OpenedSession {
    /// A session whose reachable type exposes observation only.
    Observe(Box<dyn ObservationSession>),
    /// A session with explicitly described effect authority.
    Effect(Box<dyn EffectSession>),
}

impl OpenedSession {
    /// Borrows the observation surface shared by both variants.
    pub fn observation(&mut self) -> &mut dyn ObservationSession {
        match self {
            Self::Observe(session) => session.as_mut(),
            Self::Effect(session) => session.as_mut(),
        }
    }

    /// Returns the effect surface only when the provider explicitly opened one.
    pub fn effect(&mut self) -> Option<&mut (dyn EffectSession + '_)> {
        match self {
            Self::Observe(_) => None,
            Self::Effect(session) => Some(session.as_mut()),
        }
    }
}

/// Closed vocabulary of supported provider transports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderTransport {
    /// Deterministic in-memory cassette.
    Cassette,
    /// Bluetooth Low Energy device link.
    Ble,
    /// Local USB device link.
    Usb,
    /// Local native host API.
    Native,
    /// Explicit user-supplied import.
    Import,
}

/// Policy for retrying an effect request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdempotencePolicy {
    /// Repeating the request is safe.
    Idempotent,
    /// A stable caller key is required for deduplication.
    Keyed,
    /// The effect must not be retried automatically.
    AtMostOnce,
}

/// Policy describing whether and how an effect can be reversed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReversalPolicy {
    /// No reversal exists.
    Irreversible,
    /// Invoke the named registered effect to reverse this effect.
    Effect(Symbol),
}

/// Bounds attached to every callable effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectBounds {
    /// Maximum encoded request size.
    pub max_request_bytes: usize,
    /// Maximum number of invocations for one opened session.
    pub max_invocations: u64,
}

/// Complete registered authority description for one named effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectDescriptor {
    /// Stable descriptor identity.
    pub id: Symbol,
    /// Shape reference used to validate the request payload.
    pub shape: Symbol,
    /// Capability required from the caller.
    pub capability: CapabilityName,
    /// Request and session bounds.
    pub bounds: EffectBounds,
    /// Whether an explicit arm token is mandatory.
    pub requires_arm: bool,
    /// Maximum age of an armed request.
    pub expires_after: Duration,
    /// Receipt kind emitted after execution.
    pub receipt: Symbol,
    /// Retry semantics.
    pub idempotence: IdempotencePolicy,
    /// Reversal semantics.
    pub reversal: ReversalPolicy,
    /// Locally executable stop effect.
    pub local_stop: Symbol,
}

impl EffectDescriptor {
    /// Validates that every mandatory authority field is meaningful.
    pub fn validate(&self) -> DeviceResult<()> {
        if self.shape.name.is_empty()
            || self.capability.as_str().is_empty()
            || self.bounds.max_request_bytes == 0
            || self.bounds.max_invocations == 0
            || self.expires_after.is_zero()
            || self.receipt.name.is_empty()
            || self.local_stop.name.is_empty()
        {
            return Err(DeviceError::Contract(format!(
                "effect {} has an incomplete authority descriptor",
                self.id
            )));
        }
        if matches!(&self.reversal, ReversalPolicy::Effect(effect) if effect == &self.id) {
            return Err(DeviceError::Contract(format!(
                "effect {} reverses itself",
                self.id
            )));
        }
        Ok(())
    }

    /// Checks caller authority, arming, expiry, idempotence, and payload bounds.
    pub fn authorize(&self, request: &EffectRequest) -> DeviceResult<()> {
        self.validate()?;
        if request.descriptor != self.id {
            return Err(DeviceError::Contract("forged effect descriptor".to_owned()));
        }
        if !request.grants.contains(&self.capability) {
            return Err(DeviceError::Contract(format!(
                "missing capability {}",
                self.capability
            )));
        }
        if self.requires_arm && request.arm.as_deref().is_none_or(str::is_empty) {
            return Err(DeviceError::Contract(
                "effect request is not armed".to_owned(),
            ));
        }
        if request.invoked_at_ms < request.armed_at_ms
            || Duration::from_millis(request.invoked_at_ms - request.armed_at_ms)
                > self.expires_after
        {
            return Err(DeviceError::Contract(
                "effect request arm has expired".to_owned(),
            ));
        }
        if matches!(self.idempotence, IdempotencePolicy::Keyed)
            && request.idempotence_key.as_deref().is_none_or(str::is_empty)
        {
            return Err(DeviceError::Contract(
                "effect request has no idempotence key".to_owned(),
            ));
        }
        if format!("{:?}", request.payload).len() > self.bounds.max_request_bytes {
            return Err(DeviceError::Contract(
                "effect request exceeds its size bound".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Builds the complete standard descriptor used by an in-repository device effect.
pub fn standard_effect_descriptor(id: &str) -> EffectDescriptor {
    EffectDescriptor {
        id: Symbol::qualified("device/effect", id),
        shape: Symbol::qualified("shape/device-effect", id),
        capability: CapabilityName::new(format!("device.effect.{id}")),
        bounds: EffectBounds {
            max_request_bytes: 64 * 1024,
            max_invocations: u64::MAX,
        },
        requires_arm: true,
        expires_after: Duration::from_secs(30),
        receipt: Symbol::qualified("device/receipt", id),
        idempotence: IdempotencePolicy::Keyed,
        reversal: ReversalPolicy::Irreversible,
        local_stop: Symbol::qualified("device/effect", "stop"),
    }
}

/// Registry whose entries, rather than manifest strings, create effect callables.
#[derive(Clone, Debug, Default)]
pub struct EffectRegistry(BTreeMap<Symbol, EffectDescriptor>);

impl EffectRegistry {
    /// Builds a checked registry, rejecting duplicate or incomplete descriptors.
    pub fn new(descriptors: impl IntoIterator<Item = EffectDescriptor>) -> DeviceResult<Self> {
        let mut entries = BTreeMap::new();
        for descriptor in descriptors {
            descriptor.validate()?;
            let id = descriptor.id.clone();
            if entries.insert(id.clone(), descriptor).is_some() {
                return Err(DeviceError::Contract(format!(
                    "duplicate effect descriptor {id}"
                )));
            }
        }
        Ok(Self(entries))
    }

    /// Resolves a descriptor by stable identity.
    pub fn get(&self, id: &Symbol) -> Option<&EffectDescriptor> {
        self.0.get(id)
    }
}

/// Versioned, non-secret provider declaration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderManifest {
    /// Manifest schema version. Version one is currently supported.
    pub version: u16,
    /// Stable provider identity.
    pub id: Symbol,
    /// Transport selected from the closed vocabulary.
    pub transport: ProviderTransport,
    /// Content-addressed profile reference.
    pub profile: String,
    /// Declared observation kinds.
    pub observations: Vec<Symbol>,
    /// References to registered effect descriptors.
    pub effects: Vec<Symbol>,
    /// Explicit consent capabilities.
    pub consent: Vec<CapabilityName>,
    /// Bound after which discovery data is stale.
    pub stale_after: Duration,
    /// Content-addressed fake cassette.
    pub cassette: String,
    /// Named observation-only fallback.
    pub fallback: Symbol,
}

impl ProviderManifest {
    /// Validates schema, staleness, secret hygiene, and descriptor references.
    pub fn validate(&self, registry: &EffectRegistry) -> DeviceResult<()> {
        if self.version != 1 {
            return Err(DeviceError::Contract(format!(
                "unsupported provider manifest version {}",
                self.version
            )));
        }
        if self.stale_after.is_zero() {
            return Err(DeviceError::Contract(
                "provider manifest has no stale bound".to_owned(),
            ));
        }
        for value in [&self.profile, &self.cassette] {
            let lower = value.to_ascii_lowercase();
            if lower.contains("secret") || lower.contains("token") || lower.contains("password") {
                return Err(DeviceError::Contract(
                    "provider manifest contains secret material".to_owned(),
                ));
            }
            if value.is_empty() {
                return Err(DeviceError::Contract(
                    "provider manifest has an empty content reference".to_owned(),
                ));
            }
        }
        for effect in &self.effects {
            if registry.get(effect).is_none() {
                return Err(DeviceError::Contract(format!(
                    "undeclared effect descriptor {effect}"
                )));
            }
        }
        Ok(())
    }
}

/// One invocation of a registered effect.
#[derive(Clone, Debug, PartialEq)]
pub struct EffectRequest {
    /// Registered descriptor identity.
    pub descriptor: Symbol,
    /// Shape-checked payload.
    pub payload: Expr,
    /// Explicit arm token when required by the descriptor.
    pub arm: Option<String>,
    /// Stable idempotence key when required by policy.
    pub idempotence_key: Option<String>,
    /// Capabilities explicitly presented for this invocation.
    pub grants: Vec<CapabilityName>,
    /// Monotonic timestamp at which the request was armed.
    pub armed_at_ms: u64,
    /// Monotonic timestamp at which invocation was attempted.
    pub invoked_at_ms: u64,
}

/// Durable acknowledgement returned for an invoked effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectReceipt {
    /// Receipt kind declared by the descriptor.
    pub kind: Symbol,
    /// Descriptor that authorized the effect.
    pub descriptor: Symbol,
    /// Provider-local monotonically increasing sequence.
    pub sequence: u64,
}

/// Deterministic effect session used to prove registered adapters without hardware.
pub struct FakeEffectSession {
    profile: DeviceProfile,
    registry: EffectRegistry,
    receipts: BTreeMap<String, EffectReceipt>,
    invocations: BTreeMap<Symbol, u64>,
    stopped: bool,
    sequence: u64,
}

impl FakeEffectSession {
    /// Builds a fake session from individually reviewed descriptors.
    pub fn new(profile: DeviceProfile, registry: EffectRegistry) -> Self {
        Self {
            profile,
            registry,
            receipts: BTreeMap::new(),
            invocations: BTreeMap::new(),
            stopped: false,
            sequence: 0,
        }
    }
}

impl ObservationSession for FakeEffectSession {
    fn profile(&self) -> &DeviceProfile {
        &self.profile
    }
    fn start(&mut self) -> DeviceResult<()> {
        self.stopped = false;
        Ok(())
    }
    fn poll(&mut self, _kind: &str) -> DeviceResult<Option<Expr>> {
        Ok(None)
    }
    fn stop(&mut self) -> DeviceResult<()> {
        self.stopped = true;
        Ok(())
    }
}

impl EffectSession for FakeEffectSession {
    fn invoke(&mut self, request: EffectRequest) -> DeviceResult<EffectReceipt> {
        if self.stopped {
            return Err(DeviceError::Host(
                "effect session is locally stopped".into(),
            ));
        }
        let descriptor = self
            .registry
            .get(&request.descriptor)
            .ok_or_else(|| DeviceError::Contract("effect is not registered".into()))?;
        descriptor.authorize(&request)?;
        let count = self
            .invocations
            .entry(request.descriptor.clone())
            .or_default();
        if *count >= descriptor.bounds.max_invocations {
            return Err(DeviceError::Contract(
                "effect invocation bound exhausted".into(),
            ));
        }
        if let Some(key) = request.idempotence_key.as_ref()
            && let Some(receipt) = self.receipts.get(key)
        {
            return Ok(receipt.clone());
        }
        *count += 1;
        self.sequence += 1;
        let receipt = EffectReceipt {
            kind: descriptor.receipt.clone(),
            descriptor: descriptor.id.clone(),
            sequence: self.sequence,
        };
        if let Some(key) = request.idempotence_key {
            self.receipts.insert(key, receipt.clone());
        }
        Ok(receipt)
    }
}

/// Polls and decodes a typed device sample from a session.
pub fn poll_device_sample<S>(session: &mut dyn ObservationSession) -> DeviceResult<Option<S>>
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
    fn open(&self) -> DeviceResult<OpenedSession> {
        Err(DeviceError::Unsupported)
    }
}

/// Hardware-free session that refuses all live device operations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StubSession {
    profile: DeviceProfile,
}

/// Deterministic observation-only cassette provider for tests and recipes.
#[derive(Clone, Debug)]
pub struct ObservationCassette {
    profile: DeviceProfile,
    samples: Vec<Expr>,
}

impl ObservationCassette {
    /// Builds a cassette whose samples are replayed in insertion order.
    pub fn new(profile: DeviceProfile, samples: Vec<Expr>) -> Self {
        Self { profile, samples }
    }
}

impl DeviceProvider for ObservationCassette {
    fn open(&self) -> DeviceResult<OpenedSession> {
        Ok(OpenedSession::Observe(Box::new(
            ObservationCassetteSession {
                profile: self.profile.clone(),
                samples: self.samples.clone().into(),
            },
        )))
    }
}

struct ObservationCassetteSession {
    profile: DeviceProfile,
    samples: VecDeque<Expr>,
}

impl ObservationSession for ObservationCassetteSession {
    fn profile(&self) -> &DeviceProfile {
        &self.profile
    }
    fn start(&mut self) -> DeviceResult<()> {
        Ok(())
    }
    fn poll(&mut self, _kind: &str) -> DeviceResult<Option<Expr>> {
        Ok(self.samples.pop_front())
    }
    fn stop(&mut self) -> DeviceResult<()> {
        Ok(())
    }
}

impl StubSession {
    /// Builds a stub session for the supplied profile.
    pub fn new(profile: DeviceProfile) -> Self {
        Self { profile }
    }
}

impl ObservationSession for StubSession {
    fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    fn start(&mut self) -> DeviceResult<()> {
        Err(DeviceError::Unsupported)
    }

    fn poll(&mut self, _kind: &str) -> DeviceResult<Option<Expr>> {
        Err(DeviceError::Unsupported)
    }

    fn stop(&mut self) -> DeviceResult<()> {
        Ok(())
    }
}
