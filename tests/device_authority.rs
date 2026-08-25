use std::time::Duration;

use sim_kernel::{CapabilityName, Expr, Symbol};
use sim_lib_stream_host::{
    DeviceError, DeviceProfile, DeviceProvider, EffectBounds, EffectDescriptor, EffectRegistry,
    EffectRequest, IdempotencePolicy, ObservationCassette, ProviderManifest, ProviderTransport,
    ReversalPolicy,
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
