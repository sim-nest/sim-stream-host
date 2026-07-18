//! RTP-MIDI host backend and opened port.

use std::sync::Arc;

use sim_kernel::{Cx, Error, Result, Symbol};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, MidiPacket, StreamEnvelope, StreamItem, StreamMedia, StreamPacket,
    StreamValue, TransportProfile,
};

use crate::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostCallbackQueue, HostClockInfo,
    HostDeviceInventory, HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream,
    HostPortSpec, HostStreamConfig, HostStreamConfigRequest,
};

/// Host backend implementing the selected RTP-MIDI input subset.
///
/// Enumerates and opens MIDI sources without opening sockets during normal
/// validation; host callbacks feed received packets through a bounded queue.
#[derive(Clone, Debug)]
pub struct RtpMidiBackend {
    info: HostBackendInfo,
}

/// An opened RTP-MIDI source port and its callback queue.
pub struct RtpMidiPort {
    spec: HostDeviceSpec,
    queue: HostCallbackQueue,
}

/// Returns the stable symbol identifying the RTP-MIDI backend.
pub fn rtp_midi_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "rtp-midi")
}

impl Default for RtpMidiBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RtpMidiBackend {
    /// Creates a backend advertising MIDI input and offline enumeration.
    pub fn new() -> Self {
        Self {
            info: HostBackendInfo::new(
                rtp_midi_backend_symbol(),
                Symbol::qualified("stream/transport", "rtp-midi"),
                StreamMedia::Midi,
                false,
            )
            .with_capabilities(vec![
                HostBackendCapability::MidiInput,
                HostBackendCapability::Offline,
            ]),
        }
    }

    /// Returns the backend metadata.
    pub fn info(&self) -> &HostBackendInfo {
        &self.info
    }

    /// Builds a MIDI input device spec named `id` with a bounded buffer of
    /// `capacity` packets.
    pub fn source_spec(id: impl Into<String>, capacity: usize) -> Result<HostDeviceSpec> {
        Ok(HostDeviceSpec::new(
            Symbol::new(id.into()),
            rtp_midi_backend_symbol(),
            StreamMedia::Midi,
            HostDirection::Input,
            ClockDomain::MidiTick.symbol(),
            BufferPolicy::bounded(capacity)?,
        ))
    }

    /// Opens a MIDI source port for `spec`.
    ///
    /// Returns an error when the spec belongs to another backend, is not MIDI
    /// media, or is output-only.
    pub fn open_source(&self, cx: &mut Cx, spec: HostDeviceSpec) -> Result<RtpMidiPort> {
        if spec.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "RTP-MIDI backend cannot open {} device specs",
                spec.backend()
            )));
        }
        if spec.media() != StreamMedia::Midi {
            return Err(Error::TypeMismatch {
                expected: "MIDI host device",
                found: "non-MIDI host device",
            });
        }
        if spec.direction() == HostDirection::Output {
            return Err(Error::TypeMismatch {
                expected: "RTP-MIDI input or duplex device",
                found: "output-only host device",
            });
        }
        spec.open_plan().enforce(cx)?;
        let stream = Arc::new(StreamValue::push(spec.metadata()));
        Ok(RtpMidiPort {
            spec,
            queue: HostCallbackQueue::new(stream),
        })
    }
}

impl HostBackend for RtpMidiBackend {
    fn info(&self) -> &HostBackendInfo {
        &self.info
    }

    fn enumerate(&self) -> Result<HostDeviceInventory> {
        let device = Self::source_spec("rtp-midi/default", 8)?;
        let port = HostPortSpec::new(
            Symbol::new("rtp-midi/default/in"),
            device.id().clone(),
            rtp_midi_backend_symbol(),
            StreamMedia::Midi,
            HostDirection::Input,
        );
        Ok(HostDeviceInventory::new(rtp_midi_backend_symbol())
            .with_devices(vec![device])
            .with_ports(vec![port]))
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "RTP-MIDI backend cannot open {} requests",
                request.backend()
            )));
        }
        if request.media() != StreamMedia::Midi {
            return Err(Error::TypeMismatch {
                expected: "MIDI host stream request",
                found: "non-MIDI host stream request",
            });
        }
        if request.direction() == HostDirection::Output {
            return Err(Error::TypeMismatch {
                expected: "RTP-MIDI input or duplex stream request",
                found: "output-only host stream request",
            });
        }
        let clock = HostClockInfo::new(request.clock().clone(), None, true);
        let config = HostStreamConfig::from_request(request, HostLatencyInfo::default(), clock);
        HostOpenStream::new_lan_midi_control(config)
    }
}

impl RtpMidiPort {
    /// Returns the device spec backing this port.
    pub fn spec(&self) -> &HostDeviceSpec {
        &self.spec
    }

    /// Returns the callback queue host callbacks enqueue into.
    pub fn queue(&self) -> &HostCallbackQueue {
        &self.queue
    }

    /// Returns a shared handle to the underlying stream value.
    pub fn stream(&self) -> Arc<StreamValue> {
        self.queue.stream()
    }

    /// Enqueues a MIDI packet received by a host callback.
    pub fn receive_packet_from_callback(&self, packet: MidiPacket) -> Result<()> {
        self.queue
            .callback_packet(StreamPacket::Midi(packet))
            .map(|_| ())
    }

    /// Enqueues a MIDI packet and returns the stream envelope describing it
    /// under the LAN MIDI/control transport profile.
    pub fn receive_envelope_from_callback(
        &self,
        sequence: u64,
        packet: MidiPacket,
    ) -> Result<StreamEnvelope> {
        let item = StreamItem::new(StreamPacket::Midi(packet));
        let envelope = StreamEnvelope::from_item_with_profile(
            &self.spec.metadata(),
            sequence,
            &item,
            TransportProfile::lan_midi_control(),
        )?;
        self.queue.callback_item(item)?;
        Ok(envelope)
    }

    /// Closes the port's callback queue.
    pub fn close(&self) -> Result<()> {
        self.queue.close()
    }
}
