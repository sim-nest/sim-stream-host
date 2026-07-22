use sim_kernel::Symbol;
use sim_lib_stream_core::{BufferPolicy, ClockDomain, StreamMedia, StreamPacket, TransportProfile};

use crate::{HostBackend, HostDirection, HostStreamConfigRequest, RtpMidiBackend};

use super::support::{authorized_cx, note_packet};

#[test]
fn rtp_midi_loopback_callback_uses_lan_control_envelope() {
    let backend = RtpMidiBackend::new();
    let spec = RtpMidiBackend::source_spec("rtp-midi/loopback", 4).unwrap();
    let mut cx = authorized_cx();
    let port = backend.open_source(&mut cx, spec).unwrap();

    let envelope = port
        .receive_envelope_from_callback(9, note_packet(12))
        .unwrap();

    assert_eq!(envelope.stream_id(), port.spec().id());
    assert_eq!(envelope.sequence(), 9);
    assert_eq!(envelope.media(), StreamMedia::Midi);
    assert_eq!(envelope.clock_domain(), ClockDomain::MidiTick);
    assert_eq!(
        envelope.profile().name(),
        TransportProfile::lan_midi_control().name()
    );
    assert!(matches!(envelope.packet(), StreamPacket::Midi(_)));
    assert_eq!(port.queue().drain(8).unwrap().len(), 1);
}

#[test]
fn rtp_midi_backend_open_rejects_unenumerated_device_id() {
    let backend = RtpMidiBackend::new();
    let request = HostStreamConfigRequest::new(
        crate::rtp_midi_backend_symbol(),
        Symbol::new("rtp-midi/unlisted"),
        StreamMedia::Midi,
        HostDirection::Input,
        BufferPolicy::bounded(4).unwrap(),
    )
    .with_clock(ClockDomain::MidiTick.symbol());

    let err = match HostBackend::open(&backend, request) {
        Ok(_) => panic!("RTP-MIDI should reject unenumerated device ids"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("unknown RTP-MIDI device"));
}

#[test]
fn rtp_midi_backend_open_accepts_enumerated_device_id() {
    let backend = RtpMidiBackend::new();
    let opened = HostBackend::open(
        &backend,
        HostStreamConfigRequest::new(
            crate::rtp_midi_backend_symbol(),
            Symbol::new("rtp-midi/default"),
            StreamMedia::Midi,
            HostDirection::Input,
            BufferPolicy::bounded(4).unwrap(),
        )
        .with_clock(ClockDomain::MidiTick.symbol()),
    )
    .unwrap();

    assert_eq!(opened.config().device(), &Symbol::new("rtp-midi/default"));
    opened.close().unwrap();
}

#[test]
#[ignore = "device smoke test requires an operator-provided RTP-MIDI peer"]
fn rtp_midi_device_smoke_test_is_ignored_by_default() {
    let backend = RtpMidiBackend::new();
    let spec = RtpMidiBackend::source_spec("rtp-midi/operator-smoke", 8).unwrap();
    let mut cx = authorized_cx();
    let port = backend.open_source(&mut cx, spec).unwrap();
    port.receive_packet_from_callback(note_packet(0)).unwrap();
    assert_eq!(port.queue().drain(1).unwrap().len(), 1);
}
