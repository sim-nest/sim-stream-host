# sim-lib-stream-viture

Local VITURE glasses provider for SIM XR stream samples.

The crate adapts VITURE headset pose and display-control routes to the shared
stream-device provider session surface. It publishes XR pose samples as ordinary
device-stream expressions with monotone sequence numbers, and it reports a clean
unsupported result when no local SDK is available.

Unsafe vendor loading stays in `sim-viture-ffi`; this crate keeps the provider
and command path in safe Rust.
