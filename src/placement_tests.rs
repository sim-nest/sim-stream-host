use std::collections::HashMap;

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_core::{
    BufferPolicy, ClockDomain, DomainBridgeKind, LatencyClass, PcmPacket, PlacedFragment,
    RateContract, StreamDirection, StreamItem, StreamMedia, StreamPacket, TransportProfile,
    stream_edge,
};

use crate::{
    AudioDeviceCard, AudioPlacementRequest, AudioRouter, AudioSiteKey, FakeBackend, HostDirection,
    HostStreamConfigRequest, LanPlacementMode, LanPlacementRequest, ModeledAudioSite,
    fake_backend_symbol, lan_bar_delay_mode_symbol, lan_experimental_remote_sample_capability,
    lan_jitter_buffered_mode_symbol, lan_peer_site_symbol,
    lan_pinned_sample_experimental_diagnostic, lan_pinned_sample_refusal_diagnostic,
};

#[test]
fn placement_card_round_trip() {
    let key = AudioSiteKey::new("sim:modeled-stereo");
    let card = AudioDeviceCard::modeled(key.clone(), "Modeled Stereo");

    let mut registry: HashMap<AudioSiteKey, AudioDeviceCard> = HashMap::new();
    registry.insert(key.clone(), card);

    let found = registry.get(&key).unwrap();
    assert_eq!(found.display_name, "Modeled Stereo");
    assert_eq!(found.channels_out, 2);
    assert_eq!(found.channels_in, 2);
    assert_eq!(found.sample_rates, [44_100, 48_000]);
    assert!(!found.hardware_required);
}

#[test]
fn audio_site_router_opens_modeled_backend() {
    let key = AudioSiteKey::new("sim:fake-modeled");
    let card = AudioDeviceCard::modeled(key.clone(), "Fake Modeled Stereo");
    let site = std::sync::Arc::new(ModeledAudioSite::new(
        card,
        std::sync::Arc::new(FakeBackend::new()),
    ));
    let mut router = AudioRouter::new();
    router.register(site);

    assert!(router.site(&key).is_some());
    let opened = router
        .open_placement(AudioPlacementRequest {
            site_key: key,
            stream_request: HostStreamConfigRequest::new(
                fake_backend_symbol(),
                Symbol::new("fake/pcm"),
                StreamMedia::Pcm,
                HostDirection::Output,
                BufferPolicy::bounded(8).unwrap(),
            ),
        })
        .unwrap();

    assert_eq!(opened.config().media(), StreamMedia::Pcm);
    opened.close().unwrap();
}

#[test]
fn audio_router_filters_sites_by_capability() {
    let mono_key = AudioSiteKey::new("sim:test-mono");
    let stereo_key = AudioSiteKey::new("sim:test-stereo");
    let mut router = AudioRouter::new();
    router.register(std::sync::Arc::new(ModeledAudioSite::new(
        AudioDeviceCard {
            key: mono_key,
            display_name: "Mono".to_owned(),
            channels_out: 1,
            channels_in: 0,
            sample_rates: vec![48_000],
            hardware_required: false,
        },
        std::sync::Arc::new(FakeBackend::new()),
    )));
    router.register(std::sync::Arc::new(ModeledAudioSite::new(
        AudioDeviceCard {
            key: stereo_key.clone(),
            display_name: "Stereo".to_owned(),
            channels_out: 2,
            channels_in: 0,
            sample_rates: vec![44_100, 48_000],
            hardware_required: false,
        },
        std::sync::Arc::new(FakeBackend::new()),
    )));

    assert_eq!(router.sites_by_capability(2, &[48_000]), vec![stereo_key]);
    assert!(router.sites_by_capability(1, &[96_000]).is_empty());
}

#[test]
fn lan_peer_inserts_jitter_and_latency_bridges() {
    let fragment = data_fragment(RateContract::control());
    let mode = LanPlacementMode::jitter_buffered(4, 128).unwrap();
    let report = LanPlacementRequest::new(fragment, mode).plan().unwrap();

    assert_eq!(report.site(), &lan_peer_site_symbol());
    assert_eq!(report.mode().symbol(), lan_jitter_buffered_mode_symbol());
    assert_eq!(report.latency_class(), LatencyClass::BufferedPreview);
    assert_eq!(report.bridges()[0].kind(), DomainBridgeKind::JitterBuffer);
    assert_eq!(
        report.bridges()[1].kind(),
        DomainBridgeKind::LatencyCompDelay
    );
    assert_eq!(report.added_bridge_latency().packet_count(), 4);
    assert_eq!(report.added_bridge_latency().frame_count(), 128);
    assert_eq!(
        report.output_envelopes()[0].profile().latency_class(),
        LatencyClass::BufferedPreview
    );
}

#[test]
fn bar_delay_mode_declares_musical_alignment() {
    let fragment = data_fragment(RateContract::midi_tick());
    let mode = LanPlacementMode::bar_delay(2, 120, 2, 64).unwrap();
    let report = LanPlacementRequest::new(fragment, mode).plan().unwrap();

    assert_eq!(report.mode().symbol(), lan_bar_delay_mode_symbol());
    assert_eq!(report.latency_class(), LatencyClass::CollabBarDelay);
    assert_eq!(report.bar_delay_millis(), Some(4_000));
    assert_eq!(report.added_bridge_latency().packet_count(), 2);
    assert_eq!(report.added_bridge_latency().frame_count(), 64);
    assert_eq!(
        report.output_envelopes()[0].profile().latency_class(),
        LatencyClass::CollabBarDelay
    );
}

#[test]
fn pinned_sample_domain_lan_node_requires_experimental_capability() {
    let fragment = sample_fragment();
    let mode = LanPlacementMode::jitter_buffered(3, 256).unwrap();
    let err = LanPlacementRequest::new(fragment, mode)
        .with_realtime_pin(true)
        .plan()
        .unwrap_err();

    assert!(format!("{err}").contains(&lan_pinned_sample_refusal_diagnostic().as_qualified_str()));
}

#[test]
fn experimental_pinned_sample_lan_output_is_not_sample_exact() {
    let fragment = sample_fragment();
    let mode = LanPlacementMode::bar_delay(1, 100, 3, 256).unwrap();
    let report = LanPlacementRequest::new(fragment, mode)
        .with_realtime_pin(true)
        .with_capability(lan_experimental_remote_sample_capability())
        .plan()
        .unwrap();

    assert!(
        report
            .diagnostics()
            .contains(&lan_pinned_sample_experimental_diagnostic())
    );
    assert_ne!(
        report.output_envelopes()[0].profile().latency_class(),
        LatencyClass::SampleExact
    );
    assert_eq!(
        report.output_envelopes()[0].clock_domain(),
        ClockDomain::Sample
    );
}

fn data_fragment(rate_contract: RateContract) -> PlacedFragment {
    let edge = stream_edge(
        "out",
        StreamMedia::Data,
        StreamDirection::Source,
        rate_contract,
    );
    let envelope = edge
        .result_envelope(7, Expr::String("payload".to_owned()))
        .unwrap();
    PlacedFragment::new(Symbol::new("lan-node"), Expr::Bool(true))
        .with_output_edge(edge.with_envelopes(vec![envelope]))
}

fn sample_fragment() -> PlacedFragment {
    let edge = stream_edge(
        "audio",
        StreamMedia::Pcm,
        StreamDirection::Source,
        RateContract::sample_exact(Some(48_000)),
    );
    let item = StreamItem::new(StreamPacket::Pcm(PcmPacket::i16(1, 2, vec![0, 1]).unwrap()));
    let envelope = sim_lib_stream_core::StreamEnvelope::from_item_with_profile(
        edge.metadata(),
        0,
        &item,
        TransportProfile::realtime_local_audio(),
    )
    .unwrap();
    PlacedFragment::new(Symbol::new("sample-node"), Expr::Bool(true))
        .with_output_edge(edge.with_envelopes(vec![envelope]))
}
