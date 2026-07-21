# sim-lib-stream-halo

In one line: Halo glasses receive compact, timely glance updates while local sensor routes stay private and hardware-independent.

## What it gives you

It connects Halo glasses through direct Bluetooth, a browser, or a local phone
relay. Motion, taps, buttons, microphone chunks, and deliberate camera captures
arrive as the same device-stream records used across SIM, while display updates
send only the glyph cells that actually changed.

## Why you will be glad

The tight glasses link does not waste its budget redrawing an unchanged screen.
Urgent and safety-marked content goes first, ordinary changes continue over later
ticks, camera access stays explicit, and normal validation needs no radio or
vendor service.

## Where it fits

It sits in sim-stream-host beside the VITURE and wrist providers. Shared XR
records describe inputs, the shared glance scene describes output, and the local
Lua protocol turns that output into bounded partial updates for the display.
