use std::sync::Arc;

use sim_kernel::{CapabilitySet, Cx, DefaultFactory, EagerPolicy, Error, Expr, Result, Symbol};
use sim_lib_view_device::{ConsentReceipt, EdgeId};
use sim_value::build;

use crate::{
    BoundedContentStore, GlassesCapability, glasses_camera_grant, glasses_capability_for_sample,
    glasses_hand_grant, glasses_mic_grant, glasses_pose_grant, glasses_vendor_report_grant,
    glasses_world_anchor_grant, require_glasses_sample_ingest, size_bound_reason,
    store_glasses_frame, sweep_glasses_retention,
};

#[test]
fn glasses_provider_gate_requires_capability_visible_grant_and_session() {
    let samples = [
        (GlassesCapability::Pose, xr_sample("pose")),
        (GlassesCapability::Camera, xr_sample("camera-frame")),
        (GlassesCapability::WorldAnchor, world_anchor()),
        (GlassesCapability::Hand, xr_sample("hand")),
        (GlassesCapability::Mic, xr_sample("mic-chunk")),
        (GlassesCapability::VendorReport, vendor_report()),
    ];
    for (capability, sample) in &samples {
        assert_eq!(glasses_capability_for_sample(sample).unwrap(), *capability);
    }
    assert_eq!(
        GlassesCapability::from_name("glasses/vendor-report"),
        Some(GlassesCapability::VendorReport)
    );

    let session = EdgeId::named("viture-halo");
    let other_session = EdgeId::named("other");
    let receipt = all_glasses_receipt(session.clone(), 7, 1_000);
    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    for capability in GlassesCapability::ALL {
        assert!(matches!(
            cx.require(&capability.capability_name()),
            Err(Error::CapabilityDenied { .. })
        ));
    }
    for (_, sample) in &samples {
        assert!(matches!(
            require_glasses_sample_ingest(&cx, sample, &receipt, &session),
            Err(Error::CapabilityDenied { .. })
        ));
    }

    let granted = GlassesCapability::ALL
        .into_iter()
        .fold(CapabilitySet::new(), |set, capability| {
            set.grant(capability.capability_name())
        });
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.with_capabilities(granted, |cx| -> Result<()> {
        let missing_visible =
            ConsentReceipt::new(Vec::new(), 1_000, Vec::new(), session.clone(), 8);
        assert!(matches!(
            require_glasses_sample_ingest(cx, &samples[1].1, &missing_visible, &session),
            Err(Error::HostError(message)) if message.contains("visible consent")
        ));
        assert!(matches!(
            require_glasses_sample_ingest(cx, &samples[1].1, &receipt, &other_session),
            Err(Error::HostError(message)) if message.contains("not for this session")
        ));
        for (capability, sample) in &samples {
            assert_eq!(
                require_glasses_sample_ingest(cx, sample, &receipt, &session)?,
                *capability
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn glasses_content_store_size_bound_and_retention_evicts_refs() {
    let session = EdgeId::named("viture-halo");
    let receipt = ConsentReceipt::new(
        vec![glasses_camera_grant(), glasses_mic_grant()],
        1_000,
        Vec::new(),
        session,
        10,
    );
    let mut store = BoundedContentStore::new(10).unwrap();

    let camera_ref = Symbol::qualified("xr/camera-frame", "camera-a");
    let mic_ref = Symbol::qualified("xr/mic-chunk", "mic-a");
    let (camera_key, evicted) = store_glasses_frame(
        &mut store,
        GlassesCapability::Camera,
        camera_ref,
        build::text("camera frame"),
        &receipt,
        0,
        6,
    )
    .unwrap();
    assert!(evicted.is_empty());
    assert!(store.contains(&camera_key));

    let (mic_key, evicted) = store_glasses_frame(
        &mut store,
        GlassesCapability::Mic,
        mic_ref,
        build::text("mic frame"),
        &receipt,
        0,
        6,
    )
    .unwrap();
    assert!(evicted.iter().any(|item| item.key == camera_key));
    assert!(
        evicted
            .iter()
            .all(|item| item.reason == size_bound_reason())
    );
    assert!(!store.contains(&camera_key));
    assert!(store.contains(&mic_key));

    let evicted = sweep_glasses_retention(&mut store, std::slice::from_ref(&receipt), 0, 1);
    assert!(evicted.is_empty());
    assert!(store.contains(&mic_key));
    let evicted = sweep_glasses_retention(&mut store, &[receipt], 2, 1);
    assert!(evicted.iter().any(|item| item.key == mic_key));
    assert!(store.is_empty());
}

fn all_glasses_receipt(session: EdgeId, seq: u64, retain_ms: u64) -> ConsentReceipt {
    ConsentReceipt::new(
        vec![
            glasses_pose_grant(),
            glasses_camera_grant(),
            glasses_world_anchor_grant(),
            glasses_hand_grant(),
            glasses_mic_grant(),
            glasses_vendor_report_grant(),
        ],
        retain_ms,
        Vec::new(),
        session,
        seq,
    )
}

fn xr_sample(kind: &str) -> Expr {
    build::map(vec![
        ("kind", build::qsym("stream/device-sample", "record")),
        ("sample", build::qsym("xr", kind)),
    ])
}

fn world_anchor() -> Expr {
    build::map(vec![
        ("kind", build::qsym("glasses", "world-anchor")),
        ("world-anchor", build::qsym("glasses/world-anchor", "desk")),
    ])
}

fn vendor_report() -> Expr {
    build::map(vec![("kind", build::qsym("glasses", "vendor-report"))])
}
