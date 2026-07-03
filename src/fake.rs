//! Deterministic host backend used for validation and replay tests.

use sim_kernel::{Error, Result, Symbol};
use sim_lib_stream_core::{BufferPolicy, ClockDomain, StreamMedia};

use crate::{
    HostBackend, HostBackendCapability, HostBackendInfo, HostClockInfo, HostDeviceInventory,
    HostDeviceSpec, HostDirection, HostLatencyInfo, HostOpenStream, HostPortSpec, HostStreamConfig,
    HostStreamConfigRequest,
};

/// Stable fake backend id.
pub fn fake_backend_symbol() -> Symbol {
    Symbol::qualified("stream/host", "fake")
}

/// Host-independent backend that opens push streams without hardware.
#[derive(Clone, Debug)]
pub struct FakeBackend {
    info: HostBackendInfo,
    inventory: HostDeviceInventory,
}

impl Default for FakeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeBackend {
    /// Creates a fake backend with deterministic data, MIDI, and PCM devices.
    pub fn new() -> Self {
        let backend = fake_backend_symbol();
        let data = fake_device("fake/data", StreamMedia::Data, HostDirection::Input, 8);
        let midi = fake_device("fake/midi", StreamMedia::Midi, HostDirection::Input, 8);
        let pcm = fake_device("fake/pcm", StreamMedia::Pcm, HostDirection::Output, 8);
        let ports = vec![
            fake_port(
                "fake/data/in",
                data.id(),
                StreamMedia::Data,
                HostDirection::Input,
            ),
            fake_port(
                "fake/midi/in",
                midi.id(),
                StreamMedia::Midi,
                HostDirection::Input,
            ),
            fake_port(
                "fake/pcm/out",
                pcm.id(),
                StreamMedia::Pcm,
                HostDirection::Output,
            ),
        ];
        Self {
            info: HostBackendInfo::new(
                backend.clone(),
                Symbol::qualified("stream/transport", "fake"),
                StreamMedia::Data,
                false,
            )
            .with_capabilities(vec![
                HostBackendCapability::Fake,
                HostBackendCapability::Offline,
                HostBackendCapability::MidiInput,
                HostBackendCapability::AudioOutput,
            ]),
            inventory: HostDeviceInventory::new(backend)
                .with_devices(vec![data, midi, pcm])
                .with_ports(ports),
        }
    }

    /// Builds a data stream request for the default fake data input device.
    pub fn data_request(capacity: usize) -> Result<HostStreamConfigRequest> {
        Ok(HostStreamConfigRequest::new(
            fake_backend_symbol(),
            Symbol::new("fake/data"),
            StreamMedia::Data,
            HostDirection::Input,
            BufferPolicy::bounded(capacity)?,
        ))
    }
}

impl HostBackend for FakeBackend {
    fn info(&self) -> &HostBackendInfo {
        &self.info
    }

    fn enumerate(&self) -> Result<HostDeviceInventory> {
        Ok(self.inventory.clone())
    }

    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        if request.backend() != self.info.id() {
            return Err(Error::Eval(format!(
                "fake backend cannot open {} requests",
                request.backend()
            )));
        }
        let Some(device) = self
            .inventory
            .devices()
            .iter()
            .find(|device| device.id() == request.device())
        else {
            return Err(Error::Eval(format!(
                "fake backend has no device {}",
                request.device()
            )));
        };
        if device.media() != request.media() {
            return Err(Error::TypeMismatch {
                expected: "request media matching fake device",
                found: "request media for another fake device",
            });
        }
        if device.direction() != request.direction() {
            return Err(Error::TypeMismatch {
                expected: "request direction matching fake device",
                found: "request direction for another fake device",
            });
        }
        let config = HostStreamConfig::from_request(
            request,
            HostLatencyInfo::default(),
            HostClockInfo::new(fake_clock(device.media()), None, true),
        );
        Ok(HostOpenStream::new(config))
    }
}

fn fake_device(
    id: &str,
    media: StreamMedia,
    direction: HostDirection,
    capacity: usize,
) -> HostDeviceSpec {
    HostDeviceSpec::new(
        Symbol::new(id),
        fake_backend_symbol(),
        media,
        direction,
        fake_clock(media),
        BufferPolicy::bounded(capacity).expect("valid fake buffer"),
    )
}

fn fake_clock(media: StreamMedia) -> Symbol {
    match media {
        StreamMedia::Pcm => ClockDomain::Sample.symbol(),
        StreamMedia::Midi => ClockDomain::MidiTick.symbol(),
        StreamMedia::Diagnostic | StreamMedia::Data => ClockDomain::ServerFrame.symbol(),
    }
}

fn fake_port(
    id: &str,
    device: &Symbol,
    media: StreamMedia,
    direction: HostDirection,
) -> HostPortSpec {
    HostPortSpec::new(
        Symbol::new(id),
        device.clone(),
        fake_backend_symbol(),
        media,
        direction,
    )
}
