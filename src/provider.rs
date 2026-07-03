//! Loadable audio provider registration seam.

use std::collections::BTreeMap;
use std::sync::Arc;

use sim_kernel::{CapabilityName, Cx, Error, Lib, LibSource, LoaderRegistry, Result, Symbol};

use crate::{AudioRouter, AudioSite};

/// Stable entry symbol exported by a loadable audio provider.
///
/// The host resolves this symbol after the kernel loader acquires the provider
/// artifact, then calls the entry point once with an [`AudioProviderRegistrar`].
pub const AUDIO_PROVIDER_ENTRY_V1: &str = "sim_audio_provider_v1";

/// ABI version currently accepted by the host-side audio provider registrar.
pub const AUDIO_PROVIDER_ABI_VERSION: u32 = 1;

/// Capability required before a native audio provider can be loaded.
pub fn native_audio_provider_capability() -> CapabilityName {
    CapabilityName::new("audio.provider.native")
}

/// Host-supplied registration surface for loadable audio providers.
pub trait AudioProviderRegistrar {
    /// Registers or replaces a native audio site owned by the provider.
    fn register_site(&mut self, site: Arc<dyn AudioSite>);

    /// Returns the host ABI version accepted by this registrar.
    fn host_abi_version(&self) -> u32;
}

/// Host-callable audio provider entry point.
pub type AudioProviderEntry = fn(&mut dyn AudioProviderRegistrar) -> Result<()>;

/// FFI-free table of provider entry points keyed by provider library id.
#[derive(Clone, Default)]
pub struct AudioProviderEntries {
    entries: BTreeMap<Symbol, AudioProviderEntry>,
}

impl AudioProviderEntries {
    /// Builds an empty provider entry table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a provider entry, builder-style.
    pub fn with_entry(mut self, provider: Symbol, entry: AudioProviderEntry) -> Self {
        self.insert(provider, entry);
        self
    }

    /// Adds or replaces a provider entry.
    pub fn insert(&mut self, provider: Symbol, entry: AudioProviderEntry) {
        self.entries.insert(provider, entry);
    }

    fn entry(&self, provider: &Symbol) -> Result<AudioProviderEntry> {
        self.entries.get(provider).copied().ok_or_else(|| {
            Error::HostError(format!(
                "audio provider {} did not expose {}",
                provider, AUDIO_PROVIDER_ENTRY_V1
            ))
        })
    }
}

/// Host that loads audio providers through the kernel loader and registers sites.
pub struct AudioProviderHost<'a> {
    cx: &'a mut Cx,
    loaders: &'a LoaderRegistry,
    entries: AudioProviderEntries,
}

impl<'a> AudioProviderHost<'a> {
    /// Builds a provider host over a context and loader registry.
    pub fn new(cx: &'a mut Cx, loaders: &'a LoaderRegistry) -> Self {
        Self {
            cx,
            loaders,
            entries: AudioProviderEntries::new(),
        }
    }

    /// Adds a provider entry, builder-style.
    pub fn with_entry(mut self, provider: Symbol, entry: AudioProviderEntry) -> Self {
        self.entries.insert(provider, entry);
        self
    }

    /// Acquires a provider through the loader registry and registers its sites.
    pub fn load_into(&mut self, source: LibSource, router: &mut AudioRouter) -> Result<()> {
        self.cx.require(&native_audio_provider_capability())?;
        let lib = self.loaders.load_lib(self.cx, source)?;
        register_provider_lib(lib.as_ref(), &self.entries, router)
    }
}

fn register_provider_lib(
    lib: &dyn Lib,
    entries: &AudioProviderEntries,
    router: &mut AudioRouter,
) -> Result<()> {
    let entry = entries.entry(&lib.manifest().id)?;
    let mut registrar = RouterAudioProviderRegistrar::new(router);
    entry(&mut registrar)
}

/// Registrar that wires provider sites into an [`AudioRouter`].
pub struct RouterAudioProviderRegistrar<'a> {
    router: &'a mut AudioRouter,
}

impl<'a> RouterAudioProviderRegistrar<'a> {
    /// Builds a registrar over the supplied audio router.
    pub fn new(router: &'a mut AudioRouter) -> Self {
        Self { router }
    }
}

impl AudioProviderRegistrar for RouterAudioProviderRegistrar<'_> {
    fn register_site(&mut self, site: Arc<dyn AudioSite>) {
        self.router.register(site);
    }

    fn host_abi_version(&self) -> u32 {
        AUDIO_PROVIDER_ABI_VERSION
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        AudioDeviceCard, AudioProviderRegistrar, AudioRouter, AudioSiteKey, FakeBackend,
        ModeledAudioSite, RouterAudioProviderRegistrar,
    };

    #[test]
    fn provider_seam_registers_site_into_router() {
        let key = AudioSiteKey::new("audio/native/jack-spike");
        let card = AudioDeviceCard::modeled(key.clone(), "JACK Provider Spike");
        let site = Arc::new(ModeledAudioSite::new(card, Arc::new(FakeBackend::new())));
        let mut router = AudioRouter::new();

        {
            let mut registrar = RouterAudioProviderRegistrar::new(&mut router);
            assert_eq!(registrar.host_abi_version(), 1);
            registrar.register_site(site);
        }

        assert!(router.site(&key).is_some());
    }
}
