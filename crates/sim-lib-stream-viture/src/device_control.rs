//! VITURE display and sensor command validation.

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_host::{DeviceError, DeviceResult};
use sim_value::{access, build};
use sim_viture_ffi::LegacyImuRate;

const VITURE_CONTROL_PACKET_NS: &str = "stream/viture";
const VITURE_CONTROL_PACKET: &str = "control-packet";
const VITURE_COMMAND_KIND_NS: &str = "view-viture";
const VITURE_COMMAND_KIND: &str = "command";
const VITURE_COMMAND_NS: &str = "viture/command";

/// VITURE actuator or sensor-control command kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VitureCommandKind {
    /// Enable or disable IMU reports.
    ImuReports,
    /// Set the IMU report rate.
    ImuRate,
    /// Enable or disable the 3D display mode.
    Display3d,
    /// Set display brightness from 0 through 100.
    Brightness,
    /// Set the privacy-film tint from 0 through 100.
    PrivacyFilm,
}

impl VitureCommandKind {
    /// Stable command token.
    pub fn token(self) -> &'static str {
        match self {
            Self::ImuReports => "imu-reports",
            Self::ImuRate => "imu-rate",
            Self::Display3d => "display-3d",
            Self::Brightness => "brightness",
            Self::PrivacyFilm => "privacy-film",
        }
    }

    fn from_symbol(symbol: &Symbol) -> DeviceResult<Self> {
        if symbol.namespace.as_deref() != Some(VITURE_COMMAND_NS) {
            return Err(DeviceError::Host(format!(
                "VITURE command symbol must be in {VITURE_COMMAND_NS}"
            )));
        }
        match symbol.name.as_ref() {
            "imu-reports" => Ok(Self::ImuReports),
            "imu-rate" => Ok(Self::ImuRate),
            "display-3d" => Ok(Self::Display3d),
            "brightness" => Ok(Self::Brightness),
            "privacy-film" => Ok(Self::PrivacyFilm),
            other => Err(DeviceError::Host(format!("unknown VITURE command {other}"))),
        }
    }
}

/// Route-neutral VITURE command packet after validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VitureControlPacket {
    /// IMU report toggle.
    ImuReports {
        /// Whether reports are enabled.
        enabled: bool,
    },
    /// IMU report rate.
    ImuRate {
        /// Non-zero report rate.
        rate: LegacyImuRate,
    },
    /// Stereoscopic display mode toggle.
    Display3d {
        /// Whether 3D display mode is enabled.
        enabled: bool,
    },
    /// Display brightness command.
    Brightness {
        /// Brightness level from 0 through 100.
        level: u8,
    },
    /// Privacy-film tint command.
    PrivacyFilm {
        /// Tint level from 0 through 100.
        level: u8,
    },
}

impl VitureControlPacket {
    /// Returns the command kind for this packet.
    pub fn kind(&self) -> VitureCommandKind {
        match self {
            Self::ImuReports { .. } => VitureCommandKind::ImuReports,
            Self::ImuRate { .. } => VitureCommandKind::ImuRate,
            Self::Display3d { .. } => VitureCommandKind::Display3d,
            Self::Brightness { .. } => VitureCommandKind::Brightness,
            Self::PrivacyFilm { .. } => VitureCommandKind::PrivacyFilm,
        }
    }

    /// Encodes the packet as stable expression data.
    pub fn to_expr(&self) -> Expr {
        let mut entries = vec![
            (
                "kind",
                build::qsym(VITURE_CONTROL_PACKET_NS, VITURE_CONTROL_PACKET),
            ),
            ("command", Expr::Symbol(viture_command_symbol(self.kind()))),
        ];
        match self {
            Self::ImuReports { enabled } | Self::Display3d { enabled } => {
                entries.push(("enabled", Expr::Bool(*enabled)));
            }
            Self::ImuRate { rate } => {
                entries.push(("hz", build::uint(u64::from(rate.hz()))));
            }
            Self::Brightness { level } | Self::PrivacyFilm { level } => {
                entries.push(("level", build::uint(u64::from(*level))));
            }
        }
        build::map(entries)
    }
}

/// Returns the stable command symbol for `kind`.
pub fn viture_command_symbol(kind: VitureCommandKind) -> Symbol {
    Symbol::qualified(VITURE_COMMAND_NS, kind.token())
}

/// Validates a VITURE actuator command expression.
pub fn encode_viture_command(command: &Expr) -> DeviceResult<VitureControlPacket> {
    ensure_kind(
        command,
        VITURE_COMMAND_KIND_NS,
        VITURE_COMMAND_KIND,
        "VITURE command",
    )?;
    let command_kind = VitureCommandKind::from_symbol(
        &access::required_sym(command, "command", "VITURE command").map_host_error()?,
    )?;
    match command_kind {
        VitureCommandKind::ImuReports => {
            ensure_command_fields(command, &["enabled"])?;
            Ok(VitureControlPacket::ImuReports {
                enabled: access::required_bool(command, "enabled", "VITURE IMU command")
                    .map_host_error()?,
            })
        }
        VitureCommandKind::ImuRate => {
            ensure_command_fields(command, &["hz"])?;
            let hz = required_u32(command, "hz", "VITURE IMU rate command")?;
            let Some(rate) = LegacyImuRate::new(hz) else {
                return Err(DeviceError::Host(
                    "VITURE IMU rate command hz must be greater than zero".to_owned(),
                ));
            };
            Ok(VitureControlPacket::ImuRate { rate })
        }
        VitureCommandKind::Display3d => {
            ensure_command_fields(command, &["enabled"])?;
            Ok(VitureControlPacket::Display3d {
                enabled: access::required_bool(command, "enabled", "VITURE 3D command")
                    .map_host_error()?,
            })
        }
        VitureCommandKind::Brightness => {
            ensure_command_fields(command, &["level"])?;
            Ok(VitureControlPacket::Brightness {
                level: required_percent(command, "level", "VITURE brightness command")?,
            })
        }
        VitureCommandKind::PrivacyFilm => {
            ensure_command_fields(command, &["level"])?;
            Ok(VitureControlPacket::PrivacyFilm {
                level: required_percent(command, "level", "VITURE privacy-film command")?,
            })
        }
    }
}

trait HostErrorMap<T> {
    fn map_host_error(self) -> DeviceResult<T>;
}

impl<T> HostErrorMap<T> for sim_kernel::Result<T> {
    fn map_host_error(self) -> DeviceResult<T> {
        self.map_err(|error| DeviceError::Host(error.to_string()))
    }
}

fn ensure_command_fields(command: &Expr, variant_fields: &[&str]) -> DeviceResult<()> {
    let mut fields = Vec::with_capacity(2 + variant_fields.len());
    fields.extend(["kind", "command"]);
    fields.extend(variant_fields.iter().copied());
    ensure_no_extra(command, &fields, "VITURE command")
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

fn required_percent(expr: &Expr, field: &str, context: &str) -> DeviceResult<u8> {
    let value = required_u64(expr, field, context)?;
    value
        .try_into()
        .ok()
        .filter(|level| *level <= 100)
        .ok_or_else(|| DeviceError::Host(format!("{context} field {field} must be 0 through 100")))
}

fn required_u32(expr: &Expr, field: &str, context: &str) -> DeviceResult<u32> {
    required_u64(expr, field, context)?
        .try_into()
        .map_err(|_| DeviceError::Host(format!("{context} field {field} is not u32")))
}

fn required_u64(expr: &Expr, field: &str, context: &str) -> DeviceResult<u64> {
    match access::required(expr, field, context).map_host_error()? {
        Expr::Number(number) if matches!(number.domain.name.as_ref(), "i64" | "u64") => {
            let value = number
                .canonical
                .parse::<i64>()
                .map_err(|_| DeviceError::Host(format!("{context} field {field} is not u64")))?;
            value
                .try_into()
                .map_err(|_| DeviceError::Host(format!("{context} field {field} is not u64")))
        }
        _ => Err(DeviceError::Host(format!(
            "{context} field {field} is not u64"
        ))),
    }
}
