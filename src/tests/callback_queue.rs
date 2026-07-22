use std::sync::Arc;

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_core::{
    BackpressureOutcome, BufferPolicy, PcmPacket, PushResult, StreamDirection,
    StreamInspectorStatus, StreamMedia, StreamMetadata, StreamPacket, StreamValue,
    TransportProfile,
};

use crate::{
    FakeBackend, HostBackendRegistry, HostCallbackQueue, HostDirection, HostStreamConfigRequest,
    RtpMidiBackend, fake_backend_symbol,
};

use super::support::{authorized_cx, note_packet};

#[test]
fn host_callback_queue_is_bounded_and_nonblocking() {
    let backend = RtpMidiBackend::new();
    let spec = RtpMidiBackend::source_spec("rtp-midi/test", 1).unwrap();
    let mut cx = authorized_cx();
    let port = backend.open_source(&mut cx, spec).unwrap();

    assert_eq!(
        port.queue()
            .callback_packet(sim_lib_stream_core::StreamPacket::Midi(note_packet(0)))
            .unwrap(),
        PushResult::Accepted
    );
    match port
        .queue()
        .callback_packet(sim_lib_stream_core::StreamPacket::Midi(note_packet(1)))
        .unwrap()
    {
        PushResult::DroppedNewest(item) => {
            assert_eq!(
                item.packet(),
                &sim_lib_stream_core::StreamPacket::Midi(note_packet(1))
            );
        }
        other => panic!("expected dropped newest packet, got {other:?}"),
    }

    let stats = port.queue().stats().unwrap();
    assert_eq!(stats.pushed, 2);
    assert_eq!(stats.dropped_newest, 1);
    let inspector = port
        .queue()
        .inspector(
            Symbol::qualified("stream/route", "host-callback"),
            &TransportProfile::lan_midi_control(),
            vec![Symbol::qualified("stream/diagnostic", "callback-drop")],
        )
        .unwrap();
    assert_eq!(inspector.status, StreamInspectorStatus::BufferOverflow);
    assert_eq!(inspector.queue_depth, 1);
    assert_eq!(inspector.dropped_count, 1);
    assert_eq!(port.queue().drain(8).unwrap().len(), 1);
}

#[test]
fn host_callback_cancel_projects_closed_backpressure() {
    let stream = Arc::new(StreamValue::push(StreamMetadata::new(
        Symbol::new("cancel-callback"),
        StreamMedia::Data,
        StreamDirection::Source,
        Symbol::qualified("clock", "server-frame"),
        BufferPolicy::bounded(2).unwrap(),
    )));
    let queue = HostCallbackQueue::new(Arc::clone(&stream));

    queue.cancel().unwrap();
    let result = queue
        .callback_packet(StreamPacket::data(
            Symbol::qualified("stream/data", "expr"),
            Expr::String("late".to_owned()),
        ))
        .unwrap();

    assert_eq!(result.outcome(), BackpressureOutcome::Closed);
    let stats = queue.stats().unwrap();
    assert!(stats.closed);
    assert!(stats.cancelled);
}

#[test]
fn host_callback_queue_accepts_matching_data_media() {
    let stream = Arc::new(StreamValue::push(StreamMetadata::new(
        Symbol::new("data-callback"),
        StreamMedia::Data,
        StreamDirection::Source,
        Symbol::qualified("clock", "data"),
        BufferPolicy::bounded(2).unwrap(),
    )));
    let queue = HostCallbackQueue::new(Arc::clone(&stream));

    assert_eq!(
        queue
            .callback_packet(StreamPacket::data(
                Symbol::qualified("stream/data", "expr"),
                Expr::String("payload".to_owned()),
            ))
            .unwrap(),
        PushResult::Accepted
    );
    assert!(
        queue
            .callback_packet(StreamPacket::Midi(note_packet(0)))
            .is_err()
    );
    assert_eq!(queue.drain(8).unwrap().len(), 1);
}

#[test]
fn host_callback_queue_rejects_sink_stream_injection() {
    let mut registry = HostBackendRegistry::new();
    registry.register(FakeBackend::new()).unwrap();
    let mut cx = authorized_cx();
    let opened = registry
        .open_checked(
            &mut cx,
            HostStreamConfigRequest::new(
                fake_backend_symbol(),
                Symbol::new("fake/pcm"),
                StreamMedia::Pcm,
                HostDirection::Output,
                BufferPolicy::bounded(2).unwrap(),
            ),
        )
        .unwrap();

    let err = opened
        .queue()
        .callback_packet(StreamPacket::Pcm(PcmPacket::i16(1, 1, vec![0]).unwrap()))
        .unwrap_err();

    assert!(err.to_string().contains("sink stream"));
}
