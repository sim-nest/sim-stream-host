use std::sync::Arc;

use sim_kernel::{CapabilitySet, Cx, DefaultFactory, EagerPolicy, Error, Expr, Symbol};
use sim_lib_scene::{GlanceCard, GlanceMetric};
use sim_lib_stream_device::{DeviceSample, ModeledSource};
use sim_lib_stream_host::{
    BoundedContentStore, DeviceError, DeviceProvider, DeviceSession, GlassesCapability,
};
use sim_lib_stream_xr::{
    ModeledHaloCameraSource, ModeledHaloMicSource, ModeledHaloMotionSource, ModeledHaloTapSource,
    XrCameraFrameRef, XrMicChunkRef, XrPoseSample, XrTapSample,
};
use sim_lib_view_device::{ConsentReceipt, EdgeId};

use crate::{
    BlueZLink, HALO_BUTTON_SAMPLE_KIND, HALO_CAMERA_SAMPLE_KIND, HALO_CAMERA_SIZE_PX,
    HALO_MIC_SAMPLE_KIND, HALO_MOTION_SAMPLE_KIND, HALO_TAP_SAMPLE_KIND, HaloButton,
    HaloButtonSample, HaloCameraPull, HaloProvider, HaloRouteKind, HaloSample, LuaCellPriority,
    LuaFrameBudget, LuaFrameScheduler, RelayLink, WebBluetoothLink, diff_glance,
    halo_camera_store_key, pull_halo_camera_once,
};

#[test]
fn small_change_small_diff_and_overbudget_coalesces() {
    let previous = glance("Temperature 20", "info", false);
    let next = glance("Temperature 21", "info", false);
    let full = diff_glance(&previous, &next, &LuaFrameBudget::new(512).unwrap()).unwrap();
    assert!(full.is_complete());
    assert_eq!(full.cells.len(), 1);
    assert!(full.bytes < 64);

    let urgent = glance("Battery temperature requires inspection", "error", false);
    let mut scheduler = LuaFrameScheduler::from_glance(&previous).unwrap();
    let budget = LuaFrameBudget::new(80).unwrap();
    let first = scheduler.schedule(&urgent, &budget).unwrap();
    assert!(!first.cells.is_empty());
    assert!(!first.deferred.is_empty());
    assert!(first.bytes <= budget.max_bytes_per_tick);
    assert_eq!(first.cells[0].priority(), LuaCellPriority::Urgent);

    let mut ticks = 1;
    let mut diff = first;
    while !diff.is_complete() {
        diff = scheduler.schedule(&urgent, &budget).unwrap();
        ticks += 1;
        assert!(ticks < 128, "deferred Halo cells must make progress");
    }
    assert!(ticks > 1);
}

#[test]
fn provider_routes_open_and_stub_is_unsupported() {
    let sample = HaloSample::Motion(ModeledHaloMotionSource.at(0));
    let routes = [
        (
            HaloProvider::ble(BlueZLink::with_scripted_samples(
                "hci0",
                "halo-local",
                vec![sample.clone()],
            )),
            HaloRouteKind::Ble,
        ),
        (
            HaloProvider::web_bluetooth(WebBluetoothLink::with_scripted_samples(
                "halo-browser",
                vec![sample.clone()],
            )),
            HaloRouteKind::WebBluetooth,
        ),
        (
            HaloProvider::relay(RelayLink::with_scripted_samples(
                "local://halo",
                vec![sample],
            )),
            HaloRouteKind::Relay,
        ),
    ];
    for (provider, expected) in routes {
        assert_eq!(provider.open_session().unwrap().route(), expected);
    }
    assert_eq!(open_error(HaloProvider::stub()), DeviceError::Unsupported);
}

#[test]
fn modeled_tap_round_trips_through_provider() {
    let tap = ModeledHaloTapSource.at(7);
    let provider = HaloProvider::ble(BlueZLink::with_scripted_samples(
        "hci0",
        "halo-local",
        vec![HaloSample::Tap(tap.clone())],
    ));
    let mut session = provider.open_session().unwrap();
    session.start().unwrap();
    let emitted = session.poll(HALO_TAP_SAMPLE_KIND).unwrap().unwrap();
    assert_eq!(XrTapSample::from_expr(&emitted).unwrap(), tap);
    assert!(session.poll(HALO_TAP_SAMPLE_KIND).unwrap().is_none());
}

#[test]
fn provider_publishes_every_advertised_xr_lane() {
    let motion = ModeledHaloMotionSource.at(1);
    let tap = ModeledHaloTapSource.at(2);
    let button = HaloButtonSample::new(3, HaloButton::Secondary, true, 30);
    let mic = ModeledHaloMicSource.at(4);
    let camera = ModeledHaloCameraSource.at(5);
    let provider = HaloProvider::relay(RelayLink::with_scripted_samples(
        "local://halo",
        vec![
            HaloSample::Motion(motion.clone()),
            HaloSample::Tap(tap.clone()),
            HaloSample::Button(button.clone()),
            HaloSample::Mic(mic.clone()),
            HaloSample::Camera(camera.clone()),
        ],
    ));
    let mut session = provider.open_session().unwrap();
    session.start().unwrap();

    let motion_expr = session.poll(HALO_MOTION_SAMPLE_KIND).unwrap().unwrap();
    let tap_expr = session.poll(HALO_TAP_SAMPLE_KIND).unwrap().unwrap();
    let button_expr = session.poll(HALO_BUTTON_SAMPLE_KIND).unwrap().unwrap();
    let mic_expr = session.poll(HALO_MIC_SAMPLE_KIND).unwrap().unwrap();
    let camera_expr = session.poll(HALO_CAMERA_SAMPLE_KIND).unwrap().unwrap();

    assert_eq!(XrPoseSample::from_expr(&motion_expr).unwrap(), motion);
    assert_eq!(XrTapSample::from_expr(&tap_expr).unwrap(), tap);
    assert_eq!(HaloButtonSample::from_expr(&button_expr).unwrap(), button);
    assert_eq!(XrMicChunkRef::from_expr(&mic_expr).unwrap(), mic);
    assert_eq!(XrCameraFrameRef::from_expr(&camera_expr).unwrap(), camera);
    assert!(session.poll(HALO_CAMERA_SAMPLE_KIND).unwrap().is_none());
}

#[test]
fn button_record_round_trips_and_rejects_unknown_fields() {
    let sample = HaloButtonSample::new(4, HaloButton::Primary, true, 99);
    assert_eq!(
        HaloButtonSample::from_expr(&sample.to_expr()).unwrap(),
        sample
    );
    let Expr::Map(mut entries) = sample.to_expr() else {
        panic!("Halo button encodes as a map");
    };
    entries.push((
        Expr::Symbol(Symbol::new("vendor")),
        Expr::String("cloud".to_owned()),
    ));
    assert!(HaloButtonSample::from_expr(&Expr::Map(entries)).is_err());
}

#[test]
fn scene_send_emits_budgeted_lua_diff() {
    let provider = HaloProvider::relay(RelayLink::new("local://halo"))
        .with_frame_budget(LuaFrameBudget::new(96).unwrap());
    let mut session = provider.open_session().unwrap();
    session.start().unwrap();
    let frame = glance("Ready", "info", false);
    session.send(&frame).unwrap();
    let packet = &session.sent_frames()[0];
    assert_eq!(packet.route(), HaloRouteKind::Relay);
    assert!(!packet.lua().is_empty());
    assert_eq!(packet.lua().len(), packet.diff().bytes as usize);
}

#[test]
fn camera_pull_is_one_shot_consent_gated_and_by_reference() {
    let session = EdgeId::named("halo-camera");
    let receipt = ConsentReceipt::new(
        vec![sim_lib_stream_host::glasses_camera_grant()],
        1_000,
        Vec::new(),
        session.clone(),
        17,
    );
    let frame_key = Symbol::qualified("stream/xr-frame", "halo-pull-17");
    let pull =
        HaloCameraPull::new(17, frame_key.clone(), 123, Expr::Bytes(vec![1, 2, 3, 4]), 4).unwrap();
    let mut store = BoundedContentStore::new(32).unwrap();
    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    assert!(matches!(
        pull_halo_camera_once(&cx, &mut store, &receipt, &session, pull.clone(), 0),
        Err(Error::CapabilityDenied { .. })
    ));

    let granted = CapabilitySet::new().grant(GlassesCapability::Camera.capability_name());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.with_capabilities(granted, |cx| {
        let (frame, evicted) = pull_halo_camera_once(cx, &mut store, &receipt, &session, pull, 0)?;
        assert!(evicted.is_empty());
        assert_eq!([frame.width_px(), frame.height_px()], HALO_CAMERA_SIZE_PX);
        assert!(store.contains(&halo_camera_store_key(frame_key)));
        Ok(())
    })
    .unwrap();
}

fn glance(title: &str, urgency: &str, warrant: bool) -> Expr {
    GlanceCard::new(
        title,
        Some(GlanceMetric::new("status", "nominal")),
        None,
        urgency,
        64,
    )
    .with_budget_bypass(warrant)
    .to_scene()
}

fn open_error(provider: HaloProvider) -> DeviceError {
    match provider.open() {
        Ok(_) => panic!("stub should not open"),
        Err(error) => error,
    }
}
