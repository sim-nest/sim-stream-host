//! VITURE SDK pose-status mapping for XR tracking samples.

use sim_lib_stream_xr::XrTrackingStatus;
use sim_viture_ffi::VitureStatus;

/// Maps a successful VITURE SDK pose status into the XR tracking status carried
/// by stream samples.
pub fn viture_tracking_status(status: VitureStatus) -> XrTrackingStatus {
    if status.code() == 0 {
        XrTrackingStatus::Tracked
    } else {
        XrTrackingStatus::Lost
    }
}
