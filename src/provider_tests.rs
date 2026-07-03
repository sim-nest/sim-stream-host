use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sim_kernel::{
    AbiVersion, Cx, DefaultFactory, EagerPolicy, Lib, LibLoader, LibManifest, LibSource, LibTarget,
    Linker, LoadCx, LoaderRegistry, Result, Symbol, Version,
};
use sim_lib_stream_core::{BufferPolicy, StreamMedia};

use crate::{
    AudioDeviceCard, AudioPlacementRequest, AudioProviderHost, AudioProviderRegistrar, AudioRouter,
    AudioSiteKey, DeviceCatalog, FakeBackend, HostDirection, HostStreamConfigRequest,
    ModeledAudioSite, fake_backend_symbol, native_audio_provider_capability,
};

fn modeled_provider_symbol() -> Symbol {
    Symbol::qualified("audio/provider", "modeled-fixture")
}

fn modeled_provider_entry(registrar: &mut dyn AudioProviderRegistrar) -> Result<()> {
    let key = AudioSiteKey::new("audio/provider-modeled/stereo-0");
    let card = AudioDeviceCard::modeled(key, "Modeled Provider Stereo 0");
    registrar.register_site(Arc::new(ModeledAudioSite::new(
        card,
        Arc::new(FakeBackend::new()),
    )));
    Ok(())
}

#[derive(Clone)]
struct ModeledProviderLoader {
    loads: Arc<AtomicUsize>,
}

impl ModeledProviderLoader {
    fn new(loads: Arc<AtomicUsize>) -> Self {
        Self { loads }
    }
}

impl LibLoader for ModeledProviderLoader {
    fn can_load(&self, source: &LibSource) -> bool {
        matches!(source, LibSource::Symbol(symbol) if symbol == &modeled_provider_symbol())
    }

    fn load(&self, _cx: &mut Cx, source: LibSource) -> Result<Box<dyn Lib>> {
        assert!(self.can_load(&source));
        self.loads.fetch_add(1, Ordering::SeqCst);
        Ok(Box::new(ModeledProviderLib))
    }
}

struct ModeledProviderLib;

impl Lib for ModeledProviderLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: modeled_provider_symbol(),
            version: Version("0.1.0".to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: Vec::new(),
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
        .with_entry(modeled_provider_symbol(), modeled_provider_entry);

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
    let mut router = AudioRouter::new();
    let mut host = AudioProviderHost::new(&mut cx, &loaders)
        .with_entry(modeled_provider_symbol(), modeled_provider_entry);

    host.load_into(LibSource::Symbol(modeled_provider_symbol()), &mut router)
        .unwrap();

    assert_eq!(loads.load(Ordering::SeqCst), 1);
    let key = AudioSiteKey::new("audio/provider-modeled/stereo-0");
    assert!(router.site(&key).is_some());

    let mut catalog = DeviceCatalog::default_modeled();
    catalog.register_provider_sites(&router);
    let records = catalog.enumerate_audio().unwrap();
    assert!(
        records
            .iter()
            .any(|record| record.id.to_string().contains("provider-modeled"))
    );
}

#[test]
fn missing_provider_entry_degrades_to_modeled_site() {
    let loads = Arc::new(AtomicUsize::new(0));
    let loaders = LoaderRegistry::new().with_loader(ModeledProviderLoader::new(Arc::clone(&loads)));
    let mut cx = test_cx();
    cx.grant(native_audio_provider_capability());
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
    let requested = AudioSiteKey::new("audio/provider-modeled/stereo-0");
    let resolved = router.resolve_or_modeled(&requested, &modeled).unwrap();
    assert_eq!(resolved, modeled);
    let opened = router
        .open_placement(AudioPlacementRequest {
            site_key: resolved,
            stream_request: HostStreamConfigRequest::new(
                fake_backend_symbol(),
                Symbol::new("fake/pcm"),
                StreamMedia::Pcm,
                HostDirection::Output,
                BufferPolicy::bounded(8).unwrap(),
            ),
        })
        .unwrap();
    assert_eq!(opened.config().media(), StreamMedia::Pcm);
    opened.close().unwrap();
}
