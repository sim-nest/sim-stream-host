//! Loadable audio provider registration seam.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use sim_kernel::{
    CapabilityName, Cx, Error, Export, Lib, LibManifest, LibSource, LoaderRegistry, Result, Symbol,
};

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

/// Proof-only table of provider entry points keyed by provider library id.
///
/// Native provider discovery is driven by `Export::Site` manifest records. This
/// table exists for in-process tests and trusted host-registered proof loaders
/// that need an FFI-free way to call the provider entry symbol.
#[derive(Clone, Default)]
pub struct AudioProviderProofEntries {
    entries: BTreeMap<Symbol, AudioProviderEntry>,
}

impl AudioProviderProofEntries {
    /// Builds an empty proof entry table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a proof provider entry, builder-style.
    pub fn with_proof_entry(mut self, provider: Symbol, entry: AudioProviderEntry) -> Self {
        self.insert(provider, entry);
        self
    }

    /// Adds or replaces a proof provider entry.
    pub fn insert(&mut self, provider: Symbol, entry: AudioProviderEntry) {
        self.entries.insert(provider, entry);
    }

    fn entry(&self, provider: &Symbol) -> Result<AudioProviderEntry> {
        self.entries.get(provider).copied().ok_or_else(|| {
            Error::HostError(format!(
                "audio provider {} has no proof entry for {}",
                provider, AUDIO_PROVIDER_ENTRY_V1
            ))
        })
    }
}

/// Host that loads audio providers through the kernel loader and registers sites.
pub struct AudioProviderHost<'a> {
    cx: &'a mut Cx,
    loaders: &'a LoaderRegistry,
    proof_entries: AudioProviderProofEntries,
}

impl<'a> AudioProviderHost<'a> {
    /// Builds a provider host over a context and loader registry.
    pub fn new(cx: &'a mut Cx, loaders: &'a LoaderRegistry) -> Self {
        Self {
            cx,
            loaders,
            proof_entries: AudioProviderProofEntries::new(),
        }
    }

    /// Adds a proof-only provider entry, builder-style.
    pub fn with_proof_entry(mut self, provider: Symbol, entry: AudioProviderEntry) -> Self {
        self.proof_entries.insert(provider, entry);
        self
    }

    /// Compatibility alias for existing in-process proof loaders.
    ///
    /// New proof code should use [`Self::with_proof_entry`]. Native provider
    /// discovery still requires declared `audio/site` exports before this
    /// entry point is called.
    pub fn with_entry(self, provider: Symbol, entry: AudioProviderEntry) -> Self {
        self.with_proof_entry(provider, entry)
    }

    /// Acquires a provider through the loader registry and registers its sites.
    pub fn load_into(&mut self, source: LibSource, router: &mut AudioRouter) -> Result<()> {
        self.cx.require(&native_audio_provider_capability())?;
        let lib = self.loaders.load_lib(self.cx, source)?;
        register_provider_lib(lib.as_ref(), &self.proof_entries, router)
    }
}

fn register_provider_lib(
    lib: &dyn Lib,
    proof_entries: &AudioProviderProofEntries,
    router: &mut AudioRouter,
) -> Result<()> {
    let manifest = lib.manifest();
    let site_exports = AudioProviderSiteExports::from_manifest(&manifest)?;
    let entry = proof_entries.entry(&manifest.id)?;
    let mut registrar =
        RouterAudioProviderRegistrar::for_provider(router, manifest.id, site_exports.symbols);
    entry(&mut registrar)?;
    registrar.finish()
}

struct AudioProviderSiteExports {
    symbols: BTreeSet<Symbol>,
}

impl AudioProviderSiteExports {
    fn from_manifest(manifest: &LibManifest) -> Result<Self> {
        let mut symbols = BTreeSet::new();
        for export in &manifest.exports {
            if let Export::Site { symbol, .. } = export {
                if symbol.namespace.as_deref() != Some("audio/site") {
                    return Err(Error::HostError(format!(
                        "audio provider {} declared invalid site export {}; expected audio/site",
                        manifest.id, symbol
                    )));
                }
                symbols.insert(symbol.clone());
            }
        }
        if symbols.is_empty() {
            return Err(Error::HostError(format!(
                "audio provider {} declared no audio/site exports",
                manifest.id
            )));
        }
        Ok(Self { symbols })
    }
}

/// Registrar that wires provider sites into an [`AudioRouter`].
pub struct RouterAudioProviderRegistrar<'a> {
    router: &'a mut AudioRouter,
    owner: Symbol,
    allowed_sites: Option<BTreeSet<Symbol>>,
    registration_error: Option<Error>,
}

impl<'a> RouterAudioProviderRegistrar<'a> {
    /// Builds a proof registrar over the supplied audio router.
    pub fn new(router: &'a mut AudioRouter) -> Self {
        Self {
            router,
            owner: Symbol::qualified("audio/provider", "proof"),
            allowed_sites: None,
            registration_error: None,
        }
    }

    /// Builds a provider-owned registrar constrained to declared site exports.
    pub fn for_provider(
        router: &'a mut AudioRouter,
        owner: Symbol,
        allowed_sites: BTreeSet<Symbol>,
    ) -> Self {
        Self {
            router,
            owner,
            allowed_sites: Some(allowed_sites),
            registration_error: None,
        }
    }

    /// Returns the first registration error recorded by this registrar.
    pub fn finish(self) -> Result<()> {
        match self.registration_error {
            Some(err) => Err(err),
            None => Ok(()),
        }
    }

    fn record_error(&mut self, error: Error) {
        if self.registration_error.is_none() {
            self.registration_error = Some(error);
        }
    }
}

impl AudioProviderRegistrar for RouterAudioProviderRegistrar<'_> {
    fn register_site(&mut self, site: Arc<dyn AudioSite>) {
        if self.registration_error.is_some() {
            return;
        }
        let site_symbol = site.key().0.clone();
        if let Some(allowed_sites) = &self.allowed_sites
            && !allowed_sites.contains(&site_symbol)
        {
            self.record_error(Error::HostError(format!(
                "audio provider {} registered undeclared site {}",
                self.owner, site_symbol
            )));
            return;
        }
        if let Err(err) = self.router.register_owned(self.owner.clone(), site) {
            self.record_error(err);
        }
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
            registrar.finish().unwrap();
        }

        assert!(router.site(&key).is_some());
    }
}
