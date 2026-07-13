use sim_config::{ConfigProbe, ConfigProbeCaps, ConfigProbeRequest, ConfigProbeStatus, ProbeMode};
use sim_kernel::{Error, Expr, Result, Symbol};

use crate::{
    DeviceCatalog, DeviceDirection, DeviceKind, DeviceProvider, DeviceRecord,
    HostStreamConfigProbe, Placement, StreamEvalSite, stream_host_config_lib_symbol,
};

#[test]
fn config_probe_modeled_emits_modeled_audio_and_midi_defaults() {
    let probe = HostStreamConfigProbe::modeled();
    let request = modeled_request();

    let (layer, report) = probe.probe(&request);

    assert_eq!(report.status, ConfigProbeStatus::Applied);
    assert_eq!(
        report.emitted_keys,
        [
            "audio_backend_candidates",
            "midi_backend_candidates",
            "audio_backend_regex",
            "midi_backend_regex",
            "sample_rate_hz",
            "max_block_frames",
        ]
    );
    let layer = layer.unwrap();
    let table = layer.dir.table(&stream_host_config_lib_symbol()).unwrap();
    assert_eq!(
        field(&table.table, "audio_backend_candidates"),
        &Expr::List(vec![Expr::String("modeled".to_owned())])
    );
    assert_eq!(
        field(&table.table, "midi_backend_candidates"),
        &Expr::List(vec![Expr::String("modeled".to_owned())])
    );
    assert_eq!(
        field(&table.table, "audio_backend_regex"),
        &Expr::String("^(?:modeled)$".to_owned())
    );
    assert_eq!(number_text(field(&table.table, "sample_rate_hz")), "48000");
    assert_eq!(number_text(field(&table.table, "max_block_frames")), "512");
}

#[test]
fn config_probe_real_requires_hardware_inventory_cap() {
    let probe = HostStreamConfigProbe::modeled();
    let request = ConfigProbeRequest {
        mode: ProbeMode::Real,
        ..modeled_request()
    };

    let (layer, report) = probe.probe(&request);

    assert!(layer.is_none());
    assert_eq!(
        report.status,
        ConfigProbeStatus::Denied {
            capability: "hardware_inventory".to_owned()
        }
    );
    assert!(report.emitted_keys.is_empty());
}

#[test]
fn config_probe_real_uses_safe_catalog_inventory() {
    let mut catalog = DeviceCatalog::default_modeled();
    catalog.register(Box::new(FixtureProvider));
    let probe = HostStreamConfigProbe::new(catalog);
    let request = ConfigProbeRequest {
        mode: ProbeMode::Real,
        caps: ConfigProbeCaps {
            hardware_inventory: true,
            ..ConfigProbeCaps::default()
        },
        ..modeled_request()
    };

    let (layer, report) = probe.probe(&request);

    assert_eq!(report.status, ConfigProbeStatus::Applied);
    let layer = layer.unwrap();
    let table = layer.dir.table(&stream_host_config_lib_symbol()).unwrap();
    assert_eq!(
        field(&table.table, "audio_backend_candidates"),
        &Expr::List(vec![
            Expr::String("cpal".to_owned()),
            Expr::String("modeled".to_owned()),
        ])
    );
    assert_eq!(
        field(&table.table, "midi_backend_candidates"),
        &Expr::List(vec![
            Expr::String("alsa-seq".to_owned()),
            Expr::String("modeled".to_owned()),
        ])
    );
    assert_eq!(
        field(&table.table, "midi_backend_regex"),
        &Expr::String("^(?:alsa-seq|modeled)$".to_owned())
    );
}

#[test]
fn config_probe_inventory_failure_is_reported_without_layer() {
    let mut catalog = DeviceCatalog::default_modeled();
    catalog.register(Box::new(FailingProvider));
    let probe = HostStreamConfigProbe::new(catalog);
    let request = ConfigProbeRequest {
        mode: ProbeMode::Real,
        caps: ConfigProbeCaps {
            hardware_inventory: true,
            ..ConfigProbeCaps::default()
        },
        ..modeled_request()
    };

    let (layer, report) = probe.probe(&request);

    assert!(layer.is_none());
    assert!(matches!(
        report.status,
        ConfigProbeStatus::Failed { message } if message.contains("inventory failed")
    ));
    assert!(report.emitted_keys.is_empty());
}

#[test]
fn config_probe_skips_other_libs() {
    let probe = HostStreamConfigProbe::modeled();
    let request = ConfigProbeRequest {
        lib: Symbol::qualified("sim", "cookbook"),
        ..modeled_request()
    };

    let (layer, report) = probe.probe(&request);

    assert!(layer.is_none());
    assert!(matches!(report.status, ConfigProbeStatus::Skipped { .. }));
}

fn modeled_request() -> ConfigProbeRequest {
    ConfigProbeRequest {
        lib: stream_host_config_lib_symbol(),
        mode: ProbeMode::Modeled,
        caps: ConfigProbeCaps::default(),
    }
}

fn field<'a>(expr: &'a Expr, name: &str) -> &'a Expr {
    let Expr::Map(entries) = expr else {
        panic!("expected map");
    };
    entries
        .iter()
        .find_map(|(key, value)| match key {
            Expr::Symbol(symbol) if symbol.name.as_ref() == name => Some(value),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing field {name}"))
}

fn number_text(expr: &Expr) -> &str {
    let Expr::Number(number) = expr else {
        panic!("expected number");
    };
    &number.canonical
}

struct FixtureProvider;

impl DeviceProvider for FixtureProvider {
    fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
        Ok(vec![
            DeviceRecord {
                id: Symbol::new("audio/cpal/out-0"),
                display_name: "cpal Output".to_owned(),
                kind: DeviceKind::Audio,
                direction: DeviceDirection::Output,
                placement: Placement::Hardware {
                    transport: Symbol::new("cpal"),
                },
            },
            DeviceRecord {
                id: Symbol::new("midi/alsa/in-0"),
                display_name: "ALSA MIDI Input".to_owned(),
                kind: DeviceKind::Midi,
                direction: DeviceDirection::Input,
                placement: Placement::Hardware {
                    transport: Symbol::new("alsa-seq"),
                },
            },
        ])
    }

    fn open(&self, _id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
        Err(Error::Eval("fixture provider is inventory-only".to_owned()))
    }
}

struct FailingProvider;

impl DeviceProvider for FailingProvider {
    fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
        Err(Error::Eval("fixture inventory failed".to_owned()))
    }

    fn open(&self, _id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
        Err(Error::Eval("fixture provider is inventory-only".to_owned()))
    }
}
