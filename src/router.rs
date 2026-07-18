//! Audio placement router for registered audio sites.

use std::collections::HashMap;
use std::sync::Arc;

use sim_kernel::{Cx, Error, Result};

use crate::placement::{AudioPlacementRequest, AudioSiteKey};
use crate::site::AudioSite;
use crate::{HostOpenPlan, HostOpenStream, stream_host_capability};

/// Registry and dispatcher for audio placement sites.
pub struct AudioRouter {
    sites: HashMap<AudioSiteKey, Arc<dyn AudioSite>>,
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

    /// Registers or replaces a site by its stable key.
    pub fn register(&mut self, site: Arc<dyn AudioSite>) {
        self.sites.insert(site.key().clone(), site);
    }

    /// Returns an audio site by key.
    pub fn site(&self, key: &AudioSiteKey) -> Option<&Arc<dyn AudioSite>> {
        self.sites.get(key)
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
            .filter(|site| {
                let card = site.card();
                card.channels_out >= min_channels_out
                    && preferred_rates
                        .iter()
                        .any(|rate| card.sample_rates.contains(rate))
            })
            .map(|site| site.key().clone())
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
            .open(request.stream_request)
    }
}
