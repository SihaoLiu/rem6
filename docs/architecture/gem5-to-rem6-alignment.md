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
  queues, barriers, slack, and queue assignment. Current gem5 documentation
  presents the simulator as an event-driven callback system, and the public
  event-queue API still exposes queue locks, cross-queue migration, async
  insertion, and global simulation quantum behavior. The API documentation
  warns that direct cross-queue locking can deadlock, temporary queue migration
  can make simulation nondeterministic, and deterministic async insertions are
  merged only at quantum boundaries. Recent par-gem5 and parti-gem5 work treat
  parallel timing simulation as an extension to gem5, with explicit
  synchronization and, for timing mode, accepted timing deviation. rem6
  therefore treats partition ownership, lookahead, deterministic merge order,
  per-partition snapshots, scheduler epoch and dispatch progress, bounded
  empty-epoch exposure, verifiable multi-worker batch activity, exact same-batch
  partition-set activity, and sustained same-batch streak activity as core
  kernel contracts.
- Configuration and experiment reproducibility are too script-dependent in
  gem5. Official documentation describes embedded Python configuration,
  behind-the-scenes port connection behavior, and command-line options whose
  effects must be checked in generated config output. The standard library and
  resource package reduce this burden, but they also demonstrate that kernels,
  disk images, benchmark inputs, exit events, and downloads remain workflow
  state outside the core simulator. rem6 therefore keeps typed builders,
  manifests, artifact identities, and explicit boot handoff reports as the
  authority for platform and workload state.
- Error surfaces are split across simulator fatal errors, simulator panics, and
  Python tracebacks. gem5 documentation directs users to different debugging
  paths depending on which layer raised the problem. rem6 therefore returns
  typed validation errors at subsystem boundaries, returns typed scheduler
  errors for parallel worker failures without discarding the partition's
  remaining queued work or leaving global time behind the executed partition
  clock, rolls back local events scheduled by the panicked callback, records
  successful callbacks' remote messages without leaking panicked-callback remote
  messages, records handoff summaries, and requires tests for invalid platform,
  memory, and workload state before broad compatibility claims. Recorded
  parallel runs also expose remote-send records with source, target, tick, and
  source-local order at batch, epoch, and run scope, source-target flow records
  with counts and first/last delivery ticks, plus counts at batch, epoch, run,
  source-partition activity, and target-partition activity scope. RISC-V
  cluster, coherence, full-system run, and workload-result summaries preserve
  those flow records. Workload manifests and replay plans can declare exact
  expected remote-flow counts, exact remote-flow first/last tick windows,
  minimum max-worker use, minimum total-worker activity, minimum scheduler epoch
  and dispatch progress, maximum scheduler empty epochs, minimum multi-worker
  batch counts, exact partition-set batch counts, minimum same-partition-set
  consecutive batch streaks, minimum active partition counts, per-partition
  activity minima, data-cache protocol attribution expectations, data-cache
  run-accounting consistency, data-cache protocol run-count expectations, and
  minimum fabric, DRAM, and aggregate resource activity contracts plus clean
  parallel diagnostic contracts by scheduler or resource scope, so
  cross-partition communication volume, timing drift,
  scheduler progress, scheduler idle drift, real parallel occupancy, sustained
  worker activity, sustained multi-worker batch execution, exact same-batch
  partition co-execution, sustained same-batch co-execution, per-partition
  participation, progress-free transition volume, declared-threshold livelock
  diagnostics, fabric/DRAM resource use, and subsystem-local plus merged
  full-system wait-for/deadlock/livelock cleanliness are observable and
  verifiable without replaying callbacks.
- Observability and statistics need stronger contracts. A gem5 issue about
  stats reset explicitly calls out missing reset tests and user confusion from
  inconsistent stats. rem6 therefore treats statistics, activity, wait-for
  graphs, and run summaries as typed data with tests rather than string-only
  logs or ad hoc probes.
- Simple models are not automatically cheap or transparent. Recent call-stack
  profiling work identifies gem5's layered design as difficult to profile and
  reports TimingSimpleCPU behavior that can be slower than a full out-of-order
  model because of lockup-cache behavior. rem6 therefore does not use model
  names as performance evidence; every runtime resource needs typed activity
  records, queue diagnostics, and tests that expose where simulated time and host
  work are spent.
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

Research anchors refreshed on 2026-05-25:

- Parallel M5: <https://old.gem5.org/Parallel_M5.html>
- gem5 event-driven programming:
  <https://www.gem5.org/documentation/learning_gem5/part2/events/>
- gem5 event queue API:
  <https://doxygen.gem5.org/release/v21-1-0-2/classgem5_1_1EventQueue.html>
- par-gem5: <https://past.date-conference.com/proceedings-archive/2023/DATA/16.pdf>
- parti-gem5: <https://arxiv.org/abs/2308.09445>
- gem5 Python configuration and port wiring:
  <https://www.gem5.org/documentation/learning_gem5/part1/simple_config/>
- gem5 default script behavior:
  <https://www.gem5.org/documentation/learning_gem5/part1/example_configs/>
- gem5 standard library and resources:
  <https://www.gem5.org/documentation/gem5-stdlib/overview>
- gem5 error categories:
  <https://www.gem5.org/documentation/general_docs/common-errors/>
- gem5 call-stack profiling:
  <https://arxiv.org/abs/2605.01419>
- Local read-only reference anchors: gem5 `src/sim`, `src/python`,
  `configs`, `src/mem`, `src/cpu`, and public issues for stats reset,
  syscall emulation, RISC-V vector tracing, and CHI LR/SC behavior.

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
| `src/cpu` | 363 | `rem6-cpu`, `rem6-kernel`, `rem6-system` | partial | RISC-V cluster execution exists, RISC-V data access records expose absent memory-route metadata for MMIO accesses as typed optional state instead of panic-only accessors, and CPU cluster parallel epochs retain both initial and final partition frontiers through full-system run summaries. gem5 simple, checker, Minor, O3, branch prediction, KVM-style switching, and traffic testers need typed rem6 equivalents or explicit replacement models. |
| `src/dev` | 418 | `rem6-mmio`, `rem6-uart`, `rem6-timer`, `rem6-interrupt`, `rem6-gpu`, `rem6-accelerator`, `rem6-platform` | partial | UART, timer, interrupt, an initial typed RISC-V CLINT MMIO model with crate-level snapshot/restore, typed reset policy, platform/topology attachment, typed RISC-V DTS source emission, binary FDT/DTB emission, RISC-V DTB memory/A1 handoff, typed Linux `/chosen` bootargs and initrd DTB metadata, typed DTB and initrd blob installation for store-backed and DRAM-backed memory, GPU, and accelerator paths exist. PCI, storage, network, virtio, PS/2, QEMU bridge, and broader platform-specific devices remain alignment targets. |
| `src/gpu-compute` | 73 | `rem6-gpu`, `rem6-accelerator`, `rem6-transport` | partial | Preserve command queues, compute-unit scheduling, DMA, and traceability. Current rem6 GPU execution is a smaller typed model. |
| `src/kern` | 18 | `rem6-system`, `rem6-platform`, workload resources | partial | RISC-V Linux boot handoff can install initrd bytes, emit matching `/chosen` DTB metadata, place generated or resolved-resource DTBs in guest memory, and set A1 through typed system APIs. Broader Linux symbols, panic/oops hooks, guest ABI helpers, and other ISA kernels remain open. |
| `src/mem` | 682 | `rem6-memory`, `rem6-transport`, `rem6-cache`, `rem6-directory`, `rem6-coherence`, `rem6-dram`, `rem6-fabric`, protocol crates | partial | rem6 already splits protocol state, topology, NoC, DRAM, replacement state, MSHR resources, prefetch queues, stores, directory state, and coherence harnesses into typed crates. CHI-like line states, a single-line cache controller, a multi-line cache bank, an initial directory decision model, serial plus partitioned multi-cache coherence harnesses, topology-built CHI cache-directory and DRAM routes, CHI recorded run-resource summaries, workload-replay CHI data-cache attribution, manifest-verifiable data-cache run attribution, manifest-verifiable data-cache run accounting consistency, manifest-verifiable MSI/MESI/MOESI/CHI data-cache protocol run counts, manifest-verifiable fabric/DRAM/resource activity counts, direct topology CHI data-cache attach, direct topology store-backed and DRAM-backed CPU fetch/data line-layout derivation from addressed memory regions, and MSHR-backed cache bank QoS metadata, ready arbitration, and typed downstream QoS export exist; broader CHI transactions, prefetcher breadth, cache/DRAM QoS policy breadth, and Ruby-network breadth remain open. |
| `src/python` | 253 | `rem6-workload`, `rem6-platform`, future front ends | partial | Keep gem5's ease of composition while replacing Python object wiring with checked manifests and typed builders. Workload manifests now record typed Linux boot handoff intent, including DTB address, bootargs, device-tree resource identity, initrd address range, and initrd resource identity. RISC-V core fetch and data routes must originate from the declared core partition and source endpoint before replay can build a cluster. RISC-V workload replay derives each core's fetch line layout from the memory target containing the current fetch PC instead of assuming the first target or entry target, and derives replay-injected data request line layouts from the memory target containing each data access address. RISC-V data-cache backing routes must be declared explicitly and originate from the data-cache directory partition and endpoint before replay can attach an external-memory backed cache. GPU and accelerator command routes must target the declared device partition and control endpoint, and GPU and accelerator DMA routes must originate from the declared device partition and DMA endpoint. Resolved resource payloads validate required resource id, digest, device-tree kind, initrd kind, initrd byte length, and manifest identity before workload replay installs DTB and initrd bytes into guest memory. Workload-result parallel summaries preserve CPU scheduler, data-cache scheduler, and merged full-system remote-flow records as typed partition pairs with counts and first/last ticks plus scheduler epoch, empty-epoch, dispatch counts, progress-free transition counts, livelock diagnostic counts, merged resource and full-system deadlock diagnostics, total worker counts, batch worker-count histograms, exact batch partition-set histograms, maximum consecutive batch partition-set streaks, per-partition worker, dispatch, remote-send, and remote-receive activity, data-cache total run counts, attributed and unattributed run counts, data-cache protocol run attribution, and fabric/DRAM resource activity; workload manifests include exact expected scheduler, data-cache scheduler, or full-system remote-flow counts and first/last tick windows plus minimum max-worker use, scheduler epoch and dispatch progress, maximum scheduler idle epochs, total-worker activity, multi-worker batch activity, exact batch partition-set activity, sustained same-batch partition-set streak activity, active partition counts, per-partition activity minima, minimum attributed and maximum unattributed data-cache run counts, data-cache accounting consistency, minimum data-cache protocol run counts, minimum fabric/DRAM/resource activity, and clean parallel diagnostic expectations in manifest identity, and replay plans validate wait-for, deadlock, and livelock cleanliness as part of result verification. Boot resources are reproducible data rather than Python workflow side effects. |
| `src/sim` | 176 | `rem6-kernel`, `rem6-system`, `rem6-checkpoint`, `rem6-stats`, `rem6-power` | partial | Event queues, ticks, objects, exit events, power hooks, probes, checkpoints, and statistics need typed partitioned equivalents. Core scheduling, recorded initial and final parallel-epoch partition frontiers, per-partition parallel activity summaries, typed subsystem-local and merged full-system wait-for/deadlock diagnostics, scheduler-recorded progress-free transition aggregation, typed progress-free transition livelock diagnostics, typed probe events, typed power domains, and checkpoints exist. |
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
| `configs/dram`, `configs/nvm` | 5 | `rem6-dram`, `rem6-memory` | partial | External DDR, HBM, LPDDR, and NVM profiles have typed topology, geometry, bank-group geometry, timing, burst spacing, same-bank-group burst spacing, command-window bandwidth limits, manifest identity, checkpoint encoding, and activity metadata. DRAM same-arrival QoS timing batches respect same-agent memory-ordering barriers before priority or turnaround selection. NVM media timing can model separate read-media, write-media, send latency, pending-read buffers, and pending-write queue depth. Profile breadth and fuller media behavior remain open. |
| `configs/network` | 2 | `rem6-fabric`, `rem6-transport` | partial | Network configuration must map to NoC lanes, virtual networks, credits, and wait-for diagnostics. |
| `configs/boot`, `configs/dist`, `configs/splash2`, `configs/learning_gem5`, `configs/deprecated` | 27 | `rem6-boot`, `rem6-workload`, tests | partial | Boot and benchmark examples should become manifest resources, not external scripts. Linux boot handoff manifests now make device-tree and initrd resources explicit, require matching resource definitions, validate resource kind, validate resolved payload digest and initrd size, include bootargs plus DTB/initrd placement in manifest identity, bind resolved payload sets to that manifest identity, and let replay install resolved DTB/initrd bytes without a script side effect. Deprecated examples are audit input only. |

## Detailed Module Map

### ISA and Architecture

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/arch/riscv` | `rem6-isa-riscv`, `rem6-cpu` | partial | RV64I decode and execution exist, with RV64A `LR.W`/`LR.D` and `SC.W`/`SC.D` paths that record a typed load-reservation address and size only after the read completes, submit matching store-conditionals as atomic memory requests, write architectural success or failure codes into `rd`, clear reservations on both outcomes, record local conditional failures without mutating memory, use cluster-level data-event tracking to invalidate peer reservations after completed overlapping writes, and let system data-cache response paths apply MSI, MESI, MOESI, and CHI invalidating snoops to matching core reservations before the requesting CPU response is delivered. All non-LR/SC RV64A word and doubleword AMOs now decode and execute as typed atomic memory accesses whose responses write the old memory value to `rd`, with word-width old values sign-extended to XLEN and word-width stores updating only four bytes. `MemoryRequest` carries an explicit atomic operation so the memory store and MSI, MESI, MOESI, and CHI cache-controller hit paths compute read-modify-write bytes only after capturing the old response bytes. `FENCE` and `FENCE.I` now decode as typed barrier instructions; execution records preserve fence predecessor/successor sets or instruction-cache barrier identity while advancing in-order without issuing a memory request. RV64A aq/rl flags now map to typed memory-ordering metadata on ISA memory accesses and CPU data-access records: release records a read/write fence before the access, acquire records a read/write fence after the access, and acquire-release records both. Generic `MemoryRequest`s now carry typed before/after read-write ordering metadata, and RISC-V data issue maps aq/rl metadata into the requests submitted through serial or parallel transport. rem6-cpu has an initial typed CPU translation frontend that queues virtual fetch/load/store/atomic translation misses, can attach the typed rem6-memory TLB for immediate ASID-scoped hits, fills that TLB only when queued misses complete through the page resolver, preserves CPU memory request identity separately from translation request identity, materializes mapped physical `MemoryRequest`s, records typed translation faults, snapshots pending translation metadata plus optional TLB state, and can consume the typed rem6-memory page resolver for mapped or faulted completions without gem5-style `RequestPtr` mutation or delayed callback state hidden inside packets. rem6-memory also has a typed translation TLB with bounded capacity, ASID-keyed entries, deterministic LRU, permission rechecks on hits, hit/miss/fault/insert/eviction counters, explicit all-address-space and scoped invalidation, and snapshot restore. The page resolver can emit explicit cross-page translation segments with first-failed-segment faults, and rem6-cpu can turn queued CPU translations into per-segment translated records with sliced store payloads and masks. Segmented completion can also fill ASID-scoped TLB page entries for each mapped segment so later single-page accesses hit without re-running the page resolver. A simple RISC-V data issue path can now own an optional data translation frontend, translate a load or store effective address through the page resolver and TLB before transport submission, keep the original ISA memory access in the data event, and record the physical request address explicitly. The translated RISC-V driver path polls typed translation readiness without issuing the next fetch early; the core, cluster, system run driver, and topology-built core paths can submit translated data requests through the parallel memory transport; and the MMIO-aware system path can route translated physical data addresses to either platform MMIO or memory without using the virtual address for device selection. The plain data issue path rejects a configured translation frontend when no page map is supplied. This replaces gem5's hidden split translation callback state with typed data that future RISC-V timing paths can schedule or replay deterministically. Privileged ISA, concrete page tables, fuller CPU pipeline TLB wiring, walker memory accesses, cache and memory enforcement of AQ/RL timing barriers, vectors, and richer traps are alignment targets. |
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
| `src/mem/cache` | `rem6-cache`, `rem6-coherence`, protocol crates | partial | MSI, MESI, MOESI, and an initial CHI-like state machine, cache controller, cache bank, directory model, and serial plus partitioned multi-cache coherence harnesses exist with tests. The CHI controller can issue ReadShared, ReadUnique, and MakeReadUnique-shaped downstream requests, complete shared and unique fills, preserve pending-miss snapshots, service local slices, and apply snoop downgrade or invalidation to resident data. The CHI cache bank owns multiple line controllers, assigns unique downstream request IDs across lines, records pending fills, can attach typed MSHRs, coalesces same-line read misses without extra downstream traffic, fans coalesced fills out as multiple target outcomes, and restores pending MSHR targets from snapshots. The CHI directory tracks unique clean/dirty ownership, shared clean/dirty sharers, deterministic SnoopShared and SnoopUnique decisions, owner-cache versus backing-memory data sources, MakeReadUnique no-data upgrades, dirty-peer data sourcing, and sorted snapshot restore. The CHI serial harness connects multiple cache controllers to the directory, applies directory snoops before fills, transfers owner-cache data to requesters, updates backing data when a unique dirty owner downgrades to shared clean, records CPU responses and directory decisions, and snapshots directory, cache, backing, response, and decision state. The CHI partitioned harness routes request and response work through `MemoryTransport` and `PartitionedScheduler`, waits for owner snoops before peer fills, preserves dirty owner data when downgrading to shared clean, records directory decisions and CPU responses with scheduler ticks, restores quiescent scheduler, directory, cache, backing, trace, response, and decision state, exposes recorded run summaries, source-target remote-flow records, and parallel run history, and can derive cache-directory and directory-DRAM routes from typed topology components including multi-hop and fabric-backed links. Workload replay and direct topology systems can select CHI as a typed data-cache protocol, route RISC-V data loads through the partitioned CHI harness, merge CHI cache resource activity and remote-flow records back into full-system run summaries, attribute recorded CHI data-cache runs into `RiscvSystemRun` and `WorkloadParallelExecutionSummary`, and require replay-verifiable data-cache protocol run counts for MSI, MESI, MOESI, and CHI workloads. LRU, FIFO, MRU, LFU, BRRIP, BIP, SHIP, SecondChance, and TreePLRU replacement policies have typed per-set state, victim decisions, invalidation, reset, touch, access-signature training, and snapshot restore. MSHR queues have typed entry allocation, target coalescing, prefetch reserve, ready/service state, snapshot restore, optional per-target QoS class metadata, effective QoS derived from merged targets, QoS-aware ready ordering, typed QoS profiles for target counts, effective-entry counts, requestors, and priorities, and typed conversion of effective cache QoS into downstream transport QoS classes. A typed cache write queue keeps gem5's writeback, write-clean, clean-evict, uncacheable-write, effective-capacity plus reserve, ready tick ordering, pending-conflict, functional-read satisfaction, direct mark-in-service release, and snapshot semantics while replacing Packet pointers, sender-state inheritance, and list iterators with stable handles and explicit request records. Replacement decisions can enqueue typed dirty writebacks, clean evicts, optional clean writebacks, or no entry for invalid victims, with explicit victim-way validation before the queue mutates. The MSI, MESI, MOESI, and CHI cache banks can own an optional typed write queue, enqueue writebacks or uncacheable writes, expose ready handles and conflict queries, issue ready entries as typed downstream requests, reject foreign line layouts, and restore queued entries through bank snapshots. MSI, MESI, MOESI, and CHI cache banks can accept CPU requests with explicit MSHR QoS metadata, expose effective QoS for pending lines, expose live and snapshot MSHR QoS profiles, coalesce same-line read misses without extra downstream traffic, preserve merged QoS through snapshot restore, and fan coalesced fills out as multiple target outcomes. The MSI bank directory harness can submit coalesced and parallel-cycle CPU requests with explicit MSHR QoS, record each target's effective MSHR QoS before fill service, export scheduled misses as typed downstream transport requests that carry effective MSHR QoS into `ParallelMemoryTransaction`, preserve MSHR queue configuration plus target QoS metadata in byte snapshots, aggregate pending cache-bank MSHR QoS profiles across harness snapshots and restored harness state, expose per-cycle effective MSHR QoS diagnostics on recorded parallel runs, and summarize effective QoS by requestor and priority in parallel-cycle history. Typed stride, tagged, DCPT, BOP, SBOOE, SignaturePath, SMS, FDP, PIF, ISB, STeMS, IMP, and initial AMPM access-map prefetchers have deterministic candidate generation, per-request metadata, source addresses, and snapshot restore. The DCPT model keeps gem5's per-PC delta history, signed delta overflow-to-zero rule, masked two-delta partial matching, earliest historical pair scan, and post-match delta replay while adding optional requestor isolation and explicit typed snapshots for parallel replay. The BOP model keeps gem5's best-offset learning over generated smooth offsets, left and right recent-request tables, score and round thresholds, enable-disable policy, optional delayed RR insertion, delay queue capacity/drop behavior, prefetch-fill training hook, and degree candidate generation while making the RR tables, scores, delay queue entries, selected offset, phase state, and last candidates explicit typed snapshot state. The SBOOE model keeps gem5's sandboxed sequential stride candidates, FIFO sandbox entries, score minus late-score policy, score threshold percentage, demand-fill latency tracking, and average latency feedback while making sandbox state, latency buffers, pending demand fills, selected sandbox, and last candidates explicit typed snapshot state. The SignaturePath model keeps gem5's page signature table, shifted xor signature update, pattern table stride counters, low-counter stride replacement with counter aging, confidence-gated prefetches, 0.95 lookahead cap, next-line auxiliary fallback, and page-crossing address generation while replacing opaque cache entries with explicit deterministic LRU and typed snapshots. The SMS model keeps gem5's filter table, active generation table, eviction-committed pattern history table, region offsets, FIFO filter capacity, active LRU capacity, pattern LRU capacity, and trigger-PC plus trigger-offset lookup while avoiding hidden map default insertion and exposing typed snapshots. The AMPM model keeps gem5's previous/current/next hot-zone window, positive and negative stride checks, `s2+1` early match rule, prefetch/useful/raw hit/miss counters, epoch degree adjustment, and candidate marking while making table replacement, integer threshold comparisons, epoch reports, and snapshots explicit typed state. A typed queued prefetch resource models gem5's queued prefetch latency, duplicate filtering with higher-priority duplicate updates, same-line demand squash, page-boundary dropping when no translation path is configured, in-cache or in-miss-queue redundant filtering, optional lowest-priority oldest eviction when full, next-ready-tick visibility, and accuracy throttle state with control percentage, issued/useful counters, max-permitted computation, useful-count invariant checks, and snapshot restore. The queued resource applies that throttle through an explicit enqueue path shared by typed prefetch candidates with ready-tick ordering, same-tick priority ordering, stable order ties, explicit capacity, line size, optional page size, issue width, accepted/duplicate/priority-update/redundant/page-crossing/throttled/full result counts, and full policy before packet creation or cache-controller side effects. A typed multi queued prefetcher preserves gem5's `Multi` earliest-ready query and round-robin source issue behavior while exposing source identity, keeping no-op polls side-effect free, and issuing only one entry from the chosen source. The FDP model keeps gem5's FTQ range expansion, PFQ and translation queue duplicate filtering, fetch-target squash policy, translation success/failure handling, uncacheable and cache-snoop drops, ready latency, issue ordering, queue counters, and snapshot restore while replacing raw CPU, MMU, cache, and packet pointers with explicit typed events. The PIF model keeps gem5's retired-PC training, spatial and temporal compactor records, history buffer, trigger index, stream address buffer continuation, secure-bit lookup behavior, and snapshot restore while replacing probe listeners, cache iterators, and replacement-policy callbacks with explicit typed events and stable history IDs. The ISB model keeps gem5's PC-indexed training unit, physical-to-structural and structural-to-physical address mapping caches, confidence counter update and reassignment policy, chunk-based structural address allocation, secure-bit separation, degree-limited successor prediction, deterministic LRU capacity, and snapshot restore while replacing AssociativeCache entries and raw queue output with typed records. The STeMS model keeps gem5's active generation table, pattern sequence table, region miss order buffer, trigger deltas, confidence-gated sequence reconstruction, duplicate RMOB policy, and cache-residency generation ending while replacing CacheAccessor callbacks and implicit replacement state with typed residency probes, deterministic LRU, secure-bit separation, snapshots, and line-sized reconstruction addresses. The IMP model keeps gem5's prefetch table, indirect-pattern detector, stream fallback, base-plus-index-shift matching, confidence counter, and secure-bit separation while replacing raw PT-entry pointer tags with stable typed keys, handling negative shifts through checked arithmetic, exposing explicit typed index-read events, deterministic LRU capacity, and snapshot restore. rem6-memory now has a typed translation queue with explicit request IDs, access kinds, bounded capacity, latency-derived ready ticks, deterministic ready ordering, duplicate detection, mapped or faulted completion records, snapshot restore, a typed page translation map with page-size validation, aligned virtual-to-physical mappings, overlap rejection, permission checks, page faults, explicit cross-page segment resolution, and snapshot restore, and a typed translation TLB with ASID-keyed deterministic LRU, permission rechecks, fault accounting, bounded inserts, scoped invalidation, evictions, cross-page segment fill into scoped page entries, and snapshot restore; rem6-cpu can bridge queued virtual fetch/load/store translation misses or TLB hits into typed mapped physical memory requests, issue a simple RISC-V data access through translated physical transport addresses on serial or parallel memory transport while preserving the virtual ISA access record, per-segment translated records, or fault records, while full RISC-V core/MMU pipeline integration, fuller cache/DRAM QoS policy integration, and richer cache tags remain open. |
| `src/mem/ruby` | `rem6-coherence`, `rem6-directory`, `rem6-fabric` | partial | rem6 keeps detailed coherence and NoC behavior without a second memory-stack vocabulary. |
| `src/mem/slicc` | protocol crates and typed transition records | partial | rem6 should preserve protocol expressiveness while avoiding generated controllers that hide transient behavior. |
| `src/mem/protocol` | `rem6-protocol-msi`, `rem6-protocol-mesi`, `rem6-protocol-moesi`, `rem6-protocol-chi` | partial | MSI, MESI, and MOESI exist. The CHI-like crate covers typed `I`, shared clean/dirty, unique clean/dirty, ReadShared, ReadUnique, MakeReadUnique upgrade, snoop downgrade, invalidation, busy rejection, transition trace, and directory unique-owner validation. Full CHI request, response, data, DVM, retry, credit, and Ruby-network interactions remain open. |
| `src/mem/qos` | `rem6-fabric`, `rem6-dram`, `rem6-transport`, `rem6-workload` | partial | rem6-fabric has typed QoS requestor IDs, checked priorities, fixed-priority assignment, FIFO/LIFO/LRG queue arbitration, non-mutating empty polls, queue-arbiter snapshots, and QoS-ordered fabric batch transmission that reserves shared links in grant order. rem6-transport can attach a shared QoS arbiter to parallel batch submission so request priority and requestor identity affect first-hop NoC reservation before partition events are scheduled, can order single- and multi-hop direct same-tick target deliveries with the same typed arbiter before invoking target handlers, respects same-agent memory-ordering barriers when direct QoS batches or shared-fabric first-hop reservations choose eligible requests, exposes a typed `TransportQosClass`, and lets cache-originated transactions override QoS requestor separately from the downstream request's cache-agent identity. rem6-coherence can now export MSI bank scheduled misses as typed downstream transport requests, preserving effective MSHR QoS through `TransportQosClass` so same-tick cache-originated memory requests can be batched and ordered by transport QoS without Packet sender-state inheritance. rem6-dram can order same-arrival timing batches through the same typed arbiter before bank, row, and bus timing are computed, filters same-agent acquire/release memory-ordering barriers before QoS priority or turnaround selection, prefers the current read/write bus direction among same-priority candidates, explicitly escalates queued same-requestor candidates to their best assigned batch priority without embedding controller back pointers in the queue policy, accepts memory-controller QoS batches before storage responses are generated, pairs responses with scheduled DRAM grant order, and preserves assigned priority, effective priority, requestor, byte count, and escalation status as typed DRAM activity metadata. Parallel coherence, system, DMA, and workload-result summaries expose DRAM QoS access, byte, escalation, priority, and requestor diagnostics directly from typed activity profiles. Workload manifests declare fixed-priority QoS policy, queue policy, turnaround policy, priority escalation, and per-requestor priority intent as typed replay-plan state; workload replay applies declared fixed-priority and queue policy to shared fabric first-hop reservation, applies declared fixed-priority, queue, turnaround, and escalation policy to direct profiled DRAM accesses so replay summaries carry DRAM priority and requestor metadata, lets same-tick single- and multi-hop direct DRAM deliveries observe manifest QoS before target handling, coalesces same-tick direct QoS deliveries to the same profiled DRAM target into one memory-controller batch, and keeps that batch path active when a data-cache exists by operation-filtering cache-covered data deliveries before batching the remaining DRAM requests. This preserves gem5's fixed-priority, queue-policy, turnaround, escalation, and bandwidth-accounting concepts while avoiding global requestor lookup, memory-controller back pointers, SimObject-name-only setup, and string-only stats. Broader cache/DRAM QoS policy integration remains open. |
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
| `src/gpu-compute` | `rem6-gpu` | partial | rem6 has GPU command submission, workgroup completion, DMA, traces, summaries, checkpoints with typed pending-DMA request metadata, and DMA write requests that inherit copy read-request ordering at a coarse level. |
| `src/dev/amdgpu`, `src/dev/hsa` | `rem6-gpu`, future GPU ISA and runtime modules | planned | Full GPU system support needs richer queues, address spaces, interrupts, and ISA-visible state. |
| NPU-style accelerators, not a single gem5 subtree | `rem6-accelerator` | partial | rem6 already models accelerator engines, command lanes, DMA, summaries, checkpoints with typed pending-DMA request metadata, and DMA write requests that inherit copy read-request ordering. |
| `src/dev/pci`, `src/dev/virtio`, `src/dev/storage`, `src/dev/net` | future device crates | planned | PCI, block, network, and virtio devices remain required for full-system breadth. |
| `src/dev/serial`, `src/dev/riscv`, `src/dev/lupio`, `src/dev/i2c` | `rem6-uart`, `rem6-mmio`, `rem6-interrupt`, `rem6-timer`, future device crates | partial | UART, timer, MMIO, interrupts, and an initial RISC-V CLINT model exist. The CLINT path keeps gem5's `msip`, `mtimecmp`, and read-only `mtime` MMIO layout while replacing direct `System::threads` interrupt mutation with typed interrupt ports and scheduler events, including parallel scheduling. CLINT register, timer-assertion, and RTC-driven `mtime` state can be captured and restored through typed snapshots and a system checkpoint bank, platform declarations now attach CLINT MMIO plus host checkpoints automatically, and reset is explicit through `ClintResetPolicy`: `msip` is cleared, asserted software and timer lines are typed deasserted, stale timer events are invalidated, `mtimecmp` is either preserved or reset to a declared value, and RTC-backed `mtime` resets as explicit device state. The default CLINT timebase remains scheduler ticks for compatibility, while `ClintTimebase::RtcDriven` plus `RiscvRtcSource` models gem5's RTC pulse into CLINT `mtime` without hiding the dependency in global time. Platform declarations can now emit typed RISC-V DTS source nodes and deterministic binary FDT/DTB blobs for CPUs, CPU local interrupt controllers, a `soc` simple bus, CLINT `interrupts-extended`, a generic external interrupt controller, UART interrupt-parent wiring, and Linux `/chosen` bootargs plus `linux,initrd-start` and `linux,initrd-end` metadata without Python object recursion or libfdt mutation. System topology can install the generated RISC-V DTB into store-backed or DRAM-backed guest memory and set each hart's A1 register to the DTB address, replacing gem5's external DTB filename side effect with a typed handoff. Richer wall-clock/BCD RTC behavior and other platform devices remain open. |
| platform-specific device trees under `src/dev/arm`, `src/dev/x86`, `src/dev/mips`, `src/dev/sparc` | future platform crates | planned | These should arrive with the corresponding ISA and platform support. |

### Simulation Kernel, Checkpointing, and Host Control

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| event queue and tick logic in `src/sim` | `rem6-kernel` | covered | Partitioned scheduling, conservative epochs, deterministic order, lookahead, scheduler snapshots, recorded initial and final epoch frontiers carried through CPU, data-cache/coherence, full-system, and workload-result summaries with per-partition conservative full-system frontier aggregation, worker-local remote outboxes, ordered remote-send records, source-target remote-flow records carried through RISC-V cluster, full-system, and workload-result summaries, scheduler epoch, empty-epoch, and dispatch counts, CPU scheduler, data-cache scheduler, and merged full-system progress-free transition counts plus declared-threshold livelock diagnostic counts exposed on `RiscvSystemRun` and carried into workload summaries, batch worker-count histograms, exact batch partition-set histograms, and maximum consecutive partition-set streaks carried into workload summaries, manifest-owned and result-verifiable expected remote-flow counts and first/last tick windows, minimum scheduler progress contracts backed by aggregate dispatch counts or per-partition activity, maximum scheduler idle contracts, minimum max-worker contracts backed by aggregate counts or batch-worker histograms, total-worker activity contracts backed by aggregate counts or batch-worker histograms, multi-worker batch activity contracts, exact batch partition-set contracts, sustained same-batch partition-set streak contracts, active-partition contracts backed by aggregate counts or activity-derived partition unions, per-partition activity contracts backed by explicit activity or remote-flow records, per-partition initial/final frontier contracts, clean diagnostic contracts, source and target partition counts in recorded parallel summaries, and typed parallel-worker failure reporting that preserves remaining partition events, keeps executed-time visibility, commits successful callbacks' remote messages, and rolls back local and remote events scheduled by the panicked callback exist. |
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
| `ext/libelf`, `ext/libfdt`, `ext/softfloat`, `ext/gdbremote` | `rem6-boot`, `rem6-platform`, future debug and ISA support crates | partial | rem6-platform has an initial typed DTS source tree and deterministic binary FDT/DTB writer for RISC-V platform boot descriptions, including Linux `/chosen` bootargs and initrd start/end metadata, and rem6-system installs initrd bytes plus generated or resolved-resource DTBs into guest memory with the RISC-V A1 register handoff for both store-backed and DRAM-backed memory. rem6 still needs equivalent ELF loading breadth, kernel image loaders, bootloader handoff coverage, soft-float, and debug capability without vendoring unneeded code into the core. |
| `util/gem5art`, resources tooling, disk image tooling | `rem6-workload`, future artifact tooling | partial | rem6 manifests make artifact provenance first-class and reproducible for boot images, declared resources, typed Linux device-tree resources, and typed Linux initrd handoff resources. Resolved payloads are explicit caller-provided data with manifest identity, id, digest, kind validation, and initrd handoff-size validation; workload replay rejects payload sets resolved for a different manifest, and no hidden download path is part of replay authority. Future tooling still needs richer artifact acquisition and disk-image construction records. |
| `tests`, `tests/test-progs`, `util/statetrace` | rem6 tests and trace tooling | partial | gem5 tests are audit input. rem6 acceptance remains Rust tests and typed trace comparison. |

## Evidence Already Present in rem6

- Partitioned scheduler tests cover deterministic event order, lookahead,
  conservative parallel epochs, worker limits, wait-for graphs, and scheduler
  snapshots. Progress-monitor tests cover typed livelock diagnostics for
  repeated progress-free transitions, transition-kind accounting, snapshots,
  and useful-work reset of active livelock windows. Scheduler progress tests
  cover worker-recorded progress-free transitions, deterministic batch and run
  aggregation, monitor-snapshot replay, zero-threshold rejection, and panic-path
  rollback.
- Full-system RISC-V tests drive multicore fetch, data access, traps, host stop,
  statistics, and run summaries through the partitioned scheduler; workload
  RISC-V translation tests drive topology-built translated data cores through
  the system parallel run driver with an explicit page map, assert that
  multicore responders observe physical data addresses, and cover a mixed
  translated MMIO plus memory run where platform device selection uses the
  physical address; workload
  topology tests reject RISC-V core fetch or data routes whose source partition
  or source endpoint does not match the declared core placement, reject RISC-V
  data-cache backing routes whose source partition or endpoint does not match
  the declared directory placement, workload replay tests cover RISC-V entry
  fetches, redirected fetches, and data loads from non-first memory targets with
  different line layouts, and direct topology store and DRAM tests cover
  redirected fetches and data loads into addressed targets with different line
  layouts. RISC-V ISA and frontend tests cover word and doubleword `LR`/`SC` decode,
  execution, read-shared issue, matching-reservation atomic submission,
  register completion, failed-condition completion without memory mutation,
  typed load-reservation recording and clearing, `AMOSWAP.D` old-value
  response plus new-value memory update, `AMOADD.D` typed read-modify-write
  addition, `AMOXOR.D`/`AMOOR.D`/`AMOAND.D` typed logical
  read-modify-write execution, and
  `AMOMIN.D`/`AMOMAX.D`/`AMOMINU.D`/`AMOMAXU.D` signed and unsigned selection
  through the memory store, cache hit path, ISA, and CPU frontend. Word-width
  AMO frontend tests cover all non-LR/SC operations, four-byte updates, and
  sign-extended old-value register writes. RISC-V ISA and frontend tests cover
  `FENCE` predecessor/successor set decode, `FENCE.I` decode, no-register-write
  execution, no data-request issue, and ordered fetch progression after barrier
  instructions. RISC-V ISA and frontend tests cover aq/rl metadata that maps
  release to a read/write fence before the atomic access and acquire to a
  read/write fence after it. Memory request tests cover default empty ordering
  and explicit before/after read-write ordering, and RISC-V frontend tests cover
  aq/rl ordering propagation into parallel transport atomic requests. Memory and
  cache-controller tests cover atomic responses that capture old bytes before
  masked writes. RISC-V
  cluster tests cover peer reservation invalidation after a completed overlapping store so a later
  `SC.D` fails instead of overwriting the peer's data. RISC-V topology tests
  cover MSI data-cache snoop invalidation before the peer store response reaches
  its CPU, so `SC.D` failure does not depend on late completed-write cleanup.
  RISC-V frontend tests cover MMIO data access events whose memory route and
  memory endpoint are explicitly absent. Workload
  topology tests reject GPU or accelerator command routes whose target endpoint
  does not match the declared device control endpoint, and reject GPU or
  accelerator DMA routes whose source endpoint does not match the declared
  device DMA endpoint.
- Transport, fabric, DRAM, cache, directory, and coherence crates expose typed
  activity records rather than depending on string logs, and workload manifests
  can require minimum fabric, DRAM, and aggregate resource operation counts plus
  active-resource counts.
- MSI, MESI, MOESI, and initial CHI-like line-state, cache-controller,
  cache-bank, directory, serial plus partitioned coherence-harness behavior,
  topology-built CHI cache-directory and DRAM routes, and CHI recorded
  run-resource summaries have test coverage; full-system workload replay and
  direct topology paths report MSI/MESI/MOESI/CHI data-cache protocol
  attribution, and workload replay manifests can require minimum attributed
  data-cache runs, bounded unattributed data-cache runs, equality between total
  data-cache runs and attributed plus unattributed runs, equality between
  attributed runs and summed protocol-count runs, and a minimum recorded
  data-cache protocol run count for each selected protocol.
- GPU and accelerator paths route command and DMA work through typed topology,
  transport, scheduler activity, summaries, and checkpoint banks, including
  DMA write-request ordering inherited from copy read requests and
  heterogeneous checkpoint restore of pending DMA read-request ordering
  metadata.
- CPU branch prediction exposes typed direction prediction, GShare PC-history
  indexing, BiMode choice and direction-array training, Tournament
  local/global/choice training, loop trip-count learning, LTAGE loop override
  integration, statistical-corrector GEHL override, branch-target lookup, TAGE
  folded-history indexing and provider selection, indirect-target lookup,
  deterministic replacement, update, target, speculative history, return-stack
  operation, commit, repair, and snapshot records with restore validation.
- Cache MSHR, MSI/MESI/MOESI/CHI cache-bank, and MSI bank directory harness
  tests cover typed QoS class metadata, QoS-aware ready ordering, promotion of
  merged same-line targets to the highest effective priority, recorded
  coalesced-target and parallel-cycle effective QoS, typed QoS profiles across
  MSHR queues, cache banks, snapshots, and MSI bank harness snapshot restore,
  same-agent acquire/release ordering barriers that constrain ready MSHR
  eligibility before QoS selection, MSI/MESI/MOESI/CHI downstream miss request
  ordering propagation, MSHR-to-transport QoS class export, per-cycle MSI bank
  run QoS counts by effective requestor and priority, parallel-cycle history
  counts by effective requestor and priority, and byte-snapshot restore of MSHR
  queue configuration plus target QoS and ordering state.
- Cache prefetch tests cover tagged next-line candidate generation, DCPT masked
  delta-pair matching with earliest historical replay and snapshot restore, BOP
  best-offset learning with degree candidate metadata, delayed RR training, RR
  snapshot restore, delay-queue snapshot restore, SBOOE sandbox stride selection,
  latency feedback, late-hit scoring, snapshot restore, SignaturePath page
  signature lookahead, low-confidence stride replacement, snapshot restore, SMS
  eviction-committed region patterns, filter FIFO capacity, PHT LRU capacity,
  snapshot restore, FDP FTQ range expansion, PFQ/TQ duplicate filtering,
  fetch-target squash, translation failure, cache-snoop drop, queue capacity,
  ready-latency issue, snapshot restore, PIF retired-PC compaction, trigger
  index lookup, stream address buffer continuation, capacity limits, snapshot
  restore, ISB structural stream learning, PS/SP mapping cache capacity,
  secure-bit separation, snapshot restore, STeMS active-generation commit,
  RMOB reconstruction, duplicate filtering, secure-bit separation, snapshot
  restore, IMP indirect-pattern detection, IPD base matching, stream fallback,
  typed key LRU capacity, snapshot restore, cache write queue ready ordering,
  reserve handling, functional-read satisfaction, snapshot restore, and
  replacement-triggered dirty writeback, clean evict, clean writeback, invalid
  victim suppression, and victim-way validation, plus MSI/MESI/MOESI/CHI bank
  write queue attachment, ready issue, conflict lookup, uncacheable-match
  filtering, functional-read delegation, and bank snapshot restore,
  AMPM cross-hot-zone access-map
  matching, AMPM useful-prefetch accounting, AMPM epoch degree increase and
  decrease with snapshot restore, page-boundary candidate dropping, multi-source
  queued prefetch earliest-ready
  reporting, deterministic round-robin issue, non-mutating no-op polls, and
  single-entry issue from the selected source.
- Fabric QoS tests cover explicit fixed-priority requestor mapping,
  highest-priority queue selection, FIFO/LIFO same-priority ordering, LRG
  requestor rotation, non-mutating empty polls, snapshot replay, and
  QoS-driven fabric batch reservation order on a shared link. Transport tests
  cover QoS-driven first-hop fabric reservation before parallel batch events
  are scheduled, explicit QoS requestor override for shared-fabric LRG
  arbitration, direct target batch priority assignment from an explicit
  QoS requestor, and same-agent acquire/release ordering barriers that constrain
  direct QoS batch and shared-fabric first-hop reservation eligibility. Memory
  tests cover shared read/write barrier matching and same-agent request ordering
  edges. DRAM tests cover same-agent acquire/release ordering barriers that
  constrain same-arrival QoS timing-batch eligibility, QoS-driven same-arrival
  request ordering before bank and bus timing are computed, plus typed read/write direction
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
  typed activity profiles, plus workload-level CPU scheduler, data-cache
  scheduler, merged full-system remote-flow records, scheduler epoch,
  empty-epoch, dispatch counts, progress-free transition counts,
  declared-threshold livelock diagnostic counts, merged resource and
  full-system deadlock diagnostic counts, total-worker counts, batch
  worker-count histograms, exact batch partition-set histograms, maximum
  consecutive partition-set streaks, per-partition activity summaries,
  replay-plan
  validation of exact expected remote-flow counts and first/last tick windows,
  minimum scheduler epoch and dispatch progress, maximum scheduler idle epochs,
  minimum max-worker use, minimum total-worker activity,
  minimum multi-worker batch activity, exact batch partition-set activity,
  sustained same-batch partition-set streak activity, minimum active partition
  counts, per-partition activity minima, data-cache run attribution
  expectations, data-cache run-accounting consistency, data-cache protocol
  run-count expectations, minimum fabric/DRAM/resource activity expectations,
  clean parallel diagnostic expectations including livelock counts, and
  manifest identity changes for those expected communication contracts.
  Workload replay QoS tests cover same-tick DRAM
  batching while a data-cache is present, including operation filtering so
  instruction fetches are not misclassified as cache-covered data traffic.
- Stats tests cover counter reset epochs and typed probe point, listener,
  event, payload, and snapshot records.
- Timer/MMIO tests cover typed RISC-V CLINT `msip` software interrupts,
  `mtimecmp` timer interrupt scheduling, future-deadline timer deassertion,
  read-only `mtime` from scheduler ticks, the same `mtimecmp` path under the
  parallel scheduler, RTC-driven `mtime` advancement from typed serial and
  parallel RTC pulse sources, MTIP delivery on `mtimecmp` reach, and CLINT
  snapshot/restore of per-hart `msip`, `mtimecmp`, timer assertion, and
  RTC-backed `mtime` state. CLINT reset tests cover `msip` clearing,
  `mtimecmp` reset policy, timer-assertion clearing, serial and parallel typed
  interrupt deassertion, and stale timer-event invalidation through generation
  changes.
- System action tests cover CLINT checkpoint-bank capture and restore through
  host checkpoint manifests for per-hart `msip`, `mtimecmp`, timer assertion,
  and RTC-backed `mtime` state.
- Platform and topology tests cover declared CLINT hart interrupt routes, CLINT
  MMIO bus routing, declared CLINT reset policy plumbing, and automatic host
  checkpoint-bank attachment for platform CLINT devices.
- Platform tests cover typed RISC-V device-tree source emission for CPU
  timebase, CPU local interrupt controllers, `soc` simple-bus layout, CLINT
  `reg` and `interrupts-extended`, external interrupt-controller phandles,
  UART compatible strings, UART interrupt-parent wiring, DTS serialization, and
  deterministic binary FDT/DTB serialization with header offsets, reserve-map
  terminator, structure tokens, de-duplicated string table entries, and
  rejection of UART device-tree emission when no external interrupt controller
  is declared. Platform tests also cover Linux `/chosen` bootargs and initrd
  start/end metadata encoded as deterministic DTB properties.
- System topology tests cover typed RISC-V DTB handoff from the platform model
  into store-backed and DRAM-backed guest memory, per-core A1 register setup
  for the DTB address, and preservation of Linux bootargs plus initrd metadata
  in the installed DTB. The typed RISC-V Linux boot handoff also installs
  initrd bytes and matching DTB metadata into both store-backed and DRAM-backed
  memory. Workload replay tests cover resolved Linux device-tree and initrd
  payload installation into guest memory snapshots from typed manifest handoff
  state, and reject resolved payload sets bound to a different manifest
  identity.
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
  checkpoint lineage, typed QoS policy intent, typed Linux boot handoff intent
  with device-tree and initrd resource validation, explicit required-resource
  payload resolution bound to manifest identity, RISC-V core route source
  partition and endpoint validation, explicit RISC-V data-cache backing-route
  validation and identity hashing, GPU and accelerator command plus DMA endpoint
  validation, manifest-owned parallel remote-flow count, remote-flow timing,
  scheduler progress, scheduler idle bounds, max-worker use, total-worker
  activity, active-partition, per-partition activity, data-cache run
  attribution contracts, data-cache run-accounting consistency contracts,
  data-cache protocol run-count verification contracts, resource activity
  contracts, and clean diagnostic verification contracts, result metadata,
  execution mode switches, host action summaries, checkpoint restore labels,
  and statistics snapshots.

## Open Alignment Work

- Expand ISA support beyond RISC-V while preserving crate isolation.
- Add in-order pipeline, out-of-order pipeline, checker, richer branch
  predictors, and host-assisted execution models with checkpointable state.
- Complete CHI-like coherence beyond the current line-state crate, single-line
  cache controller, multi-line cache bank, initial directory model, and serial
  plus initial partitioned, topology-built cache-directory, topology-built
  DRAM-backed harnesses with recorded resource summaries, workload-replay
  data-cache attribution, and direct topology CHI data-cache attach, plus
  richer cache internals such as additional prefetcher algorithms, prefetch
  translation and snoop integration, fuller cache/DRAM QoS policy integration,
  and sector/compressed tags.
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
