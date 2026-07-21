//! File-import route for exported watch data.

use sim_kernel::Expr;
use sim_lib_stream_host::{DeviceError, DeviceResult};
use sim_value::build;

use crate::WornEvent;

/// Supported file-import formats.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportFormat {
    /// Comma-separated rows.
    Csv,
    /// GPX route data.
    Gpx,
    /// TCX training data.
    Tcx,
    /// FIT data represented as decoded text rows.
    Fit,
    /// Zepp export rows.
    ZeppExport,
}

impl ImportFormat {
    /// Stable local format token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Gpx => "gpx",
            Self::Tcx => "tcx",
            Self::Fit => "fit",
            Self::ZeppExport => "zepp-export",
        }
    }
}

/// In-memory watch export used by the import route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportSource {
    format: ImportFormat,
    body: String,
}

impl ImportSource {
    /// Builds an import source.
    pub fn new(format: ImportFormat, body: impl Into<String>) -> Self {
        Self {
            format,
            body: body.into(),
        }
    }

    /// Builds a CSV import source.
    pub fn csv(body: impl Into<String>) -> Self {
        Self::new(ImportFormat::Csv, body)
    }

    /// Builds a GPX import source.
    pub fn gpx(body: impl Into<String>) -> Self {
        Self::new(ImportFormat::Gpx, body)
    }

    /// Builds a TCX import source.
    pub fn tcx(body: impl Into<String>) -> Self {
        Self::new(ImportFormat::Tcx, body)
    }

    /// Builds a FIT import source represented as decoded rows.
    pub fn fit(body: impl Into<String>) -> Self {
        Self::new(ImportFormat::Fit, body)
    }

    /// Builds a Zepp export import source.
    pub fn zepp_export(body: impl Into<String>) -> Self {
        Self::new(ImportFormat::ZeppExport, body)
    }

    /// Import format.
    pub fn format(&self) -> ImportFormat {
        self.format
    }

    /// Original import body.
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Parses the source into deterministic worn events.
    pub fn events(&self) -> DeviceResult<Vec<WornEvent>> {
        match self.format {
            ImportFormat::Csv | ImportFormat::Fit | ImportFormat::ZeppExport => {
                parse_row_export(self.format, &self.body)
            }
            ImportFormat::Gpx => parse_gpx(&self.body),
            ImportFormat::Tcx => parse_tcx(&self.body),
        }
    }
}

fn parse_row_export(format: ImportFormat, body: &str) -> DeviceResult<Vec<WornEvent>> {
    let mut events = Vec::new();
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if is_header(line) {
            continue;
        }
        let fields = split_row(line);
        let sensor = fields
            .first()
            .map(|value| normalize_sensor(value))
            .unwrap_or("motion");
        let value = fields
            .get(1)
            .map(|value| parse_value(value))
            .unwrap_or_else(|| build::text(line));
        events.push(WornEvent::from_sensor_name(
            events.len() as u64,
            sensor,
            value,
        )?);
    }
    if format == ImportFormat::ZeppExport && events.is_empty() {
        return Err(DeviceError::Sample(
            "zepp export did not contain worn rows".to_owned(),
        ));
    }
    Ok(events)
}

fn parse_gpx(body: &str) -> DeviceResult<Vec<WornEvent>> {
    let mut events = Vec::new();
    for line in body.lines().map(str::trim) {
        if line.contains("<trkpt") {
            events.push(WornEvent::from_sensor_name(
                events.len() as u64,
                "gps",
                build::text(line),
            )?);
        }
    }
    Ok(events)
}

fn parse_tcx(body: &str) -> DeviceResult<Vec<WornEvent>> {
    let mut events = Vec::new();
    for line in body.lines().map(str::trim) {
        if line.contains("<HeartRateBpm") || line.contains("<Value>") {
            events.push(WornEvent::from_sensor_name(
                events.len() as u64,
                "heart-rate",
                build::text(line),
            )?);
        } else if line.contains("<Trackpoint") {
            events.push(WornEvent::from_sensor_name(
                events.len() as u64,
                "route",
                build::text(line),
            )?);
        }
    }
    Ok(events)
}

fn split_row(line: &str) -> Vec<&str> {
    line.split([',', ';', '\t'])
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect()
}

fn is_header(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("sensor,") || lower.starts_with("type,") || lower == "sensor"
}

fn normalize_sensor(value: &str) -> &'static str {
    match value.trim().to_ascii_lowercase().as_str() {
        "hr" | "heart_rate" | "heart rate" | "heartrate" | "heart-rate" => "heart-rate",
        "gps" | "latlon" | "location" => "gps",
        "route" | "track" => "route",
        "battery" | "bat" => "battery",
        "connection" | "link" => "connection",
        "mic" | "audio" | "mic-audio" => "mic-audio",
        _ => "motion",
    }
}

fn parse_value(value: &str) -> Expr {
    let trimmed = value.trim();
    trimmed
        .parse::<u64>()
        .map(build::uint)
        .unwrap_or_else(|_| build::text(trimmed))
}
