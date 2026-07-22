//! Consent-gated one-shot Halo camera capture.

use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_stream_device::DeviceSample;
use sim_lib_stream_host::{
    BoundedContentStore, GlassesCapability, StoreEvicted, StoreKey, require_glasses_sample_ingest,
    store_glasses_frame,
};
use sim_lib_stream_xr::{XrCameraFrameRef, halo_device_symbol};
use sim_lib_view_device::{ConsentReceipt, EdgeId};

/// Fixed Halo camera output dimensions.
pub const HALO_CAMERA_SIZE_PX: [u32; 2] = [640, 480];

/// One explicit camera pull before its payload is moved to the bounded store.
#[derive(Clone, Debug, PartialEq)]
pub struct HaloCameraPull {
    seq: u64,
    frame_key: Symbol,
    t_ns: u64,
    payload: Expr,
    size_bytes: usize,
}

impl HaloCameraPull {
    /// Builds one opt-in camera pull.
    pub fn new(
        seq: u64,
        frame_key: Symbol,
        t_ns: u64,
        payload: Expr,
        size_bytes: usize,
    ) -> Result<Self> {
        if size_bytes == 0 {
            return Err(Error::Eval(
                "Halo camera pull size must be nonzero".to_owned(),
            ));
        }
        Ok(Self {
            seq,
            frame_key,
            t_ns,
            payload,
            size_bytes,
        })
    }

    /// Builds the by-reference XR camera sample for this pull.
    pub fn to_ref(&self) -> Result<XrCameraFrameRef> {
        XrCameraFrameRef::new(
            self.seq,
            halo_device_symbol(),
            Symbol::qualified("stream/xr-camera", "halo-rgb"),
            self.frame_key.clone(),
            HALO_CAMERA_SIZE_PX,
            self.t_ns,
            false,
        )
        .map_err(|error| Error::HostError(error.to_string()))
    }
}

/// Performs one camera pull under kernel authority and same-session consent.
///
/// The consumed request represents exactly one capture. The payload is stored
/// by reference and only its fixed 640x480 metadata leaves this function.
pub fn pull_halo_camera_once(
    cx: &Cx,
    store: &mut BoundedContentStore,
    receipt: &ConsentReceipt,
    session: &EdgeId,
    pull: HaloCameraPull,
    inserted_tick: u64,
) -> Result<(XrCameraFrameRef, Vec<StoreEvicted>)> {
    let sample = pull.to_ref()?;
    require_glasses_sample_ingest(cx, &sample.to_expr(), receipt, session)?;
    let (key, evicted) = store_glasses_frame(
        store,
        GlassesCapability::Camera,
        pull.frame_key,
        pull.payload,
        receipt,
        inserted_tick,
        pull.size_bytes,
    )?;
    if key.as_symbol() != sample.frame_key() {
        return Err(Error::HostError(
            "stored Halo frame key did not match emitted camera ref".to_owned(),
        ));
    }
    Ok((sample, evicted))
}

/// Builds the bounded-store key for a Halo camera pull.
pub fn halo_camera_store_key(frame_key: Symbol) -> StoreKey {
    StoreKey::new(frame_key)
}
