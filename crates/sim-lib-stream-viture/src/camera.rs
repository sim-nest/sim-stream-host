//! VITURE camera frame references and consent-gated content storage.

use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_stream_device::DeviceSample;
use sim_lib_stream_host::{
    BoundedContentStore, GlassesCapability, StoreEvicted, StoreKey, require_glasses_sample_ingest,
    store_glasses_frame,
};
use sim_lib_stream_xr::{XrCameraFrameRef, xr_camera_frame_sample_kind_symbol};
use sim_lib_view_device::{ConsentReceipt, EdgeId};

/// Bare XR camera-frame sample kind emitted by VITURE sessions.
pub const VITURE_CAMERA_SAMPLE_KIND: &str = "xr/camera-frame";

/// Local VITURE camera lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VitureCameraKind {
    /// USB Video Class RGB pass-through camera.
    UvcRgb,
    /// Carina stereo/depth camera lane used by VIO.
    StereoVio,
}

impl VitureCameraKind {
    /// Stable camera token.
    pub fn token(self) -> &'static str {
        match self {
            Self::UvcRgb => "viture-uvc-rgb",
            Self::StereoVio => "viture-stereo-vio",
        }
    }

    /// Whether this lane carries paired stereo frames.
    pub fn stereo(self) -> bool {
        matches!(self, Self::StereoVio)
    }
}

/// One captured VITURE camera frame before its bytes are put in the content store.
#[derive(Clone, Debug, PartialEq)]
pub struct VitureCameraFrame {
    /// Monotone stream sequence.
    pub seq: u64,
    /// Camera lane identity.
    pub camera: VitureCameraKind,
    /// Store key carried by the emitted `XrCameraFrameRef`.
    pub frame_key: Symbol,
    /// Pixel dimensions.
    pub size_px: [u32; 2],
    /// Sample time in nanoseconds.
    pub t_ns: u64,
    /// Payload stored by reference.
    pub payload: Expr,
    /// Byte count enforced by the bounded store.
    pub size_bytes: usize,
}

impl VitureCameraFrame {
    /// Builds a camera frame descriptor.
    pub fn new(
        seq: u64,
        camera: VitureCameraKind,
        frame_key: Symbol,
        size_px: [u32; 2],
        t_ns: u64,
        payload: Expr,
        size_bytes: usize,
    ) -> Result<Self> {
        if size_bytes == 0 {
            return Err(Error::Eval(
                "VITURE camera frame size must be nonzero".to_owned(),
            ));
        }
        Ok(Self {
            seq,
            camera,
            frame_key,
            size_px,
            t_ns,
            payload,
            size_bytes,
        })
    }

    /// Builds the strict XR camera-frame reference for this stored payload.
    pub fn to_ref(&self) -> Result<XrCameraFrameRef> {
        XrCameraFrameRef::new(
            self.seq,
            viture_camera_device_symbol(),
            viture_camera_symbol(self.camera),
            self.frame_key.clone(),
            self.size_px,
            self.t_ns,
            self.camera.stereo(),
        )
        .map_err(|error| Error::HostError(error.to_string()))
    }
}

/// Stores bytes under the glasses camera consent contract and returns a stream ref.
pub fn store_viture_camera_frame(
    cx: &Cx,
    store: &mut BoundedContentStore,
    receipt: &ConsentReceipt,
    session: &EdgeId,
    frame: VitureCameraFrame,
    inserted_tick: u64,
) -> Result<(XrCameraFrameRef, Vec<StoreEvicted>)> {
    let sample = frame.to_ref()?;
    require_glasses_sample_ingest(cx, &sample.to_expr(), receipt, session)?;
    let (key, evicted) = store_glasses_frame(
        store,
        GlassesCapability::Camera,
        frame.frame_key,
        frame.payload,
        receipt,
        inserted_tick,
        frame.size_bytes,
    )?;
    if key.as_symbol() != sample.frame_key() {
        return Err(Error::HostError(
            "stored VITURE frame key did not match emitted camera ref".to_owned(),
        ));
    }
    Ok((sample, evicted))
}

/// Returns the VITURE device identity used in camera frame references.
pub fn viture_camera_device_symbol() -> Symbol {
    Symbol::qualified("device", "viture-glasses")
}

/// Returns the camera lane symbol for `kind`.
pub fn viture_camera_symbol(kind: VitureCameraKind) -> Symbol {
    Symbol::qualified("stream/xr-camera", kind.token())
}

/// Returns the stream sample-kind symbol for VITURE camera frame references.
pub fn viture_camera_sample_kind_symbol() -> Symbol {
    xr_camera_frame_sample_kind_symbol()
}

/// Builds the store key expected for `frame_key`.
pub fn viture_camera_store_key(frame_key: Symbol) -> StoreKey {
    StoreKey::new(frame_key)
}
