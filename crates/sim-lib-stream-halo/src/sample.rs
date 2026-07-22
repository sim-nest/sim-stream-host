//! Halo-specific XR button sample records.

use std::collections::BTreeSet;

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_device::{
    DeviceSample, DeviceSampleError, DeviceSampleResult, device_sample_record_symbol,
    sample_kind_symbol,
};
use sim_lib_stream_xr::halo_device_symbol;
use sim_value::{access, build};

/// Bare sample kind used by Halo button records.
pub const HALO_BUTTON_SAMPLE_KIND: &str = "xr/button";

const BUTTON_NAMESPACE: &str = "stream/xr-button";
const BUTTON_FIELDS: &[&str] = &[
    "kind", "sample", "seq", "device", "button", "pressed", "t-ns",
];

/// Physical Halo button identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaloButton {
    /// Primary temple button.
    Primary,
    /// Secondary temple button.
    Secondary,
}

impl HaloButton {
    /// Stable button token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Secondary => "secondary",
        }
    }

    /// Stable button symbol.
    pub fn symbol(self) -> Symbol {
        Symbol::qualified(BUTTON_NAMESPACE, self.token())
    }

    fn from_symbol(symbol: &Symbol) -> DeviceSampleResult<Self> {
        if symbol.namespace.as_deref() != Some(BUTTON_NAMESPACE) {
            return Err(sample_error(format!(
                "Halo button must be in {BUTTON_NAMESPACE}, found {symbol}"
            )));
        }
        match symbol.name.as_ref() {
            "primary" => Ok(Self::Primary),
            "secondary" => Ok(Self::Secondary),
            other => Err(sample_error(format!("unknown Halo button {other}"))),
        }
    }
}

/// Strict XR button input emitted by Halo routes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HaloButtonSample {
    seq: u64,
    button: HaloButton,
    pressed: bool,
    t_ns: u64,
}

impl HaloButtonSample {
    /// Builds a Halo button sample.
    pub fn new(seq: u64, button: HaloButton, pressed: bool, t_ns: u64) -> Self {
        Self {
            seq,
            button,
            pressed,
            t_ns,
        }
    }

    /// Returns the monotone sequence number.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Returns the physical button.
    pub fn button(&self) -> HaloButton {
        self.button
    }

    /// Returns whether the button is pressed.
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    /// Returns the sample timestamp in nanoseconds.
    pub fn t_ns(&self) -> u64 {
        self.t_ns
    }
}

impl DeviceSample for HaloButtonSample {
    fn sample_kind() -> &'static str {
        HALO_BUTTON_SAMPLE_KIND
    }

    fn seq(&self) -> u64 {
        self.seq
    }

    fn to_expr(&self) -> Expr {
        build::map(vec![
            ("kind", Expr::Symbol(device_sample_record_symbol())),
            ("sample", Expr::Symbol(halo_button_sample_kind_symbol())),
            ("seq", build::uint(self.seq)),
            ("device", Expr::Symbol(halo_device_symbol())),
            ("button", Expr::Symbol(self.button.symbol())),
            ("pressed", Expr::Bool(self.pressed)),
            ("t-ns", build::uint(self.t_ns)),
        ])
    }

    fn from_expr(expr: &Expr) -> DeviceSampleResult<Self> {
        ensure_fields(expr)?;
        expect_symbol(expr, "kind", &device_sample_record_symbol())?;
        expect_symbol(expr, "sample", &halo_button_sample_kind_symbol())?;
        expect_symbol(expr, "device", &halo_device_symbol())?;
        Ok(Self::new(
            required_u64(expr, "seq")?,
            HaloButton::from_symbol(&required_symbol(expr, "button")?)?,
            access::required_bool(expr, "pressed", "Halo button")
                .map_err(|error| sample_error(error.to_string()))?,
            required_u64(expr, "t-ns")?,
        ))
    }
}

/// Returns the qualified sample-kind symbol for Halo buttons.
pub fn halo_button_sample_kind_symbol() -> Symbol {
    sample_kind_symbol(HaloButtonSample::sample_kind())
}

fn ensure_fields(expr: &Expr) -> DeviceSampleResult<()> {
    let Expr::Map(entries) = expr else {
        return Err(sample_error("Halo button sample must be a map"));
    };
    let mut found = BTreeSet::new();
    for (key, _) in entries {
        let name = match key {
            Expr::Symbol(symbol) if symbol.namespace.is_none() => symbol.name.as_ref(),
            Expr::String(text) => text.as_str(),
            _ => return Err(sample_error("Halo button field names must be bare names")),
        };
        if !BUTTON_FIELDS.contains(&name) {
            return Err(sample_error(format!("unknown Halo button field {name}")));
        }
        if !found.insert(name.to_owned()) {
            return Err(sample_error(format!("duplicate Halo button field {name}")));
        }
    }
    if found.len() != BUTTON_FIELDS.len() {
        return Err(sample_error(
            "Halo button sample is missing a required field",
        ));
    }
    Ok(())
}

fn required_symbol(expr: &Expr, field: &str) -> DeviceSampleResult<Symbol> {
    access::required_sym(expr, field, "Halo button")
        .map_err(|error| sample_error(error.to_string()))
}

fn expect_symbol(expr: &Expr, field: &str, expected: &Symbol) -> DeviceSampleResult<()> {
    let found = required_symbol(expr, field)?;
    if &found == expected {
        Ok(())
    } else {
        Err(sample_error(format!(
            "Halo button field {field} must be {expected}, found {found}"
        )))
    }
}

fn required_u64(expr: &Expr, field: &str) -> DeviceSampleResult<u64> {
    let value = access::required(expr, field, "Halo button")
        .map_err(|error| sample_error(error.to_string()))?;
    let Expr::Number(number) = value else {
        return Err(sample_error(format!(
            "Halo button field {field} must be u64"
        )));
    };
    if !matches!(number.domain.name.as_ref(), "i64" | "u64") {
        return Err(sample_error(format!(
            "Halo button field {field} must be u64"
        )));
    }
    number
        .canonical
        .parse::<u64>()
        .map_err(|_| sample_error(format!("Halo button field {field} must be u64")))
}

fn sample_error(message: impl Into<String>) -> DeviceSampleError {
    DeviceSampleError::new(message)
}
