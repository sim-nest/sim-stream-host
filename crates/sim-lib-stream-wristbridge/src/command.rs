//! Watch command validation and route packet encoding.

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_host::{DeviceError, DeviceResult};
use sim_value::{access, build};

use crate::WatchRouteKind;
use crate::event::required_u64;

const WATCH_COMMAND_KIND_NS: &str = "view-wrist";
const WATCH_COMMAND_KIND: &str = "command";
const WATCH_COMMAND_NS: &str = "watch/command";
const HAPTIC_PATTERN_KIND: &str = "haptic-pattern";

/// Watch actuator commands accepted by the provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchCommandKind {
    /// Render a notification on the watch.
    Notify,
    /// Play a haptic pattern.
    Haptic,
    /// Set a watch-face data slot.
    SetFaceSlot,
    /// Set or update an alarm.
    SetAlarm,
    /// Toggle privacy mode.
    PrivacyMode,
}

impl WatchCommandKind {
    /// Stable local command token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Notify => "notify",
            Self::Haptic => "haptic",
            Self::SetFaceSlot => "set-face-slot",
            Self::SetAlarm => "set-alarm",
            Self::PrivacyMode => "privacy-mode",
        }
    }

    fn from_symbol(symbol: &Symbol) -> DeviceResult<Self> {
        if symbol.namespace.as_deref() != Some(WATCH_COMMAND_NS) {
            return Err(DeviceError::Host(format!(
                "watch command symbol must be in {WATCH_COMMAND_NS}"
            )));
        }
        match symbol.name.as_ref() {
            "notify" => Ok(Self::Notify),
            "haptic" => Ok(Self::Haptic),
            "set-face-slot" => Ok(Self::SetFaceSlot),
            "set-alarm" => Ok(Self::SetAlarm),
            "privacy-mode" => Ok(Self::PrivacyMode),
            other => Err(DeviceError::Host(format!("unknown watch command {other}"))),
        }
    }
}

/// Route-local packet produced after a watch command validates.
#[derive(Clone, Debug, PartialEq)]
pub struct WatchCommandPacket {
    route: WatchRouteKind,
    command: WatchCommandKind,
    payload: Expr,
}

impl WatchCommandPacket {
    /// Builds a validated command packet.
    pub fn new(route: WatchRouteKind, command: WatchCommandKind, payload: Expr) -> Self {
        Self {
            route,
            command,
            payload,
        }
    }

    /// Route that owns this packet.
    pub fn route(&self) -> WatchRouteKind {
        self.route
    }

    /// Command kind in this packet.
    pub fn command(&self) -> WatchCommandKind {
        self.command
    }

    /// Original command payload.
    pub fn payload(&self) -> &Expr {
        &self.payload
    }

    /// Encodes the packet as route-neutral expression data.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            ("kind", build::qsym("stream/wristbridge", "command-packet")),
            (
                "route",
                build::qsym("stream/wristbridge-route", self.route.token()),
            ),
            (
                "command",
                build::qsym(WATCH_COMMAND_NS, self.command.token()),
            ),
            ("payload", self.payload.clone()),
        ])
    }
}

/// Validates a watch actuator command and wraps it for a route.
pub fn encode_watch_command(
    route: WatchRouteKind,
    command: &Expr,
) -> DeviceResult<WatchCommandPacket> {
    ensure_kind(
        command,
        WATCH_COMMAND_KIND_NS,
        WATCH_COMMAND_KIND,
        "watch command",
    )?;
    let command_kind = WatchCommandKind::from_symbol(
        &access::required_sym(command, "command", "watch command").map_host_error()?,
    )?;
    match command_kind {
        WatchCommandKind::Notify => validate_notify(command)?,
        WatchCommandKind::Haptic => validate_haptic(command)?,
        WatchCommandKind::SetFaceSlot => validate_set_face_slot(command)?,
        WatchCommandKind::SetAlarm => validate_set_alarm(command)?,
        WatchCommandKind::PrivacyMode => validate_privacy(command)?,
    }
    Ok(WatchCommandPacket::new(
        route,
        command_kind,
        command.clone(),
    ))
}

trait HostErrorMap<T> {
    fn map_host_error(self) -> DeviceResult<T>;
}

impl<T> HostErrorMap<T> for sim_kernel::Result<T> {
    fn map_host_error(self) -> DeviceResult<T> {
        self.map_err(|error| DeviceError::Host(error.to_string()))
    }
}

fn validate_notify(command: &Expr) -> DeviceResult<()> {
    ensure_command_fields(command, &["title", "lines", "urgency"])?;
    access::required_str(command, "title", "watch notify command").map_host_error()?;
    let lines = required_list(command, "lines", "watch notify command")?;
    for line in lines {
        if !matches!(line, Expr::String(_)) {
            return Err(DeviceError::Host(
                "watch notify command lines contain a non-string".to_owned(),
            ));
        }
    }
    let urgency =
        access::required_sym(command, "urgency", "watch notify command").map_host_error()?;
    match urgency.name.as_ref() {
        "info" | "warn" | "error" | "critical" => Ok(()),
        other => Err(DeviceError::Host(format!("unknown watch urgency {other}"))),
    }
}

fn validate_haptic(command: &Expr) -> DeviceResult<()> {
    ensure_command_fields(command, &["pattern"])?;
    let pattern = access::required(command, "pattern", "watch haptic command").map_host_error()?;
    ensure_kind(
        pattern,
        WATCH_COMMAND_KIND_NS,
        HAPTIC_PATTERN_KIND,
        "watch haptic pattern",
    )?;
    ensure_no_extra(
        pattern,
        &["kind", "id", "steps", "meaning", "repeat"],
        "watch haptic pattern",
    )?;
    access::required_sym(pattern, "id", "watch haptic pattern").map_host_error()?;
    access::required_sym(pattern, "meaning", "watch haptic pattern").map_host_error()?;
    let steps = required_list(pattern, "steps", "watch haptic pattern")?;
    if steps.is_empty() {
        return Err(DeviceError::Host(
            "watch haptic pattern requires at least one step".to_owned(),
        ));
    }
    for step in steps {
        ensure_no_extra(step, &["on-ms", "off-ms"], "watch haptic step")?;
        required_u64(step, "on-ms", "watch haptic step")?;
        required_u64(step, "off-ms", "watch haptic step")?;
    }
    let repeat = required_u64(pattern, "repeat", "watch haptic pattern")?;
    if repeat == 0 {
        return Err(DeviceError::Host(
            "watch haptic pattern repeat must be greater than zero".to_owned(),
        ));
    }
    Ok(())
}

fn validate_set_face_slot(command: &Expr) -> DeviceResult<()> {
    ensure_command_fields(command, &["slot", "value"])?;
    access::required_str(command, "slot", "watch face-slot command").map_host_error()?;
    access::required(command, "value", "watch face-slot command").map_host_error()?;
    Ok(())
}

fn validate_set_alarm(command: &Expr) -> DeviceResult<()> {
    ensure_command_fields(command, &["id", "at-ms", "label"])?;
    access::required_str(command, "id", "watch alarm command").map_host_error()?;
    required_u64(command, "at-ms", "watch alarm command")?;
    access::required_str(command, "label", "watch alarm command").map_host_error()?;
    Ok(())
}

fn validate_privacy(command: &Expr) -> DeviceResult<()> {
    ensure_command_fields(command, &["enabled", "window-ms"])?;
    access::required_bool(command, "enabled", "watch privacy command").map_host_error()?;
    required_u64(command, "window-ms", "watch privacy command")?;
    Ok(())
}

fn ensure_command_fields(command: &Expr, variant_fields: &[&str]) -> DeviceResult<()> {
    let mut fields = Vec::with_capacity(2 + variant_fields.len());
    fields.extend(["kind", "command"]);
    fields.extend(variant_fields.iter().copied());
    ensure_no_extra(command, &fields, "watch command")
}

fn ensure_kind(expr: &Expr, namespace: &str, name: &str, context: &str) -> DeviceResult<()> {
    match access::field_sym(expr, "kind") {
        Some(kind)
            if kind.namespace.as_deref() == Some(namespace) && kind.name.as_ref() == name =>
        {
            Ok(())
        }
        _ => Err(DeviceError::Host(format!(
            "expected {namespace}/{name} {context}"
        ))),
    }
}

fn ensure_no_extra(expr: &Expr, known: &[&str], context: &str) -> DeviceResult<()> {
    let Expr::Map(entries) = expr else {
        return Err(DeviceError::Host(format!("{context} is not a map")));
    };
    for (key, _) in entries {
        let allowed = match key {
            Expr::Symbol(symbol) if symbol.namespace.is_none() => {
                known.contains(&symbol.name.as_ref())
            }
            Expr::String(text) => known.contains(&text.as_str()),
            _ => false,
        };
        if !allowed {
            return Err(DeviceError::Host(format!(
                "{context} has unknown field {key:?}"
            )));
        }
    }
    Ok(())
}

fn required_list<'a>(expr: &'a Expr, field: &str, context: &str) -> DeviceResult<&'a [Expr]> {
    match access::required(expr, field, context).map_host_error()? {
        Expr::List(items) => Ok(items),
        _ => Err(DeviceError::Host(format!(
            "{context} field {field} is not a list"
        ))),
    }
}
