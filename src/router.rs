//! Audio placement router for registered audio sites.

use std::collections::HashMap;
use std::sync::Arc;

use sim_kernel::{Cx, Error, Result, Symbol};

use crate::placement::{AudioPlacementRequest, AudioSiteKey};
use crate::site::AudioSite;
use crate::{HostOpenPlan, HostOpenStream, stream_host_capability};

/// A router row for an audio site and the provider that owns it.
#[derive(Clone)]
pub struct RegisteredAudioSite {
    /// Provider or local fixture that owns this site key.
    pub owner: Symbol,
    /// Runtime-openable audio site.
    pub site: Arc<dyn AudioSite>,
}

/// Registry and dispatcher for audio placement sites.
pub struct AudioRouter {
    sites: HashMap<AudioSiteKey, RegisteredAudioSite>,
}

impl Default for AudioRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRouter {
    /// Builds an empty audio placement router.
    pub fn new() -> Self {
        Self {
            sites: HashMap::new(),
        }
    }

    /// Registers a local modeled site by its stable key.
    ///
    /// Provider loading uses [`Self::register_owned`] so duplicate site keys can
    /// be reported as load errors instead of silently replacing an owner.
    pub fn register(&mut self, site: Arc<dyn AudioSite>) {
        self.register_owned(local_audio_site_owner_symbol(), site)
            .expect("duplicate local audio site key");
    }

    /// Registers a provider-owned site by its stable key.
    pub fn register_owned(&mut self, owner: Symbol, site: Arc<dyn AudioSite>) -> Result<()> {
        let key = site.key().clone();
        if self.sites.contains_key(&key) {
            return Err(Error::Eval(format!(
                "audio site {} is already registered",
                key.0
            )));
        }
        self.sites.insert(key, RegisteredAudioSite { owner, site });
        Ok(())
    }

    /// Explicitly reloads a provider-owned site, replacing only that owner's row.
    pub fn reload_owned(&mut self, owner: Symbol, site: Arc<dyn AudioSite>) -> Result<()> {
        let key = site.key().clone();
        if let Some(existing) = self.sites.get(&key)
            && existing.owner != owner
        {
            return Err(Error::Eval(format!(
                "audio site {} is already owned by {}",
                key.0, existing.owner
            )));
        }
        self.sites.insert(key, RegisteredAudioSite { owner, site });
        Ok(())
    }

    /// Removes every site owned by `owner`, returning the removed row count.
    pub fn unregister_owner(&mut self, owner: &Symbol) -> usize {
        let before = self.sites.len();
        self.sites
            .retain(|_, registered| &registered.owner != owner);
        before - self.sites.len()
    }

    /// Returns an audio site by key.
    pub fn site(&self, key: &AudioSiteKey) -> Option<&Arc<dyn AudioSite>> {
        self.sites.get(key).map(|registered| &registered.site)
    }

    /// Returns the provider owner for a registered audio site.
    pub fn site_owner(&self, key: &AudioSiteKey) -> Option<&Symbol> {
        self.sites.get(key).map(|registered| &registered.owner)
    }

    /// Iterates over registered audio site rows.
    pub fn registered_sites(&self) -> impl Iterator<Item = &RegisteredAudioSite> {
        self.sites.values()
    }

    /// Iterates over registered audio site keys.
    pub fn site_keys(&self) -> impl Iterator<Item = &AudioSiteKey> {
        self.sites.keys()
    }

    /// Returns site keys whose device cards satisfy the requested output shape.
    pub fn sites_by_capability(
        &self,
        min_channels_out: u16,
        preferred_rates: &[u32],
    ) -> Vec<AudioSiteKey> {
        let mut keys = self
            .sites
            .values()
            .filter(|registered| {
                let card = registered.site.card();
                card.channels_out >= min_channels_out
                    && preferred_rates
                        .iter()
                        .any(|rate| card.sample_rates.contains(rate))
            })
            .map(|registered| registered.site.key().clone())
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| key.0.to_string());
        keys
    }

    /// Resolves `key` to a registered site, or returns the modeled fallback key.
    ///
    /// Missing native providers are treated as a placement miss rather than an
    /// error when the modeled site is available.
    pub fn resolve_or_modeled(
        &self,
        key: &AudioSiteKey,
        modeled: &AudioSiteKey,
    ) -> Result<AudioSiteKey> {
        if self.sites.contains_key(key) {
            Ok(key.clone())
        } else if self.sites.contains_key(modeled) {
            Ok(modeled.clone())
        } else {
            Err(Error::Eval(format!(
                "no audio site registered for {:?} and no modeled fallback {:?}",
                key, modeled
            )))
        }
    }

    /// Opens a placement request through its registered audio site after
    /// checking authority and recording the declared device effects.
    pub fn open_placement_checked(
        &self,
        cx: &mut Cx,
        request: AudioPlacementRequest,
    ) -> Result<HostOpenStream> {
        if !self.sites.contains_key(&request.site_key) {
            return Err(Error::Eval(format!(
                "no audio site registered for {:?}",
                request.site_key
            )));
        }
        HostOpenPlan::new(
            request.stream_request.backend().clone(),
            request.stream_request.device().clone(),
            request.stream_request.direction().effect_kinds(),
            vec![stream_host_capability()],
        )
        .enforce(cx)?;
        self.open_placement(request)
    }

    /// Opens a placement request through its registered audio site via the
    /// site-level compatibility dispatch path.
    ///
    /// Runtime and public host opens should use [`Self::open_placement_checked`]
    /// so the request's authority and device effects are handled first.
    pub fn open_placement(&self, request: AudioPlacementRequest) -> Result<HostOpenStream> {
        self.sites
            .get(&request.site_key)
            .ok_or_else(|| {
                Error::Eval(format!(
                    "no audio site registered for {:?}",
                    request.site_key
                ))
            })?
            .site
            .open(request.stream_request)
    }
}

fn local_audio_site_owner_symbol() -> Symbol {
    Symbol::qualified("audio/provider", "local-modeled")
}
