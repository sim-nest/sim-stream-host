//! Live MIDI stream evaluation-site adapter.

use std::convert::Infallible;

use sim_kernel::{Error, Result, Symbol};
use sim_lib_midi_core::{MidiSink, MidiSource};
use sim_lib_midi_live::{LiveMidiDirection, LiveMidiError, LiveMidiSession};

use crate::eval_site::StreamEvalSite;
use crate::placement::{DeviceDirection, DeviceKind, DeviceRecord, Placement};

/// Live MIDI evaluation site opened from a cataloged device record.
pub struct MidiLiveEvalSite {
    record: DeviceRecord,
    session: LiveMidiSession,
    site: Box<dyn StreamEvalSite>,
}

impl MidiLiveEvalSite {
    /// Wraps an opened catalog site in a live MIDI session.
    pub fn from_eval_site(id: &Symbol, site: Box<dyn StreamEvalSite>) -> Result<Self> {
        let record = site.device_record().clone();
        if record.kind != DeviceKind::Midi {
            site.close()?;
            return Err(Error::Eval(format!("open_live: device '{id}' is not MIDI")));
        }
        let direction = live_direction(record.direction);
        let session = LiveMidiSession::modeled(direction).map_err(live_error)?;
        Ok(Self {
            record,
            session,
            site,
        })
    }

    /// Returns the live MIDI source side.
    pub fn source_mut(&mut self) -> &mut dyn MidiSource<Err = Infallible> {
        self.session.source_mut()
    }

    /// Returns the live MIDI sink side when this site supports output.
    pub fn sink_mut(&mut self) -> Option<&mut dyn MidiSink<Err = Infallible>> {
        self.session.sink_mut()
    }
}

impl StreamEvalSite for MidiLiveEvalSite {
    fn placement(&self) -> &Placement {
        &self.record.placement
    }

    fn device_record(&self) -> &DeviceRecord {
        &self.record
    }

    fn close(self: Box<Self>) -> Result<()> {
        let Self { session, site, .. } = *self;
        session.close().map_err(live_error)?;
        site.close()
    }
}

fn live_direction(direction: DeviceDirection) -> LiveMidiDirection {
    match direction {
        DeviceDirection::Input => LiveMidiDirection::Source,
        DeviceDirection::Output => LiveMidiDirection::Sink,
        DeviceDirection::Duplex => LiveMidiDirection::Duplex,
    }
}

fn live_error(err: LiveMidiError) -> Error {
    Error::Eval(format!("midi live: {err}"))
}
