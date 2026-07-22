use sim_kernel::{Error, Expr, Symbol};
use sim_lib_stream_core::{BufferPolicy, PushResult, StreamMedia, StreamPacket};

use crate::{
    FakeBackend, HostBackendCapability, HostBackendRegistry, HostDeviceSpec, HostDirection,
    RtpMidiBackend, fake_backend_symbol, rtp_midi_backend_symbol, stream_host_capability,
    stream_host_device_read_effect_kind, stream_host_device_write_effect_kind,
};

use super::support::{authorized_cx, field, recorded_effect_kinds, test_cx};

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
