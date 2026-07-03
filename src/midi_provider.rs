//! Modeled MIDI device provider for the shared device catalog.

use sim_kernel::{Error, Result, Symbol};

use crate::eval_site::{DeviceProvider, StreamEvalSite};
use crate::placement::{DeviceRecord, Placement};

/// Deterministic MIDI provider used by default validation.
pub struct ModeledMidiProvider {
    ports: Vec<DeviceRecord>,
}

impl Default for ModeledMidiProvider {
    fn default() -> Self {
        Self {
            ports: vec![
                DeviceRecord::modeled_midi_input("midi/model/in-0", "Modeled MIDI Input 0"),
                DeviceRecord::modeled_midi_output("midi/model/out-0", "Modeled MIDI Output 0"),
            ],
        }
    }
}

impl DeviceProvider for ModeledMidiProvider {
    fn enumerate(&self) -> Result<Vec<DeviceRecord>> {
        Ok(self.ports.clone())
    }

    fn open(&self, id: &Symbol) -> Result<Box<dyn StreamEvalSite>> {
        let record = self
            .ports
            .iter()
            .find(|record| &record.id == id)
            .ok_or_else(|| Error::Eval(format!("ModeledMidiProvider: unknown id '{id}'")))?
            .clone();
        Ok(Box::new(ModeledMidiEvalSite { record }))
    }
}

struct ModeledMidiEvalSite {
    record: DeviceRecord,
}

impl StreamEvalSite for ModeledMidiEvalSite {
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
