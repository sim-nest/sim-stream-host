use sim_kernel::Symbol;

use crate::{
    DevicePlacement, DeviceProfile, DeviceSite, DeviceSiteLocality, GlassesAdapterKind,
    GlassesPlacementError, PlacementError, resolve_glasses_placement,
};

#[test]
fn glasses_route_placements_keep_adapters_edge_local() {
    let cases = [
        (
            "direct-linux",
            DeviceSiteLocality::HostLocal,
            GlassesAdapterKind::Reprojector,
        ),
        (
            "android-usb",
            DeviceSiteLocality::HostLocal,
            GlassesAdapterKind::Reprojector,
        ),
        (
            "neckband-local",
            DeviceSiteLocality::EdgeLocal,
            GlassesAdapterKind::Reprojector,
        ),
        (
            "neckband-relay",
            DeviceSiteLocality::Remote,
            GlassesAdapterKind::Reprojector,
        ),
        (
            "mobile-dock-display",
            DeviceSiteLocality::HostLocal,
            GlassesAdapterKind::Mirror,
        ),
        (
            "ble-direct",
            DeviceSiteLocality::HostLocal,
            GlassesAdapterKind::Glance,
        ),
        (
            "web-bluetooth",
            DeviceSiteLocality::HostLocal,
            GlassesAdapterKind::Glance,
        ),
        (
            "phone-relay",
            DeviceSiteLocality::Remote,
            GlassesAdapterKind::Glance,
        ),
        (
            "controller-hid",
            DeviceSiteLocality::HostLocal,
            GlassesAdapterKind::ControllerIntent,
        ),
    ];

    for (route, encoder_locality, adapter) in cases {
        let plan = resolve_glasses_placement(
            Symbol::qualified("device/route", route),
            DeviceProfile::modeled_edge(),
        )
        .expect("valid glasses placement");

        assert_eq!(plan.route.name.as_ref(), route);
        assert_eq!(plan.adapter, adapter);
        assert_eq!(plan.placement.encoder.locality, encoder_locality);
        assert_eq!(
            plan.placement.adapter.locality,
            DeviceSiteLocality::EdgeLocal
        );
        assert_eq!(plan.placement.validate(), Ok(()));
    }
}

#[test]
fn glasses_placement_rejects_unknown_route() {
    let route = Symbol::qualified("device/route", "vendor-cloud");
    assert_eq!(
        resolve_glasses_placement(route.clone(), DeviceProfile::modeled_edge()),
        Err(GlassesPlacementError::UnknownRoute(route))
    );
}

#[test]
fn glasses_placement_rejects_non_edge_adapter() {
    let profile = DeviceProfile::modeled_edge();
    let codec = Symbol::qualified("codec", "scene-spatial");
    let placement = DevicePlacement::new(
        DeviceSite::host_local(
            Symbol::qualified("device/site", "glasses-encoder"),
            profile.clone(),
            codec.clone(),
        ),
        DeviceSite::remote(
            Symbol::qualified("device/site", "glasses-reprojector"),
            profile,
            codec,
        ),
    );

    assert_eq!(
        placement.validate(),
        Err(PlacementError::AdapterMustBeEdgeLocal)
    );
}
