//! Direct BLE and Web Bluetooth route descriptions.

use sim_lib_stream_host::{DeviceError, DeviceResult};

use crate::HaloSample;

/// Direct BLE 5.3 route through a local BlueZ adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct BlueZLink {
    adapter: String,
    device: String,
    samples: Vec<HaloSample>,
}

impl BlueZLink {
    /// Builds an empty direct BLE route.
    pub fn new(adapter: impl Into<String>, device: impl Into<String>) -> Self {
        Self {
            adapter: adapter.into(),
            device: device.into(),
            samples: Vec::new(),
        }
    }

    /// Builds a direct BLE route with deterministic samples.
    pub fn with_scripted_samples(
        adapter: impl Into<String>,
        device: impl Into<String>,
        samples: Vec<HaloSample>,
    ) -> Self {
        Self {
            adapter: adapter.into(),
            device: device.into(),
            samples,
        }
    }

    /// Local BlueZ adapter name.
    pub fn adapter(&self) -> &str {
        &self.adapter
    }

    /// Device address or stable local label.
    pub fn device(&self) -> &str {
        &self.device
    }

    pub(crate) fn open_samples(&self) -> DeviceResult<Vec<HaloSample>> {
        require_text(&self.adapter, "BlueZ adapter")?;
        require_text(&self.device, "BlueZ device")?;
        Ok(self.samples.clone())
    }
}

/// Browser-mediated Web Bluetooth route.
#[derive(Clone, Debug, PartialEq)]
pub struct WebBluetoothLink {
    device: String,
    samples: Vec<HaloSample>,
}

impl WebBluetoothLink {
    /// Builds an empty Web Bluetooth route.
    pub fn new(device: impl Into<String>) -> Self {
        Self {
            device: device.into(),
            samples: Vec::new(),
        }
    }

    /// Builds a Web Bluetooth route with deterministic samples.
    pub fn with_scripted_samples(device: impl Into<String>, samples: Vec<HaloSample>) -> Self {
        Self {
            device: device.into(),
            samples,
        }
    }

    /// Browser-visible device identity.
    pub fn device(&self) -> &str {
        &self.device
    }

    pub(crate) fn open_samples(&self) -> DeviceResult<Vec<HaloSample>> {
        require_text(&self.device, "Web Bluetooth device")?;
        Ok(self.samples.clone())
    }
}

fn require_text(value: &str, name: &str) -> DeviceResult<()> {
    if value.trim().is_empty() {
        Err(DeviceError::Host(format!("{name} must not be empty")))
    } else {
        Ok(())
    }
}
