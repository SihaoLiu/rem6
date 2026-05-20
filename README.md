# rem6: Rust Emulator 6

A Rust-reimplementation of gem5

## Workspace

rem6 is organized as one Rust workspace with one crate per simulator subsystem.
Current subsystem crates cover deterministic scheduling, threaded partition
epochs, typed topology, typed guest-host system events, typed statistics, typed
boot images, typed CPU fetch initiation, multicore CPU cluster validation,
fetched RV64I execution, transport-backed RV64I data accesses, ready-driven
RV64I core actions, RV64I multicore cluster driving, deterministic ready-core
sweeps, host-side RISC-V cluster turns, bounded RISC-V cluster run traces, RV64I
decode, architectural execution, typed RISC-V traps, scheduler-delivered trap
events, CPU pending-trap host delivery, scheduler-owned and batched CPU trap
scheduling, interrupts, host-stopped RISC-V system cluster runs, RISC-V
committed-instruction stats, host-integrated full-system checkpoint capture and
restore, MMIO, timers, UART devices, platform assembly, memory transactions and
storage, transport, memory store snapshots and checkpoints, fabric timing, DRAM
timing, MSI protocol state, cache controllers, directory arbitration, and
coherence harnesses.

Run the current verification suite with:

```bash
cargo test --workspace
```
