use sim_lib_stream_host::{DeviceError, DeviceProvider};
use sim_value::access;

use crate::ble::BlueZLink;
use crate::bringup::{
    BRINGUP_LEDGER_FIXTURE, BringupLedger, BringupRoute, RouteEnableGuard, RouteProof,
};
use crate::import::{ImportFormat, ImportSource};
use crate::stub::watch_stub_provider;
use crate::{WatchProvider, WatchRoute};

#[test]
fn bringup_gates_real_lanes_but_not_ci() {
    let ledger = BringupLedger::default();

    for route in [
        BringupRoute::Ble,
        BringupRoute::Relay,
        BringupRoute::Zepp,
        BringupRoute::Import,
        BringupRoute::WifiLan,
    ] {
        assert!(!ledger.entry(route).verified);
        assert_eq!(ledger.entry(route).proof, None);
    }

    let ble = WatchRoute::Ble(BlueZLink::new("hci0", "trex3pro-48"));
    let err = RouteEnableGuard::enable(&ble, &ledger).expect_err("BLE should require proof");
    match err {
        DeviceError::Host(message) => {
            assert!(message.contains("ble"));
            assert!(message.contains("bring-up proof"));
        }
        other => panic!("expected missing-proof host error, got {other:?}"),
    }

    let import = WatchRoute::Import(ImportSource::new(ImportFormat::Gpx, "runs/2026-07-20.gpx"));
    RouteEnableGuard::enable(&import, &ledger).expect("import stays enabled for CI");

    let stub = WatchRoute::Stub;
    RouteEnableGuard::enable(&stub, &ledger).expect("stub stays enabled for CI");

    let mut verified = ledger.clone();
    verified.verify(
        BringupRoute::Ble,
        "BlueZ hci0 HR/Battery/DeviceInfo/CurrentTime log 2026-07-20",
    );
    RouteEnableGuard::enable(&ble, &verified).expect("verified BLE proof enables BLE");
}

#[test]
fn watch_provider_applies_bringup_guard_to_hardware_routes() {
    let provider = WatchProvider::new(WatchRoute::Ble(BlueZLink::new("hci0", "trex3pro-48")));
    assert!(matches!(provider.open(), Err(DeviceError::Host(message)) if message.contains("ble")));

    let mut ledger = BringupLedger::default();
    ledger.verify(
        BringupRoute::Ble,
        "BlueZ hci0 HR/Battery/DeviceInfo/CurrentTime log 2026-07-20",
    );
    let provider = WatchProvider::new(WatchRoute::Ble(BlueZLink::new("hci0", "trex3pro-48")))
        .with_bringup_ledger(ledger);
    provider.open().expect("verified BLE route should open");
}

#[test]
fn bringup_fixture_freezes_import_proof_without_enabling_hardware() {
    assert!(BRINGUP_LEDGER_FIXTURE.contains(":ble (:verified false"));
    assert!(BRINGUP_LEDGER_FIXTURE.contains(":relay (:verified false"));
    assert!(BRINGUP_LEDGER_FIXTURE.contains(":zepp (:verified false"));
    assert!(BRINGUP_LEDGER_FIXTURE.contains(":import (:verified true"));
    assert!(BRINGUP_LEDGER_FIXTURE.contains(":wifi-lan (:verified false"));

    let ledger = BringupLedger::import_fixture();
    assert_eq!(ledger.ble, RouteProof::default());
    assert_eq!(ledger.relay, RouteProof::default());
    assert_eq!(ledger.zepp, RouteProof::default());
    assert_eq!(ledger.wifi_lan, RouteProof::default());
    assert!(ledger.import.verified);

    let expr = ledger.to_expr();
    assert_eq!(
        access::required_sym(&expr, "kind", "bring-up ledger")
            .unwrap()
            .as_qualified_str(),
        "stream/wristbridge/bringup-ledger"
    );
    assert_eq!(
        access::required_str(&expr, "fixture", "bring-up ledger").unwrap(),
        "bringup/ledger"
    );
    assert!(access::required_map(&expr, "ble", "bring-up ledger").is_ok());
    assert!(access::required_map(&expr, "relay", "bring-up ledger").is_ok());
    assert!(access::required_map(&expr, "zepp", "bring-up ledger").is_ok());
    assert!(access::required_map(&expr, "import", "bring-up ledger").is_ok());
    assert!(access::required_map(&expr, "wifi-lan", "bring-up ledger").is_ok());
}

#[test]
fn default_provider_still_allows_import_and_reports_stub_unsupported() {
    let import = WatchProvider::new(WatchRoute::Import(ImportSource::new(
        ImportFormat::Csv,
        "steps.csv",
    )));
    import
        .open()
        .expect("import route should open without hardware proof");

    assert!(matches!(
        watch_stub_provider().open(),
        Err(DeviceError::Unsupported)
    ));
}
