//! Shared stream evaluation-site traits for cataloged host devices.

use sim_kernel::{Result, Symbol};

use crate::placement::{DeviceRecord, Placement};

/// Open stream site returned by a device catalog.
pub trait StreamEvalSite: Send {
    /// Returns the placement tier used by this site.
    fn placement(&self) -> &Placement;

    /// Returns the catalog row that opened this site.
    fn device_record(&self) -> &DeviceRecord;

    /// Closes the site.
    fn close(self: Box<Self>) -> Result<()>;
}

/// Provider of cataloged stream devices.
pub trait DeviceProvider: Send + Sync {
    /// Enumerates devices owned by this provider.
    fn enumerate(&self) -> Result<Vec<DeviceRecord>>;

    /// Opens one provider-owned device by catalog id.
    ///
    /// This is provider-level dispatch for an already checked catalog row.
    /// Public catalog opens should use
    /// [`DeviceCatalog::open_checked`](crate::DeviceCatalog::open_checked).
    fn open(&self, id: &Symbol) -> Result<Box<dyn StreamEvalSite>>;
}
