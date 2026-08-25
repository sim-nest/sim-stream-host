use std::time::Duration;

use sim_kernel::{CapabilityName, Expr, Symbol};
use sim_lib_stream_host::{
    DeviceError, DeviceProfile, DeviceProvider, EffectBounds, EffectDescriptor, EffectRegistry,
    EffectRequest, EffectSession, FakeEffectSession, IdempotencePolicy, MusicEffect,
    ObservationCassette, ObservationSession, ProviderManifest, ProviderTransport, ReversalPolicy,
    music_effect_registry,
};

fn descriptor() -> EffectDescriptor {
    EffectDescriptor {
        id: Symbol::qualified("device/effect", "fixture"),
        shape: Symbol::qualified("shape/device-effect", "fixture"),
        capability: CapabilityName::new("device.effect.fixture"),
        bounds: EffectBounds {
            max_request_bytes: 64,
            max_invocations: 1,
        },
        requires_arm: true,
        expires_after: Duration::from_millis(10),
        receipt: Symbol::qualified("device/receipt", "fixture"),
        idempotence: IdempotencePolicy::Keyed,
        reversal: ReversalPolicy::Irreversible,
        local_stop: Symbol::qualified("device/effect", "stop"),
    }
}

fn music_request(
    effect: MusicEffect,
    key: &str,
    armed_at_ms: u64,
    invoked_at_ms: u64,
) -> EffectRequest {
    let descriptor = effect.descriptor();
    EffectRequest {
        descriptor: descriptor.id,
        payload: Expr::Bool(true),
        arm: Some("operator-arm".into()),
        idempotence_key: Some(key.into()),
        grants: vec![descriptor.capability],
        armed_at_ms,
        invoked_at_ms,
    }
}

#[test]
fn named_music_effects_deduplicate_expire_reverse_and_stop_locally() {
    let registry = music_effect_registry().unwrap();
    assert!(
        registry
            .get(&Symbol::qualified("device/effect", "camera-ptz"))
            .is_none()
    );
    assert!(
        registry
            .get(&Symbol::qualified("device/effect", "printer-start"))
            .is_none()
    );
    let mut fake = FakeEffectSession::new(DeviceProfile::modeled_edge(), registry);
    let first = fake
        .invoke(music_request(
            MusicEffect::AudioRouteOpen,
            "route-1",
            10,
            11,
        ))
        .unwrap();
    let duplicate = fake
        .invoke(music_request(
            MusicEffect::AudioRouteOpen,
            "route-1",
            10,
            12,
        ))
        .unwrap();
    assert_eq!(first, duplicate);
    assert!(
        fake.invoke(music_request(MusicEffect::MidiSend, "expired", 0, 6_000))
            .is_err()
    );
    assert!(matches!(
        MusicEffect::AudioRouteOpen.descriptor().reversal,
        ReversalPolicy::Effect(_)
    ));
    fake.stop().unwrap();
    assert!(
        fake.invoke(music_request(MusicEffect::EmergencyStop, "stop", 20, 21))
            .is_err()
    );
}

#[test]
fn hostile_generic_caller_cannot_escalate_observation_cassette() {
    let provider = ObservationCassette::new(DeviceProfile::modeled_edge(), vec![Expr::Bool(true)]);
    let mut opened = provider.open().unwrap();
    assert!(opened.effect().is_none());
    assert_eq!(
        opened.observation().poll("fixture").unwrap(),
        Some(Expr::Bool(true))
    );
}

#[test]
fn manifest_and_descriptor_fail_closed() {
    let descriptor = descriptor();
    let registry = EffectRegistry::new([descriptor.clone()]).unwrap();
    let mut manifest = ProviderManifest {
        version: 1,
        id: Symbol::qualified("device/provider", "fixture"),
        transport: ProviderTransport::Cassette,
        profile: "sha256:profile".into(),
        observations: vec![],
        effects: vec![descriptor.id.clone()],
        consent: vec![],
        stale_after: Duration::from_secs(1),
        cassette: "sha256:cassette".into(),
        fallback: Symbol::qualified("device/provider", "read-only"),
    };
    manifest.validate(&registry).unwrap();
    manifest.effects = vec![Symbol::qualified("device/effect", "forged")];
    assert!(matches!(
        manifest.validate(&registry),
        Err(DeviceError::Contract(_))
    ));

    let request = EffectRequest {
        descriptor: descriptor.id.clone(),
        payload: Expr::Bool(true),
        arm: Some("armed".into()),
        idempotence_key: Some("one".into()),
        grants: vec![descriptor.capability.clone()],
        armed_at_ms: 1,
        invoked_at_ms: 100,
    };
    assert!(matches!(
        descriptor.authorize(&request),
        Err(DeviceError::Contract(_))
    ));
}
