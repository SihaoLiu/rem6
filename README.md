# rem6: Rust Emulator 6

A Rust-reimplementation of gem5

## Workspace

rem6 is organized as one Rust workspace with one crate per simulator subsystem.
Current subsystem crates cover deterministic scheduling, threaded partition
epochs, typed topology, typed guest-host system events, typed statistics, typed
boot images, typed CPU fetch initiation, parallel CPU and RISC-V fetch issue,
parallel RISC-V cluster fetch and data turns, parallel RISC-V cluster MMIO data
turns, multicore CPU cluster validation, fetched RV64I execution,
transport-backed RV64I data accesses, ready-driven RV64I core actions, RV64I
multicore cluster driving, deterministic ready-core sweeps, host-side RISC-V
cluster turns, bounded RISC-V cluster run traces, RV64I decode, architectural
execution, typed RISC-V traps, scheduler-delivered trap events, CPU
pending-trap host delivery,
scheduler-owned serial and parallel CPU trap scheduling, parallel interrupt,
timer, and UART RX signal delivery, host-stopped parallel RISC-V system cluster
runs, system-level parallel RISC-V MMIO data paths, RISC-V
committed-instruction stats, host-integrated full-system checkpoint capture and
restore, serial and parallel MMIO channels and buses, parallel timer MMIO
programming, parallel interrupt-controller MMIO, parallel UART MMIO devices,
MMIO-programmable interrupt priorities, priority-aware interrupt arbitration,
platform assembly, memory transactions and storage, serial and parallel memory
transport, memory store snapshots and checkpoints, fabric timing, DRAM timing,
typed MSI and MESI protocol state, MSI and MESI cache controllers, directory
arbitration for MSI and MESI, MSI coherence harnesses, and serial, partitioned,
and parallel-drain MESI directory harnesses with owner downgrade writeback.

Run the current verification suite with:

```bash
cargo test --workspace
```
