# sim-stream-host

Plug SIM's live streams into real audio/MIDI gear and across the network: host
backends move stream packets between the runtime and external transports or
platform devices, over a bounded, allocation-free, non-blocking queue.

SIM is a small Rust protocol kernel plus loadable libraries; the `sim` CLI
installs with `cargo install sim-run`, and `sim-say` is the full walkthrough.

## Example: bounded backpressure ring

The path between a host callback and the runtime is a preallocated bounded ring
that drops rather than blocks or allocates when full. Add the crate:

```bash
cargo add sim-lib-stream-host
```

```rust
use sim_lib_stream_host::{ProcessRingPush, ProcessSharedRing};

let mut ring = ProcessSharedRing::with_capacity(2).unwrap();
assert_eq!(ring.try_push(1), ProcessRingPush::Accepted);
assert_eq!(ring.try_push(2), ProcessRingPush::Accepted);
assert_eq!(ring.try_push(3), ProcessRingPush::DroppedNewest(3));
assert_eq!(ring.try_pop(), Some(1));
```

Source: passing doctest in `src/ring.rs:42` (`ProcessSharedRing::with_capacity`).
For a deterministic, device-free host-backend descriptor see the recipe at
`recipes/01-basics/fake-backend/`.

## About

sim-stream-host is the host-device stream substrate of the SIM constellation.
SIM is an expandable Rust runtime built around a small protocol kernel plus a
large set of loadable libraries: the kernel defines contracts, libraries provide
behavior. This repository holds the runtime-side host backends that move stream
packets across the boundary between SIM and external transports or platform
devices, layered on the stream model from `sim-lib-stream-core`.

A host backend opens a stream from a configuration request and hands callbacks a
cloneable bounded queue to enqueue packets through non-blocking calls. Backends
advertise their integration features as capability metadata, enumerate devices
and ports, and are looked up through a deterministic registry. The crate ships a
deterministic fake backend for validation and replay alongside an RTP-MIDI
backend; sockets and platform devices are never opened during normal
validation, and device smoke tests stay ignored unless a matching external peer
or device is present.

## Crates

| Crate | Role |
| --- | --- |
| `sim-lib-stream-host` | Host-device stream backend substrate: the `HostBackend` trait and opened-stream handle, host stream configuration and capability records, device/port inventory, a deterministic backend registry, the cloneable bounded callback queue and preallocated process-site ring buffer, callback cassette recording/replay, LAN peer placement policy, and the `FakeBackend` and feature-gated `RtpMidiBackend` implementations. |
| `sim-lib-stream-halo` | Local Halo glasses provider for direct BLE, Web Bluetooth, and phone-relay routes, with shared XR inputs, consent-gated one-shot camera references, and byte-budgeted Lua cell diffs for `scene/glance` output. |
| `sim-lib-stream-viture` | Local VITURE glasses provider that publishes XR pose samples through the shared stream-device session surface and accepts display or IMU-control commands while keeping normal validation hardware-free. |
| `sim-lib-stream-wristbridge` | Local wrist bridge routes that adapt watch imports, BLE, phone relay, and Zepp companion data into the shared worn-device provider surface without enabling hardware lanes during CI. |
| `sim-viture-ffi` | Unsafe-isolated VITURE SDK boundary that discovers a local dynamic SDK, wraps Carina pose and IMU/control entry points, and returns a hardware-free unsupported result when no SDK is loadable. |

## Architecture

A `HostBackend` turns a `HostStreamConfigRequest` into an opened stream, exposing
clock, latency, and reconnect configuration through host config records. The path
between a host callback and the runtime is bounded: callbacks hold a cloneable
`HostCallbackQueue` and packets cross the process-site boundary through a
preallocated `ProcessSharedRing`, so enqueue calls stay non-blocking and
allocation-free. Backends declare `HostBackendCapability` metadata gating each
integration feature (media direction, hotplug/reconnect, the fake transport) and
register through a deterministic `HostBackendRegistry`. LAN placement policy
selects sites for host-managed stream fragments hosted by non-real-time peers,
reporting refusals and experimental diagnostics rather than silently degrading.

The audio-provider seam loads native audio placement providers through the
kernel loader under the `audio.provider.native` capability. Providers register
`AudioSite` values with an `AudioRouter`, and `DeviceCatalog` exposes those
sites beside modeled devices. A missing provider leaves modeled placement live,
so default validation stays hardware-free.

## Validation

This repository builds standalone against the published SIM crates on
crates.io. The public validation gate is:

```bash
cargo fmt --all --check
cargo run -p xtask -- check-file-sizes
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo clippy --workspace --all-features --all-targets -- -D warnings
cargo test --workspace --all-features
cargo run -p xtask -- simdoc --check
```

## Documentation Lanes

`cargo run -p xtask -- simdoc` builds the public documentation lanes:

- API docs: `target/doc/`
- Agent cards: `docs/agents/cards.jsonl` and `docs/agents/card-index.json`
- Human docs: `docs/humans/`
- Diagrams: `docs/diagrams/src/` and `docs/diagrams/generated/`

The same command writes split contract files under `docs/generated/`.

## File Size Gate

`cargo run -p xtask -- check-file-sizes` scans Rust source files and fails when
an entrypoint (`lib.rs`, `main.rs`, or `mod.rs`) exceeds 250 lines or any other
Rust source file exceeds 700 lines.
