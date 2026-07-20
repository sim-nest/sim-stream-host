use sim_kernel::{Expr, Symbol};

use crate::{
    BoundedContentStore, ContentFrame, DeviceCapability, DeviceError, DevicePlacement,
    DeviceProvider, DeviceSession, DeviceSite, PlacementError, RetentionWindow, StoreKey,
    StubProvider, retention_reason, size_bound_reason,
};

#[test]
fn device_stub_unsupported_and_placement_guard() {
    let profile = crate::DeviceProfile::modeled_edge();
    let provider = StubProvider::new(profile.clone());
    assert_eq!(provider.profile(), &profile);
    assert!(matches!(provider.open(), Err(DeviceError::Unsupported)));

    let mut session = provider.session();
    assert_eq!(session.profile(), provider.profile());
    assert!(matches!(session.start(), Err(DeviceError::Unsupported)));
    assert!(matches!(
        session.poll("device-caps"),
        Err(DeviceError::Unsupported)
    ));
    assert!(matches!(
        session.send(&Expr::Symbol(Symbol::qualified("device/command", "haptic"))),
        Err(DeviceError::Unsupported)
    ));
    session.stop().unwrap();

    let codec = Symbol::qualified("codec", "lisp");
    let encoder = DeviceSite::edge_local(
        Symbol::qualified("device/site", "encoder"),
        provider.profile().clone(),
        codec.clone(),
    );
    let adapter = DeviceSite::remote(
        Symbol::qualified("device/site", "adapter"),
        provider.profile().clone(),
        codec,
    );
    let placement = DevicePlacement::new(encoder, adapter);

    assert_eq!(
        placement.validate(),
        Err(PlacementError::AdapterMustBeEdgeLocal)
    );
}

#[test]
fn content_store_obeys_size_bound_and_retention_reaper() {
    assert_eq!(
        DeviceCapability::Pose.capability_name().as_str(),
        "device/pose"
    );
    assert_eq!(
        DeviceCapability::Pose.grant_symbol(),
        Symbol::qualified("device", "pose")
    );

    let session = Symbol::qualified("device/session", "primary");
    let key_a = StoreKey::named("a");
    let key_b = StoreKey::named("b");
    let key_c = StoreKey::named("c");
    let mut store = BoundedContentStore::new(6).unwrap();

    let evicted = store
        .insert(ContentFrame::new(
            key_a.clone(),
            session.clone(),
            1,
            0,
            4,
            Expr::String("first".to_owned()),
        ))
        .unwrap();
    assert!(evicted.is_empty());

    let evicted = store
        .insert(ContentFrame::new(
            key_b.clone(),
            session.clone(),
            1,
            0,
            4,
            Expr::String("second".to_owned()),
        ))
        .unwrap();
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].key, key_a);
    assert_eq!(evicted[0].reason, size_bound_reason());
    assert!(!store.contains(&key_a));
    assert!(store.contains(&key_b));

    store
        .insert(ContentFrame::new(
            key_c.clone(),
            session.clone(),
            2,
            0,
            2,
            Expr::String("third".to_owned()),
        ))
        .unwrap();
    let evicted = store.sweep_retention(2, 1, &[RetentionWindow::new(session.clone(), 1, 1_000)]);
    assert_eq!(evicted.len(), 2);
    assert!(evicted.iter().any(|item| item.key == key_b));
    assert!(evicted.iter().any(|item| item.key == key_c));
    assert!(evicted.iter().all(|item| item.reason == retention_reason()));
    assert!(store.is_empty());
}
