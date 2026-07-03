//! Shared host-device catalog for modeled stream placements.

use sim_kernel::{
    Error, Result, Symbol,
    library::{ExportKind, ExportRecord, ExportState, Registry},
};

use crate::audio_provider::ModeledAudioProvider;
use crate::eval_site::{DeviceProvider, StreamEvalSite};
use crate::midi_live_eval_site::MidiLiveEvalSite;
use crate::midi_provider::ModeledMidiProvider;
use crate::placement::{DeviceDirection, DeviceKind, DeviceRecord};
use crate::{AudioRouter, Placement};

/// Registry of stream-device providers.
pub struct DeviceCatalog {
    providers: Vec<Box<dyn DeviceProvider>>,
}

impl Default for DeviceCatalog {
    fn default() -> Self {
        Self::default_modeled()
    }
}

impl DeviceCatalog {
    /// Builds an empty device catalog.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Builds a catalog with deterministic modeled MIDI and audio providers.
    pub fn default_modeled() -> Self {
        let mut catalog = Self::new();
        catalog.register(Box::new(ModeledMidiProvider::default()));
        catalog.register(Box::new(ModeledAudioProvider::default()));
        catalog
    }

    /// Builds a modeled catalog plus audio devices exported by loaded libs.
    pub fn with_registry_audio_devices(registry: &Registry) -> Self {
        let mut catalog = Self::default_modeled();
        catalog.register_registry_audio_devices(registry);
        catalog
    }

    /// Builds a catalog with modeled providers plus one caller-supplied native MIDI provider.
    #[cfg(any(feature = "rtmidi-hardware", feature = "ble-midi-hardware"))]
    pub fn with_native_midi(provider: Box<dyn DeviceProvider>) -> Self {
        let mut catalog = Self::default_modeled();
        catalog.register(provider);
        catalog
    }

    /// Builds a catalog with modeled providers plus a caller-supplied ALSA MIDI provider.
    #[cfg(feature = "rtmidi-hardware")]
    pub fn with_alsa_midi(provider: Box<dyn DeviceProvider>) -> Self {
        Self::with_native_midi(provider)
    }

    /// Builds a catalog with modeled providers plus a caller-supplied BLE-MIDI provider.
    #[cfg(feature = "ble-midi-hardware")]
    pub fn with_ble_midi(provider: Box<dyn DeviceProvider>) -> Self {
        Self::with_native_midi(provider)
    }

    /// Registers a device provider.
    pub fn register(&mut self, provider: Box<dyn DeviceProvider>) {
        self.providers.push(provider);
    }

    /// Adds a snapshot of the audio sites currently registered by a provider.
    pub fn register_provider_sites(&mut self, router: &AudioRouter) {
        let provider = ProviderAudioSites::from_router(router);
        if !provider.is_empty() {
            self.register(Box::new(provider));
        }
    }

    /// Adds audio-device records currently owned by loaded registry libs.
    pub fn register_registry_audio_devices(&mut self, registry: &Registry) {
        let provider = RegistryAudioDevices::from_registry(registry);
        if !provider.is_empty() {
            self.register(Box::new(provider));
        }
    }

    /// Enumerates every registered provider.
    pub fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
        let mut records = Vec::new();
        for provider in &self.providers {
            records.extend(provider.enumerate()?);
        }
        Ok(records)
    }

    /// Opens a cataloged stream evaluation site by device id.
    pub fn open(&self, id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
        for provider in &self.providers {
            let records = provider.enumerate()?;
            if records.iter().any(|record| &record.id == id) {
                return provider.open(id);
            }
        }
        Err(Error::Eval(format!("DeviceCatalog: no device '{id}'")))
    }

    /// Opens a cataloged MIDI device as a live MIDI evaluation site.
    pub fn open_live(&self, id: &Symbol) -> Result<MidiLiveEvalSite> {
        MidiLiveEvalSite::from_eval_site(id, self.open(id)?)
    }

    /// Enumerates MIDI device rows.
    pub fn enumerate_midi(&self) -> Result<Vec<DeviceRecord>> {
        self.enumerate_kind(DeviceKind::Midi)
    }

    /// Enumerates audio device rows.
    pub fn enumerate_audio(&self) -> Result<Vec<DeviceRecord>> {
        self.enumerate_kind(DeviceKind::Audio)
    }

    fn enumerate_kind(&self, kind: DeviceKind) -> Result<Vec<DeviceRecord>> {
        Ok(self
            .enumerate()?
            .into_iter()
            .filter(|record| record.kind == kind)
            .collect())
    }
}

/// Stable export symbol for an audio device owned by a loaded lib.
pub fn audio_device_export_symbol(name: &str) -> Symbol {
    Symbol::qualified("audio/device", name)
}

struct ProviderAudioSites {
    records: Vec<DeviceRecord>,
}

impl ProviderAudioSites {
    fn from_router(router: &AudioRouter) -> Self {
        let mut records = router
            .site_keys()
            .filter_map(|key| router.site(key))
            .map(|site| DeviceRecord::modeled_audio_from_card(site.card()))
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.id.to_string());
        Self { records }
    }

    fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl DeviceProvider for ProviderAudioSites {
    fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
        Ok(self.records.clone())
    }

    fn open(&self, id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
        let record = self
            .records
            .iter()
            .find(|record| &record.id == id)
            .ok_or_else(|| Error::Eval(format!("ProviderAudioSites: unknown id '{id}'")))?
            .clone();
        Ok(Box::new(ProviderAudioSite { record }))
    }
}

struct RegistryAudioDevices {
    records: Vec<DeviceRecord>,
}

impl RegistryAudioDevices {
    fn from_registry(registry: &Registry) -> Self {
        let mut records = registry
            .libs()
            .iter()
            .flat_map(|lib| lib.exports.iter())
            .filter_map(registry_audio_device_record)
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.id.to_string());
        records.dedup_by(|left, right| left.id == right.id);
        Self { records }
    }

    fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl DeviceProvider for RegistryAudioDevices {
    fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
        Ok(self.records.clone())
    }

    fn open(&self, id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
        let record = self
            .records
            .iter()
            .find(|record| &record.id == id)
            .ok_or_else(|| Error::Eval(format!("RegistryAudioDevices: unknown id '{id}'")))?
            .clone();
        Ok(Box::new(ProviderAudioSite { record }))
    }
}

fn registry_audio_device_record(record: &ExportRecord) -> Option<DeviceRecord> {
    if record.kind != ExportKind::named(ExportKind::VALUE) {
        return None;
    }
    if record.symbol.namespace.as_deref() != Some("audio/device") {
        return None;
    }
    if matches!(record.state, ExportState::Invalid { .. }) {
        return None;
    }
    Some(DeviceRecord {
        id: record.symbol.clone(),
        display_name: format!("{} audio device", record.symbol.name),
        kind: DeviceKind::Audio,
        direction: DeviceDirection::Duplex,
        placement: Placement::Modeled,
    })
}

struct ProviderAudioSite {
    record: DeviceRecord,
}

impl StreamEvalSite for ProviderAudioSite {
    fn placement(&self) -> &Placement {
        &self.record.placement
    }

    fn device_record(&self) -> &DeviceRecord {
        &self.record
    }

    fn close(self: Box<Self>) -> Result<()> {
        Ok(())
    }
}
