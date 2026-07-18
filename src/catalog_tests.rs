use std::sync::Arc;

use sim_kernel::{
    AbiVersion, Cx, DefaultFactory, EagerPolicy, Export, Lib, LibManifest, LibTarget, Linker,
    LoadCx, Result, Symbol, Version,
};

use crate::{
    DeviceCatalog, DeviceDirection, DeviceKind, Placement, StreamEvalSite,
    audio_device_export_symbol, stream_host_capability,
};

#[cfg(any(feature = "rtmidi-hardware", feature = "ble-midi-hardware"))]
use crate::{DeviceProvider, DeviceRecord};

#[test]
fn catalog_default_modeled_enumerates_midi_and_audio() {
    let catalog = DeviceCatalog::default_modeled();
    let records = catalog.enumerate().unwrap();

    assert!(records.iter().any(|record| record.kind == DeviceKind::Midi));
    assert!(
        records
            .iter()
            .any(|record| record.kind == DeviceKind::Audio)
    );
    assert_eq!(catalog.enumerate_midi().unwrap().len(), 2);
    assert_eq!(catalog.enumerate_audio().unwrap().len(), 1);
}

#[test]
fn unload_retracts_backend_registry_records() {
    let mut cx = test_cx();
    let lib = ModeledBackendLib::new("alsa");
    let lib_id = cx.load_lib(&lib).unwrap();

    assert!(
        DeviceCatalog::with_registry_audio_devices(cx.registry())
            .enumerate_audio()
            .unwrap()
            .iter()
            .any(|record| record.id.to_string().contains("alsa"))
    );

    cx.unload_lib(lib_id).unwrap();

    assert!(
        DeviceCatalog::with_registry_audio_devices(cx.registry())
            .enumerate_audio()
            .unwrap()
            .iter()
            .all(|record| !record.id.to_string().contains("alsa"))
    );
}

#[test]
fn default_audio_catalog_has_no_retired_transport_targets() {
    let records = DeviceCatalog::default_modeled().enumerate_audio().unwrap();
    for transport in ["alsa", "pipewire", "portaudio", "jack", "coreaudio", "asio"] {
        assert!(
            records
                .iter()
                .all(|record| !record.id.to_string().contains(transport)),
            "{transport} appears as a live audio catalog target"
        );
    }
}

#[test]
fn catalog_open_modeled_midi_input_returns_modeled_placement() {
    let catalog = DeviceCatalog::default_modeled();
    let mut cx = authorized_cx();
    let site = catalog
        .open_checked(&mut cx, &Symbol::new("midi/model/in-0"))
        .unwrap();

    assert_eq!(site.placement(), &Placement::Modeled);
    assert_eq!(site.device_record().kind, DeviceKind::Midi);
    assert_eq!(site.device_record().direction, DeviceDirection::Input);
    site.close().unwrap();
}

#[test]
fn catalog_open_unknown_id_returns_error() {
    let catalog = DeviceCatalog::default_modeled();
    let mut cx = test_cx();
    let err = catalog
        .open_checked(&mut cx, &Symbol::new("midi/no-such-device"))
        .err()
        .unwrap();

    assert!(format!("{err}").contains("DeviceCatalog: no device"));
}

#[test]
fn catalog_open_live_modeled_midi_input_yields_modeled_placement() {
    let catalog = DeviceCatalog::default_modeled();
    let mut cx = authorized_cx();
    let mut live = catalog
        .open_live_checked(&mut cx, &Symbol::new("midi/model/in-0"))
        .unwrap();

    assert_eq!(live.placement(), &Placement::Modeled);
    assert_eq!(live.device_record().kind, DeviceKind::Midi);
    assert_eq!(live.device_record().direction, DeviceDirection::Input);
    assert_eq!(live.source_mut().tpq(), 480);
    assert!(live.sink_mut().is_none());
    Box::new(live).close().unwrap();
}

#[test]
fn catalog_open_live_modeled_midi_output_exposes_sink() {
    let catalog = DeviceCatalog::default_modeled();
    let mut cx = authorized_cx();
    let mut live = catalog
        .open_live_checked(&mut cx, &Symbol::new("midi/model/out-0"))
        .unwrap();

    assert_eq!(live.placement(), &Placement::Modeled);
    assert_eq!(live.device_record().kind, DeviceKind::Midi);
    assert_eq!(live.device_record().direction, DeviceDirection::Output);
    assert!(live.sink_mut().is_some());
    Box::new(live).close().unwrap();
}

#[test]
fn catalog_open_live_non_midi_returns_error() {
    let catalog = DeviceCatalog::default_modeled();
    let mut cx = authorized_cx();
    let err = catalog
        .open_live_checked(&mut cx, &Symbol::new("audio/model/stereo-0"))
        .err()
        .unwrap();

    assert!(format!("{err}").contains("is not MIDI"));
}

#[cfg(feature = "rtmidi-hardware")]
#[test]
fn catalog_with_native_midi_keeps_hardware_registration_opt_in() {
    let default_records = DeviceCatalog::default_modeled().enumerate().unwrap();
    assert!(
        default_records
            .iter()
            .all(|record| record.placement == Placement::Modeled)
    );

    let catalog = DeviceCatalog::with_native_midi(Box::new(FixtureHardwareMidiProvider::new(
        "rtmidi/alsa/in-0",
        "ALSA Input 0",
        "alsa-seq",
    )));
    let records = catalog.enumerate().unwrap();
    assert!(records.iter().any(|record| {
        record.id == Symbol::new("rtmidi/alsa/in-0")
            && record.placement
                == Placement::Hardware {
                    transport: Symbol::new("alsa-seq"),
                }
    }));
}

#[cfg(feature = "ble-midi-hardware")]
#[test]
fn catalog_with_ble_midi_keeps_hardware_registration_opt_in() {
    let default_records = DeviceCatalog::default_modeled().enumerate().unwrap();
    assert!(
        default_records
            .iter()
            .all(|record| record.placement == Placement::Modeled)
    );

    let catalog = DeviceCatalog::with_ble_midi(Box::new(FixtureHardwareMidiProvider::new(
        "ble-midi/bluez-0",
        "BLE MIDI Device 0",
        "ble-midi",
    )));
    let records = catalog.enumerate().unwrap();
    assert!(records.iter().any(|record| {
        record.id == Symbol::new("ble-midi/bluez-0")
            && record.placement
                == Placement::Hardware {
                    transport: Symbol::new("ble-midi"),
                }
    }));
    let mut cx = authorized_cx();
    let site = catalog
        .open_checked(&mut cx, &Symbol::new("ble-midi/bluez-0"))
        .unwrap();
    assert_eq!(site.device_record().direction, DeviceDirection::Duplex);
    site.close().unwrap();
}

#[cfg(any(feature = "rtmidi-hardware", feature = "ble-midi-hardware"))]
struct FixtureHardwareMidiProvider {
    id: &'static str,
    name: &'static str,
    transport: &'static str,
}

#[cfg(any(feature = "rtmidi-hardware", feature = "ble-midi-hardware"))]
impl FixtureHardwareMidiProvider {
    fn new(id: &'static str, name: &'static str, transport: &'static str) -> Self {
        Self {
            id,
            name,
            transport,
        }
    }
}

#[cfg(any(feature = "rtmidi-hardware", feature = "ble-midi-hardware"))]
impl DeviceProvider for FixtureHardwareMidiProvider {
    fn enumerate(&self) -> sim_kernel::Result<Vec<DeviceRecord>> {
        Ok(vec![DeviceRecord {
            id: Symbol::new(self.id),
            display_name: self.name.to_owned(),
            kind: DeviceKind::Midi,
            direction: DeviceDirection::Duplex,
            placement: Placement::Hardware {
                transport: Symbol::new(self.transport),
            },
        }])
    }

    fn open(&self, id: &Symbol) -> sim_kernel::Result<Box<dyn StreamEvalSite>> {
        let record = self
            .enumerate()?
            .into_iter()
            .find(|record| &record.id == id)
            .ok_or_else(|| sim_kernel::Error::Eval(format!("unknown fixture port '{id}'")))?;
        Ok(Box::new(FixtureHardwareMidiSite { record }))
    }
}

#[cfg(any(feature = "rtmidi-hardware", feature = "ble-midi-hardware"))]
struct FixtureHardwareMidiSite {
    record: DeviceRecord,
}

#[cfg(any(feature = "rtmidi-hardware", feature = "ble-midi-hardware"))]
impl StreamEvalSite for FixtureHardwareMidiSite {
    fn placement(&self) -> &Placement {
        &self.record.placement
    }

    fn device_record(&self) -> &DeviceRecord {
        &self.record
    }

    fn close(self: Box<Self>) -> sim_kernel::Result<()> {
        Ok(())
    }
}

struct ModeledBackendLib {
    transport: &'static str,
}

impl ModeledBackendLib {
    fn new(transport: &'static str) -> Self {
        Self { transport }
    }

    fn lib_symbol(&self) -> Symbol {
        Symbol::new(format!("stream-{}", self.transport))
    }

    fn device_symbol(&self) -> Symbol {
        audio_device_export_symbol(self.transport)
    }
}

impl Lib for ModeledBackendLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: self.lib_symbol(),
            version: Version("0.1.0".to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Value {
                symbol: self.device_symbol(),
            }],
        }
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker) -> Result<()> {
        linker.value(self.device_symbol(), cx.factory().bool(true)?)
    }
}

fn test_cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}

fn authorized_cx() -> Cx {
    let mut cx = test_cx();
    cx.grant(stream_host_capability());
    cx
}
