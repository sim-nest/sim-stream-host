//! Local phone-relay route description.

use sim_lib_stream_host::{DeviceError, DeviceResult};

use crate::HaloSample;

/// Phone relay over a local endpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct RelayLink {
    endpoint: String,
    samples: Vec<HaloSample>,
}

impl RelayLink {
    /// Builds an empty relay route.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            samples: Vec::new(),
        }
    }

    /// Builds a relay route with deterministic samples.
    pub fn with_scripted_samples(endpoint: impl Into<String>, samples: Vec<HaloSample>) -> Self {
        Self {
            endpoint: endpoint.into(),
            samples,
        }
    }

    /// Local relay endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn open_samples(&self) -> DeviceResult<Vec<HaloSample>> {
        if self.endpoint.trim().is_empty() {
            return Err(DeviceError::Host(
                "Halo relay endpoint must not be empty".to_owned(),
            ));
        }
        Ok(self.samples.clone())
    }
}
