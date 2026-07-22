use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sim_kernel::{
    AbiVersion, Cx, DefaultFactory, EagerPolicy, Export, Lib, LibLoader, LibManifest, LibSource,
    LibTarget, Linker, LoadCx, LoaderRegistry, Result, Symbol, Version,
};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};

use crate::{
    AudioDeviceCard, AudioPlacementRequest, AudioProviderHost, AudioProviderRegistrar, AudioRouter,
    AudioSiteKey, DeviceCatalog, FakeBackend, HostDirection, HostStreamConfigRequest,
    ModeledAudioSite, Placement, audio_site_export_symbol, fake_backend_symbol,
    native_audio_provider_capability, stream_host_capability,
};

fn modeled_provider_symbol() -> Symbol {
    Symbol::qualified("audio/provider", "modeled-fixture")
}

fn modeled_site_symbol() -> Symbol {
    audio_site_export_symbol("provider-modeled-stereo-0")
}

fn modeled_site_key() -> AudioSiteKey {
    AudioSiteKey(modeled_site_symbol())
}

fn modeled_provider_entry(registrar: &mut dyn AudioProviderRegistrar) -> Result<()> {
    let card = AudioDeviceCard {
        key: modeled_site_key(),
        display_name: "Modeled Provider Stereo 0".to_owned(),
        channels_out: 2,
        channels_in: 2,
        sample_rates: vec![44_100, 48_000],
        hardware_required: true,
    };
    registrar.register_site(Arc::new(ModeledAudioSite::new(
        card,
        Arc::new(FakeBackend::new()),
    )));
    Ok(())
}

#[derive(Clone)]
struct ModeledProviderLoader {
    loads: Arc<AtomicUsize>,
    exports: ProviderExports,
}

impl ModeledProviderLoader {
    fn new(loads: Arc<AtomicUsize>) -> Self {
        Self {
            loads,
            exports: ProviderExports::Valid,
        }
    }

    fn with_exports(mut self, exports: ProviderExports) -> Self {
        self.exports = exports;
        self
    }
}

#[derive(Clone, Copy)]
enum ProviderExports {
    Valid,
    Missing,
    InvalidNamespace,
}

impl LibLoader for ModeledProviderLoader {
    fn can_load(&self, source: &LibSource) -> bool {
        matches!(source, LibSource::Symbol(symbol) if symbol == &modeled_provider_symbol())
    }

    fn load(&self, _cx: &mut Cx, source: LibSource) -> Result<Box<dyn Lib>> {
        assert!(self.can_load(&source));
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(ModeledProviderLib {
            exports: self.exports,
        }))
    }
}

struct ModeledProviderLib {
    exports: ProviderExports,
}

impl Lib for ModeledProviderLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: modeled_provider_symbol(),
            version: Version("0.1.0".to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: match self.exports {
                ProviderExports::Valid => vec![Export::Site {
                    symbol: modeled_site_symbol(),
                    runtime_id: None,
                }],
                ProviderExports::Missing => Vec::new(),
                ProviderExports::InvalidNamespace => vec![Export::Site {
                    symbol: Symbol::qualified("audio/device", "provider-modeled-stereo-0"),
                    runtime_id: None,
                }],
            },
        }
    }

    fn load(&self, _cx: &mut LoadCx, _linker: &mut Linker) -> Result<()> {
        Ok(())
    }
}

fn test_cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}

#[test]
fn modeled_provider_load_denied_without_capability() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loaders = LoaderRegistry::new().with_loader(ModeledProviderLoader::new(Arc::clone(&loads)));
    let mut cx = test_cx();
    let mut router = AudioRouter::new();
    let mut host = AudioProviderHost::new(&mut cx, &loaders)
        .with_proof_entry(modeled_provider_symbol(), modeled_provider_entry);

    let err = host
        .load_into(LibSource::Symbol(modeled_provider_symbol()), &mut router)
        .unwrap_err();

    assert!(err.to_string().contains("audio.provider.native"));
    assert_eq!(loads.load(Ordering::SeqCst), 0);
    assert!(router.site_keys().next().is_none());
}

#[test]
fn modeled_provider_registers_site_and_catalog_discovers_it() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loaders = LoaderRegistry::new().with_loader(ModeledProviderLoader::new(Arc::clone(&loads)));
    let mut cx = test_cx();
    cx.grant(native_audio_provider_capability());
    cx.grant(stream_host_capability());
    let mut router = AudioRouter::new();
    let mut host = AudioProviderHost::new(&mut cx, &loaders)
        .with_proof_entry(modeled_provider_symbol(), modeled_provider_entry);

    host.load_into(LibSource::Symbol(modeled_provider_symbol()), &mut router)
        .unwrap();

    assert_eq!(loads.load(Ordering::SeqCst), 1);
    let key = modeled_site_key();
    assert!(router.site(&key).is_some());
    assert_eq!(router.site_owner(&key), Some(&modeled_provider_symbol()));

    let mut catalog = DeviceCatalog::default_modeled();
    catalog.register_provider_sites(&router);
    let records = catalog.enumerate_audio().unwrap();
    let record = records
        .iter()
        .find(|record| record.id == key.0)
        .expect("provider site catalog record");
    assert_eq!(
        record.placement,
        Placement::Hardware {
            transport: modeled_provider_symbol(),
        }
    );
}

#[test]
fn provider_key_collision_is_rejected() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loaders = LoaderRegistry::new().with_loader(ModeledProviderLoader::new(Arc::clone(&loads)));
    let mut cx = test_cx();
    cx.grant(native_audio_provider_capability());
    let mut router = AudioRouter::new();
    router.register(Arc::new(ModeledAudioSite::new(
        AudioDeviceCard::modeled(modeled_site_key(), "Existing Site"),
        Arc::new(FakeBackend::new()),
    )));
    let mut host = AudioProviderHost::new(&mut cx, &loaders)
        .with_proof_entry(modeled_provider_symbol(), modeled_provider_entry);

    let err = host
        .load_into(LibSource::Symbol(modeled_provider_symbol()), &mut router)
        .unwrap_err();

    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert!(err.to_string().contains("already registered"));
    assert_ne!(
        router.site_owner(&modeled_site_key()),
        Some(&modeled_provider_symbol())
    );
}

#[test]
fn provider_unload_removes_owned_sites() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loaders = LoaderRegistry::new().with_loader(ModeledProviderLoader::new(Arc::clone(&loads)));
    let mut cx = test_cx();
    cx.grant(native_audio_provider_capability());
    let mut router = AudioRouter::new();
    let mut host = AudioProviderHost::new(&mut cx, &loaders)
        .with_proof_entry(modeled_provider_symbol(), modeled_provider_entry);

    host.load_into(LibSource::Symbol(modeled_provider_symbol()), &mut router)
        .unwrap();

    assert!(router.site(&modeled_site_key()).is_some());
    assert_eq!(router.unregister_owner(&modeled_provider_symbol()), 1);
    assert!(router.site(&modeled_site_key()).is_none());
}

#[test]
fn missing_site_export_is_rejected() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loaders = LoaderRegistry::new().with_loader(
        ModeledProviderLoader::new(Arc::clone(&loads)).with_exports(ProviderExports::Missing),
    );
    let mut cx = test_cx();
    cx.grant(native_audio_provider_capability());
    let mut router = AudioRouter::new();
    let mut host = AudioProviderHost::new(&mut cx, &loaders)
        .with_proof_entry(modeled_provider_symbol(), modeled_provider_entry);

    let err = host
        .load_into(LibSource::Symbol(modeled_provider_symbol()), &mut router)
        .unwrap_err();

    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert!(err.to_string().contains("no audio/site exports"));
    assert!(router.site_keys().next().is_none());
}

#[test]
fn invalid_site_export_namespace_is_rejected() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loaders = LoaderRegistry::new().with_loader(
        ModeledProviderLoader::new(Arc::clone(&loads))
            .with_exports(ProviderExports::InvalidNamespace),
    );
    let mut cx = test_cx();
    cx.grant(native_audio_provider_capability());
    let mut router = AudioRouter::new();
    let mut host = AudioProviderHost::new(&mut cx, &loaders)
        .with_proof_entry(modeled_provider_symbol(), modeled_provider_entry);

    let err = host
        .load_into(LibSource::Symbol(modeled_provider_symbol()), &mut router)
        .unwrap_err();

    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert!(err.to_string().contains("invalid site export"));
    assert!(router.site_keys().next().is_none());
}

#[test]
fn missing_provider_proof_entry_keeps_modeled_fallback() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loaders = LoaderRegistry::new().with_loader(ModeledProviderLoader::new(Arc::clone(&loads)));
    let mut cx = test_cx();
    cx.grant(native_audio_provider_capability());
    cx.grant(stream_host_capability());
    let modeled = AudioSiteKey::new("audio/modeled/stereo-0");
    let mut router = AudioRouter::new();
    router.register(Arc::new(ModeledAudioSite::new(
        AudioDeviceCard::modeled(modeled.clone(), "Modeled Stereo 0"),
        Arc::new(FakeBackend::new()),
    )));
    let mut host = AudioProviderHost::new(&mut cx, &loaders);

    let err = host
        .load_into(LibSource::Symbol(modeled_provider_symbol()), &mut router)
        .unwrap_err();

    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert!(err.to_string().contains("sim_audio_provider_v1"));
    let requested = modeled_site_key();
    let resolved = router.resolve_or_modeled(&requested, &modeled).unwrap();
    assert_eq!(resolved, modeled);
    let opened = router
        .open_placement_checked(
            &mut cx,
            AudioPlacementRequest {
                site_key: resolved,
                stream_request: HostStreamConfigRequest::new(
                    fake_backend_symbol(),
                    Symbol::new("fake/pcm"),
                    StreamMedia::Pcm,
                    HostDirection::Output,
                    BufferPolicy::bounded(8).unwrap(),
                ),
            },
        )
        .unwrap();
    assert_eq!(opened.config().media(), StreamMedia::Pcm);
    opened.close().unwrap();
}
