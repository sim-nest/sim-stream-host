//! Modeled audio device provider for the shared device catalog.

use sim_kernel::{Error, Result, Symbol};

use crate::eval_site::{DeviceProvider, StreamEvalSite};
use crate::placement::{AudioDeviceCard, AudioSiteKey, DeviceRecord, Placement};

/// Deterministic audio provider used by default validation.
pub struct ModeledAudioProvider {
    records: Vec<DeviceRecord>,
}

impl Default for ModeledAudioProvider {
    fn default() -> Self {
        let card = AudioDeviceCard::modeled(
            AudioSiteKey::new("audio/model/stereo-0"),
            "Modeled Audio Stereo 0",
        );
        Self {
            records: vec![DeviceRecord::modeled_audio_from_card(&card)],
        }
    }
}

impl DeviceProvider for ModeledAudioProvider {
    fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
        Ok(self.records.clone())
    }

    fn open(&self, id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
        let record = self
            .records
            .iter()
            .find(|record| &record.id == id)
            .ok_or_else(|| Error::Eval(format!("ModeledAudioProvider: unknown id '{id}'")))?
            .clone();
        Ok(Box::new(ModeledAudioEvalSite { record }))
    }
}

struct ModeledAudioEvalSite {
    record: DeviceRecord,
}

impl StreamEvalSite for ModeledAudioEvalSite {
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
