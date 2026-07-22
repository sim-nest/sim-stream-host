//! Hardware-free stub provider for watch routes.

use sim_lib_stream_host::StubProvider;

use crate::watch_device_profile;

/// Builds a watch stub provider that returns `Unsupported` when opened.
pub fn watch_stub_provider() -> StubProvider {
    StubProvider::new(watch_device_profile())
}
