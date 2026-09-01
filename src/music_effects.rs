//! Individually reviewed effect adapters for the media-edge music vertical.

use crate::{
    DeviceResult, EffectBounds, EffectDescriptor, EffectRegistry, IdempotencePolicy, ReversalPolicy,
};
use sim_kernel::{CapabilityName, Symbol};
use std::time::Duration;

/// Closed set of effects admitted by the music vertical.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusicEffect {
    /// Send one bounded MIDI performance packet.
    MidiSend,
    /// Open one named audio route.
    AudioRouteOpen,
    /// Close a previously opened named audio route.
    AudioRouteClose,
    /// Stop the local music output path without a remote round trip.
    EmergencyStop,
}

impl MusicEffect {
    /// Stable reviewed effect name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::MidiSend => "music-midi-send",
            Self::AudioRouteOpen => "music-audio-route-open",
            Self::AudioRouteClose => "music-audio-route-close",
            Self::EmergencyStop => "music-emergency-stop",
        }
    }
    /// Constructs the complete authority descriptor for this exact adapter.
    pub fn descriptor(self) -> EffectDescriptor {
        let reversal = match self {
            Self::AudioRouteOpen => ReversalPolicy::Effect(Symbol::qualified(
                "device/effect",
                Self::AudioRouteClose.name(),
            )),
            _ => ReversalPolicy::Irreversible,
        };
        EffectDescriptor {
            id: Symbol::qualified("device/effect", self.name()),
            shape: Symbol::qualified("shape/music-effect", self.name()),
            capability: CapabilityName::new(format!("music.effect.{}", self.name())),
            bounds: EffectBounds {
                max_request_bytes: 4096,
                max_invocations: 4096,
            },
            requires_arm: true,
            expires_after: Duration::from_secs(5),
            receipt: Symbol::qualified("music/effect-receipt", self.name()),
            idempotence: IdempotencePolicy::Keyed,
            reversal,
            local_stop: Symbol::qualified("device/effect", Self::EmergencyStop.name()),
        }
    }
}

/// Registry containing every and only the reviewed music effects.
pub fn music_effect_registry() -> DeviceResult<EffectRegistry> {
    EffectRegistry::new(
        [
            MusicEffect::MidiSend,
            MusicEffect::AudioRouteOpen,
            MusicEffect::AudioRouteClose,
            MusicEffect::EmergencyStop,
        ]
        .map(MusicEffect::descriptor),
    )
}
