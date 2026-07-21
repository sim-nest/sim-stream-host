use sim_kernel::Expr;
use sim_lib_stream_host::{DeviceError, DeviceProvider, DeviceSample, DeviceSession};
use sim_value::{access, build};

use crate::ble::BlueZLink;
use crate::import::ImportSource;
use crate::relay::RelayLink;
use crate::zepp::ZeppBridgeLink;
use crate::{
    WatchCommandKind, WatchProvider, WatchRouteKind, WornEvent, encode_watch_command,
    watch_stub_provider,
};

#[test]
fn import_yields_deterministic_worn_events() {
    let source = ImportSource::csv(
        "sensor,value\nheart-rate,71\ngps,59.1;18.0\nbattery,94\nconnection,paired\n",
    );
    let first = imported_events(&source);
    let second = imported_events(&source);

    assert_eq!(first, second);
    assert_eq!(
        first.iter().map(WornEvent::seq).collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert_eq!(
        first[0].sensor().as_qualified_str(),
        "stream/worn-sensor/heart-rate"
    );
    assert_eq!(
        first[1].sensor().as_qualified_str(),
        "stream/worn-sensor/gps"
    );
}

#[test]
fn stub_returns_unsupported() {
    match WatchProvider::stub().open() {
        Err(DeviceError::Unsupported) => {}
        Err(error) => panic!("expected unsupported watch provider, got {error}"),
        Ok(_) => panic!("expected unsupported watch provider to refuse opening"),
    }
    match watch_stub_provider().open() {
        Err(DeviceError::Unsupported) => {}
        Err(error) => panic!("expected unsupported stub provider, got {error}"),
        Ok(_) => panic!("expected unsupported stub provider to refuse opening"),
    }
}

#[test]
fn watch_command_serializes_for_relay_and_mini_program_bridge() {
    let relay = RelayLink::new("ws://127.0.0.1:9911/watch");
    let zepp = ZeppBridgeLink::new("local-zepp-companion");
    let command = notify_command();

    let relay_packet = relay.command_packet(&command).unwrap();
    assert_eq!(relay_packet.route(), WatchRouteKind::Relay);
    assert_eq!(relay_packet.command(), WatchCommandKind::Notify);
    assert_eq!(
        access::required_sym(&relay.serialize_command(&command).unwrap(), "kind", "relay")
            .unwrap()
            .as_qualified_str(),
        "stream/wristbridge/relay-command"
    );

    let zepp_packet = zepp.command_packet(&command).unwrap();
    assert_eq!(zepp_packet.route(), WatchRouteKind::ZeppBridge);
    assert_eq!(zepp_packet.command(), WatchCommandKind::Notify);
    assert_eq!(
        access::required_sym(&zepp.serialize_command(&command).unwrap(), "kind", "zepp")
            .unwrap()
            .as_qualified_str(),
        "stream/wristbridge/zepp-bridge-command"
    );

    let mut relay_session = WatchProvider::relay(relay).open_session().unwrap();
    relay_session.send(&command).unwrap();
    assert_eq!(
        relay_session.sent_commands()[0].route(),
        WatchRouteKind::Relay
    );

    let mut zepp_session = WatchProvider::zepp_bridge(zepp).open_session().unwrap();
    zepp_session.send(&privacy_command()).unwrap();
    assert_eq!(
        zepp_session.sent_commands()[0].command(),
        WatchCommandKind::PrivacyMode
    );
}

#[test]
fn ble_and_import_sessions_accept_watch_commands() {
    let command = haptic_command();
    let event = WornEvent::from_sensor_name(0, "motion", build::text("raised")).unwrap();
    let mut ble_session = WatchProvider::ble(BlueZLink::with_scripted_events(
        "hci0",
        "AA:BB:CC:DD:EE:FF",
        vec![event],
    ))
    .open_session()
    .unwrap();

    ble_session.send(&command).unwrap();
    assert_eq!(ble_session.sent_commands()[0].route(), WatchRouteKind::Ble);

    let mut import_session = WatchProvider::import(ImportSource::csv("heart-rate,73"))
        .open_session()
        .unwrap();
    import_session.send(&command).unwrap();
    assert_eq!(
        import_session.sent_commands()[0].route(),
        WatchRouteKind::Import
    );
}

fn imported_events(source: &ImportSource) -> Vec<WornEvent> {
    let mut session = WatchProvider::import(source.clone())
        .open_session()
        .unwrap();
    let mut events = Vec::new();
    while let Some(expr) = session.poll(crate::WORN_EVENT_SAMPLE_KIND).unwrap() {
        events.push(WornEvent::from_expr(&expr).unwrap());
    }
    events
}

fn notify_command() -> Expr {
    build::map(vec![
        ("kind", build::qsym("view-wrist", "command")),
        ("command", build::qsym("watch/command", "notify")),
        ("title", build::text("timer")),
        (
            "lines",
            build::list(vec![build::text("done"), build::text("ack")]),
        ),
        ("urgency", build::sym("info")),
    ])
}

fn haptic_command() -> Expr {
    build::map(vec![
        ("kind", build::qsym("view-wrist", "command")),
        ("command", build::qsym("watch/command", "haptic")),
        (
            "pattern",
            build::map(vec![
                ("kind", build::qsym("view-wrist", "haptic-pattern")),
                ("id", build::qsym("watch/haptic", "confirm")),
                (
                    "steps",
                    build::list(vec![build::map(vec![
                        ("on-ms", build::uint(40)),
                        ("off-ms", build::uint(80)),
                    ])]),
                ),
                ("meaning", build::qsym("watch/haptic-meaning", "confirm")),
                ("repeat", build::uint(1)),
            ]),
        ),
    ])
}

fn privacy_command() -> Expr {
    build::map(vec![
        ("kind", build::qsym("view-wrist", "command")),
        ("command", build::qsym("watch/command", "privacy-mode")),
        ("enabled", Expr::Bool(true)),
        ("window-ms", build::uint(60_000)),
    ])
}

#[test]
fn unknown_command_is_rejected() {
    let command = build::map(vec![
        ("kind", build::qsym("view-wrist", "command")),
        ("command", build::qsym("watch/command", "vendor-cloud")),
    ]);
    assert!(encode_watch_command(WatchRouteKind::Relay, &command).is_err());
}
