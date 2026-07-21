//! Halo provider and session wiring.

use std::collections::{BTreeMap, VecDeque};

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_device::DeviceSample as StreamDeviceSample;
use sim_lib_stream_host::{
    DeviceError, DeviceProfile, DeviceProvider, DeviceResult, DeviceSession,
};
use sim_lib_stream_xr::{
    XrCameraFrameRef, XrMicChunkRef, XrPoseSample, XrTapSample, halo_device_symbol,
    xr_camera_frame_sample_kind_symbol, xr_mic_chunk_sample_kind_symbol,
    xr_pose_sample_kind_symbol, xr_tap_sample_kind_symbol,
};

use crate::ble::{BlueZLink, WebBluetoothLink};
use crate::frame_budget::{FrameDiff, LuaFrameBudget, LuaFrameScheduler};
use crate::lua_render::encode_lua_cells;
use crate::relay::RelayLink;
use crate::sample::{HaloButtonSample, halo_button_sample_kind_symbol};

pub use crate::sample::HALO_BUTTON_SAMPLE_KIND;

/// Bare sample kind for Halo orientation hints.
pub const HALO_MOTION_SAMPLE_KIND: &str = "xr/pose";
/// Bare sample kind for Halo tap input.
pub const HALO_TAP_SAMPLE_KIND: &str = "xr/tap";
/// Bare sample kind for Halo microphone chunk references.
pub const HALO_MIC_SAMPLE_KIND: &str = "xr/mic-chunk";
/// Bare sample kind for one-shot Halo camera references.
pub const HALO_CAMERA_SAMPLE_KIND: &str = "xr/camera-frame";

const DEFAULT_FRAME_BYTES_PER_TICK: u32 = 512;

/// Local route used by a Halo provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HaloRouteKind {
    /// Direct BLE 5.3 through BlueZ.
    Ble,
    /// Browser-mediated Web Bluetooth.
    WebBluetooth,
    /// Local phone relay.
    Relay,
    /// Hardware-free unsupported route.
    Stub,
}

impl HaloRouteKind {
    /// Stable route token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Ble => "ble",
            Self::WebBluetooth => "web-bluetooth",
            Self::Relay => "relay",
            Self::Stub => "stub",
        }
    }
}

/// One normalized input sample accepted from a Halo route.
#[derive(Clone, Debug, PartialEq)]
pub enum HaloSample {
    /// Three-degree-of-freedom motion or orientation hint.
    Motion(XrPoseSample),
    /// Temple tap input.
    Tap(XrTapSample),
    /// Physical button input.
    Button(HaloButtonSample),
    /// Raw microphone audio reference for an external ASR site.
    Mic(XrMicChunkRef),
    /// One-shot 640x480 camera reference.
    Camera(XrCameraFrameRef),
}

impl HaloSample {
    /// Stable bare sample kind.
    pub fn sample_kind(&self) -> &'static str {
        match self {
            Self::Motion(_) => HALO_MOTION_SAMPLE_KIND,
            Self::Tap(_) => HALO_TAP_SAMPLE_KIND,
            Self::Button(_) => HALO_BUTTON_SAMPLE_KIND,
            Self::Mic(_) => HALO_MIC_SAMPLE_KIND,
            Self::Camera(_) => HALO_CAMERA_SAMPLE_KIND,
        }
    }

    /// Monotone sequence number within this sample kind.
    pub fn seq(&self) -> u64 {
        match self {
            Self::Motion(sample) => sample.seq(),
            Self::Tap(sample) => sample.seq(),
            Self::Button(sample) => sample.seq(),
            Self::Mic(sample) => sample.seq(),
            Self::Camera(sample) => sample.seq(),
        }
    }

    /// Encodes this sample using its strict XR record contract.
    pub fn to_expr(&self) -> Expr {
        match self {
            Self::Motion(sample) => sample.to_expr(),
            Self::Tap(sample) => sample.to_expr(),
            Self::Button(sample) => sample.to_expr(),
            Self::Mic(sample) => sample.to_expr(),
            Self::Camera(sample) => sample.to_expr(),
        }
    }

    fn validate(&self) -> DeviceResult<()> {
        match self {
            Self::Motion(sample) if sample.dof() != 3 || sample.position_m().is_some() => {
                Err(DeviceError::Sample(
                    "Halo motion samples must be 3DoF orientation hints".to_owned(),
                ))
            }
            Self::Tap(sample) if sample.device() != &halo_device_symbol() => Err(
                DeviceError::Sample("Halo tap sample has the wrong device identity".to_owned()),
            ),
            Self::Camera(sample)
                if sample.device() != &halo_device_symbol()
                    || sample.width_px() != 640
                    || sample.height_px() != 480
                    || sample.stereo() =>
            {
                Err(DeviceError::Sample(
                    "Halo camera samples must be mono 640x480 frame references".to_owned(),
                ))
            }
            _ => Ok(()),
        }
    }
}

/// Configured route for opening a Halo provider session.
#[derive(Clone, Debug, PartialEq)]
pub enum HaloRoute {
    /// Direct BlueZ route.
    Ble(BlueZLink),
    /// Web Bluetooth route.
    WebBluetooth(WebBluetoothLink),
    /// Local phone-relay route.
    Relay(RelayLink),
    /// Hardware-free unsupported route.
    Stub,
}

/// Halo provider backed by one local route.
#[derive(Clone, Debug, PartialEq)]
pub struct HaloProvider {
    route: HaloRoute,
    profile: DeviceProfile,
    frame_budget: LuaFrameBudget,
}

impl HaloProvider {
    /// Builds a provider for `route`.
    pub fn new(route: HaloRoute) -> Self {
        Self {
            route,
            profile: halo_device_profile(),
            frame_budget: LuaFrameBudget {
                max_bytes_per_tick: DEFAULT_FRAME_BYTES_PER_TICK,
            },
        }
    }

    /// Builds a direct BlueZ provider.
    pub fn ble(link: BlueZLink) -> Self {
        Self::new(HaloRoute::Ble(link))
    }

    /// Builds a Web Bluetooth provider.
    pub fn web_bluetooth(link: WebBluetoothLink) -> Self {
        Self::new(HaloRoute::WebBluetooth(link))
    }

    /// Builds a local phone-relay provider.
    pub fn relay(link: RelayLink) -> Self {
        Self::new(HaloRoute::Relay(link))
    }

    /// Builds the hardware-free unsupported provider.
    pub fn stub() -> Self {
        Self::new(HaloRoute::Stub)
    }

    /// Replaces the per-tick Lua byte budget.
    pub fn with_frame_budget(mut self, frame_budget: LuaFrameBudget) -> Self {
        self.frame_budget = frame_budget;
        self
    }

    /// Returns the advertised Halo profile.
    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    /// Opens a concrete Halo session without boxing.
    pub fn open_session(&self) -> DeviceResult<HaloSession> {
        let (route, samples) = match &self.route {
            HaloRoute::Ble(link) => (HaloRouteKind::Ble, link.open_samples()?),
            HaloRoute::WebBluetooth(link) => (HaloRouteKind::WebBluetooth, link.open_samples()?),
            HaloRoute::Relay(link) => (HaloRouteKind::Relay, link.open_samples()?),
            HaloRoute::Stub => return Err(DeviceError::Unsupported),
        };
        validate_samples(&samples)?;
        Ok(HaloSession::new(
            route,
            samples,
            self.profile.clone(),
            self.frame_budget,
        ))
    }
}

impl Default for HaloProvider {
    fn default() -> Self {
        Self::stub()
    }
}

impl DeviceProvider for HaloProvider {
    fn open(&self) -> DeviceResult<Box<dyn DeviceSession>> {
        Ok(Box::new(self.open_session()?))
    }
}

/// One route-local Lua diff packet accepted by a Halo session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HaloFramePacket {
    route: HaloRouteKind,
    diff: FrameDiff,
    lua: Vec<u8>,
}

impl HaloFramePacket {
    /// Route carrying this packet.
    pub fn route(&self) -> HaloRouteKind {
        self.route
    }

    /// Budgeted changed cells in the packet.
    pub fn diff(&self) -> &FrameDiff {
        &self.diff
    }

    /// Encoded open Lua primitives.
    pub fn lua(&self) -> &[u8] {
        &self.lua
    }
}

/// Open Halo session over one local route.
#[derive(Clone, Debug, PartialEq)]
pub struct HaloSession {
    profile: DeviceProfile,
    route: HaloRouteKind,
    samples: VecDeque<HaloSample>,
    frame_budget: LuaFrameBudget,
    scheduler: LuaFrameScheduler,
    sent: Vec<HaloFramePacket>,
    started: bool,
}

impl HaloSession {
    fn new(
        route: HaloRouteKind,
        samples: Vec<HaloSample>,
        profile: DeviceProfile,
        frame_budget: LuaFrameBudget,
    ) -> Self {
        Self {
            profile,
            route,
            samples: samples.into(),
            frame_budget,
            scheduler: LuaFrameScheduler::empty(),
            sent: Vec::new(),
            started: false,
        }
    }

    /// Active route kind.
    pub fn route(&self) -> HaloRouteKind {
        self.route
    }

    /// Lua frame packets accepted by this session.
    pub fn sent_frames(&self) -> &[HaloFramePacket] {
        &self.sent
    }

    fn require_started(&self) -> DeviceResult<()> {
        if self.started {
            Ok(())
        } else {
            Err(DeviceError::Host("Halo session is not started".to_owned()))
        }
    }
}

impl DeviceSession for HaloSession {
    fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    fn start(&mut self) -> DeviceResult<()> {
        self.started = true;
        Ok(())
    }

    fn poll(&mut self, kind: &str) -> DeviceResult<Option<Expr>> {
        self.require_started()?;
        let Some(index) = self
            .samples
            .iter()
            .position(|sample| sample.sample_kind() == kind)
        else {
            return Ok(None);
        };
        Ok(self.samples.remove(index).map(|sample| sample.to_expr()))
    }

    fn send(&mut self, command: &Expr) -> DeviceResult<()> {
        self.require_started()?;
        let diff = self
            .scheduler
            .schedule(command, &self.frame_budget)
            .map_err(|error| DeviceError::Host(error.to_string()))?;
        let lua = encode_lua_cells(&diff.cells);
        if usize::try_from(diff.bytes).ok() != Some(lua.len()) {
            return Err(DeviceError::Host(
                "Halo Lua frame byte count mismatch".to_owned(),
            ));
        }
        self.sent.push(HaloFramePacket {
            route: self.route,
            diff,
            lua,
        });
        Ok(())
    }

    fn stop(&mut self) -> DeviceResult<()> {
        self.started = false;
        Ok(())
    }
}

/// Returns the stream-device profile advertised by Halo routes.
pub fn halo_device_profile() -> DeviceProfile {
    DeviceProfile::new(
        halo_device_symbol(),
        vec![
            Symbol::qualified("device/stream", "orientation"),
            Symbol::qualified("device/stream", "microphone-ref"),
            Symbol::qualified("device/stream", "camera-pull-ref"),
        ],
        vec![
            Symbol::qualified("device/input", "tap"),
            Symbol::qualified("device/input", "button"),
        ],
        vec![Symbol::qualified("glasses/output", "lua-glance")],
        vec![
            xr_pose_sample_kind_symbol(),
            xr_tap_sample_kind_symbol(),
            halo_button_sample_kind_symbol(),
            xr_mic_chunk_sample_kind_symbol(),
            xr_camera_frame_sample_kind_symbol(),
        ],
    )
}

fn validate_samples(samples: &[HaloSample]) -> DeviceResult<()> {
    let mut last_seq = BTreeMap::new();
    let mut camera_count = 0usize;
    for sample in samples {
        sample.validate()?;
        if matches!(sample, HaloSample::Camera(_)) {
            camera_count += 1;
            if camera_count > 1 {
                return Err(DeviceError::Sample(
                    "Halo routes accept at most one opt-in camera pull per session".to_owned(),
                ));
            }
        }
        if let Some(previous) = last_seq.insert(sample.sample_kind(), sample.seq())
            && sample.seq() < previous
        {
            return Err(DeviceError::Sample(format!(
                "Halo {} sequence moved backwards",
                sample.sample_kind()
            )));
        }
    }
    Ok(())
}
