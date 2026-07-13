//! Configuration probes for stream-host defaults.

use sim_config::{
    ConfigDir, ConfigLayer, ConfigProbe, ConfigProbeReport, ConfigProbeRequest, ConfigProbeStatus,
    ConfigSource, ProbeMode,
};
use sim_kernel::{Expr, NumberLiteral, Symbol};

use crate::DeviceCatalog;

const DEFAULT_SAMPLE_RATE_HZ: u32 = 48_000;
const DEFAULT_MAX_BLOCK_FRAMES: u32 = 512;

const EMITTED_KEYS: [&str; 6] = [
    "audio_backend_candidates",
    "midi_backend_candidates",
    "audio_backend_regex",
    "midi_backend_regex",
    "sample_rate_hz",
    "max_block_frames",
];

/// Returns the stable config library id for stream-host defaults.
pub fn stream_host_config_lib_symbol() -> Symbol {
    Symbol::qualified("stream", "host")
}

/// Returns the stable stream-host config probe id.
pub fn host_stream_config_probe_symbol() -> Symbol {
    Symbol::qualified("config-probe", "stream-host")
}

/// Safe config probe for stream-host audio and MIDI defaults.
pub struct HostStreamConfigProbe {
    catalog: DeviceCatalog,
}

impl HostStreamConfigProbe {
    /// Builds a probe over a caller-supplied device catalog.
    pub fn new(catalog: DeviceCatalog) -> Self {
        Self { catalog }
    }

    /// Builds a deterministic modeled probe for validation.
    pub fn modeled() -> Self {
        Self::new(DeviceCatalog::default_modeled())
    }
}

impl Default for HostStreamConfigProbe {
    fn default() -> Self {
        Self::modeled()
    }
}

impl ConfigProbe for HostStreamConfigProbe {
    fn symbol(&self) -> Symbol {
        host_stream_config_probe_symbol()
    }

    fn probe(&self, request: &ConfigProbeRequest) -> (Option<ConfigLayer>, ConfigProbeReport) {
        if request.lib != stream_host_config_lib_symbol() {
            return (
                None,
                report(
                    self.symbol(),
                    request,
                    ConfigProbeStatus::Skipped {
                        reason: "stream-host probe only serves stream/host".to_owned(),
                    },
                    &[],
                ),
            );
        }

        if request.mode == ProbeMode::Real && !request.caps.hardware_inventory {
            return (
                None,
                report(
                    self.symbol(),
                    request,
                    ConfigProbeStatus::Denied {
                        capability: "hardware_inventory".to_owned(),
                    },
                    &[],
                ),
            );
        }

        let candidates = match request.mode {
            ProbeMode::Modeled => Ok((vec!["modeled".to_owned()], vec!["modeled".to_owned()])),
            ProbeMode::Real => self.real_candidates(),
        };
        let (audio_candidates, midi_candidates) = match candidates {
            Ok(candidates) => candidates,
            Err(message) => {
                return (
                    None,
                    report(
                        self.symbol(),
                        request,
                        ConfigProbeStatus::Failed { message },
                        &[],
                    ),
                );
            }
        };

        let layer = ConfigLayer::new(
            ConfigSource::Probe {
                probe: self.symbol(),
                mode: request.mode,
            },
            ConfigDir::one(
                request.lib.clone(),
                stream_host_table(&audio_candidates, &midi_candidates),
            )
            .expect("stream-host config probe builds a map table"),
        );
        (
            Some(layer),
            report(
                self.symbol(),
                request,
                ConfigProbeStatus::Applied,
                &EMITTED_KEYS,
            ),
        )
    }
}

impl HostStreamConfigProbe {
    fn real_candidates(&self) -> Result<(Vec<String>, Vec<String>), String> {
        let audio = self
            .catalog
            .audio_backend_names()
            .map_err(|error| format!("audio backend inventory failed: {error}"))?;
        let midi = self
            .catalog
            .midi_backend_names()
            .map_err(|error| format!("MIDI backend inventory failed: {error}"))?;
        Ok((audio, midi))
    }
}

fn stream_host_table(audio_candidates: &[String], midi_candidates: &[String]) -> Expr {
    Expr::Map(vec![
        (
            key("audio_backend_candidates"),
            string_list(audio_candidates),
        ),
        (key("midi_backend_candidates"), string_list(midi_candidates)),
        (
            key("audio_backend_regex"),
            Expr::String(candidate_regex(audio_candidates)),
        ),
        (
            key("midi_backend_regex"),
            Expr::String(candidate_regex(midi_candidates)),
        ),
        (key("sample_rate_hz"), int(DEFAULT_SAMPLE_RATE_HZ)),
        (key("max_block_frames"), int(DEFAULT_MAX_BLOCK_FRAMES)),
    ])
}

fn report(
    probe: Symbol,
    request: &ConfigProbeRequest,
    status: ConfigProbeStatus,
    keys: &[&str],
) -> ConfigProbeReport {
    ConfigProbeReport {
        probe,
        lib: request.lib.clone(),
        mode: request.mode,
        status,
        emitted_keys: keys.iter().map(|key| (*key).to_owned()).collect(),
    }
}

fn key(name: &str) -> Expr {
    Expr::Symbol(Symbol::new(name))
}

fn string_list(values: &[String]) -> Expr {
    Expr::List(
        values
            .iter()
            .map(|value| Expr::String(value.clone()))
            .collect(),
    )
}

fn int(value: u32) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new("i64"),
        canonical: value.to_string(),
    })
}

fn candidate_regex(values: &[String]) -> String {
    if values.is_empty() {
        return "(?!)".to_owned();
    }
    format!(
        "^(?:{})$",
        values
            .iter()
            .map(|value| regex_escape(value))
            .collect::<Vec<_>>()
            .join("|")
    )
}

fn regex_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '.' | '+' | '*' | '?' | '^' | '$' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}
