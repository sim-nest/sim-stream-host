//! Worn-event sample encoding.

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_host::{DeviceError, DeviceResult, DeviceSample, device_sample_kind_symbol};
use sim_value::{access, build};

/// Bare device-sample kind used by watch worn events.
pub const WORN_EVENT_SAMPLE_KIND: &str = "worn-event";

const RECORD_SAMPLE_KIND: &str = "record";
const SAMPLE_NAMESPACE: &str = "stream/device-sample";
const SENSOR_NAMESPACE: &str = "stream/worn-sensor";

/// One normalized sample emitted by a watch provider route.
#[derive(Clone, Debug, PartialEq)]
pub struct WornEvent {
    seq: u64,
    sensor: Symbol,
    value: Expr,
}

impl WornEvent {
    /// Builds a worn event with a stable sequence number.
    pub fn new(seq: u64, sensor: Symbol, value: Expr) -> DeviceResult<Self> {
        if sensor.namespace.as_deref() != Some(SENSOR_NAMESPACE) {
            return Err(DeviceError::Sample(format!(
                "worn sensor must be in {SENSOR_NAMESPACE}, found {sensor}"
            )));
        }
        Ok(Self { seq, sensor, value })
    }

    /// Builds an event from a bare worn-sensor name.
    pub fn from_sensor_name(seq: u64, sensor: &str, value: Expr) -> DeviceResult<Self> {
        Self::new(seq, Symbol::qualified(SENSOR_NAMESPACE, sensor), value)
    }

    /// Monotone sequence number assigned by the route.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    /// Sensor symbol, such as `stream/worn-sensor/heart-rate`.
    pub fn sensor(&self) -> &Symbol {
        &self.sensor
    }

    /// Payload value carried by the worn event.
    pub fn value(&self) -> &Expr {
        &self.value
    }
}

impl DeviceSample for WornEvent {
    fn sample_kind() -> &'static str {
        WORN_EVENT_SAMPLE_KIND
    }

    fn to_expr(&self) -> Expr {
        build::map(vec![
            ("kind", build::qsym(SAMPLE_NAMESPACE, RECORD_SAMPLE_KIND)),
            (
                "sample",
                build::qsym(SAMPLE_NAMESPACE, WORN_EVENT_SAMPLE_KIND),
            ),
            ("seq", build::uint(self.seq)),
            ("sensor", Expr::Symbol(self.sensor.clone())),
            ("value", self.value.clone()),
        ])
    }

    fn from_expr(expr: &Expr) -> DeviceResult<Self> {
        ensure_sample(expr)?;
        let seq = required_u64(expr, "seq", "worn event")?;
        let sensor = access::required_sym(expr, "sensor", "worn event").map_device_error()?;
        let value = access::required(expr, "value", "worn event")
            .map_device_error()?
            .clone();
        Self::new(seq, sensor, value)
    }
}

/// Returns the qualified sample-kind symbol for watch worn events.
pub fn worn_event_sample_kind() -> Symbol {
    device_sample_kind_symbol(WORN_EVENT_SAMPLE_KIND)
}

pub(crate) trait DeviceErrorMap<T> {
    fn map_device_error(self) -> DeviceResult<T>;
}

impl<T> DeviceErrorMap<T> for sim_kernel::Result<T> {
    fn map_device_error(self) -> DeviceResult<T> {
        self.map_err(|error| DeviceError::Sample(error.to_string()))
    }
}

pub(crate) fn required_u64(expr: &Expr, field: &str, context: &str) -> DeviceResult<u64> {
    match access::required(expr, field, context).map_device_error()? {
        Expr::Number(number) if matches!(number.domain.name.as_ref(), "i64" | "u64") => {
            let value = number
                .canonical
                .parse::<i64>()
                .map_err(|_| DeviceError::Sample(format!("{context} field {field} is not u64")))?;
            value
                .try_into()
                .map_err(|_| DeviceError::Sample(format!("{context} field {field} is not u64")))
        }
        _ => Err(DeviceError::Sample(format!(
            "{context} field {field} is not u64"
        ))),
    }
}

fn ensure_sample(expr: &Expr) -> DeviceResult<()> {
    let sample = access::required_sym(expr, "sample", "worn event").map_device_error()?;
    if sample.namespace.as_deref() == Some(SAMPLE_NAMESPACE)
        && sample.name.as_ref() == WORN_EVENT_SAMPLE_KIND
    {
        Ok(())
    } else {
        Err(DeviceError::Sample(
            "expected stream/device-sample worn-event".to_owned(),
        ))
    }
}
