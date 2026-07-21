# sim-lib-stream-viture

In one line: A local VITURE glasses bridge turns headset pose and display controls
into SIM device-stream records.

## What it gives you

It gives SIM a focused glasses route for VITURE hardware. The crate advertises a
device profile, opens a local provider session, publishes head-pose records with
stable sequence numbers, and accepts display and sensor-control packets in the
same device session shape used by the rest of the worn stack.

## Why you will be glad

It keeps vendor details out of the runtime while still making the glasses useful
to projection, placement, and replay code. CI stays hardware-free, but the same
provider surface is ready to drive a connected headset when the local SDK is
available.

## Where it fits

It sits in sim-stream-host beside the wrist bridge and uses the shared stream
device and XR sample contracts. The unsafe SDK boundary stays isolated in
sim-viture-ffi; this crate remains ordinary safe Rust.
