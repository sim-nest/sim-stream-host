//! VITURE device provider and session wiring.

use std::collections::VecDeque;

use sim_kernel::{Expr, Symbol};
use sim_lib_stream_device::DeviceSample as XrDeviceSample;
use sim_lib_stream_host::{
    DeviceError, DeviceProfile, DeviceProvider, DeviceResult, DeviceSession,
};
use sim_lib_stream_xr::{
    XrCameraFrameRef, XrPoseSample, XrTrackingStatus, xr_camera_frame_sample_kind_symbol,
    xr_pose_sample_kind_symbol,
};
use sim_viture_ffi::{
    LegacyImuRate, VitureError, VitureHandle, VitureLib, VitureSdkDiscovery, unsupported_viture_lib,
};

use crate::{
    camera::VITURE_CAMERA_SAMPLE_KIND,
    device_control::{VitureControlPacket, encode_viture_command},
    vio::viture_tracking_status,
};

/// Bare XR pose sample kind emitted by VITURE sessions.
pub const VITURE_POSE_SAMPLE_KIND: &str = "xr/pose";

/// Default pose sample period used when the SDK does not provide one.
pub const DEFAULT_POSE_STEP_NS: u64 = 16_666_667;

/// Default IMU report rate used for the 3DoF route.
pub const DEFAULT_LEGACY_IMU_RATE_HZ: u32 = 120;

/// Local VITURE route kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VitureRouteKind {
    /// Carina route that polls six degree-of-freedom headset pose.
    Carina,
    /// IMU-control route that emits three degree-of-freedom orientation samples.
    LegacyImu,
    /// Deterministic hardware-free route for tests and replay.
    Scripted,
    /// Unsupported route used when no SDK-backed provider is installed.
    Stub,
}

impl VitureRouteKind {
    /// Stable route token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Carina => "carina",
            Self::LegacyImu => "legacy-imu",
            Self::Scripted => "scripted",
            Self::Stub => "stub",
        }
    }
}

/// Configured route for opening a VITURE provider session.
#[derive(Clone, Debug)]
pub enum VitureRoute {
    /// Carina route backed by the VITURE SDK provider handle.
    Carina {
        /// Loaded SDK wrapper.
        lib: VitureLib,
        /// Prediction horizon passed to the pose call.
        predict_ns: u64,
    },
    /// IMU-control route backed by the VITURE SDK.
    LegacyImu {
        /// Loaded SDK wrapper.
        lib: VitureLib,
        /// IMU report rate.
        rate: LegacyImuRate,
        /// Orientation used for emitted 3DoF samples until host IMU decoding is
        /// connected.
        orientation: [f64; 4],
    },
    /// Deterministic pose queue for hardware-free validation.
    Scripted {
        /// Samples returned from `poll` in order.
        samples: Vec<XrPoseSample>,
        /// Camera frame references returned from `poll` in order.
        camera_frames: Vec<XrCameraFrameRef>,
    },
    /// Hardware-free unsupported route.
    Stub,
}

/// VITURE provider backed by one local route.
#[derive(Clone, Debug)]
pub struct VitureProvider {
    route: VitureRoute,
    profile: DeviceProfile,
}

impl VitureProvider {
    /// Builds a VITURE provider for `route`.
    pub fn new(route: VitureRoute) -> Self {
        Self {
            route,
            profile: viture_device_profile(),
        }
    }

    /// Builds a Carina provider from a loaded SDK wrapper.
    pub fn carina(lib: VitureLib, predict_ns: u64) -> Self {
        Self::new(VitureRoute::Carina { lib, predict_ns })
    }

    /// Attempts SDK discovery and builds a Carina provider from the result.
    pub fn discover_carina(discovery: &VitureSdkDiscovery, predict_ns: u64) -> DeviceResult<Self> {
        Ok(Self::carina(
            VitureLib::discover(discovery).map_err(map_viture_error)?,
            predict_ns,
        ))
    }

    /// Builds an IMU-control provider from a loaded SDK wrapper.
    pub fn legacy_imu(lib: VitureLib, rate: LegacyImuRate) -> Self {
        Self::legacy_imu_with_orientation(lib, rate, [1.0, 0.0, 0.0, 0.0])
    }

    /// Builds an IMU-control provider with an explicit orientation.
    pub fn legacy_imu_with_orientation(
        lib: VitureLib,
        rate: LegacyImuRate,
        orientation: [f64; 4],
    ) -> Self {
        Self::new(VitureRoute::LegacyImu {
            lib,
            rate,
            orientation,
        })
    }

    /// Builds a deterministic hardware-free provider from pose samples.
    pub fn scripted(samples: Vec<XrPoseSample>) -> Self {
        Self::scripted_with_camera_frames(samples, Vec::new())
    }

    /// Builds a deterministic hardware-free provider from pose and camera samples.
    pub fn scripted_with_camera_frames(
        samples: Vec<XrPoseSample>,
        camera_frames: Vec<XrCameraFrameRef>,
    ) -> Self {
        Self::new(VitureRoute::Scripted {
            samples,
            camera_frames,
        })
    }

    /// Builds an unsupported provider for hardware-free defaults.
    pub fn stub() -> Self {
        Self::new(VitureRoute::Stub)
    }

    /// Builds an unsupported provider using the FFI stub.
    pub fn unsupported_sdk(predict_ns: u64) -> Self {
        Self::carina(unsupported_viture_lib(), predict_ns)
    }

    /// Returns the advertised VITURE profile.
    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    /// Opens a concrete VITURE session without boxing.
    pub fn open_session(&self) -> DeviceResult<VitureSession> {
        match &self.route {
            VitureRoute::Stub => Err(DeviceError::Unsupported),
            VitureRoute::Carina { lib, predict_ns } => {
                let handle = lib.open_carina().map_err(map_viture_error)?;
                Ok(VitureSession::carina(
                    lib.clone(),
                    handle,
                    *predict_ns,
                    self.profile.clone(),
                ))
            }
            VitureRoute::LegacyImu {
                lib,
                rate,
                orientation,
            } => {
                lib.legacy_init().map_err(map_viture_error)?;
                Ok(VitureSession::legacy_imu(
                    lib.clone(),
                    *rate,
                    *orientation,
                    self.profile.clone(),
                )?)
            }
            VitureRoute::Scripted {
                samples,
                camera_frames,
            } => Ok(VitureSession::scripted(
                samples.clone(),
                camera_frames.clone(),
                self.profile.clone(),
            )),
        }
    }
}

impl Default for VitureProvider {
    fn default() -> Self {
        Self::stub()
    }
}

impl DeviceProvider for VitureProvider {
    fn open(&self) -> DeviceResult<Box<dyn DeviceSession>> {
        Ok(Box::new(self.open_session()?))
    }
}

/// Open VITURE session over one local route.
#[derive(Debug)]
pub struct VitureSession {
    profile: DeviceProfile,
    route: VitureSessionRoute,
    sent: Vec<VitureControlPacket>,
    started: bool,
}

#[derive(Debug)]
enum VitureSessionRoute {
    Carina {
        lib: VitureLib,
        handle: VitureHandle,
        predict_ns: u64,
        seq: u64,
    },
    LegacyImu {
        lib: VitureLib,
        rate: LegacyImuRate,
        orientation: [f64; 4],
        seq: u64,
    },
    Scripted {
        samples: VecDeque<XrPoseSample>,
        camera_frames: VecDeque<XrCameraFrameRef>,
    },
}

impl VitureSession {
    fn carina(
        lib: VitureLib,
        handle: VitureHandle,
        predict_ns: u64,
        profile: DeviceProfile,
    ) -> Self {
        Self {
            profile,
            route: VitureSessionRoute::Carina {
                lib,
                handle,
                predict_ns,
                seq: 0,
            },
            sent: Vec::new(),
            started: false,
        }
    }

    fn legacy_imu(
        lib: VitureLib,
        rate: LegacyImuRate,
        orientation: [f64; 4],
        profile: DeviceProfile,
    ) -> DeviceResult<Self> {
        XrPoseSample::new(0, None, orientation, 0, 0, 3, XrTrackingStatus::Limited)
            .map_err(map_sample_error)?;
        Ok(Self {
            profile,
            route: VitureSessionRoute::LegacyImu {
                lib,
                rate,
                orientation,
                seq: 0,
            },
            sent: Vec::new(),
            started: false,
        })
    }

    fn scripted(
        samples: Vec<XrPoseSample>,
        camera_frames: Vec<XrCameraFrameRef>,
        profile: DeviceProfile,
    ) -> Self {
        Self {
            profile,
            route: VitureSessionRoute::Scripted {
                samples: samples.into(),
                camera_frames: camera_frames.into(),
            },
            sent: Vec::new(),
            started: false,
        }
    }

    /// Returns the active route kind.
    pub fn route_kind(&self) -> VitureRouteKind {
        match self.route {
            VitureSessionRoute::Carina { .. } => VitureRouteKind::Carina,
            VitureSessionRoute::LegacyImu { .. } => VitureRouteKind::LegacyImu,
            VitureSessionRoute::Scripted { .. } => VitureRouteKind::Scripted,
        }
    }

    /// Command packets accepted by this session.
    pub fn sent_commands(&self) -> &[VitureControlPacket] {
        &self.sent
    }

    /// Returns whether the session has been started.
    pub fn is_started(&self) -> bool {
        self.started
    }
}

impl DeviceSession for VitureSession {
    fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    fn start(&mut self) -> DeviceResult<()> {
        match &self.route {
            VitureSessionRoute::Carina { lib, handle, .. } => {
                lib.initialize_carina(handle).map_err(map_viture_error)?;
                lib.start_carina(handle).map_err(map_viture_error)?;
            }
            VitureSessionRoute::LegacyImu { lib, rate, .. } => {
                lib.legacy_set_imu_fq(*rate).map_err(map_viture_error)?;
                lib.legacy_set_imu(true).map_err(map_viture_error)?;
            }
            VitureSessionRoute::Scripted { .. } => {}
        }
        self.started = true;
        Ok(())
    }

    fn poll(&mut self, kind: &str) -> DeviceResult<Option<Expr>> {
        if kind != VITURE_POSE_SAMPLE_KIND && kind != VITURE_CAMERA_SAMPLE_KIND {
            return Ok(None);
        }
        match &mut self.route {
            VitureSessionRoute::Carina { .. } | VitureSessionRoute::LegacyImu { .. }
                if kind == VITURE_CAMERA_SAMPLE_KIND =>
            {
                Ok(None)
            }
            VitureSessionRoute::Carina {
                lib,
                handle,
                predict_ns,
                seq,
            } => {
                let pose = lib
                    .carina_pose(handle, *predict_ns)
                    .map_err(map_viture_error)?;
                let sample = carina_pose_sample(
                    *seq,
                    *predict_ns,
                    pose.pose,
                    viture_tracking_status(pose.status),
                )?;
                *seq = seq.saturating_add(1);
                Ok(Some(sample.to_expr()))
            }
            VitureSessionRoute::LegacyImu {
                orientation, seq, ..
            } => {
                let sample = XrPoseSample::new(
                    *seq,
                    None,
                    *orientation,
                    sample_time_ns(*seq),
                    0,
                    3,
                    XrTrackingStatus::Limited,
                )
                .map_err(map_sample_error)?;
                *seq = seq.saturating_add(1);
                Ok(Some(sample.to_expr()))
            }
            VitureSessionRoute::Scripted {
                samples,
                camera_frames,
            } => {
                if kind == VITURE_CAMERA_SAMPLE_KIND {
                    return Ok(camera_frames.pop_front().map(|sample| sample.to_expr()));
                }
                Ok(samples.pop_front().map(|sample| sample.to_expr()))
            }
        }
    }

    fn send(&mut self, command: &Expr) -> DeviceResult<()> {
        let packet = encode_viture_command(command)?;
        apply_control_packet(&self.route, &packet)?;
        self.sent.push(packet);
        Ok(())
    }

    fn stop(&mut self) -> DeviceResult<()> {
        match &self.route {
            VitureSessionRoute::Carina { lib, handle, .. } => {
                lib.stop_carina(handle).map_err(map_viture_error)?;
            }
            VitureSessionRoute::LegacyImu { lib, .. } => {
                lib.legacy_set_imu(false).map_err(map_viture_error)?;
            }
            VitureSessionRoute::Scripted { .. } => {}
        }
        self.started = false;
        Ok(())
    }
}

/// Returns the VITURE stream-device profile advertised by route providers.
pub fn viture_device_profile() -> DeviceProfile {
    DeviceProfile::new(
        Symbol::qualified("device", "viture-glasses"),
        vec![
            Symbol::qualified("device/stream", "xr-pose"),
            Symbol::qualified("device/stream", "xr-imu"),
            Symbol::qualified("device/stream", "xr-camera"),
            Symbol::qualified("device/stream", "display-control"),
        ],
        vec![
            Symbol::qualified("device/input", "head-pose"),
            Symbol::qualified("device/input", "imu"),
            Symbol::qualified("device/input", "camera"),
        ],
        vec![
            Symbol::qualified("glasses/output", "display-3d"),
            Symbol::qualified("glasses/output", "brightness"),
            Symbol::qualified("glasses/output", "privacy-film"),
        ],
        vec![
            xr_pose_sample_kind_symbol(),
            xr_camera_frame_sample_kind_symbol(),
        ],
    )
}

fn apply_control_packet(
    route: &VitureSessionRoute,
    packet: &VitureControlPacket,
) -> DeviceResult<()> {
    match (route, packet) {
        (VitureSessionRoute::Scripted { .. }, _) => Ok(()),
        (_, VitureControlPacket::ImuReports { enabled }) => {
            route_lib(route)?
                .legacy_set_imu(*enabled)
                .map_err(map_viture_error)?;
            Ok(())
        }
        (_, VitureControlPacket::ImuRate { rate }) => {
            route_lib(route)?
                .legacy_set_imu_fq(*rate)
                .map_err(map_viture_error)?;
            Ok(())
        }
        (_, VitureControlPacket::Display3d { enabled }) => {
            route_lib(route)?
                .legacy_set_3d(*enabled)
                .map_err(map_viture_error)?;
            Ok(())
        }
        (_, VitureControlPacket::Brightness { .. } | VitureControlPacket::PrivacyFilm { .. }) => {
            Ok(())
        }
    }
}

fn route_lib(route: &VitureSessionRoute) -> DeviceResult<&VitureLib> {
    match route {
        VitureSessionRoute::Carina { lib, .. } | VitureSessionRoute::LegacyImu { lib, .. } => {
            Ok(lib)
        }
        VitureSessionRoute::Scripted { .. } => Err(DeviceError::Unsupported),
    }
}

fn carina_pose_sample(
    seq: u64,
    predict_ns: u64,
    pose: [f64; 7],
    status: XrTrackingStatus,
) -> DeviceResult<XrPoseSample> {
    XrPoseSample::new(
        seq,
        Some([pose[0], pose[1], pose[2]]),
        [pose[3], pose[4], pose[5], pose[6]],
        sample_time_ns(seq),
        predict_ns,
        6,
        status,
    )
    .map_err(map_sample_error)
}

fn sample_time_ns(seq: u64) -> u64 {
    seq.saturating_mul(DEFAULT_POSE_STEP_NS)
}

fn map_sample_error(error: sim_lib_stream_device::DeviceSampleError) -> DeviceError {
    DeviceError::Sample(error.to_string())
}

fn map_viture_error(error: VitureError) -> DeviceError {
    match error {
        VitureError::Unsupported => DeviceError::Unsupported,
        other => DeviceError::Host(other.to_string()),
    }
}
