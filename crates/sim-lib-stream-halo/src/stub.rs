//! Hardware-free Halo provider stub.

use crate::HaloProvider;

/// Builds a Halo provider that returns `Unsupported` when opened.
pub fn halo_stub_provider() -> HaloProvider {
    HaloProvider::stub()
}
