# sim-stream-host

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

These commands run in the constellation workspace; only `sim-kernel` builds from
a lone clone today (see `DEVELOPING.md` in `sim-sdk`).

```bash
cargo fmt --check && cargo test --workspace && cargo clippy --workspace -- -D warnings && cargo doc --workspace --no-deps
cargo run -p xtask -- simdoc --check
```

## Documentation Lanes

`cargo run -p xtask -- simdoc` builds the public documentation lanes:

- API docs: `target/doc/`
- Agent cards: `docs/agents/cards.jsonl` and `docs/agents/card-index.json`
- Human docs: `docs/humans/`
- Diagrams: `docs/diagrams/src/` and `docs/diagrams/generated/`

The same command writes split contract files under `docs/generated/`.
