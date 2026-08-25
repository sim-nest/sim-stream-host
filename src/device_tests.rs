use std::time::Duration;

use sim_kernel::{CapabilityName, Expr, Symbol};

use crate::{
    BoundedContentStore, ContentFrame, DeviceCapability, DeviceError, DevicePlacement,
    DeviceProvider, DeviceSite, EffectBounds, EffectDescriptor, EffectRegistry, EffectRequest,
    IdempotencePolicy, ObservationCassette, ObservationSession, PlacementError, ProviderManifest,
    ProviderTransport, RetentionWindow, ReversalPolicy, StoreKey, StubProvider, retention_reason,
    size_bound_reason,
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

fn descriptor(id: &str) -> EffectDescriptor {
    EffectDescriptor {
        id: Symbol::qualified("device/effect", id),
        shape: Symbol::qualified("shape/device-effect", id),
        capability: CapabilityName::new(format!("device.effect.{id}")),
        bounds: EffectBounds {
            max_request_bytes: 1024,
            max_invocations: 8,
        },
        requires_arm: true,
        expires_after: Duration::from_secs(5),
        receipt: Symbol::qualified("device/receipt", id),
        idempotence: IdempotencePolicy::Keyed,
        reversal: ReversalPolicy::Irreversible,
        local_stop: Symbol::qualified("device/effect", "stop"),
    }
}

fn manifest(effects: Vec<Symbol>) -> ProviderManifest {
    ProviderManifest {
        version: 1,
        id: Symbol::qualified("device/provider", "fixture"),
        transport: ProviderTransport::Cassette,
        profile: "sha256:profile".to_owned(),
        observations: vec![Symbol::qualified("device/sample", "fixture")],
        effects,
        consent: vec![],
        stale_after: Duration::from_secs(30),
        cassette: "sha256:cassette".to_owned(),
        fallback: Symbol::qualified("device/provider", "fixture-read-only"),
    }
}

#[test]
fn manifest_requires_registered_complete_effects_and_hygienic_bounds() {
    let effect = descriptor("fixture");
    let registry = EffectRegistry::new([effect.clone()]).unwrap();
    manifest(vec![effect.id.clone()])
        .validate(&registry)
        .unwrap();

    assert!(matches!(
        manifest(vec![Symbol::qualified("device/effect", "forged")]).validate(&registry),
        Err(DeviceError::Contract(_))
    ));
    let mut stale = manifest(vec![]);
    stale.stale_after = Duration::ZERO;
    assert!(matches!(
        stale.validate(&registry),
        Err(DeviceError::Contract(_))
    ));
    let mut secret = manifest(vec![]);
    secret.cassette = "token=do-not-store".to_owned();
    assert!(matches!(
        secret.validate(&registry),
        Err(DeviceError::Contract(_))
    ));
    let mut incomplete = descriptor("incomplete");
    incomplete.bounds.max_invocations = 0;
    assert!(matches!(
        EffectRegistry::new([incomplete]),
        Err(DeviceError::Contract(_))
    ));

    let mut forged = EffectRequest {
        descriptor: effect.id.clone(),
        payload: Expr::Bool(true),
        arm: Some("armed".to_owned()),
        idempotence_key: Some("fixture-1".to_owned()),
        grants: vec![],
        armed_at_ms: 1,
        invoked_at_ms: 2,
    };
    assert!(matches!(
        effect.authorize(&forged),
        Err(DeviceError::Contract(_))
    ));
    forged.grants.push(effect.capability.clone());
    forged.invoked_at_ms = 10_000;
    assert!(matches!(
        effect.authorize(&forged),
        Err(DeviceError::Contract(_))
    ));
}

#[test]
fn observation_cassette_has_no_runtime_effect_lookup() {
    let cassette =
        ObservationCassette::new(crate::DeviceProfile::modeled_edge(), vec![Expr::Bool(true)]);
    let mut opened = cassette.open().unwrap();
    assert!(opened.effect().is_none());
    assert_eq!(
        opened.observation().poll("device-caps").unwrap(),
        Some(Expr::Bool(true))
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
