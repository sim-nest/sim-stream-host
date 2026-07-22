use sim_kernel::{Expr, Symbol};
use sim_lib_stream_core::{StreamPacket, TransportProfile};

use crate::{FakeBackend, HostBackendRegistry, HostCallbackCassette, HostCallbackReplayReport};

use super::support::authorized_cx;

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

    let report = cassette.replay(opened.queue()).unwrap();

    let replayed = opened.queue().drain(4).unwrap();
    assert_eq!(
        report,
        HostCallbackReplayReport {
            accepted: 2,
            ..HostCallbackReplayReport::default()
        }
    );
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
fn host_callback_cassette_replay_reports_dropped_newest() {
    let mut registry = HostBackendRegistry::new();
    registry.register(FakeBackend::new()).unwrap();
    let mut cx = authorized_cx();
    let opened = registry
        .open_checked(&mut cx, FakeBackend::data_request(1).unwrap())
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

    let report = cassette.replay(opened.queue()).unwrap();

    assert_eq!(
        report,
        HostCallbackReplayReport {
            accepted: 1,
            dropped_newest: 1,
            ..HostCallbackReplayReport::default()
        }
    );
    assert_eq!(opened.queue().drain(4).unwrap().len(), 1);
}

#[test]
fn host_callback_cassette_replay_reports_closed_queue() {
    let mut registry = HostBackendRegistry::new();
    registry.register(FakeBackend::new()).unwrap();
    let mut cx = authorized_cx();
    let opened = registry
        .open_checked(&mut cx, FakeBackend::data_request(2).unwrap())
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

    opened.close().unwrap();
    let report = cassette.replay(opened.queue()).unwrap();

    assert_eq!(
        report,
        HostCallbackReplayReport {
            closed: 2,
            ..HostCallbackReplayReport::default()
        }
    );
}
