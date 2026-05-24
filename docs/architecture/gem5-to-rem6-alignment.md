# gem5 to rem6 Alignment

This document tracks the module-by-module relationship between the local gem5
reference tree and rem6. It is the working index for making rem6 a
gem5-class, cycle-accurate, heterogeneous full-system simulator while removing
the accumulated debt called out in
`docs/architecture/parallel-first-full-system-kernel.md`.

The local gem5 tree is a read-only reference. rem6 production code must not
import it, execute it, depend on its build outputs, or require it at runtime.
The reference is used only to identify modeling scope, useful design ideas, and
gaps in rem6 coverage.

## External Pain Point Research

External research reinforces that rem6 must be a new implementation, not a
line-for-line port. The important gem5 pain points are architectural rather than
isolated bugs:

- Parallel simulation is not a natural baseline in gem5. Historical Parallel M5
  material describes a migration away from a global event queue toward per-object
  queues, barriers, slack, and queue assignment. Current gem5 event-queue code
  still exposes global simulation quantum and main event queue concepts. Recent
  par-gem5 and parti-gem5 work treat parallel timing simulation as an extension
  to gem5, with explicit synchronization and, for timing mode, accepted timing
  deviation. rem6 therefore treats partition ownership, lookahead, deterministic
  merge order, and per-partition snapshots as core kernel contracts.
- Configuration and experiment reproducibility are too script-dependent in
  gem5. Official documentation says parts of Learning gem5 are outdated, and the
  standard library exists partly because hand-built scripts can grow to hundreds
  of lines. Full-system runs also require firmware, kernels, disk images, and
  resource coordination outside the simulator binary. rem6 therefore keeps
  typed builders and manifests as the authority for platform and workload state.
- Observability and statistics need stronger contracts. A gem5 issue about
  stats reset explicitly calls out missing reset tests and user confusion from
  inconsistent stats. rem6 therefore treats statistics, activity, wait-for
  graphs, and run summaries as typed data with tests rather than string-only
  logs or ad hoc probes.
- Power equations should not depend on late string lookup into global
  statistics. gem5's MathExprPowerModel accepts equations that reference stat
  names plus automatic variables. rem6 keeps the equation idea, but binds
  metric inputs, temperature, voltage, and clock period through typed records
  before evaluation.
- Compatibility bugs cluster around cross-subsystem seams. Recent public gem5
  issues include syscall-emulation gaps for modern libc behavior, RISC-V vector
  tracing crashes, and a three-level CHI LR/SC race in multicore RISC-V
  workloads. rem6 therefore keeps ISA, guest ABI, coherence, memory ordering,
  and device behavior behind typed crate boundaries with focused regression
  tests before broad parity claims.

Research anchors: gem5 documentation; Parallel M5; gem5 `src/sim/eventq.hh`;
par-gem5; parti-gem5; gem5 issues for stats reset, syscall emulation, RISC-V
vector tracing, and CHI LR/SC behavior.

## Audit Method

The audit is recursive. Each gem5 subtree gets an entry with four facts:

- the stable gem5 source anchor;
- the useful behavior or modeling idea worth preserving;
- the rem6 crate or crates that own the equivalent behavior;
- the current evidence and remaining alignment target.

Coverage levels:

- `covered`: rem6 has typed runtime behavior and tests for the corresponding
  capability.
- `partial`: rem6 has a narrower equivalent or an early model, but not yet the
  full gem5-class surface.
- `planned`: the gem5 capability is in scope but no production rem6 model owns
  it yet.
- `external-adapter`: gem5 integrates an external simulator or library; rem6
  should keep the interoperability value but isolate it behind typed adapters,
  not make the external package part of the core simulator.

The file counts below come from the local reference tree and are used only to
size the audit. They are not acceptance evidence. Acceptance evidence must be a
rem6 test, typed trace, runtime summary, checkpoint record, or explicit error.

## Top-Level Module Map

| gem5 source anchor | Local files | rem6 owner | Coverage | Alignment target |
| --- | ---: | --- | --- | --- |
| `src/arch` | 1187 | `rem6-isa-riscv`, future ISA crates | partial | Keep per-ISA decoding and architectural state as isolated crates. RISC-V exists; ARM, x86, Power, SPARC, MIPS, and AMDGPU ISA support need equivalent crate ownership before claiming parity. |
| `src/base` | 199 | `rem6-kernel`, `rem6-stats`, shared crate utilities | partial | Preserve useful statistics, loader, debug, and helper concepts without a large untyped utility layer. Runtime-visible data must remain typed. |
| `src/cpu` | 363 | `rem6-cpu`, `rem6-kernel`, `rem6-system` | partial | RISC-V cluster execution exists. gem5 simple, checker, Minor, O3, branch prediction, KVM-style switching, and traffic testers need typed rem6 equivalents or explicit replacement models. |
| `src/dev` | 418 | `rem6-mmio`, `rem6-uart`, `rem6-timer`, `rem6-interrupt`, `rem6-gpu`, `rem6-accelerator`, `rem6-platform` | partial | UART, timer, interrupt, GPU, and accelerator paths exist. PCI, storage, network, virtio, PS/2, QEMU bridge, and platform-specific devices remain alignment targets. |
| `src/gpu-compute` | 73 | `rem6-gpu`, `rem6-accelerator`, `rem6-transport` | partial | Preserve command queues, compute-unit scheduling, DMA, and traceability. Current rem6 GPU execution is a smaller typed model. |
| `src/kern` | 18 | `rem6-system`, `rem6-platform`, workload resources | planned | Linux and guest ABI helpers need a typed full-system boundary instead of ad hoc scripts. |
| `src/mem` | 682 | `rem6-memory`, `rem6-transport`, `rem6-cache`, `rem6-directory`, `rem6-coherence`, `rem6-dram`, `rem6-fabric`, protocol crates | partial | rem6 already splits protocol state, topology, NoC, DRAM, replacement state, MSHR resources, prefetch queues, stores, directory state, and coherence harnesses into typed crates. CHI-like line states, a single-line cache controller, a multi-line cache bank, an initial directory decision model, serial plus partitioned multi-cache coherence harnesses, topology-built CHI cache-directory and DRAM routes, and CHI recorded run-resource summaries exist; broader CHI transactions, prefetcher breadth, cache QoS, and Ruby-network breadth remain open. |
| `src/python` | 253 | `rem6-workload`, `rem6-platform`, future front ends | partial | Keep gem5's ease of composition while replacing Python object wiring with checked manifests and typed builders. |
| `src/sim` | 176 | `rem6-kernel`, `rem6-system`, `rem6-checkpoint`, `rem6-stats`, `rem6-power` | partial | Event queues, ticks, objects, exit events, power hooks, probes, checkpoints, and statistics need typed partitioned equivalents. Core scheduling, typed probe events, typed power domains, and checkpoints exist. |
| `src/systemc` | 3911 | future `rem6-systemc` or adapter crate | external-adapter | Preserve interoperability only through an adapter boundary. Core rem6 timing must not depend on SystemC. |
| `src/sst` | 6 | future SST adapter crate | external-adapter | Preserve co-simulation value behind a typed boundary that cannot bypass rem6 partition ownership. |
| `src/proto` | 9 | `rem6-proto`, future adapters | partial | Protobuf-like exchange must produce typed rem6 data before entering simulation. rem6-proto has typed instruction, packet, and O3 dependency trace records, validation, canonical maps, window-checked dependencies, stable identity, checked binary frame envelopes, length-delimited frame streams with stream magic, version, varint32 record lengths, embedded-frame validation, a resettable cursor that exposes record indexes and byte offsets, a validated stream index with kind counts, payload byte totals, identities, and byte ranges, deterministic shard plans over contiguous records, shard-local cursors that support independent out-of-order ingestion while preserving global record indexes, deterministic worker assignment plans that separate parallel shard ownership from merge order, worker-local cursors over non-contiguous assigned shards, a merge buffer that turns out-of-order worker records back into global record order, a parallel reader that hides worker poll order from output order, and a stream-bound parallel ingestion plan that derives the validated index, shard plan, worker plan, reader, threaded worker decode path, and per-worker decode summary from one stream; concrete protobuf and gzip adapters remain open. |
| `src/learning_gem5`, `src/doc`, `src/doxygen`, `src/test_objects` | 39 | docs and tests | partial | Keep useful examples as audit input, but rem6 acceptance is through Rust tests and architecture docs. |

## Configuration and Experiment Surface

| gem5 source anchor | Local files | rem6 owner | Coverage | Alignment target |
| --- | ---: | --- | --- | --- |
| `configs/common` | 25 | `rem6-platform`, `rem6-workload` | partial | Common system assembly should become typed platform builders and manifests with validation. |
| `configs/example` | 81 | `rem6-workload`, `rem6-system` tests | partial | Preserve easy examples, but every example should be reconstructable from a manifest and tested where practical. |
| `configs/ruby` | 17 | `rem6-coherence`, protocol crates, `rem6-system` | partial | Keep multi-protocol examples while avoiding a separate Ruby-like engine. |
| `configs/topologies` | 10 | `rem6-topology`, `rem6-fabric`, `rem6-transport` | partial | Topology definitions should be protocol-neutral and reusable across CPU, GPU, DMA, and accelerator traffic. |
| `configs/dram`, `configs/nvm` | 5 | `rem6-dram`, `rem6-memory` | partial | External DDR, HBM, LPDDR, and NVM profiles have typed topology, geometry, bank-group geometry, timing, burst spacing, same-bank-group burst spacing, command-window bandwidth limits, manifest identity, checkpoint encoding, and activity metadata. NVM media timing can model separate read-media, write-media, send latency, pending-read buffers, and pending-write queue depth. Profile breadth and fuller media behavior remain open. |
| `configs/network` | 2 | `rem6-fabric`, `rem6-transport` | partial | Network configuration must map to NoC lanes, virtual networks, credits, and wait-for diagnostics. |
| `configs/boot`, `configs/dist`, `configs/splash2`, `configs/learning_gem5`, `configs/deprecated` | 27 | `rem6-boot`, `rem6-workload`, tests | partial | Boot and benchmark examples should become manifest resources, not external scripts. Deprecated examples are audit input only. |

## Detailed Module Map

### ISA and Architecture

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/arch/riscv` | `rem6-isa-riscv`, `rem6-cpu` | partial | RV64I decode and execution exist. Privileged ISA, MMU, atomics, vectors, and richer traps are alignment targets. |
| `src/arch/arm`, `src/arch/x86`, `src/arch/power`, `src/arch/sparc`, `src/arch/mips` | future ISA crates | planned | rem6 should add each ISA as a crate with isolated decode, architectural state, and tests. |
| `src/arch/amdgpu` | `rem6-gpu`, future GPU ISA crate | planned | Current GPU model is command-level. ISA-level GPU execution remains open. |
| `src/arch/generic`, `src/arch/null`, `src/arch/isa_parser` | shared ISA traits and build tooling | planned | rem6 should prefer Rust traits and generated tables only when generated artifacts are checked and reviewable. |

### CPU Models

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/cpu/simple` | `rem6-cpu`, `rem6-system` | partial | rem6 has simple RISC-V core and cluster tests with fetch, data access, traps, and host stop. |
| `src/cpu/minor` | future in-order pipeline crate or `rem6-cpu` module | planned | Needs cycle-visible pipeline state, stalls, branch effects, and checkpoints. |
| `src/cpu/o3` | future out-of-order CPU crate | planned | Needs rename, issue, reorder, load/store queue, speculation, squash, and typed traces. |
| `src/cpu/pred` | `rem6-cpu` branch predictor modules | partial | A local two-bit predictor, GShare predictor, BiMode predictor, Tournament predictor, loop predictor, TAGE base predictor, LTAGE predictor, TAGE-SC-L wrapper, standalone multiperspective perceptron predictor, 8KB statistical corrector, branch target buffer, indirect target predictor, and return-address stack have independent typed prediction, lookup, update, target, replacement, speculative history, commit, repair, and snapshot state. GShare keeps gem5's PC xor GHR indexing while replacing opaque history pointers with typed records, per-CPU GHR snapshots, masked squash repair, and restore validation. BiMode keeps gem5's PC-indexed choice table and PC xor GHR direction tables while exposing selected-array training and choice-counter policy as typed records. Tournament keeps gem5's shared local history table, per-CPU global history, global-history-indexed choice table, disagreement-only choice training, and squash repair while exposing each record as typed state. The loop predictor keeps gem5's set/way indexing, tag matching, confidence threshold, use counter, and optional speculative iteration state while replacing random allocation with deterministic per-set cursors for replayable parallel runs. TAGE base keeps gem5's bimodal table, tagged-table provider selection, folded-history index and tag hashing, alt-on-new counter, useful-bit reset, and repairable speculative history while exposing deterministic allocation records. LTAGE composes TAGE and loop prediction with explicit final provider records, loop-before-TAGE conditional training, combined repair, matching thread and instruction-shift validation, and nested snapshot restore. TAGE-SC-L composes LTAGE with the statistical corrector in gem5 order: TAGE and loop predict before SC override, and SC trains before loop and TAGE updates, with explicit repair and nested snapshot records. The standalone multiperspective perceptron keeps gem5's 8KB feature profile shape, transfer tables, filter behavior, low-confidence best-table path, adaptive training threshold, and local/global/path/IMLI/recency histories while making all table and history state typed and per CPU. The 8KB statistical corrector keeps gem5's bias tables, global/backward/local/IMLI GEHL sums, confidence chooser, threshold training, and repairable histories while making histories per CPU for parallel replay instead of hidden shared global state. The indirect target predictor replaces opaque history pointers and random target replacement with typed records, per-CPU history, deterministic LRU, and restore validation. The specialized 8KB/64KB TAGE-SC-L table geometry, 64KB statistical corrector extensions, and MPP_TAGE integration remain open. |
| `src/cpu/checker` | `rem6-cpu` verification harness | planned | Checker behavior should compare architectural commits without hidden simulator state. |
| `src/cpu/kvm` | host-controlled execution modes | partial | rem6 models execution modes and statistics scope; host-assisted native execution is not present yet. |
| `src/cpu/testers`, `src/cpu/trace`, `src/cpu/probes` | tests, trace, stats crates | partial | Traffic generation, trace replay, and probes should feed typed events and summaries. |

### Memory, Cache, Coherence, and NoC

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/mem/cache` | `rem6-cache`, `rem6-coherence`, protocol crates | partial | MSI, MESI, MOESI, and an initial CHI-like state machine, cache controller, cache bank, directory model, and serial plus partitioned coherence harnesses exist with tests. The CHI controller can issue ReadShared, ReadUnique, and MakeReadUnique-shaped downstream requests, complete shared and unique fills, preserve pending-miss snapshots, service local slices, and apply snoop downgrade or invalidation to resident data. The CHI cache bank owns multiple line controllers, assigns unique downstream request IDs across lines, records pending fills, can attach typed MSHRs, coalesces same-line read misses without extra downstream traffic, fans coalesced fills out as multiple target outcomes, and restores pending MSHR targets from snapshots. The CHI directory tracks unique clean/dirty ownership, shared clean/dirty sharers, deterministic SnoopShared and SnoopUnique decisions, owner-cache versus backing-memory data sources, MakeReadUnique no-data upgrades, dirty-peer data sourcing, and sorted snapshot restore. The CHI serial harness connects multiple cache controllers to the directory, applies directory snoops before fills, transfers owner-cache data to requesters, updates backing data when a unique dirty owner downgrades to shared clean, records CPU responses and directory decisions, and snapshots directory, cache, backing, response, and decision state. The CHI partitioned harness routes request and response work through `MemoryTransport` and `PartitionedScheduler`, waits for owner snoops before peer fills, preserves dirty owner data when downgrading to shared clean, records directory decisions and CPU responses with scheduler ticks, restores quiescent scheduler, directory, cache, backing, trace, response, and decision state, exposes recorded run summaries and parallel run history, and can derive cache-directory and directory-DRAM routes from typed topology components including multi-hop and fabric-backed links. LRU, FIFO, MRU, LFU, BRRIP, BIP, SHIP, SecondChance, and TreePLRU replacement policies have typed per-set state, victim decisions, invalidation, reset, touch, access-signature training, and snapshot restore. MSHR queues have typed entry allocation, target coalescing, prefetch reserve, ready/service state, and snapshot restore. MSI cache banks can attach typed MSHRs, coalesce same-line read misses without extra downstream traffic, replay all coalesced targets on fill, restore coalesced targets from snapshots, and fan coalesced fills out through the MSI bank directory harness and MSI transport/deferred/snoop response collectors as multiple CPU response records. MESI and MOESI cache banks can attach typed MSHRs, coalesce same-line read misses, and fan coalesced fills out as multiple target outcomes. Typed stride and tagged prefetchers have deterministic candidate generation, per-request metadata, source addresses, and snapshot restore. A typed queued prefetch resource models gem5's queued prefetch latency, duplicate filtering with higher-priority duplicate updates, same-line demand squash, page-boundary dropping when no translation path is configured, in-cache or in-miss-queue redundant filtering, optional lowest-priority oldest eviction when full, next-ready-tick visibility, and accuracy throttle state with control percentage, issued/useful counters, max-permitted computation, useful-count invariant checks, and snapshot restore. The queued resource applies that throttle through an explicit enqueue path shared by typed prefetch candidates with ready-tick ordering, same-tick priority ordering, stable order ties, explicit capacity, line size, optional page size, issue width, accepted/duplicate/priority-update/redundant/page-crossing/throttled/full result counts, and full policy before packet creation or cache-controller side effects. A typed multi queued prefetcher preserves gem5's `Multi` earliest-ready query and round-robin source issue behavior while exposing source identity, keeping no-op polls side-effect free, and issuing only one entry from the chosen source. Prefetcher breadth, full MMU translation queues, cache QoS, richer cache tags, full-system CHI data-cache attribution, and full CHI transaction coverage remain open. |
| `src/mem/ruby` | `rem6-coherence`, `rem6-directory`, `rem6-fabric` | partial | rem6 keeps detailed coherence and NoC behavior without a second memory-stack vocabulary. |
| `src/mem/slicc` | protocol crates and typed transition records | partial | rem6 should preserve protocol expressiveness while avoiding generated controllers that hide transient behavior. |
| `src/mem/protocol` | `rem6-protocol-msi`, `rem6-protocol-mesi`, `rem6-protocol-moesi`, `rem6-protocol-chi` | partial | MSI, MESI, and MOESI exist. The CHI-like crate covers typed `I`, shared clean/dirty, unique clean/dirty, ReadShared, ReadUnique, MakeReadUnique upgrade, snoop downgrade, invalidation, busy rejection, transition trace, and directory unique-owner validation. Full CHI request, response, data, DVM, retry, credit, and Ruby-network interactions remain open. |
| `src/mem/qos` | `rem6-fabric`, `rem6-dram`, `rem6-transport`, `rem6-workload` | partial | rem6-fabric has typed QoS requestor IDs, checked priorities, fixed-priority assignment, FIFO/LIFO/LRG queue arbitration, non-mutating empty polls, queue-arbiter snapshots, and QoS-ordered fabric batch transmission that reserves shared links in grant order. rem6-transport can attach a shared QoS arbiter to parallel batch submission so request priority and requestor identity affect first-hop NoC reservation before partition events are scheduled, and can order single- and multi-hop direct same-tick target deliveries with the same typed arbiter before invoking target handlers. rem6-dram can order same-arrival timing batches through the same typed arbiter before bank, row, and bus timing are computed, prefer the current read/write bus direction among same-priority candidates, explicitly escalate queued same-requestor candidates to their best assigned batch priority without embedding controller back pointers in the queue policy, accept memory-controller QoS batches before storage responses are generated, pair responses with scheduled DRAM grant order, and preserve assigned priority, effective priority, requestor, byte count, and escalation status as typed DRAM activity metadata. Parallel coherence, system, DMA, and workload-result summaries expose DRAM QoS access, byte, escalation, priority, and requestor diagnostics directly from typed activity profiles. Workload manifests declare fixed-priority QoS policy, queue policy, turnaround policy, priority escalation, and per-requestor priority intent as typed replay-plan state; workload replay applies declared fixed-priority and queue policy to shared fabric first-hop reservation, applies declared fixed-priority, queue, turnaround, and escalation policy to direct profiled DRAM accesses so replay summaries carry DRAM priority and requestor metadata, lets same-tick single- and multi-hop direct DRAM deliveries observe manifest QoS before target handling, and coalesces same-tick direct QoS deliveries to the same profiled DRAM target into one memory-controller batch when no data-cache override handles the target. This preserves gem5's fixed-priority, queue-policy, turnaround, escalation, and bandwidth-accounting concepts while avoiding global requestor lookup, memory-controller back pointers, SimObject-name-only setup, and string-only stats. Cached target-handler batching and mixed cache/DRAM coalescing remain open. |
| `src/mem/probes` | `rem6-stats`, runtime summaries | partial | Observability should be typed counters, typed probe points/listeners/events, and run summaries, not string-only probes. |
| memory ports, packets, requests in `src/mem` root | `rem6-transport`, `rem6-memory` | partial | Shared request/response transport exists; more gem5 packet semantics need mapping as features are added. |

### DRAM and External Memory

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `configs/dram`, `ext/drampower`, `ext/dramsim2`, `ext/dramsim3`, `ext/dramsys` | `rem6-dram`, adapter crates | partial | rem6 has internal DRAM timing, burst spacing, same-bank-group burst spacing, command-window bandwidth limits, bank-group geometry, activity, and profiles. External DRAM simulators should be optional adapters. |
| `configs/nvm`, `src/mem/NVMInterface.py`, `src/mem/nvm_interface.*`, memory profile code | `rem6-memory`, `rem6-dram` | partial | NVM targets have typed controller/media-bank topology and can round-trip through manifests, checkpoints, and DRAM target activity metadata. DRAM activity profiles preserve typed read/write byte counts, and NVM target activity exposes persistent write access, byte counters, max pending NVM reads, max pending persistent writes, profile-level media timing, access-level persistent-ready cycles, checkpointed pending read/write completions, NVM read-buffer/write-queue wait-for diagnostics, and manifest identity for NVM media timing without string stats. Richer NVM-specific bandwidth behavior remains open. |
| HBM, LPDDR, DDR class profiles | `rem6-dram` | partial | The profile shape exists for DDR, HBM, LPDDR, and NVM; a broader library of validated profiles is still needed. |

### Heterogeneous Devices

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/gpu-compute` | `rem6-gpu` | partial | rem6 has GPU command submission, workgroup completion, DMA, traces, summaries, and checkpoints at a coarse level. |
| `src/dev/amdgpu`, `src/dev/hsa` | `rem6-gpu`, future GPU ISA and runtime modules | planned | Full GPU system support needs richer queues, address spaces, interrupts, and ISA-visible state. |
| NPU-style accelerators, not a single gem5 subtree | `rem6-accelerator` | partial | rem6 already models accelerator engines, command lanes, DMA, summaries, and checkpoints. |
| `src/dev/pci`, `src/dev/virtio`, `src/dev/storage`, `src/dev/net` | future device crates | planned | PCI, block, network, and virtio devices remain required for full-system breadth. |
| `src/dev/serial`, `src/dev/riscv`, `src/dev/lupio`, `src/dev/i2c` | `rem6-uart`, `rem6-mmio`, `rem6-interrupt`, `rem6-timer`, future device crates | partial | UART, timer, MMIO, and interrupts exist. Other platform devices need typed models. |
| platform-specific device trees under `src/dev/arm`, `src/dev/x86`, `src/dev/mips`, `src/dev/sparc` | future platform crates | planned | These should arrive with the corresponding ISA and platform support. |

### Simulation Kernel, Checkpointing, and Host Control

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| event queue and tick logic in `src/sim` | `rem6-kernel` | covered | Partitioned scheduling, conservative epochs, deterministic order, lookahead, and scheduler snapshots exist. |
| SimObject and Python configuration in `src/sim` and `src/python` | `rem6-platform`, `rem6-workload` | partial | rem6 should keep ease of composition through typed builders and manifests rather than dynamic object graphs. |
| checkpoint support in `src/sim` | `rem6-checkpoint`, `rem6-system` checkpoint banks | partial | Protocol-neutral checkpoint records exist for several subsystems. More devices and pending-state rejection remain open. |
| statistics, probes, and power hooks | `rem6-stats`, `rem6-power`, run summaries | partial | Counters, stats snapshots, typed probe registries, probe listener state, typed power states/domains, power residency snapshots, typed state-weighted dynamic/static power models, typed expression-based dynamic/static power models, typed stat-snapshot metric binding, typed RC thermal domains, typed multi-domain thermal-network solving with resistor and capacitor edges, and probe event snapshots exist. Broader power-controller and external-analysis adapter breadth remains open. |
| guest-host events and pseudo instructions | `rem6-system`, `rem6-workload` | partial | ROI, stats, checkpoint, checkpoint restore, stop, and execution mode actions are typed. Broader guest ABI support remains open. |

### External Integration and Tooling

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `ext/systemc`, `src/systemc`, `util/systemc`, `util/tlm` | future adapter crates | external-adapter | Interoperability is useful, but rem6 timing authority must stay in the partitioned runtime. |
| `ext/sst`, `src/sst`, `configs/example/sst` | future SST adapter | external-adapter | SST co-simulation should be explicit and checkpoint-aware. |
| `ext/nomali`, `ext/mcpat`, `ext/dsent` | future optional analysis adapters | external-adapter | Preserve modeling value behind typed records and stable APIs. |
| `ext/libelf`, `ext/libfdt`, `ext/softfloat`, `ext/gdbremote` | `rem6-boot`, future debug and ISA support crates | planned | rem6 needs equivalent ELF/FDT/soft-float/debug capability without vendoring unneeded code into the core. |
| `util/gem5art`, resources tooling, disk image tooling | `rem6-workload`, future artifact tooling | partial | rem6 manifests should make artifact provenance first-class and reproducible. |
| `tests`, `tests/test-progs`, `util/statetrace` | rem6 tests and trace tooling | partial | gem5 tests are audit input. rem6 acceptance remains Rust tests and typed trace comparison. |

## Evidence Already Present in rem6

- Partitioned scheduler tests cover deterministic event order, lookahead,
  conservative parallel epochs, worker limits, wait-for graphs, and scheduler
  snapshots.
- Full-system RISC-V tests drive multicore fetch, data access, traps, host stop,
  statistics, and run summaries through the partitioned scheduler.
- Transport, fabric, DRAM, cache, directory, and coherence crates expose typed
  activity records rather than depending on string logs.
- MSI, MESI, MOESI, and initial CHI-like line-state, cache-controller,
  cache-bank, directory, serial plus partitioned coherence-harness behavior,
  topology-built CHI cache-directory and DRAM routes, and CHI recorded
  run-resource summaries have test coverage; full system paths already report
  MSI/MESI/MOESI data-cache protocol attribution.
- GPU and accelerator paths route command and DMA work through typed topology,
  transport, scheduler activity, summaries, and checkpoint banks.
- CPU branch prediction exposes typed direction prediction, GShare PC-history
  indexing, BiMode choice and direction-array training, Tournament
  local/global/choice training, loop trip-count learning, LTAGE loop override
  integration, statistical-corrector GEHL override, branch-target lookup, TAGE
  folded-history indexing and provider selection, indirect-target lookup,
  deterministic replacement, update, target, speculative history, return-stack
  operation, commit, repair, and snapshot records with restore validation.
- Cache prefetch tests cover tagged next-line candidate generation,
  page-boundary candidate dropping, multi-source queued prefetch earliest-ready
  reporting, deterministic round-robin issue, non-mutating no-op polls, and
  single-entry issue from the selected source.
- Fabric QoS tests cover explicit fixed-priority requestor mapping,
  highest-priority queue selection, FIFO/LIFO same-priority ordering, LRG
  requestor rotation, non-mutating empty polls, snapshot replay, and
  QoS-driven fabric batch reservation order on a shared link. Transport tests
  cover QoS-driven first-hop fabric reservation before parallel batch events
  are scheduled. DRAM tests cover QoS-driven same-arrival request ordering
  before bank and bus timing are computed, plus typed read/write direction
  preference among same-priority timing candidates, explicit same-requestor
  priority escalation inside timing batches, per-priority/per-requestor QoS
  access and byte accounting in DRAM activity profiles, gem5-like burst
  spacing across same-direction commands on a shared port, gem5-like
  same-bank-group burst spacing for bank-group memories, and gem5-like
  command-window bandwidth limits across row and data commands. NVM profile tests
  cover typed read/write byte accounting, persistent write counters, NVM media
  timing, pending-read buffer limits, pending-write queue limits, checkpoint
  round-trip of media/pending state, NVM queue wait-for diagnostics, and
  manifest identity changes for media timing. Checkpoint and workload identity
  tests cover command-window timing, bank-group timing, and per-port command
  history state. Coherence, system, DMA, and
  workload-result summary tests cover direct DRAM QoS diagnostics over those
  typed activity profiles.
- Stats tests cover counter reset epochs and typed probe point, listener,
  event, payload, and snapshot records.
- Proto-boundary tests cover typed instruction, packet, and O3 dependency trace
  records, one-of instruction encoding, memory-access and packet-size
  validation, duplicate id-string rejection, canonical id-string ordering,
  dependency sequence/window validation, duplicate dependency-record rejection,
  stable trace identity, binary frame round-trip, frame kind validation,
  truncation rejection, checksum mismatch rejection, ordered frame-stream
  round-trip, empty stream rejection, stream magic and version validation,
  zero-length and overlong varint32 length validation, corrupt embedded-frame
  rejection, cursor byte-offset reporting, cursor reset, cursor EOF behavior,
  cursor non-advancement after corrupt input, validated stream-index metadata,
  kind filtering, payload byte totals, index rejection of corrupt streams,
  contiguous shard-plan construction, shard byte-range reporting, per-shard
  kind counts, oversized-frame preservation, zero-budget rejection, shard-local
  cursor reset, out-of-order shard reads, per-shard corruption isolation, stable
  least-loaded worker assignment, worker load totals, merge-order preservation,
  zero-worker rejection, worker-local cursor reset, non-contiguous assigned-shard
  reads, unknown-worker rejection, corruption isolation across workers,
  deterministic worker-record merge buffering, duplicate-record rejection,
  wrong-worker rejection, out-of-range worker-record rejection, poll-order
  independent parallel-reader output, round-robin full drain, and reader-level
  unknown-worker rejection, end-to-end parallel-ingestion plan construction,
  ingestion-plan budget and worker error propagation, and ingestion-plan stream
  error propagation, including reader rejection when bytes do not match the
  planned stream, threaded worker decode with deterministic global-order merge,
  threaded decode rejection of unplanned bytes, and threaded decode summaries
  with per-worker assignment, record, frame-byte, and payload-byte totals.
- Power tests cover typed power state domains, leader/follower matching,
  residency accounting, transition counters, invalid transition rejection, and
  snapshot restore. Power-model tests cover residency-weighted dynamic/static
  watt aggregation, static/dynamic-only modes, temperature updates, missing
  state-model rejection, and snapshot restore. Power-expression tests cover
  typed metric inputs, automatic temperature/voltage/clock-period variables,
  expression arithmetic, residency-weighted dynamic/static aggregation, missing
  metric rejection, invalid expression-result rejection, duplicate state-model
  rejection, typed `StatId` to power-metric binding from stats snapshots,
  duplicate binding rejection, missing bound-stat rejection, input updates, and
  snapshot restore. Thermal tests cover RC domain temperature updates from typed
  power estimates, expression input temperature coupling, invalid thermal
  parameter rejection, time-order rejection, update history, and snapshot
  restore. Thermal-network tests cover fixed references, thermal resistors,
  thermal capacitors, multi-domain implicit temperature solving, typed power
  inputs, expression input temperature coupling, invalid topology rejection,
  update history, and snapshot restore.
- Workload manifests record boot images, resources, topology, host events,
  checkpoint lineage, typed QoS policy intent, result metadata, execution mode
  switches, host action summaries, checkpoint restore labels, and statistics
  snapshots.

## Open Alignment Work

- Expand ISA support beyond RISC-V while preserving crate isolation.
- Add in-order pipeline, out-of-order pipeline, checker, richer branch
  predictors, and host-assisted execution models with checkpointable state.
- Complete CHI-like coherence beyond the current line-state crate, single-line
  cache controller, multi-line cache bank, initial directory model, and serial
  plus initial partitioned, topology-built cache-directory, and topology-built
  DRAM-backed harnesses with recorded resource summaries, plus
  richer cache internals such as additional prefetcher algorithms, prefetch
  translation and snoop integration, QoS,
  sector/compressed tags, and cache write queues.
- Broaden device coverage to PCI, virtio, storage, network, richer GPU runtime,
  and platform-specific devices.
- Add optional adapters for SystemC, TLM, SST, DRAM simulators, power models,
  and debug tooling without weakening the rem6 core runtime boundary.
- Grow the manifest and workload resource model until gem5-style full-system
  experiments are reproducible without external scripts as the authority.
- Continue replacing indirect profiling with typed summaries for every runtime
  resource, including queues, credits, banks, lanes, commands, and wait-for
  diagnostics.

## Maintenance Rules

- When a gem5 subtree is audited in detail, update this file with its stable
  source anchor, rem6 owner, coverage level, evidence, and remaining target.
- Do not cite exact line ranges from gem5 or rem6. Use directory names,
  function names, type names, test names, or documentation headings.
- Do not treat a green local test as broad parity. A test proves only the
  capability it actually exercises.
- Do not add rem6 production dependencies on the local reference tree.
- Keep module mapping honest. `partial` is the correct status until current
  evidence proves gem5-equivalent or stronger behavior.
