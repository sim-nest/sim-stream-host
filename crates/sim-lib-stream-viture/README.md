# sim-lib-stream-viture

Local VITURE glasses provider for SIM XR stream samples.

The crate adapts VITURE headset pose and display-control routes to the shared
stream-device provider session surface. It publishes XR pose samples as ordinary
device-stream expressions with monotone sequence numbers, and it reports a clean
unsupported result when no local SDK is available.

Unsafe vendor loading stays in the `sim-platform`-owned `sim-viture-ffi`
capsule. This crate keeps the provider, profile, sample conversion, consent,
retention, and command semantics in safe Rust.
