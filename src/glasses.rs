//! Provider-side glasses consent gates for XR stream expressions.

use sim_kernel::{CapabilityName, Cx, Error, Expr, Result, Symbol};
use sim_lib_view_device::{ConsentReceipt, EdgeId, require_with_consent};
use sim_value::{access, build};

use crate::{BoundedContentStore, ContentFrame, RetentionWindow, StoreEvicted, StoreKey};

/// Capability required for glasses pose and tracking samples.
pub const CAP_GLASSES_POSE: &str = "glasses/pose";

/// Capability required for glasses camera frames.
pub const CAP_GLASSES_CAMERA: &str = "glasses/camera";

/// Capability required for stable world-anchor observations.
pub const CAP_GLASSES_WORLD_ANCHOR: &str = "glasses/world-anchor";

/// Capability required for glasses hand-ray samples.
pub const CAP_GLASSES_HAND: &str = "glasses/hand";

/// Capability required for glasses microphone capture.
pub const CAP_GLASSES_MIC: &str = "glasses/mic";

/// Capability required for vendor diagnostic reporting.
pub const CAP_GLASSES_VENDOR_REPORT: &str = "glasses/vendor-report";

const XR_NAMESPACE: &str = "xr";
const GLASSES_NAMESPACE: &str = "glasses";

/// Glasses-sensitive provider capability classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassesCapability {
    /// Pose or spatial tracking samples.
    Pose,
    /// Camera frame references.
    Camera,
    /// Stable world-anchor observations.
    WorldAnchor,
    /// Hand-ray samples.
    Hand,
    /// Raw microphone chunk references.
    Mic,
    /// Vendor diagnostics, off unless explicitly granted.
    VendorReport,
}

impl GlassesCapability {
    /// All provider-side glasses capabilities.
    pub const ALL: [Self; 6] = [
        Self::Pose,
        Self::Camera,
        Self::WorldAnchor,
        Self::Hand,
        Self::Mic,
        Self::VendorReport,
    ];

    /// Stable kernel capability name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pose => CAP_GLASSES_POSE,
            Self::Camera => CAP_GLASSES_CAMERA,
            Self::WorldAnchor => CAP_GLASSES_WORLD_ANCHOR,
            Self::Hand => CAP_GLASSES_HAND,
            Self::Mic => CAP_GLASSES_MIC,
            Self::VendorReport => CAP_GLASSES_VENDOR_REPORT,
        }
    }

    /// Stable local token after the `glasses/` prefix.
    pub fn local_name(self) -> &'static str {
        match self {
            Self::Pose => "pose",
            Self::Camera => "camera",
            Self::WorldAnchor => "world-anchor",
            Self::Hand => "hand",
            Self::Mic => "mic",
            Self::VendorReport => "vendor-report",
        }
    }

    /// Kernel capability value.
    pub fn capability_name(self) -> CapabilityName {
        CapabilityName::new(self.as_str())
    }

    /// Visible consent grant symbol carried by a receipt.
    pub fn grant_symbol(self) -> Symbol {
        Symbol::qualified(GLASSES_NAMESPACE, self.local_name())
    }

    /// Resolves a glasses capability name.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|capability| capability.as_str() == name)
    }
}

/// Returns the visible grant symbol for glasses pose samples.
pub fn glasses_pose_grant() -> Symbol {
    GlassesCapability::Pose.grant_symbol()
}

/// Returns the visible grant symbol for glasses camera frames.
pub fn glasses_camera_grant() -> Symbol {
    GlassesCapability::Camera.grant_symbol()
}

/// Returns the visible grant symbol for glasses world-anchor observations.
pub fn glasses_world_anchor_grant() -> Symbol {
    GlassesCapability::WorldAnchor.grant_symbol()
}

/// Returns the visible grant symbol for glasses hand-ray samples.
pub fn glasses_hand_grant() -> Symbol {
    GlassesCapability::Hand.grant_symbol()
}

/// Returns the visible grant symbol for glasses microphone capture.
pub fn glasses_mic_grant() -> Symbol {
    GlassesCapability::Mic.grant_symbol()
}

/// Returns the visible grant symbol for glasses vendor diagnostics.
pub fn glasses_vendor_report_grant() -> Symbol {
    GlassesCapability::VendorReport.grant_symbol()
}

/// Classifies an XR stream expression into the glasses capability it needs.
pub fn glasses_capability_for_sample(sample: &Expr) -> Result<GlassesCapability> {
    if access::field(sample, "world-anchor").is_some() {
        return Ok(GlassesCapability::WorldAnchor);
    }
    let symbol = access::field_sym(sample, "sample")
        .or_else(|| access::field_sym(sample, "kind"))
        .ok_or_else(|| Error::Eval("missing glasses sample or kind field".to_owned()))?;
    match (symbol.namespace.as_deref(), symbol.name.as_ref()) {
        (Some(XR_NAMESPACE), "pose") => Ok(GlassesCapability::Pose),
        (Some(XR_NAMESPACE), "camera-frame") => Ok(GlassesCapability::Camera),
        (Some(XR_NAMESPACE), "hand") => Ok(GlassesCapability::Hand),
        (Some(XR_NAMESPACE), "mic-chunk") => Ok(GlassesCapability::Mic),
        (Some(GLASSES_NAMESPACE), "world-anchor") => Ok(GlassesCapability::WorldAnchor),
        (Some(GLASSES_NAMESPACE), "vendor-report") => Ok(GlassesCapability::VendorReport),
        _ => Err(Error::HostError(format!(
            "no glasses consent capability for {}",
            symbol.as_qualified_str()
        ))),
    }
}

/// Requires kernel authority, visible consent, and same-session receipt ownership.
pub fn require_glasses_consent(
    cx: &Cx,
    capability: GlassesCapability,
    receipt: &ConsentReceipt,
    session: &EdgeId,
) -> Result<()> {
    require_with_consent(cx, capability.as_str(), receipt, session)
}

/// Requires the authority needed to ingest one glasses-sensitive stream expression.
pub fn require_glasses_sample_ingest(
    cx: &Cx,
    sample: &Expr,
    receipt: &ConsentReceipt,
    session: &EdgeId,
) -> Result<GlassesCapability> {
    let capability = glasses_capability_for_sample(sample)?;
    require_glasses_consent(cx, capability, receipt, session)?;
    Ok(capability)
}

/// Stores one by-reference glasses frame in a bounded content store.
pub fn store_glasses_frame(
    store: &mut BoundedContentStore,
    capability: GlassesCapability,
    ref_id: Symbol,
    value: Expr,
    receipt: &ConsentReceipt,
    inserted_tick: u64,
    size_bytes: usize,
) -> Result<(StoreKey, Vec<StoreEvicted>)> {
    let key = StoreKey::new(ref_id);
    let evicted = store.insert(ContentFrame::new(
        key.clone(),
        receipt.session.as_symbol().clone(),
        receipt.seq,
        inserted_tick,
        size_bytes,
        stored_glasses_value(capability, value),
    ))?;
    Ok((key, evicted))
}

/// Builds bounded-store retention windows from glasses consent receipts.
pub fn glasses_retention_windows(receipts: &[ConsentReceipt]) -> Vec<RetentionWindow> {
    receipts
        .iter()
        .filter(|receipt| {
            receipt
                .grants
                .iter()
                .any(|grant| grant.namespace.as_deref() == Some(GLASSES_NAMESPACE))
        })
        .map(|receipt| {
            RetentionWindow::new(
                receipt.session.as_symbol().clone(),
                receipt.seq,
                receipt.retain_ms,
            )
        })
        .collect()
}

/// Sweeps stored glasses frames under modeled-clock retention windows.
pub fn sweep_glasses_retention(
    store: &mut BoundedContentStore,
    receipts: &[ConsentReceipt],
    now_tick: u64,
    adapt_hz: u16,
) -> Vec<StoreEvicted> {
    store.sweep_retention(now_tick, adapt_hz, &glasses_retention_windows(receipts))
}

fn stored_glasses_value(capability: GlassesCapability, value: Expr) -> Expr {
    build::map(vec![
        ("kind", build::qsym("glasses", "content-frame")),
        ("capability", Expr::Symbol(capability.grant_symbol())),
        (capability.local_name(), value),
    ])
}
