//! Cataloged host device placement records.

use sim_kernel::Symbol;

use crate::model::{
    HostOpenPlan, stream_host_capability, stream_host_device_read_effect_kind,
    stream_host_device_write_effect_kind,
};

use super::AudioDeviceCard;

/// Placement tier for a host-visible stream device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    /// Deterministic in-process fixture that needs no host hardware.
    Modeled,
    /// Real host hardware behind the named transport.
    Hardware {
        /// Transport responsible for opening the hardware device.
        transport: Symbol,
    },
}

/// Stream-device media family surfaced by the shared device catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    /// Audio PCM device.
    Audio,
    /// MIDI event device.
    Midi,
}

/// Direction supported by a cataloged stream device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceDirection {
    /// Device produces stream input.
    Input,
    /// Device consumes stream output.
    Output,
    /// Device supports input and output.
    Duplex,
}

impl DeviceDirection {
    /// Returns the device effects performed when this catalog direction opens.
    pub fn effect_kinds(self) -> Vec<Symbol> {
        match self {
            Self::Input => vec![stream_host_device_read_effect_kind()],
            Self::Output => vec![stream_host_device_write_effect_kind()],
            Self::Duplex => vec![
                stream_host_device_read_effect_kind(),
                stream_host_device_write_effect_kind(),
            ],
        }
    }
}

/// Catalog row for a host-visible stream device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRecord {
    /// Stable catalog identifier.
    pub id: Symbol,
    /// Human-facing device label.
    pub display_name: String,
    /// Device media family.
    pub kind: DeviceKind,
    /// Device stream direction.
    pub direction: DeviceDirection,
    /// Placement tier for the device.
    pub placement: Placement,
}

impl DeviceRecord {
    /// Builds a modeled MIDI input record.
    pub fn modeled_midi_input(id: &str, name: impl Into<String>) -> Self {
        Self::modeled(id, name, DeviceKind::Midi, DeviceDirection::Input)
    }

    /// Builds a modeled MIDI output record.
    pub fn modeled_midi_output(id: &str, name: impl Into<String>) -> Self {
        Self::modeled(id, name, DeviceKind::Midi, DeviceDirection::Output)
    }

    /// Builds a modeled audio output record.
    pub fn modeled_audio_output(id: &str, name: impl Into<String>) -> Self {
        Self::modeled(id, name, DeviceKind::Audio, DeviceDirection::Output)
    }

    /// Builds a modeled audio record from an audio device card.
    pub fn modeled_audio_from_card(card: &AudioDeviceCard) -> Self {
        Self::audio_from_card(card, Placement::Modeled)
    }

    /// Builds an audio record from an audio device card and explicit placement.
    pub fn audio_from_card(card: &AudioDeviceCard, placement: Placement) -> Self {
        let direction = match (card.channels_in > 0, card.channels_out > 0) {
            (true, true) => DeviceDirection::Duplex,
            (true, false) => DeviceDirection::Input,
            (false, true) | (false, false) => DeviceDirection::Output,
        };
        Self {
            id: card.key.0.clone(),
            display_name: card.display_name.clone(),
            kind: DeviceKind::Audio,
            direction,
            placement,
        }
    }

    /// Builds the host-open authority plan for this cataloged device.
    pub fn open_plan(&self) -> HostOpenPlan {
        HostOpenPlan::new(
            self.plan_backend_symbol(),
            self.id.clone(),
            self.direction.effect_kinds(),
            vec![stream_host_capability()],
        )
    }

    fn modeled(
        id: &str,
        name: impl Into<String>,
        kind: DeviceKind,
        direction: DeviceDirection,
    ) -> Self {
        Self {
            id: Symbol::new(id),
            display_name: name.into(),
            kind,
            direction,
            placement: Placement::Modeled,
        }
    }

    fn plan_backend_symbol(&self) -> Symbol {
        match &self.placement {
            Placement::Modeled => Symbol::qualified("stream/host", "modeled-catalog"),
            Placement::Hardware { transport } => transport.clone(),
        }
    }
}
