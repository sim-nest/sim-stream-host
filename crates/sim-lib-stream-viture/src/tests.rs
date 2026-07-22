use std::sync::Arc;

use sim_kernel::{CapabilitySet, Cx, DefaultFactory, EagerPolicy, Error, Expr, Symbol};
use sim_lib_stream_device::DeviceSample as XrDeviceSample;
use sim_lib_stream_host::{DeviceError, DeviceProvider, DeviceSession};
use sim_lib_stream_xr::{XrCameraFrameRef, XrPoseSample, XrTrackingStatus};
use sim_lib_view_device::{ConsentReceipt, EdgeId};
use sim_value::build;
use sim_viture_ffi::{LegacyImuRate, VitureStatus, unsupported_viture_lib};

use crate::{
    VITURE_CAMERA_SAMPLE_KIND, VITURE_POSE_SAMPLE_KIND, VitureCameraFrame, VitureCameraKind,
    VitureCommandKind, VitureControlPacket, VitureProvider, encode_viture_command,
    store_viture_camera_frame, viture_camera_sample_kind_symbol, viture_camera_store_key,
    viture_command_symbol, viture_device_profile, viture_tracking_status,
};

#[test]
fn no_device_returns_unsupported() {
    assert_eq!(open_error(VitureProvider::stub()), DeviceError::Unsupported);
    assert_eq!(
        open_error(VitureProvider::unsupported_sdk(1_000)),
        DeviceError::Unsupported
    );
    assert_eq!(
        open_error(VitureProvider::legacy_imu(
            unsupported_viture_lib(),
            LegacyImuRate::new(120).unwrap()
        )),
        DeviceError::Unsupported
    );
}

#[test]
fn scripted_pose_session_emits_monotone_xr_pose() {
    let samples = vec![pose_sample(10), pose_sample(11)];
    let provider = VitureProvider::scripted(samples);
    let mut session = provider.open_session().unwrap();

    assert!(
        session
            .profile()
            .supports_sample_kind(&sim_lib_stream_xr::xr_pose_sample_kind_symbol())
    );
    session.start().unwrap();

    let first = session.poll(VITURE_POSE_SAMPLE_KIND).unwrap().unwrap();
    let second = session.poll(VITURE_POSE_SAMPLE_KIND).unwrap().unwrap();
    assert!(session.poll(VITURE_POSE_SAMPLE_KIND).unwrap().is_none());
    assert!(session.poll("xr/hand").unwrap().is_none());

    let first = XrPoseSample::from_expr(&first).unwrap();
    let second = XrPoseSample::from_expr(&second).unwrap();
    assert_eq!(first.seq(), 10);
    assert_eq!(second.seq(), 11);
    assert!(first.seq() < second.seq());
}

#[test]
fn scripted_camera_session_emits_frame_refs() {
    let frame = camera_ref(3);
    let provider = VitureProvider::scripted_with_camera_frames(Vec::new(), vec![frame.clone()]);
    let mut session = provider.open_session().unwrap();

    assert!(
        session
            .profile()
            .supports_sample_kind(&viture_camera_sample_kind_symbol())
    );
    session.start().unwrap();

    let emitted = session.poll(VITURE_CAMERA_SAMPLE_KIND).unwrap().unwrap();
    assert!(session.poll(VITURE_CAMERA_SAMPLE_KIND).unwrap().is_none());
    assert_eq!(XrCameraFrameRef::from_expr(&emitted).unwrap(), frame);
}

#[test]
fn camera_frame_store_is_consent_gated_and_by_reference() {
    let session = EdgeId::named("viture-camera");
    let other_session = EdgeId::named("other");
    let receipt = ConsentReceipt::new(
        vec![sim_lib_stream_host::glasses_camera_grant()],
        1_000,
        Vec::new(),
        session.clone(),
        9,
    );
    let frame = camera_frame(7, 8);
    let mut store = sim_lib_stream_host::BoundedContentStore::new(64).unwrap();
    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));

    assert!(matches!(
        store_viture_camera_frame(&cx, &mut store, &receipt, &session, frame.clone(), 0),
        Err(Error::CapabilityDenied { .. })
    ));

    let granted = CapabilitySet::new()
        .grant(sim_lib_stream_host::GlassesCapability::Camera.capability_name());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.with_capabilities(granted, |cx| {
        assert!(matches!(
            store_viture_camera_frame(cx, &mut store, &receipt, &other_session, frame.clone(), 0),
            Err(Error::HostError(message)) if message.contains("not for this session")
        ));
        let (sample, evicted) =
            store_viture_camera_frame(cx, &mut store, &receipt, &session, frame.clone(), 0)?;
        assert!(evicted.is_empty());
        assert_eq!(sample.frame_key(), &frame.frame_key);
        assert!(store.contains(&viture_camera_store_key(frame.frame_key.clone())));
        Ok(())
    })
    .unwrap();
}

#[test]
fn camera_frame_reaper_evicts_expired_ref() {
    let session = EdgeId::named("viture-camera");
    let receipt = ConsentReceipt::new(
        vec![sim_lib_stream_host::glasses_camera_grant()],
        1_000,
        Vec::new(),
        session.clone(),
        11,
    );
    let frame = camera_frame(12, 6);
    let key = viture_camera_store_key(frame.frame_key.clone());
    let mut store = sim_lib_stream_host::BoundedContentStore::new(64).unwrap();
    let granted = CapabilitySet::new()
        .grant(sim_lib_stream_host::GlassesCapability::Camera.capability_name());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));

    cx.with_capabilities(granted, |cx| {
        store_viture_camera_frame(cx, &mut store, &receipt, &session, frame, 0)?;
        Ok(())
    })
    .unwrap();
    assert!(store.contains(&key));

    let early = sim_lib_stream_host::sweep_glasses_retention(
        &mut store,
        std::slice::from_ref(&receipt),
        0,
        1,
    );
    assert!(early.is_empty());
    let evicted = sim_lib_stream_host::sweep_glasses_retention(&mut store, &[receipt], 2, 1);
    assert!(evicted.iter().any(|item| item.key == key));
    assert!(store.is_empty());
}

#[test]
fn viture_pose_status_maps_to_xr_tracking_status() {
    assert_eq!(
        viture_tracking_status(VitureStatus::from_code(0).unwrap()),
        XrTrackingStatus::Tracked
    );
}

#[test]
fn control_packets_validate_display_and_privacy_commands() {
    let display = encode_viture_command(&command(
        VitureCommandKind::Display3d,
        vec![("enabled", Expr::Bool(true))],
    ))
    .unwrap();
    assert_eq!(display, VitureControlPacket::Display3d { enabled: true });

    let privacy = encode_viture_command(&command(
        VitureCommandKind::PrivacyFilm,
        vec![("level", build::uint(66))],
    ))
    .unwrap();
    assert_eq!(privacy, VitureControlPacket::PrivacyFilm { level: 66 });
    assert_eq!(
        privacy.to_expr(),
        build::map(vec![
            ("kind", build::qsym("stream/viture", "control-packet")),
            (
                "command",
                Expr::Symbol(viture_command_symbol(VitureCommandKind::PrivacyFilm)),
            ),
            ("level", build::uint(66)),
        ])
    );

    let too_bright = encode_viture_command(&command(
        VitureCommandKind::Brightness,
        vec![("level", build::uint(101))],
    ))
    .unwrap_err();
    assert!(too_bright.to_string().contains("0 through 100"));
}

#[test]
fn scripted_session_records_display_commands_without_hardware() {
    let mut session = VitureProvider::scripted(vec![pose_sample(1)])
        .open_session()
        .unwrap();

    session
        .send(&command(
            VitureCommandKind::Display3d,
            vec![("enabled", Expr::Bool(false))],
        ))
        .unwrap();
    session
        .send(&command(
            VitureCommandKind::Brightness,
            vec![("level", build::uint(40))],
        ))
        .unwrap();

    assert_eq!(
        session.sent_commands(),
        &[
            VitureControlPacket::Display3d { enabled: false },
            VitureControlPacket::Brightness { level: 40 },
        ]
    );
}

#[test]
fn viture_profile_advertises_xr_pose_and_controls() {
    let profile = viture_device_profile();

    assert_eq!(
        profile.device,
        sim_kernel::Symbol::qualified("device", "viture-glasses")
    );
    assert!(profile.supports_sample_kind(&sim_lib_stream_xr::xr_pose_sample_kind_symbol()));
    assert!(profile.supports_sample_kind(&viture_camera_sample_kind_symbol()));
    assert!(profile.outputs.contains(&sim_kernel::Symbol::qualified(
        "glasses/output",
        "privacy-film"
    )));
}

fn camera_ref(seq: u64) -> XrCameraFrameRef {
    XrCameraFrameRef::new(
        seq,
        Symbol::qualified("device", "viture-glasses"),
        Symbol::qualified("stream/xr-camera", "viture-stereo-vio"),
        Symbol::qualified("stream/content", format!("viture-frame-{seq}")),
        [1280, 720],
        seq * 8_333_333,
        true,
    )
    .unwrap()
}

fn camera_frame(seq: u64, size_bytes: usize) -> VitureCameraFrame {
    VitureCameraFrame::new(
        seq,
        VitureCameraKind::UvcRgb,
        Symbol::qualified("stream/content", format!("viture-uvc-{seq}")),
        [640, 480],
        seq * 8_333_333,
        build::text(format!("camera bytes {seq}")),
        size_bytes,
    )
    .unwrap()
}

fn pose_sample(seq: u64) -> XrPoseSample {
    XrPoseSample::new(
        seq,
        Some([1.0, 2.0, 3.0]),
        [1.0, 0.0, 0.0, 0.0],
        seq * 100,
        1_000,
        6,
        XrTrackingStatus::Tracked,
    )
    .unwrap()
}

fn command(kind: VitureCommandKind, fields: Vec<(&str, Expr)>) -> Expr {
    let mut entries = vec![
        ("kind", build::qsym("view-viture", "command")),
        ("command", Expr::Symbol(viture_command_symbol(kind))),
    ];
    entries.extend(fields);
    build::map(entries)
}

fn open_error(provider: VitureProvider) -> DeviceError {
    match provider.open() {
        Ok(_) => panic!("VITURE provider unexpectedly opened"),
        Err(error) => error,
    }
}
