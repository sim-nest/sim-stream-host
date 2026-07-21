# sim-lib-stream-wristbridge

In one line: A local wrist bridge turns watch exports into SIM worn events without cloud accounts.

## What it gives you

The crate gives SIM one watch provider surface for four local routes: Linux BLE,
a phone relay, a Zepp companion bridge, and file imports. Each route speaks the
same worn-event shape and accepts the same notification, haptic, face, alarm, and
privacy commands, so a watch integration can swap transport without changing the
rest of the stream host.

It also gives test and CI runs a clean hardware-free path. Synthetic imports and
scripted link samples produce stable event sequences, while the stub route says
plainly that no device is available.

## Why you will be glad

You can work on watch behavior without pairing a real device, signing into a
vendor service, or carrying private captures in the repo. Live routes and
offline exports meet at the same event contract, which makes failures easier to
replay and command delivery easier to inspect.

## Where it fits

This crate sits inside the stream host. It adapts watch-specific transports into
the shared device provider and session contracts, while the device base, consent
checks, stream storage, and view commands stay in their owning libraries.
