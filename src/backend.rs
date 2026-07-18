//! Generic host backend trait and opened stream handle.

use std::{rc::Rc, sync::Arc};

use sim_kernel::Result;
use sim_lib_stream_core::StreamValue;

use crate::{
    HostBackendInfo, HostCallbackQueue, HostDeviceInventory, HostStreamConfig,
    HostStreamConfigRequest,
};

/// Backend contract for host-controlled stream devices.
pub trait HostBackend: Send + Sync {
    /// Stable backend metadata.
    fn info(&self) -> &HostBackendInfo;

    /// Enumerates available devices and ports without opening hardware streams.
    fn enumerate(&self) -> Result<HostDeviceInventory>;

    /// Opens a stream according to the requested host configuration.
    ///
    /// This is backend-level dispatch for implementations and already checked
    /// callers. Public host opens should route through
    /// [`HostBackendRegistry::open_checked`](crate::HostBackendRegistry::open_checked)
    /// so `stream.host` authority and device effects are handled before the
    /// backend is reached.
    fn open(&self, request: HostStreamConfigRequest) -> Result<HostOpenStream>;
}

/// Native or external stream resource attached to an opened host stream.
///
/// Implementations own platform resources that must outlive the SIM-side queue.
/// Closing or canceling the host stream calls [`HostStreamDriver::shutdown`];
/// dropping the driver should also release the underlying resource.
///
/// The trait is intentionally thread-affinity-neutral because some platform
/// stream handles cannot be sent or shared across threads.
pub trait HostStreamDriver {
    /// Stops the external stream resource.
    fn shutdown(&self) -> Result<()>;
}

/// Open stream plus host-side callback queue.
#[derive(Clone)]
pub struct HostOpenStream {
    config: HostStreamConfig,
    queue: HostCallbackQueue,
    driver: Option<Rc<dyn HostStreamDriver>>,
}

impl HostOpenStream {
    /// Creates an opened host stream backed by a push stream value.
    pub fn new(config: HostStreamConfig) -> Self {
        let stream = Arc::new(StreamValue::push(config.metadata()));
        Self {
            config,
            queue: HostCallbackQueue::new(stream),
            driver: None,
        }
    }

    /// Creates an opened host stream with an attached external driver.
    pub fn new_with_driver(config: HostStreamConfig, driver: Rc<dyn HostStreamDriver>) -> Self {
        let stream = Arc::new(StreamValue::push(config.metadata()));
        Self {
            config,
            queue: HostCallbackQueue::new(stream),
            driver: Some(driver),
        }
    }

    /// Creates an opened host stream by building its driver from the callback
    /// queue that will be stored on the stream.
    pub fn try_new_with_driver(
        config: HostStreamConfig,
        build_driver: impl FnOnce(HostCallbackQueue) -> Result<Rc<dyn HostStreamDriver>>,
    ) -> Result<Self> {
        let stream = Arc::new(StreamValue::push(config.metadata()));
        let queue = HostCallbackQueue::new(stream);
        let driver = build_driver(queue.clone())?;
        Ok(Self {
            config,
            queue,
            driver: Some(driver),
        })
    }

    /// Creates a realtime local audio stream after validating callback limits.
    pub fn new_realtime_local_audio(config: HostStreamConfig) -> Result<Self> {
        config.validate_realtime_local_audio()?;
        Ok(Self::new(config))
    }

    /// Creates a realtime local audio stream with an attached external driver.
    pub fn new_realtime_local_audio_with_driver(
        config: HostStreamConfig,
        driver: Rc<dyn HostStreamDriver>,
    ) -> Result<Self> {
        config.validate_realtime_local_audio()?;
        Ok(Self::new_with_driver(config, driver))
    }

    /// Creates a realtime local audio stream by building its driver from the
    /// stored callback queue.
    pub fn try_new_realtime_local_audio_with_driver(
        config: HostStreamConfig,
        build_driver: impl FnOnce(HostCallbackQueue) -> Result<Rc<dyn HostStreamDriver>>,
    ) -> Result<Self> {
        config.validate_realtime_local_audio()?;
        Self::try_new_with_driver(config, build_driver)
    }

    /// Creates a LAN MIDI/control stream after validating media and clock shape.
    pub fn new_lan_midi_control(config: HostStreamConfig) -> Result<Self> {
        config.validate_lan_midi_control()?;
        Ok(Self::new(config))
    }

    /// Creates a LAN buffered audio preview stream after validating media shape.
    pub fn new_lan_buffered_audio_preview(config: HostStreamConfig) -> Result<Self> {
        config.validate_lan_buffered_audio_preview()?;
        Ok(Self::new(config))
    }

    /// Returns the accepted stream configuration.
    pub fn config(&self) -> &HostStreamConfig {
        &self.config
    }

    /// Returns the callback queue used by host callbacks or deterministic fakes.
    pub fn queue(&self) -> &HostCallbackQueue {
        &self.queue
    }

    /// Returns the stream value consumed by graph/runtime code.
    pub fn stream(&self) -> Arc<StreamValue> {
        self.queue.stream()
    }

    /// Closes the host callback queue.
    pub fn close(&self) -> Result<()> {
        self.queue.close()?;
        self.shutdown_driver()
    }

    /// Cancels the callback queue and drops buffered packets.
    pub fn cancel(&self) -> Result<()> {
        self.queue.cancel()?;
        self.shutdown_driver()
    }

    fn shutdown_driver(&self) -> Result<()> {
        if let Some(driver) = &self.driver {
            driver.shutdown()?;
        }
        Ok(())
    }
}
