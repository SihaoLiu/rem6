# rem6: Rust Emulator 6

A Rust-reimplementation of gem5

## Workspace

rem6 is organized as one Rust workspace with one crate per simulator subsystem.
Current subsystem crates cover deterministic scheduling, typed topology,
typed guest-host system events, typed statistics, typed boot images, typed
CPU fetch initiation, multicore CPU cluster validation, fetched RV64I execution,
transport-backed RV64I data accesses, RV64I decode and architectural execution,
interrupts, MMIO, timers, UART devices, platform assembly, memory transactions
and storage, transport, fabric timing, DRAM timing, MSI protocol state, cache
controllers, directory arbitration, and coherence harnesses.

Run the current verification suite with:

```bash
cargo test --workspace
```

Code under `temp/` is scratch/reference material only. It is not part of the
workspace and must not be used as a build dependency.
