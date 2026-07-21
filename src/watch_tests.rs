use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Error, Expr, Symbol};

use crate::{
    BoundedContentStore, ContentFrame, RetentionWindow, StoreKey, WatchCapability,
    require_watch_worn_ingest, retention_reason, watch_capability_for_worn_event,
    watch_health_grant, watch_location_grant, watch_mic_grant,
};

#[test]
fn watch_provider_gate_requires_capability_visible_grant_and_session() {
    let health = worn_event("heart-rate");
    let location = worn_event("gps");
    let mic = worn_event("mic-audio");
    assert_eq!(
        watch_capability_for_worn_event(&health).unwrap(),
        WatchCapability::Health
    );
    assert_eq!(
        watch_capability_for_worn_event(&location).unwrap(),
        WatchCapability::Location
    );
    assert_eq!(
        watch_capability_for_worn_event(&mic).unwrap(),
        WatchCapability::Mic
    );

    let session = Symbol::qualified("device/session", "trex");
    let other_session = Symbol::qualified("device/session", "other");
    let grants = vec![
        watch_health_grant(),
        watch_location_grant(),
        watch_mic_grant(),
    ];
    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    for event in [&health, &location, &mic] {
        assert!(matches!(
            require_watch_worn_ingest(&cx, event, &grants, &session, &session),
            Err(Error::CapabilityDenied { .. })
        ));
    }

    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.grant(WatchCapability::Health.capability_name());
    assert!(matches!(
        require_watch_worn_ingest(&cx, &health, &[], &session, &session),
        Err(Error::HostError(message)) if message.contains("visible consent")
    ));
    assert!(matches!(
        require_watch_worn_ingest(&cx, &health, &grants, &other_session, &session),
        Err(Error::HostError(message)) if message.contains("not for this session")
    ));
    assert_eq!(
        require_watch_worn_ingest(&cx, &health, &grants, &session, &session).unwrap(),
        WatchCapability::Health
    );
}

#[test]
fn watch_retention_window_evicts_sensitive_frames_on_modeled_ticks() {
    let session = Symbol::qualified("device/session", "trex");
    let hr = StoreKey::named("watch/hr/sample");
    let location = StoreKey::named("watch/gps/ref");
    let mut store = BoundedContentStore::new(64).unwrap();
    store
        .insert(ContentFrame::new(
            hr.clone(),
            session.clone(),
            8,
            0,
            8,
            Expr::String("heart-rate raw".to_owned()),
        ))
        .unwrap();
    store
        .insert(ContentFrame::new(
            location.clone(),
            session.clone(),
            8,
            0,
            8,
            Expr::String("location raw".to_owned()),
        ))
        .unwrap();

    let window = RetentionWindow::new(session, 8, 1_000);
    assert!(
        store
            .sweep_retention(0, 1, std::slice::from_ref(&window))
            .is_empty()
    );
    assert!(store.contains(&hr));
    assert!(store.contains(&location));

    let evicted = store.sweep_retention(2, 1, &[window]);
    assert!(evicted.iter().any(|item| item.key == hr));
    assert!(evicted.iter().any(|item| item.key == location));
    assert!(evicted.iter().all(|item| item.reason == retention_reason()));
    assert!(store.is_empty());
}

fn worn_event(sensor: &str) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("sample")),
            Expr::Symbol(Symbol::qualified("stream/device-sample", "worn-event")),
        ),
        (
            Expr::Symbol(Symbol::new("sensor")),
            Expr::Symbol(Symbol::qualified("stream/worn-sensor", sensor)),
        ),
    ])
}
