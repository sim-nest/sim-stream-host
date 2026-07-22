//! Placement planning for glasses encoders and latency-critical adapters.

use std::fmt;

use sim_kernel::Symbol;

use crate::{DevicePlacement, DeviceProfile, DeviceSite, DeviceSiteLocality, PlacementError};

/// Local adapter selected for a resolved glasses profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassesAdapterKind {
    /// Stereo pose-coupled Viture reprojector.
    Reprojector,
    /// Mono Halo glance adapter.
    Glance,
    /// Display-only mirror adapter.
    Mirror,
    /// Ordinary HID-to-Intent adapter.
    ControllerIntent,
}

impl GlassesAdapterKind {
    /// Returns the stable adapter-site token.
    pub const fn token(self) -> &'static str {
        match self {
            Self::Reprojector => "glasses-reprojector",
            Self::Glance => "glasses-glance-adapter",
            Self::Mirror => "glasses-mirror-adapter",
            Self::ControllerIntent => "glasses-controller-intent",
        }
    }

    /// Returns the surface codec selected by this adapter.
    pub fn surface_codec_id(self) -> Symbol {
        let name = match self {
            Self::Reprojector => "scene-spatial",
            Self::Glance => "scene-glance",
            Self::Mirror => "scene-mirror",
            Self::ControllerIntent => "intent",
        };
        Symbol::qualified("codec", name)
    }
}

/// Placement request for one already-resolved glasses route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlassesPlacementRequest {
    /// Stable `device/route` symbol selected by profile resolution.
    pub route: Symbol,
    /// Stream-facing provider profile carried by both sites.
    pub profile: DeviceProfile,
    /// Locality chosen for the non-latency-critical encoder.
    pub encoder_locality: DeviceSiteLocality,
    /// Latency-critical local adapter implementation.
    pub adapter: GlassesAdapterKind,
}

impl GlassesPlacementRequest {
    /// Builds a placement request.
    pub fn new(
        route: Symbol,
        profile: DeviceProfile,
        encoder_locality: DeviceSiteLocality,
        adapter: GlassesAdapterKind,
    ) -> Self {
        Self {
            route,
            profile,
            encoder_locality,
            adapter,
        }
    }

    /// Resolves and validates the placement plan.
    pub fn resolve(self) -> Result<ResolvedGlassesPlacement, PlacementError> {
        let codec = self.adapter.surface_codec_id();
        let encoder = DeviceSite::new(
            route_site_symbol(&self.route, "encoder"),
            self.profile.clone(),
            codec.clone(),
            self.encoder_locality,
        );
        let adapter = DeviceSite::edge_local(
            Symbol::qualified("device/site", self.adapter.token()),
            self.profile,
            codec,
        );
        let placement = DevicePlacement::new(encoder, adapter);
        placement.validate()?;
        Ok(ResolvedGlassesPlacement {
            route: self.route,
            adapter: self.adapter,
            placement,
        })
    }
}

/// Validated placement for one glasses route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedGlassesPlacement {
    /// Stable route symbol.
    pub route: Symbol,
    /// Adapter selected for the resolved device tier.
    pub adapter: GlassesAdapterKind,
    /// Validated encoder and edge-local adapter placement.
    pub placement: DevicePlacement,
}

/// Error returned while resolving a glasses route placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GlassesPlacementError {
    /// The route token is not part of the glasses route contract.
    UnknownRoute(Symbol),
    /// The resulting device placement violated a locality invariant.
    InvalidPlacement(PlacementError),
}

impl fmt::Display for GlassesPlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRoute(route) => write!(f, "unknown glasses route: {route}"),
            Self::InvalidPlacement(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for GlassesPlacementError {}

impl From<PlacementError> for GlassesPlacementError {
    fn from(error: PlacementError) -> Self {
        Self::InvalidPlacement(error)
    }
}

/// Resolves a stable glasses route symbol to a validated placement.
pub fn resolve_glasses_placement(
    route: Symbol,
    profile: DeviceProfile,
) -> Result<ResolvedGlassesPlacement, GlassesPlacementError> {
    if route.namespace.as_deref() != Some("device/route") {
        return Err(GlassesPlacementError::UnknownRoute(route));
    }
    let (encoder_locality, adapter) = match route.name.as_ref() {
        "direct-linux" | "android-usb" => (
            DeviceSiteLocality::HostLocal,
            GlassesAdapterKind::Reprojector,
        ),
        "neckband-local" => (
            DeviceSiteLocality::EdgeLocal,
            GlassesAdapterKind::Reprojector,
        ),
        "neckband-relay" => (DeviceSiteLocality::Remote, GlassesAdapterKind::Reprojector),
        "mobile-dock-display" => (DeviceSiteLocality::HostLocal, GlassesAdapterKind::Mirror),
        "ble-direct" | "web-bluetooth" => {
            (DeviceSiteLocality::HostLocal, GlassesAdapterKind::Glance)
        }
        "phone-relay" => (DeviceSiteLocality::Remote, GlassesAdapterKind::Glance),
        "controller-hid" => (
            DeviceSiteLocality::HostLocal,
            GlassesAdapterKind::ControllerIntent,
        ),
        _ => return Err(GlassesPlacementError::UnknownRoute(route)),
    };
    GlassesPlacementRequest::new(route, profile, encoder_locality, adapter)
        .resolve()
        .map_err(Into::into)
}

fn route_site_symbol(route: &Symbol, role: &str) -> Symbol {
    Symbol::qualified("device/site", format!("{}-{role}", route.name))
}
