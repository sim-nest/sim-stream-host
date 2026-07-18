use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Error, Expr, Symbol};
use sim_lib_stream_core::{
    BackpressureOutcome, BufferPolicy, ClockDomain, MidiPacket, MidiPacketEvent, PcmPacket,
    PushResult, StreamDirection, StreamInspectorStatus, StreamMedia, StreamMetadata, StreamPacket,
    StreamValue, TransportProfile,
};

use crate::{
    FakeBackend, HostBackendCapability, HostBackendRegistry, HostCallbackCassette,
    HostCallbackQueue, HostClockInfo, HostDeviceSpec, HostDirection, HostLatencyInfo,
    HostOpenStream, HostStreamConfig, HostStreamConfigRequest, HostStreamDriver, RtpMidiBackend,
    fake_backend_symbol, rtp_midi_backend_symbol, stream_host_capability,
    stream_host_device_read_effect_kind, stream_host_device_write_effect_kind,
};

#[test]
fn rtp_midi_backend_card_is_host_independent_and_bounded() {
    let backend = RtpMidiBackend::new();
    assert_eq!(backend.info().id(), &rtp_midi_backend_symbol());
    assert_eq!(backend.info().media(), StreamMedia::Midi);
    assert!(!backend.info().hardware_required());
    assert!(backend.info().callbacks_bounded());

    let card = backend.info().card_expr();
    assert_eq!(
        field(&card, "kind"),
        Some(&Expr::Symbol(Symbol::qualified("stream", "host-backend")))
    );
    assert_eq!(
        field(&card, "transport"),
        Some(&Expr::Symbol(Symbol::qualified(
            "stream/transport",
            "rtp-midi"
        )))
    );
}

#[test]
fn host_backend_registry_opens_fake_stream_without_hardware() {
    let mut registry = HostBackendRegistry::new();
    registry.register(FakeBackend::new()).unwrap();

    let inventories = registry.enumerate().unwrap();
    assert_eq!(inventories.len(), 1);
    assert!(
        inventories[0]
            .devices()
            .iter()
            .any(|device| device.id() == &Symbol::new("fake/data"))
    );

    let denied = match registry.open_checked(&mut test_cx(), FakeBackend::data_request(2).unwrap())
    {
        Ok(_) => panic!("host stream open should require stream.host"),
        Err(err) => err,
    };
    assert!(matches!(
        denied,
        Error::CapabilityDenied { capability } if capability == stream_host_capability()
    ));

    let mut cx = authorized_cx();
    let opened = registry
        .open_checked(&mut cx, FakeBackend::data_request(2).unwrap())
        .unwrap();
    assert_eq!(opened.config().backend(), &fake_backend_symbol());
    assert_eq!(
        opened
            .queue()
            .callback_packet(StreamPacket::data(
                Symbol::qualified("stream/data", "expr"),
                Expr::String("from fake".to_owned()),
            ))
            .unwrap(),
        PushResult::Accepted
    );
    assert_eq!(opened.queue().drain(4).unwrap().len(), 1);
}

#[test]
fn host_backend_registry_records_duplex_read_and_write_effects() {
    let mut registry = HostBackendRegistry::new();
    registry.register(FakeBackend::new()).unwrap();
    let request = FakeBackend::duplex_data_request(2).unwrap();
    let plan = registry.plan_open(&request).unwrap();
    assert_eq!(
        plan.effect_kinds(),
        &[
            stream_host_device_read_effect_kind(),
            stream_host_device_write_effect_kind()
        ]
    );

    let mut cx = authorized_cx();
    let opened = registry.open_checked(&mut cx, request).unwrap();

    assert_eq!(opened.config().direction(), HostDirection::Duplex);
    assert_eq!(
        recorded_effect_kinds(&cx),
        vec![
            stream_host_device_read_effect_kind(),
            stream_host_device_write_effect_kind(),
        ]
    );
}

#[test]
fn host_backend_registry_rejects_duplicate_backend_ids() {
    let mut registry = HostBackendRegistry::new();
    registry.register(FakeBackend::new()).unwrap();

    assert!(registry.register(FakeBackend::new()).is_err());
    assert!(registry.backend(&fake_backend_symbol()).is_some());
}

#[test]
fn host_callback_cassette_replays_callback_timeline() {
    let mut registry = HostBackendRegistry::new();
    registry.register(FakeBackend::new()).unwrap();
    let mut cx = authorized_cx();
    let opened = registry
        .open_checked(&mut cx, FakeBackend::data_request(4).unwrap())
        .unwrap();
    let mut cassette = HostCallbackCassette::new();
    cassette.record_packet(StreamPacket::data(
        Symbol::qualified("stream/data", "expr"),
        Expr::String("first".to_owned()),
    ));
    cassette.record_packet(StreamPacket::data(
        Symbol::qualified("stream/data", "expr"),
        Expr::String("second".to_owned()),
    ));

    let shared = cassette
        .to_stream_cassette(
            opened.config().metadata(),
            TransportProfile::remote_stream_fabric(),
        )
        .unwrap();
    let cassette = HostCallbackCassette::from_stream_cassette(&shared).unwrap();

    cassette.replay(opened.queue()).unwrap();

    let replayed = opened.queue().drain(4).unwrap();
    assert_eq!(replayed.len(), 2);
    assert_eq!(
        replayed[0].packet(),
        &StreamPacket::data(
            Symbol::qualified("stream/data", "expr"),
            Expr::String("first".to_owned()),
        )
    );
}

#[test]
fn host_browse_cards_cover_backend_devices_ports_and_missing_capabilities() {
    let mut registry = HostBackendRegistry::new();
    registry.register(FakeBackend::new()).unwrap();

    let cards = registry.card_exprs().unwrap();
    assert!(cards.iter().any(|card| {
        field(card, "kind") == Some(&Expr::Symbol(Symbol::qualified("stream", "host-backend")))
    }));
    assert!(cards.iter().any(|card| {
        field(card, "kind") == Some(&Expr::Symbol(Symbol::qualified("stream", "host-device")))
    }));
    assert!(cards.iter().any(|card| {
        field(card, "kind") == Some(&Expr::Symbol(Symbol::qualified("stream", "host-port")))
    }));

    let missing =
        registry.missing_capability_card(&fake_backend_symbol(), HostBackendCapability::Hotplug);
    assert_eq!(
        field(&missing, "kind"),
        Some(&Expr::Symbol(Symbol::qualified(
            "stream",
            "host-missing-capability"
        )))
    );
}

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

#[test]
fn realtime_local_audio_opening_rejects_wrong_media_and_clock() {
    let config = realtime_audio_config(StreamMedia::Pcm, ClockDomain::Sample);
    let opened = HostOpenStream::new_realtime_local_audio(config).unwrap();
    assert_eq!(opened.config().media(), StreamMedia::Pcm);

    let err = match HostOpenStream::new_realtime_local_audio(realtime_audio_config(
        StreamMedia::Data,
        ClockDomain::Sample,
    )) {
        Ok(_) => panic!("data stream should not open as realtime audio"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("PCM"));

    let err = match HostOpenStream::new_realtime_local_audio(realtime_audio_config(
        StreamMedia::Pcm,
        ClockDomain::Wall,
    )) {
        Ok(_) => panic!("wall-clock stream should not open as realtime audio"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("sample clock"));
}

#[test]
fn realtime_local_audio_close_shutdowns_attached_driver() {
    let shutdowns = Arc::new(AtomicUsize::new(0));
    let driver: Rc<dyn HostStreamDriver> = Rc::new(CountingDriver {
        shutdowns: Arc::clone(&shutdowns),
    });
    let opened = HostOpenStream::new_realtime_local_audio_with_driver(
        realtime_audio_config(StreamMedia::Pcm, ClockDomain::Sample),
        driver,
    )
    .unwrap();

    opened.close().unwrap();

    assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
}

#[test]
fn lan_midi_control_opening_rejects_wrong_media_and_clock() {
    let opened = HostOpenStream::new_lan_midi_control(lan_midi_config(
        StreamMedia::Midi,
        ClockDomain::MidiTick,
    ))
    .unwrap();
    assert_eq!(opened.config().media(), StreamMedia::Midi);

    let err = match HostOpenStream::new_lan_midi_control(lan_midi_config(
        StreamMedia::Pcm,
        ClockDomain::MidiTick,
    )) {
        Ok(_) => panic!("PCM stream should not open as LAN MIDI/control"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("MIDI media"));

    let err = match HostOpenStream::new_lan_midi_control(lan_midi_config(
        StreamMedia::Midi,
        ClockDomain::Wall,
    )) {
        Ok(_) => panic!("wall-clock stream should not open as LAN MIDI/control"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("MIDI tick or control clock"));
}

#[test]
fn host_open_plan_uses_device_effect_and_stream_capability() {
    let spec = RtpMidiBackend::source_spec("rtp-midi/effect", 4).unwrap();
    let plan = spec.open_plan();
    assert_eq!(plan.backend(), &rtp_midi_backend_symbol());
    assert_eq!(plan.device(), spec.id());
    assert_eq!(
        plan.effect_kinds(),
        &[stream_host_device_read_effect_kind()]
    );
    assert_eq!(plan.requires(), &[stream_host_capability()]);
}

fn test_cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}

fn authorized_cx() -> Cx {
    let mut cx = test_cx();
    cx.grant(stream_host_capability());
    cx
}

fn recorded_effect_kinds(cx: &Cx) -> Vec<Symbol> {
    cx.effect_ledger()
        .records()
        .iter()
        .filter_map(|record| cx.effect_ledger().effect(&record.effect))
        .map(|effect| effect.kind.clone())
        .collect()
}

fn realtime_audio_config(media: StreamMedia, clock_domain: ClockDomain) -> HostStreamConfig {
    let clock = clock_domain.symbol();
    let request = HostStreamConfigRequest::new(
        fake_backend_symbol(),
        Symbol::new("fake/pcm"),
        media,
        HostDirection::Output,
        BufferPolicy::bounded(4).unwrap(),
    )
    .with_clock(clock.clone());
    HostStreamConfig::from_request(
        request,
        HostLatencyInfo::default(),
        HostClockInfo::new(clock, Some(48_000), true),
    )
}

fn lan_midi_config(media: StreamMedia, clock_domain: ClockDomain) -> HostStreamConfig {
    let clock = clock_domain.symbol();
    let request = HostStreamConfigRequest::new(
        rtp_midi_backend_symbol(),
        Symbol::new("rtp-midi/config"),
        media,
        HostDirection::Input,
        BufferPolicy::bounded(4).unwrap(),
    )
    .with_clock(clock.clone());
    HostStreamConfig::from_request(
        request,
        HostLatencyInfo::default(),
        HostClockInfo::new(clock, None, true),
    )
}

struct CountingDriver {
    shutdowns: Arc<AtomicUsize>,
}

impl HostStreamDriver for CountingDriver {
    fn shutdown(&self) -> sim_kernel::Result<()> {
        self.shutdowns.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn rtp_midi_rejects_wrong_backend_or_media() {
    let backend = RtpMidiBackend::new();
    let mut cx = authorized_cx();
    let wrong_backend = HostDeviceSpec::new(
        Symbol::new("bad"),
        Symbol::qualified("stream/host", "alsa"),
        StreamMedia::Midi,
        HostDirection::Input,
        Symbol::qualified("clock", "midi"),
        BufferPolicy::bounded(2).unwrap(),
    );
    assert!(backend.open_source(&mut cx, wrong_backend).is_err());

    let wrong_media = HostDeviceSpec::new(
        Symbol::new("audio"),
        rtp_midi_backend_symbol(),
        StreamMedia::Pcm,
        HostDirection::Input,
        Symbol::qualified("clock", "audio"),
        BufferPolicy::bounded(2).unwrap(),
    );
    assert!(backend.open_source(&mut cx, wrong_media).is_err());
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

fn note_packet(ticks: i64) -> MidiPacket {
    MidiPacket::new(vec![
        MidiPacketEvent::new(ticks, 480, vec![0x90, 60, 100]).unwrap(),
    ])
    .unwrap()
}

fn field<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries.iter().find_map(|(key, value)| match key {
        Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == name => {
            Some(value)
        }
        _ => None,
    })
}
