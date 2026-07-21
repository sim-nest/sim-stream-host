use sim_kernel::Expr;
use sim_lib_stream_device::DeviceSample as XrDeviceSample;
use sim_lib_stream_host::{DeviceError, DeviceProvider, DeviceSession};
use sim_lib_stream_xr::{XrPoseSample, XrTrackingStatus};
use sim_value::build;
use sim_viture_ffi::{LegacyImuRate, unsupported_viture_lib};

use crate::{
    VITURE_POSE_SAMPLE_KIND, VitureCommandKind, VitureControlPacket, VitureProvider,
    encode_viture_command, viture_command_symbol, viture_device_profile,
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
    assert!(profile.outputs.contains(&sim_kernel::Symbol::qualified(
        "glasses/output",
        "privacy-film"
    )));
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
