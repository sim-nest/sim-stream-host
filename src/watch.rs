//! Provider-side watch consent gates for worn stream expressions.

use sim_kernel::{CapabilityName, Cx, Error, Expr, Result, Symbol};

/// Capability required for watch health and biometric streams.
pub const CAP_WATCH_HEALTH: &str = "watch/health";

/// Capability required for watch location and route streams.
pub const CAP_WATCH_LOCATION: &str = "watch/location";

/// Capability required for watch microphone audio.
pub const CAP_WATCH_MIC: &str = "watch/mic";

/// Capability required for vendor diagnostic reports.
pub const CAP_WATCH_VENDOR_REPORT: &str = "watch/vendor-report";

const WORN_SENSOR_NAMESPACE: &str = "stream/worn-sensor";
const WORN_SAMPLE_NAMESPACE: &str = "stream/device-sample";
const WORN_SAMPLE_KIND: &str = "worn-event";

/// Watch-sensitive provider capability classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchCapability {
    /// Health and biometric worn streams.
    Health,
    /// GPS and route worn streams.
    Location,
    /// Raw microphone audio.
    Mic,
    /// Vendor diagnostics, off unless explicitly granted.
    VendorReport,
}

impl WatchCapability {
    /// All provider-side watch capabilities.
    pub const ALL: [Self; 4] = [Self::Health, Self::Location, Self::Mic, Self::VendorReport];

    /// Stable kernel capability name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Health => CAP_WATCH_HEALTH,
            Self::Location => CAP_WATCH_LOCATION,
            Self::Mic => CAP_WATCH_MIC,
            Self::VendorReport => CAP_WATCH_VENDOR_REPORT,
        }
    }

    /// Stable local token after the `watch/` prefix.
    pub fn local_name(self) -> &'static str {
        match self {
            Self::Health => "health",
            Self::Location => "location",
            Self::Mic => "mic",
            Self::VendorReport => "vendor-report",
        }
    }

    /// Kernel capability value.
    pub fn capability_name(self) -> CapabilityName {
        CapabilityName::new(self.as_str())
    }

    /// Visible consent grant symbol.
    pub fn grant_symbol(self) -> Symbol {
        Symbol::qualified("watch", self.local_name())
    }

    /// Resolves a watch capability name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_str() == name)
    }
}

/// Returns the visible grant symbol for watch health streams.
pub fn watch_health_grant() -> Symbol {
    WatchCapability::Health.grant_symbol()
}

/// Returns the visible grant symbol for watch location streams.
pub fn watch_location_grant() -> Symbol {
    WatchCapability::Location.grant_symbol()
}

/// Returns the visible grant symbol for watch microphone input.
pub fn watch_mic_grant() -> Symbol {
    WatchCapability::Mic.grant_symbol()
}

/// Returns the visible grant symbol for watch vendor diagnostics.
pub fn watch_vendor_report_grant() -> Symbol {
    WatchCapability::VendorReport.grant_symbol()
}

/// Classifies a worn-event expression into the watch capability it needs.
pub fn watch_capability_for_worn_event(event: &Expr) -> Result<WatchCapability> {
    ensure_worn_event_sample(event)?;
    let sensor = required_symbol_field(event, "sensor")?;
    if sensor.namespace.as_deref() != Some(WORN_SENSOR_NAMESPACE) {
        return Err(Error::HostError(format!(
            "watch worn sensor must be in {WORN_SENSOR_NAMESPACE}, found {sensor}"
        )));
    }
    Ok(match sensor.name.as_ref() {
        "gps" | "route" => WatchCapability::Location,
        "mic-audio" => WatchCapability::Mic,
        _ => WatchCapability::Health,
    })
}

/// Requires kernel authority, visible consent, and same-session receipt ownership.
pub fn require_watch_consent(
    cx: &Cx,
    capability: WatchCapability,
    grants: &[Symbol],
    receipt_session: &Symbol,
    session: &Symbol,
) -> Result<()> {
    cx.require(&capability.capability_name())?;
    if receipt_session != session {
        return Err(Error::HostError(format!(
            "{}: consent not for this session",
            capability.as_str()
        )));
    }
    if !grants
        .iter()
        .any(|grant| grant.as_qualified_str() == capability.as_str())
    {
        return Err(Error::HostError(format!(
            "{} requires visible consent",
            capability.as_str()
        )));
    }
    Ok(())
}

/// Requires the authority needed to ingest one worn-event expression.
pub fn require_watch_worn_ingest(
    cx: &Cx,
    event: &Expr,
    grants: &[Symbol],
    receipt_session: &Symbol,
    session: &Symbol,
) -> Result<WatchCapability> {
    let capability = watch_capability_for_worn_event(event)?;
    require_watch_consent(cx, capability, grants, receipt_session, session)?;
    Ok(capability)
}

fn ensure_worn_event_sample(event: &Expr) -> Result<()> {
    let sample = required_symbol_field(event, "sample")?;
    if sample.namespace.as_deref() == Some(WORN_SAMPLE_NAMESPACE)
        && sample.name.as_ref() == WORN_SAMPLE_KIND
    {
        Ok(())
    } else {
        Err(Error::HostError(
            "expected stream/device-sample worn-event".to_owned(),
        ))
    }
}

fn required_symbol_field(expr: &Expr, name: &str) -> Result<Symbol> {
    match field(expr, name) {
        Some(Expr::Symbol(symbol)) => Ok(symbol.clone()),
        Some(_) => Err(Error::TypeMismatch {
            expected: "symbol field",
            found: "non-symbol",
        }),
        None => Err(Error::Eval(format!("missing field {name}"))),
    }
}

fn field<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == name => {
            Some(value)
        }
        _ => None,
    })
}
