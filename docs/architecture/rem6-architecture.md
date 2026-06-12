# rem6 Architecture

This document is the stable architecture overview for rem6. It explains the
design problems inherited from gem5-style simulators, the rem6 runtime shape,
and the invariants that production components must satisfy. It does not track
current migration percentages; that belongs in
`docs/architecture/gem5-to-rem6-migration.md`.

The local gem5 reference tree under `temp/reference_designs/gem5` is audit
input only. rem6 production code must not import it, execute it, depend on its
build outputs, or require it at runtime.

## gem5 Pain Points

rem6 is designed around recurring failure modes in large gem5 deployments:

- A global event queue makes cross-component timing hard to inspect, hard to
  parallelize safely, and hard to checkpoint with pending work.
- Python SimObject composition is flexible but weakly checked; topology owner,
  port, and route errors often surface late, after construction or after a run
  begins.
- Classic caches, Ruby protocols, DRAM timing, and external traffic consumers
  are split across different vocabularies, which makes end-to-end attribution
  difficult.
- SLICC-generated protocol controllers hide transient state, stalls, and wakeup
  causality behind generated code and implicit queues.
- Statistics, debug output, and traces can become string-oriented side channels
  instead of runtime contracts.
- Full-system experiments often rely on external scripts, manually assembled
  artifacts, and implicit guest-host communication.

rem6 keeps gem5's modeling ambition but rejects hidden global state as the
default implementation style.

## rem6 Architecture

rem6 is a Rust workspace for deterministic, parallel-first full-system
simulation. The core runtime is partitioned: components that can schedule work
or hold timing state have explicit ownership, and cross-partition interaction is
a typed message with source, target, timing, and ordering identity.

The simulator's main design innovations are ordinary Rust crate ownership,
typed runtime contracts, and explicit cross-partition authority. It uses these
instead of generated protocol or Python object graphs. ISA, CPU, cache,
coherence, fabric, DRAM, devices, platforms, workloads, checkpoints, stats, and
CLI surfaces are separate owners with typed APIs and tests at their boundaries.

The memory path uses a shared transaction vocabulary. CPU, GPU, accelerator,
DMA, trace replay, cache, coherence, fabric, and DRAM code should expose
timing, protocol state, data movement, and stall causes as inspectable records
instead of requiring consumers to infer behavior from logs.

Host control is part of the modeled boundary. Guest exits, stats resets,
checkpoints, workload dispatch, device launches, debug requests, and syscall or
host-assist traffic pass through typed events or explicit runtime state.

External integration stays behind typed adapters. Co-simulation frameworks,
external power tools, external trace formats, and native loader libraries may
be useful at the boundary, but rem6 keeps scheduler authority, checkpoint
authority, and runtime evidence in rem6-owned types.

## Runtime Invariants

1. Partition ownership is explicit.
   A timed component must declare the scheduler partition that owns its state.

2. Cross-partition communication is a message.
   A partition does not directly mutate another partition's timed state.

3. Remote messages respect lookahead.
   Legal remote delivery cannot schedule work in the past or bypass the
   scheduler's minimum remote delay.

4. Local events and remote messages stay distinct.
   A zero-delay local pipeline event cannot masquerade as a remote transfer.

5. Equal-tick ordering is deterministic.
   Tie breaks must be stable enough for tests, traces, and checkpoints.

6. Serial execution is a compatibility view.
   Serial execution may reduce overhead, but it must not allow behavior that
   the parallel runtime rejects.

7. Backpressure is explicit.
   Queues, credits, MSHRs, transaction buffers, DRAM banks, NoC resources, and
   device command queues expose occupancy and stall causes.

8. Blocking creates a wait-for edge.
   A blocked component records what resource, component, or protocol event it
   is waiting for.

9. Protocol transitions are typed data.
   Coherence transitions, snoops, retries, ownership transfers, and stalls are
   records, not string-only traces.

10. Checkpoints are protocol neutral.
    A snapshot must capture authoritative state, pending timing work, dirty
    ownership, queues, and frontiers without depending on a specific flush-only
    coherence protocol.

11. Observability is a runtime feature.
    Run artifacts expose scheduler activity, stop reasons, transport traffic,
    data movement, device work, stats, probes, and host actions in structured
    form.

12. Model switching preserves authority.
    Any transition between execution modes must transfer architectural and
    timing authority through typed snapshots or handoff records.

## Workspace Responsibilities

| Area | Primary owners | Responsibility |
| --- | --- | --- |
| Kernel, topology, and transport | `rem6-kernel`, `rem6-topology`, `rem6-transport`, `rem6-fabric` | Partitioned scheduling, topology graphs, remote delivery, route timing, fabric activity, wait-for diagnostics. |
| ISA and CPU | `rem6-isa-riscv`, `rem6-isa-x86`, `rem6-cpu` | Decode, architectural state, traps, execution engines, frontend and pipeline state, branch prediction. |
| Memory hierarchy | `rem6-memory`, `rem6-cache`, `rem6-directory`, `rem6-coherence`, `rem6-protocol-msi`, `rem6-protocol-mesi`, `rem6-protocol-moesi`, `rem6-protocol-chi`, `rem6-dram` | Stores, page maps, cache banks, replacement, MSHRs, coherence state machines, DRAM/NVM timing. |
| Devices and platforms | `rem6-mmio`, `rem6-amba`, `rem6-uart`, `rem6-timer`, `rem6-interrupt`, `rem6-pci`, `rem6-virtio`, `rem6-storage`, `rem6-net`, `rem6-platform`, `rem6-boot` | MMIO, bus protocols, interrupt, timer, block, network, PCI, VirtIO, board, DTB, boot, and handoff models. |
| System integration | `rem6-system`, `rem6` | Topology assembly, host actions, syscall emulation, full-system runs, CLI execution, stats artifacts. |
| Workloads and resources | `rem6-workload`, `rem6-traffic`, `rem6-proto` | Manifests, resources, trace replay, traffic generation, suite planning, execution evidence. |
| Observability | `rem6-stats`, `rem6-debug`, `rem6-power`, `rem6-checkpoint` | Stats, probes, GDB/RSP, power and thermal records, checkpoint manifests and restore validation. |
| Heterogeneous execution | `rem6-gpu`, `rem6-accelerator` | GPU and accelerator commands, DMA routes, queueing, topology validation, replay evidence. |

## Evidence Policy

Architecture claims require executable rem6 evidence: Rust tests, typed traces,
stats dumps, checkpoint records, CLI artifacts, workload manifests, or runtime
summaries. External references and the gem5 tree can motivate design, but they
are not acceptance evidence for rem6 behavior.

Documentation should use stable semantic anchors: directory names, type names,
function names, test names, or section headings. Do not cite fragile exact line
ranges.

Detailed evidence belongs in tests and runtime artifacts. This file should stay
stable when a feature is added; migration percentages and current gaps belong
in the migration document.

## Disallowed Patterns

- Hidden global timing authority.
- String-only protocol, stats, or debug evidence for behavior that tests need
  to reason about.
- Topology construction that accepts unknown owners, ports, routes, or
  resource shapes and waits for runtime failure.
- Production dependencies on the read-only gem5 reference tree.
- Checkpoint correctness that depends on a single protocol-specific flush
  mechanism.
- Migration claims based only on related helper types without executable
  behavior evidence.
