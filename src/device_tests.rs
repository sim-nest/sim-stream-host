use sim_kernel::{Expr, Symbol};

use crate::{
    DeviceError, DevicePlacement, DeviceProvider, DeviceSession, DeviceSite, PlacementError,
    StubProvider,
};

#[test]
fn device_stub_unsupported_and_placement_guard() {
    let profile = crate::DeviceProfile::modeled_edge();
    let provider = StubProvider::new(profile.clone());
    assert_eq!(provider.profile(), &profile);
    assert!(matches!(provider.open(), Err(DeviceError::Unsupported)));

    let mut session = provider.session();
    assert_eq!(session.profile(), provider.profile());
    assert!(matches!(session.start(), Err(DeviceError::Unsupported)));
    assert!(matches!(
        session.poll("device-caps"),
        Err(DeviceError::Unsupported)
    ));
    assert!(matches!(
        session.send(&Expr::Symbol(Symbol::qualified("device/command", "haptic"))),
        Err(DeviceError::Unsupported)
    ));
    session.stop().unwrap();

    let codec = Symbol::qualified("codec", "lisp");
    let encoder = DeviceSite::edge_local(
        Symbol::qualified("device/site", "encoder"),
        provider.profile().clone(),
        codec.clone(),
    );
    let adapter = DeviceSite::remote(
        Symbol::qualified("device/site", "adapter"),
        provider.profile().clone(),
        codec,
    );
    let placement = DevicePlacement::new(encoder, adapter);

    assert_eq!(
        placement.validate(),
        Err(PlacementError::AdapterMustBeEdgeLocal)
    );
}
