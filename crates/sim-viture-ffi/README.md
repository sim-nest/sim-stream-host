# sim-viture-ffi

`sim-viture-ffi` is the unsafe-isolated VITURE SDK loading boundary for
`sim-stream-host`. The crate loads a local SDK dynamically, exposes safe handles
and result types, and returns a hardware-free unsupported result when no SDK is
available.

The normal stream-host workspace remains `unsafe_code = "forbid"`; this package
is the exception that owns the dynamic-link boundary.
