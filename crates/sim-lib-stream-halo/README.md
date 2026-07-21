# sim-lib-stream-halo

Local Brilliant Labs Halo provider and byte-budgeted Lua glance renderer.

The crate adapts direct BlueZ, Web Bluetooth, and local phone-relay routes to the
shared stream-device session surface. It emits shared XR motion, tap,
microphone-reference, and camera-reference records plus a strict Halo button
record. Camera output is a consent-gated one-shot pull whose pixels remain in a
bounded content store.

`scene/glance` frames are converted to injection-safe Lua cell primitives. A
stateful scheduler sends changed cells only, respects a hard per-tick byte
ceiling, prioritizes urgency and safety warrants, and carries deferred cells
into later ticks.
