//! Registry for host stream backends.

use std::{collections::BTreeMap, sync::Arc};

use sim_kernel::{Error, Expr, Result, Symbol};

use crate::{
    HostBackend, HostDeviceInventory, HostOpenStream, HostStreamConfigRequest,
    missing_capability_card_expr,
};

/// Deterministic registry of host stream backends.
#[derive(Default)]
pub struct HostBackendRegistry {
    backends: BTreeMap<Symbol, Arc<dyn HostBackend>>,
}

impl HostBackendRegistry {
    /// Creates an empty backend registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a backend by its stable backend id.
    pub fn register<B>(&mut self, backend: B) -> Result<()>
    where
        B: HostBackend + 'static,
    {
        self.register_arc(Arc::new(backend))
    }

    /// Registers an already shared backend.
    pub fn register_arc(&mut self, backend: Arc<dyn HostBackend>) -> Result<()> {
        let id = backend.info().id().clone();
        if self.backends.contains_key(&id) {
            return Err(Error::Eval(format!(
                "stream host backend {id} is already registered"
            )));
        }
        self.backends.insert(id, backend);
        Ok(())
    }

    /// Returns a registered backend by id.
    pub fn backend(&self, id: &Symbol) -> Option<Arc<dyn HostBackend>> {
        self.backends.get(id).cloned()
    }

    /// Enumerates every registered backend.
    pub fn enumerate(&self) -> Result<Vec<HostDeviceInventory>> {
        self.backends
            .values()
            .map(|backend| backend.enumerate())
            .collect()
    }

    /// Opens a stream using the backend named in the request.
    pub fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream> {
        let backend = self.backends.get(request.backend()).ok_or_else(|| {
            Error::Eval(format!(
                "stream host backend {} is not registered",
                request.backend()
            ))
        })?;
        backend.open(request)
    }

    /// Emits backend, device, and port browse card expressions.
    pub fn card_exprs(&self) -> Result<Vec<Expr>> {
        let mut cards = Vec::new();
        for backend in self.backends.values() {
            cards.push(backend.info().card_expr());
            cards.extend(backend.enumerate()?.card_exprs());
        }
        Ok(cards)
    }

    /// Emits a browse card for a missing backend capability.
    pub fn missing_capability_card(
        &self,
        backend: &Symbol,
        capability: crate::HostBackendCapability,
    ) -> Expr {
        missing_capability_card_expr(backend, capability)
    }
}
