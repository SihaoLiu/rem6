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
  empty-epoch exposure, verifiable minimum max-worker use, total-worker use,
  and multi-worker batch activity derived from the strongest same-scope
  worker-count, exact partition-set, or same-partition-set streak evidence,
  exact same-batch partition-set activity, minimum-worker duration-weighted
  tick activity, sustained minimum-worker tick streak activity, sustained
  same-batch streak activity, and manifest-verifiable batch worker-ticks
  computed as `worker_count * duration_ticks` under a declared minimum
  worker-count threshold as core kernel contracts. When a full-system merged
  worker-count summary is explicitly present, replay verification also rejects
  thresholded batch-activity totals that are weaker than the same-scope CPU,
  cache, GPU DMA, and accelerator DMA lower-bound evidence, so a hand-written
  aggregate cannot hide available parallelism. The same lower-bound rule now
  covers full-system batch worker-tick buckets, thresholded batch tick
  activity, thresholded batch worker-ticks, and longest minimum-worker tick
  streaks recorded through explicit merged summaries, along with exact
  full-system partition-set batch counts, same-partition-set streak counts, and
  explicit full-system scheduler dispatch counts.
  Planned and recorded kernel scheduler batches also expose worker-capacity
  ticks, idle-worker ticks, utilization ratios, and worker-slot active and idle
  tick summaries directly, so pre-dispatch and post-dispatch multicore
  efficiency checks do not need to infer scheduler capacity from timeline shape,
  worker records, or private worker-limit state. Workload replay summaries carry
  planned capacity totals into result artifacts and derive planned worker-ticks,
  idle ticks, and utilization ratios from the preserved planned timelines, so
  replay diagnostics keep the same pre-dispatch authority. Workload results
  also carry recorded worker-capacity ticks, idle-worker ticks, utilization
  ratios, and per-worker-slot active/idle summaries separately from
  multi-worker parallel-evidence contracts. Kernel recorded
  epoch and run summaries expose the same actual worker-capacity, idle-worker,
  utilization, and worker-slot occupancy evidence after callbacks and remote
  wakeups run, and `RiscvSystemRun` carries that recorded capacity evidence for
  CPU-scheduler, data-cache scheduler, and merged full-system scopes. Workload
  manifests and replay plans can now require those planned
  worker-slot active and idle tick budgets across CPU-scheduler, data-cache
  scheduler, GPU DMA, accelerator DMA, combined DMA, and merged full-system
  planned scopes.
  Heterogeneous DMA scheduler work also keeps exact typed batch timelines for
  GPU and accelerator read/write scheduler runs, so full-system occupancy
  checks and dedicated DMA scheduler timeline checks can validate when DMA work
  overlapped CPU and cache work instead of inferring it from aggregate
  counters. Direct GPU and accelerator DMA scheduler dispatch progress now
  treats typed batch worker-count evidence as a lower bound, and combined DMA
  dispatch progress preserves those direct lower bounds before full-system
  progress checks consume the merged evidence. Manifest-declared exact batch
  timeline expectations reject
  one-worker or one-partition records, so timeline evidence cannot be satisfied
  by serial occupancy. Direct DMA scheduler epoch and dispatch progress,
  active-partition, worker-count, multi-worker batch-activity,
  initial/final frontier, duration-weighted tick, sustained tick-streak,
  thresholded worker-tick, exact partition-set, and same-partition-set streak
  contracts use dedicated manifest scopes. Direct GPU and accelerator DMA
  per-partition activity contracts now use both DMA timeline-derived sets and
  streaks plus typed remote-flow and exact remote-send endpoints, and merged
  full-system partition checks include the same per-partition worker, dispatch,
  send, and receive activity. Direct GPU DMA, accelerator DMA, and combined
  DMA active-partition contracts now also derive partition use from typed
  remote-flow and exact remote-send endpoints, so device-side cross-partition
  traffic remains visible even when no batch timeline was recorded. Remote-flow
  and exact remote-send contracts reject same-partition endpoints, and exact
  remote sends also reject inverted source/delivery ticks, so remote traffic
  evidence cannot be satisfied by local partition traffic or non-causal timing.
  Wait-for
  edge-kind observation windows are now owned by `rem6-kernel`, so every
  subsystem can report distinct edge counts plus first and last observed ticks
  with the same deterministic semantics before the data is converted into
  workload summaries. Blocked-node observation windows are also kernel-owned,
  so merged resource or full-system diagnostics can identify which partition,
  component, resource, or transaction was blocked, how many distinct
  dependencies contributed to that block, and the observed tick window without
  rebuilding ad hoc sets in each subsystem. Target-node observation windows use
  the same kernel semantics for the resources, queues, transactions, or
  components being waited on, so parallel runs can identify shared contention
  hotspots without scanning raw edges after the run. Workload results carry
  blocked-node and target-node windows across data-cache, fabric, DRAM,
  compute, DMA, and merged full-system scopes, so replay artifacts can preserve
  both the blocked participant and the contended resource identity instead of
  only recording edge-kind totals.
- Configuration and experiment reproducibility are too script-dependent in
  gem5. Official documentation describes embedded Python configuration,
  behind-the-scenes port connection behavior, and command-line options whose
  effects must be checked in generated config output. The standard library and
  resource package reduce this burden, but they also demonstrate that kernels,
  disk images, benchmark inputs, exit events, workload suites, dispatch worker
  choices, and downloads remain workflow state outside the core simulator. rem6
  therefore keeps typed builders, manifests, artifact identities, custom
  guest-host calls, manifest-declared responses, deterministic suite
  identities, suite dispatch records, weighted dispatch load summaries,
  planned-load expectations, planned dispatch timelines, planned
  timeline-derived execution summaries, planned timeline expectation checks,
  planned timeline wall-clock, worker-idle, occupancy-window, occupancy
  utilization, worker-count tick histograms, worker-count tick threshold
  contracts, per-worker-slot planned active/idle ticks, sustained-occupancy,
  full-occupancy, underoccupied-tick, and
  thresholded batch worker-tick evidence, suite result-derived execution evidence, runtime execution
  occupancy windows, runtime worker-count tick histograms and threshold
  contracts, runtime full-occupancy and underoccupied-tick contracts,
  runtime per-worker-slot active/idle ticks, per-workload result start/final
  windows, worker-level suite completion summaries, worker-slot suite
  active/idle summaries, round-robin or weighted suite dispatch plans,
  dispatch-declared suite execution
  expectations with runtime occupancy thresholds, suite-level execution efficiency summaries, declared suite
  speedup and utilization thresholds, planned workload-result scheduler
  capacity totals, planned worker-ticks, planned idle-worker ticks, planned
  utilization ratios, and explicit boot handoff reports as the authority for
  platform and workload state.
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
  with counts, first/last delivery ticks, and min/max delay bounds, plus counts
  and unique source/target partition sets at batch, epoch, run,
  source-partition activity, and target-partition activity scope. RISC-V
  cluster, coherence, full-system run, and workload-result summaries preserve
  those flow records across epoch merges, and workload-result replay derives route-level flow count,
  first/last delivery tick, and min/max delay evidence from exact remote-send
  records when aggregate flow records are absent or weaker. Workload manifests and replay
  plans can declare exact expected remote-send records, exact progress-free
  transition records, remote-flow actual sets, and remote source/target endpoint
  partition sets for scheduler, data-cache scheduler, or merged full-system
  scope, exact remote-flow first/last tick windows, manifest-verifiable
  minimum remote-delay floors and maximum remote-delay ceilings,
  and manifest-verifiable
  consistency between aggregate remote-flow evidence and exact remote-send evidence,
  minimum max-worker use derived from aggregate, worker-count, exact
  partition-set, or streak evidence, batch counts derived from the strongest
  available aggregate, worker-count, exact partition-set, or streak evidence,
  exact worker-count bucket contracts derived from worker-count, exact
  partition-set, or streak evidence, minimum-worker duration-weighted tick
  activity contracts, sustained minimum-worker tick streak contracts, and
  minimum thresholded batch worker-tick contracts derived from exact batch timeline evidence,
  minimum total-worker activity derived from the strongest available aggregate,
  worker-count, exact partition-set, streak, or multi-worker per-partition evidence, minimum scheduler epoch progress plus dispatch
  progress from the strongest available aggregate counts, batch-histogram,
  exact partition-set, or per-partition evidence, direct GPU and accelerator
  DMA scheduler max-worker, total-worker activity, batch-activity, and frontier
  contracts, per-partition worker and dispatch activity derived from exact
  partition-set histograms or streaks, and
  non-overcounting same-scope activity evidence merges, maximum scheduler empty
  epochs, minimum multi-worker batch counts, exact partition-set batch counts,
  minimum same-partition-set consecutive batch streaks, minimum active partition counts derived from
  aggregate, exact partition-set, activity, remote-send, or remote-flow evidence, per-partition
  activity minima, data-cache protocol attribution expectations, data-cache
  run-accounting consistency, data-cache protocol run-count expectations, and
  minimum fabric, DRAM, and aggregate resource activity contracts plus clean
  parallel diagnostic contracts by scheduler or resource scope, so
  cross-partition communication volume, timing drift,
  scheduler progress, scheduler idle drift, real parallel occupancy, dispatch-declared runtime occupancy thresholds, sustained
  worker activity, runtime suite worker-count occupancy, sustained multi-worker batch execution, exact same-batch
  partition co-execution, sustained same-batch co-execution, per-partition
  participation, partition-attributed worker activity, progress-free
  transition volume, declared-threshold livelock diagnostic records, subject queries, subject summaries, kind summaries with exact kind tick windows, kind-filtered subject and record queries, tick windows, and dirty-subject failure evidence, fabric/DRAM
  resource use, and subsystem-local plus merged
  full-system wait-for/deadlock/livelock cleanliness are observable and
  verifiable without replaying callbacks. Workload result summaries preserve
  wait-for edge-kind counts and first/last tick windows across data-cache,
  fabric, DRAM, GPU and accelerator compute, GPU and accelerator DMA,
  resource, compute, DMA, and full-system scopes, so barrier, queue, protocol,
  credit, message, resource, and host-action waits do not collapse into an
  aggregate dirty count or lose when each kind was observed. They also preserve
  blocked-node wait-for windows for blocked partitions, components, resources,
  or transactions, plus target-node wait-for windows for contended resources,
  queues, transactions, and components. Manifests can declare exact scoped
  wait-for edge-kind, blocked-node, and target-node windows, binding edge count
  and first/last tick evidence into replay verification and manifest identity.
  Workload result summaries also treat
  remote-flow-derived active partitions and recorded frontiers as parallel work
  evidence, so sparse typed traces do not disappear behind empty worker or batch
  aggregates.
- Observability and statistics need stronger contracts. A gem5 issue about
  stats reset explicitly calls out missing reset tests and user confusion from
  inconsistent stats. rem6 therefore treats statistics, activity, wait-for
  graphs, and run summaries as typed data with tests rather than string-only
  logs or ad hoc probes. Stats reset windows are monotonic: a reset request
  before the previous reset tick is rejected without changing the active epoch
  or counter values, successful reset records are retained in typed
  registry-owned history with stable reset ids, and dump/reset events are also
  kept in one typed interleaved history. Workload replay carries that typed
  reset/dump history into workload results, so replay artifacts preserve both
  the final snapshot and the ordered stats-control record stream. Manifests can
  require minimum reset/dump counts, exact first/last stats-history ticks, and
  exact reset/dump event sequences with stable ids, epochs, and dump reset
  windows, so stats-control behavior becomes replay-verifiable workload
  identity rather than a log-only side effect. Once a registry has emitted any
  dump or reset history record, counter and group registration is locked so the
  history stream cannot silently mix multiple schemas without a typed schema
  event. Counter paths are structured
  scope/name identities with a checked dot-separated spelling before
  registration, registry-owned stat groups preserve reusable hierarchy ids,
  and snapshots carry a group catalog so exported samples and deltas can
  resolve group ids without a live registry.
  Counter units and descriptions are registered as typed metadata: units can
  preserve nested rate spelling while rejecting empty, whitespace-shaped, or
  malformed rate strings, and descriptions preserve gem5's useful `Info.desc`
  semantics while rejecting empty metadata before consuming counter ids. This
  keeps gem5's useful stat name, hierarchy, description, and unit discipline
  while returning typed errors instead of panicking. Stats dumps are
  registry-owned typed records with stable dump ids and self-describing snapshot
  payloads rather than global output callbacks. Stats deltas are also typed
  records derived only from snapshots in the same reset scope with matching
  group catalogs, matching descriptions, nondecreasing tick, and nondecreasing
  counter values. Probe snapshots preserve point, listener, and event cursors,
  and restore rejects duplicate ids, unknown point references, nonmonotonic
  event sequences, time-regressing event ticks, and stale cursors before
  replacing the live registry, so removed callback/listener state cannot be
  reintroduced as reused ids after a checkpoint and probe timelines cannot
  look sequence-ordered while moving backward in simulated time. Probe events
  also carry the listener ids and names that observed the event, so later
  listener removal does not turn historical callback delivery into a count-only
  log entry. Probe component names, point names, and listener names also use
  the same checked identifier spelling as stat path segments, so probe
  identity cannot fall back to whitespace, separator, or punctuation-shaped log
  labels.
  The May 2026 public gem5 issue sweep still shows stats-reset test debt,
  syscall-emulation correctness gaps, and a multicore CHI LR/SC race report.
  A previously reported RISC-V vector tracing crash is now closed, but remains
  evidence that ISA, trace, and execution-mode behavior can fail at
  cross-subsystem boundaries. rem6 keeps turning cross-subsystem activity into
  typed, identity-preserving evidence before broad parity claims.
- Simple models are not automatically cheap or transparent. Recent call-stack
  profiling work identifies gem5's layered design as difficult to profile and
  reports TimingSimpleCPU behavior that can be slower than a full out-of-order
  model because of lockup-cache behavior. rem6 therefore does not use model
  names as performance evidence; every runtime resource needs typed activity
  records, queue diagnostics, and tests that expose where simulated time and host
  work are spent. Fabric/NoC summaries therefore preserve per-transfer hop
  timing, per-link, per-lane, and per-virtual-network transfer, byte,
  occupancy, queue-delay, contention, and tick-window evidence instead of
  collapsing all network activity into one aggregate counter. Workload
  manifests and replay plans can also declare
  minimum per-link transfer, active-virtual-network, queue-delay, and contended
  virtual-network contracts, minimum per-virtual-network transfer, active-lane,
  queue-delay, and contended-lane contracts, and minimum per-lane transfer,
  byte, occupancy, queue-delay, peak queue-delay, per-link, per-lane, and
  per-virtual-network queue-delay budgets, per-virtual-network lane fanout,
  contention budgets, and required-link coverage, and per-link, per-lane, and
  per-virtual-network activity-window coverage contracts so NoC hotspots and
  bounded congestion remain typed replay evidence. Link and virtual-network
  activity-window merges now preserve identity-backed unique virtual-network
  and lane coverage, so repeated activity on the same fabric resources cannot
  inflate apparent NoC parallelism.
- Power equations should not depend on late string lookup into global
  statistics. gem5's MathExprPowerModel accepts equations that reference stat
  names plus automatic variables. rem6 keeps the equation idea, but binds
  metric inputs, temperature, voltage, and clock period through typed records
  before evaluation. Power metric inputs can also be derived from two stats
  snapshots only when they share the same reset epoch and reset tick, the second
  snapshot is not earlier in simulated time, and every bound counter is
  monotonic within that scope. Stat group-catalog, path/unit descriptor, and
  description drift are surfaced as typed power errors rather than falling
  through a panic boundary.
- Compatibility bugs cluster around cross-subsystem seams. Recent public gem5
  issues include syscall-emulation gaps for modern libc behavior, RISC-V vector
  tracing crashes, and a three-level CHI LR/SC race in multicore RISC-V
  workloads. rem6 therefore keeps ISA, guest ABI, coherence, memory ordering,
  and device behavior behind typed crate boundaries with focused regression
  tests before broad parity claims.

Research anchors refreshed on 2026-05-28:

- Parallel M5: <https://old.gem5.org/Parallel_M5.html>
- gem5 event-driven programming:
  <https://www.gem5.org/documentation/learning_gem5/part2/events/>
- gem5 event queue API:
  <https://doxygen.gem5.org/release/v21-1-0-2/classgem5_1_1EventQueue.html>
- gem5 multi-event-queue review:
  <https://reviews.gem5.org/r/1667/index.html>
- par-gem5: <https://past.date-conference.com/proceedings-archive/2023/DATA/16.pdf>
- parti-gem5: <https://arxiv.org/abs/2308.09445>
- gem5 Python configuration and port wiring:
  <https://www.gem5.org/documentation/learning_gem5/part1/simple_config/>
- gem5 default script behavior:
  <https://www.gem5.org/documentation/learning_gem5/part1/example_configs/>
- gem5 standard library and resources:
  <https://www.gem5.org/documentation/gem5-stdlib/overview>
- GEM5ART reproducible full-system workflow:
  <https://www.gem5.org/assets/files/papers/enabling2021ispass.pdf>
- gem5 error categories:
  <https://www.gem5.org/documentation/general_docs/common-errors/>
- gem5 call-stack profiling:
  <https://arxiv.org/abs/2605.01419>
- VirtIO 1.2 split virtqueue descriptor and notification requirements:
  <https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html>
- Local read-only reference anchors: gem5 `src/sim`, `src/python`,
  `configs`, `src/mem`, `src/cpu`, and public issues for open stats-reset
  test debt, open syscall-emulation `wait4` status behavior, a closed RISC-V
  vector tracing crash, open CHI LR/SC behavior, and Ruby checkpoint restore
  schedule-in-past failures.

Implementation evidence on 2026-05-28:

- `rem6-net` has a typed distributed Ethernet link endpoint aligned with gem5
  `DistEtherLink::TxLink` and `DistEtherLink::RxLink`: local transmits encode
  endian-stable distributed data messages, preserve deterministic
  serialization plus delay-variation ticks, mark the bound interface busy until
  transmit completion, emit typed send-done events through the interface
  registry, schedule remote data messages through the distributed receive
  scheduler, inject ready packets into the bound local peer, and snapshot codec,
  delay, transmit, and receive-scheduler state.
- `rem6-net` has a typed SINIC register nucleus aligned with gem5
  `sinicreg.hh` and the interrupt paths in `sinic.cc`: register offsets,
  access widths, read/write permissions, RX/TX data descriptors, RX/TX done
  status words, RX status packing, reset parameter validation, interrupt masks,
  software interrupts, delayed packet/DMA-style interrupts, RX-high and TX-low
  watermark latches, read-clear status behavior, and snapshot restore are
  explicit Rust state with typed errors instead of register macro expansion and
  panic paths.
- `rem6-net` also has typed SINIC FIFO ingress and egress state aligned with
  gem5 `Sinic::recvPacket` and `Sinic::transmit`: RX disabled drops, RX FIFO
  capacity rejection without mutation, RX packet and high/empty watermark
  interrupts, TX FIFO capacity and full watermark interrupts, peer-busy
  transmit backpressure without popping queued data, TX packet and low
  watermark interrupts, derived RX/TX done status words, and snapshot restore
  are explicit records.
- `rem6-net` now has typed SINIC DMA copy state aligned with gem5 `rxKick` and
  `txKick`: RX copy plans derive guest address, packet offset, copy length,
  zero/delay-copy limiting, More completion status, RX DMA and empty
  interrupts, and packet removal from explicit state; TX copy plans accumulate
  multi-descriptor fragments, preserve pending fragment buffers across
  snapshots, reject nested or mismatched completions with typed errors, enqueue
  complete packets into the TX FIFO, and post TX DMA plus TX-full interrupts.
- `rem6-net` now drives SINIC descriptor copies through typed guest memory:
  RX completions split descriptor writes by cache-line layout, issue
  `MemoryRequest::write` operations into `PartitionedMemoryStore`, then
  complete the FIFO DMA state only after memory succeeds; TX completions split
  descriptor reads through `MemoryRequest::read_shared`, assemble the returned
  bytes through the existing TX DMA path, preserve pending DMA state across
  memory errors, and retain request id, target, address, and byte-count records
  for replayable evidence.
- `rem6-net` also exposes that SINIC state through a typed `rem6-mmio` device
  window: register width and access permissions match the gem5 SINIC register
  table, interrupt-status reads clear pending bits, command writes can post
  software interrupts, RX/TX data writes create explicit DMA copy plans,
  RX/TX done and wait reads merge DMA completion status with live FIFO state,
  and the device participates in serial and parallel MMIO bus routing with
  typed `MmioError` failures instead of panic paths.
- `rem6-net` now has a typed SINIC PCI endpoint spec aligned with gem5
  `Ethernet.py`: vendor `0x1291`, device `0x1293`, network class codes,
  BAR0 as a non-prefetchable 64KiB memory BAR, INTA line metadata, and
  minimum-grant/maximum-latency header bytes are explicit endpoint state. Host
  BAR ranges can wrap the SINIC MMIO device through `PciBarMmioDevice`, so
  serial or parallel guest BAR accesses route through typed PCI host decoding
  into BAR-local SINIC registers. BAR MMIO devices can also share the live
  `PciHostBridge` and revalidate current bridge-window forwarding before each
  runtime access, replacing gem5 range-change side effects with typed
  `DeviceError` completions when guest bridge configuration no longer forwards
  the stale host range. SINIC interrupt records can now bind to a validated PCI
  legacy INTx port, preserve the device-scheduled interrupt tick, add PCI signal
  latency, and deliver serial or parallel assertions and deassertions through
  `rem6-interrupt` without direct controller mutation. MMIO writes and reads
  now invoke that wiring automatically through the typed SINIC PCI device
  wrapper. The SINIC FIFO path now covers checksum offload:
  RX DMA completion reports IPv4/TCP/UDP packet and checksum-error status bits,
  and TX DMA completion fills IPv4 plus TCP/UDP checksums when
  `TxData_Checksum` is set.

Implementation evidence on 2026-05-26:

- `rem6-cpu` RISC-V cluster scheduler epochs now retain exact batch
  partition-set histograms and same-partition-set streak summaries computed by
  `rem6-kernel`, and cluster runs expose scheduler-scope partition-set counts
  without rescanning batches. Ordered streak queries still use the recorded
  batch sequence so cross-epoch evidence remains timeline-faithful. `rem6-system`
  reuses the CPU epoch evidence for scheduler-scope partition-set counts while
  keeping full-system streaks ordered across CPU and data-cache scheduler
  scopes.
- `rem6-cpu` also retains kernel-computed batch worker-count histograms,
  duration-weighted worker-count tick histograms, exact worker-count batch
  counts, thresholded batch counts, worker-count tick totals, thresholded tick
  totals, total worker-ticks, and thresholded worker-ticks at RISC-V cluster
  scheduler epoch and run scope. `rem6-system` reuses that CPU evidence for
  scheduler-scope worker-count queries and additive full-system worker-count
  summaries while leaving ordered longest-streak queries on the scoped batch
  timeline.
- `rem6-cpu` RISC-V cluster scheduler epochs and runs now expose ordered
  parallel batch timeline records with start tick, horizon, duration, worker
  count, and partition set, plus thresholded longest-window queries. The
  scheduler-scope `rem6-system` batch timeline maps that CPU evidence instead
  of rebuilding timeline records from raw batches; data-cache and full-system
  scoped timelines retain their existing cross-scope ordering.
- `rem6-cpu` RISC-V cluster scheduler epochs and runs now expose exact
  remote-send totals, source/target endpoint counts, ordered send records, and
  deterministic source/target partition sets at CPU scheduler scope.
  `rem6-system` reuses those CPU epoch APIs for scheduler remote-send counts
  and endpoint sets, while data-cache and full-system remote-send merging keep
  their existing scoped evidence paths.
- `rem6-system` scheduler checkpoint ports and banks now expose quiescence
  reports before capture. The reports identify pending-event totals, pending
  partition totals, first/last pending ticks, and serial/parallel pending-event
  counts per component and per partition, so checkpoint barriers can diagnose
  unsafe scheduler state before restore rather than discovering a scheduled
  event in the past later.
- Scheduler checkpoint capture now returns a typed non-quiescent scheduler
  error carrying that quiescence report, and scheduler checkpoint banks
  preflight all scheduler ports before writing scheduler chunks. This prevents
  partial scheduler checkpoint writes when any scheduler still has pending
  serial or parallel events.
- System checkpoint host actions now run the scheduler quiescence preflight
  before capturing any attached checkpoint bank, so a non-quiescent scheduler
  cannot leave CPU, memory, device, or scheduler checkpoint chunks from a
  rejected full-system checkpoint attempt.
- System checkpoint host actions stage all attached bank captures and
  execution-mode chunks in a cloned registry, then publish the registry and
  manifest only after final manifest capture succeeds. Invalid final capture
  inputs, such as empty labels, cannot leave partial CPU or device chunks in
  the live checkpoint registry.
- Execution-mode checkpoint restore now pre-registers internal host checkpoint
  components only in the staged registry. A rejected execution-mode restore
  cannot leave an internal host component registered in the live registry or
  cause later checkpoints with no execution-mode state to grow empty
  execution-mode chunks.
- Checkpoint manifest restore now treats the manifest as the authoritative
  state set for all registered components. Registered components absent from a
  restore manifest have their chunks cleared in the staged registry, so
  attached banks cannot pass validation by reusing stale chunks from an older
  full-system checkpoint.
- Checkpoint manifests now expose typed summaries with component count, chunk
  count, total payload bytes, and per-component chunk and byte counts. This
  gives checkpoint artifacts machine-checkable coverage evidence without
  relying on logs or ad hoc manifest scans.
- Workload replay results now carry checkpoint and checkpoint-restore manifest
  summaries with label, manifest tick, component count, chunk count, and total
  payload bytes, so replay artifacts retain checkpoint coverage evidence
  without rereading host action logs.
- Workload manifests can now declare minimum checkpoint and checkpoint-restore
  manifest component, chunk, and payload-byte totals. Replay-plan verification
  rejects missing or under-covered checkpoint summaries, which turns
  full-system checkpoint coverage into a checked manifest contract.
- Workload checkpoint summaries now preserve per-component chunk and
  payload-byte counts in replay results, and workload manifests can require
  minimum coverage for named checkpoint components during capture or restore.
- Workload resources now carry optional typed acquisition provenance, including
  acquisition kind, acquisition locator, tool, and revision. The manifest
  identity hashes this provenance, so artifact acquisition state cannot drift
  outside the reproducible workload contract.
- Disk-image workload resources now carry optional typed construction records
  with image format, virtual size, tool, operation, input, and argument data.
  These records are disk-image-only, checked with typed errors, hashed into the
  manifest identity, and owned by a dedicated resource module rather than the
  manifest facade.
- Workload parallel worker-use expectations now reject one-worker thresholds as
  invalid. A manifest contract that claims parallel worker use must require at
  least two workers, so serial execution cannot satisfy the natural parallelism
  proof by construction.
- Workload parallel worker-activity expectations now apply the same
  at-least-two-worker rule to total worker activity, so positive but serial
  activity cannot satisfy a manifest-declared parallel activity contract.
- Workload parallel partition-use expectations now reject one-partition
  thresholds, so a declared active-partition contract must prove at least two
  independently active scheduler partitions.
- Workload parallel batch timeline expectations now reject one-worker or
  one-partition records. Exact timeline contracts must prove at least two
  workers spanning at least two scheduler partitions before replay accepts them
  as parallel evidence.
- Workload parallel batch timeline records now also require positive duration
  before they can feed exact timeline contracts or derived batch-count,
  worker-count, partition-set, and partition-activity evidence. Zero-length or
  inverted windows no longer masquerade as natural parallel execution.
- Workload actual batch timeline records remain available for audit, but only
  records with positive duration, at least two workers, and at least two
  partitions feed derived batch worker, tick, partition-set, streak, and
  partition-activity evidence. Serial or single-partition records no longer
  satisfy parallelism contracts through secondary summaries.
- Workload batch timeline summaries now preserve malformed nonblank actual
  records for exact replay validation while excluding them from derived batch
  counts and DMA timeline fallback decisions. Inverted or zero-duration batch
  windows can be rejected as unexpected actual evidence instead of disappearing
  before replay validation.
- Workload batch partition-set replay also validates scoped raw batch timeline
  windows before deriving partition-set counts. A partition-set contract can no
  longer pass by filtering out an inverted or zero-duration actual timeline
  record while another valid record satisfies the minimum count.
- Workload batch partition-streak replay applies the same scoped raw batch
  timeline validation before deriving sustained partition-set streak counts.
  Streak contracts can no longer pass by filtering out a malformed timeline
  window while another valid record satisfies the consecutive-count threshold.
- Workload batch worker-count, tick-bucket, tick-activity, tick-streak, and
  worker-tick replay paths now validate scoped raw batch timeline records
  before deriving worker evidence. Duration or worker-count contracts cannot
  pass by silently discarding malformed timeline windows.
- Workload batch-activity replay now applies the same scoped raw batch
  timeline validation before deriving minimum-worker batch counts, so a valid
  batch cannot hide a malformed raw timeline record in the same scope.
- Workload full-system batch timeline reporting can now accept an explicit
  global timeline. Exact full-system replay records and duration-weighted
  worker/tick summaries can use a run-produced occupancy view directly instead
  of requiring every full-system window to be reconstructed from scoped CPU,
  data-cache, GPU DMA, and accelerator DMA timelines.
- Explicit full-system batch timelines must preserve scoped scheduler,
  data-cache, GPU DMA, and accelerator DMA records before replay accepts exact
  full-system timeline contracts. A merged execution timeline can no longer
  hide device-local batch evidence behind a smaller global record set.
- Workload parallel max-worker and total-worker replay now validate scoped raw
  batch timeline records before deriving upper-level worker evidence from
  batch histograms or timelines.
- Workload active-partition and per-partition activity replay now validate
  scoped raw batch timeline records before deriving partition evidence from
  batch windows, so valid partition evidence cannot hide malformed timeline
  records in the same scope.
- Workload scheduler-progress replay now validates scoped raw batch timeline
  records before deriving dispatch counts from batch summaries, so scheduler
  liveness contracts cannot pass through malformed batch windows.
- Workload full-system scheduler-progress replay also checks explicit merged
  dispatch counts against scoped CPU, data-cache, GPU DMA, and accelerator DMA
  lower-bound dispatch evidence. A global scheduler summary can merge epoch
  accounting without under-reporting worker-owned dispatch progress.
- Workload full-system scheduler-progress replay also keeps explicit merged
  epoch counts at or above the strongest scoped CPU, data-cache, GPU DMA, and
  accelerator DMA epoch evidence, including the combined GPU-plus-accelerator
  DMA scheduler epoch lower bound. A global scheduler summary can merge epoch
  accounting without hiding concurrent subsystem progress.
- Workload scheduler progress, idle, and frontier replay now reject scheduler
  summaries whose empty-epoch count exceeds the total epoch count before
  applying liveness, idle, or frontier thresholds.
- Workload frontier replay now also rejects raw frontier summaries whose
  safe-until tick is before the recorded now tick, or whose next event tick is
  before the recorded now tick. Full-system frontier checks scan scheduler,
  data-cache, GPU DMA, accelerator DMA, and explicit full-system frontier
  records before conservative merging, so malformed safe-time evidence cannot
  disappear behind a valid partition frontier.
- Workload wait-for diagnostic replay now rejects same-scope summaries whose
  edge-kind aggregate count falls below the exact edge-kind window count before
  applying clean-diagnostic, edge-kind count, edge-kind window, blocked-node
  window, or target-node window expectations. Exact diagnostic windows can no
  longer be used to satisfy replay while a weaker same-kind summary hides the
  stronger evidence.
- Workload wait-for diagnostic replay also rejects same-scope summaries whose
  total wait-for edge count falls below the strongest typed edge-kind,
  edge-kind window, blocked-node window, or target-node window evidence before
  applying diagnostic expectations. Aggregate clean or dirty diagnostic checks
  therefore cannot hide stronger typed blocking evidence behind a smaller total
  counter.
- Workload wait-for result ingestion now merges exact edge-kind window evidence
  into existing edge-kind aggregate maps without dropping unrelated kinds and
  without weakening same-kind aggregate counts. Adding exact tick-window
  evidence therefore preserves earlier typed wait-for totals instead of
  replacing them with a narrower diagnostic view.
- Workload full-system wait-for edge-kind reporting now accepts explicit
  merged full-system counts from a global scheduler while preserving scoped
  resource, data-cache, compute, and DMA counts as lower bounds. A merged
  full-system count that is weaker than same-kind scoped evidence is rejected
  before replay applies manifest diagnostic expectations.
- Workload full-system wait-for edge-kind window reporting now accepts
  explicit merged full-system windows from a global scheduler without adding
  them again to scoped windows for the same kind. A merged window must preserve
  at least the same edge count and tick coverage as scoped evidence before
  replay applies exact window expectations.
- Workload full-system wait-for blocked-node and target-node window reporting
  now accepts explicit merged full-system windows from a global scheduler while
  preserving scoped resource, data-cache, compute, and DMA node windows as
  lower-bound evidence. Same-node explicit full-system windows merge by the
  strongest edge count and widest tick range instead of double-counting scoped
  records, and weaker merged windows are rejected before replay applies exact
  node-window expectations.
- Workload clean parallel diagnostic replay now applies declared livelock
  transition thresholds to raw scoped progress-free transition records. A
  result with no materialized livelock diagnostic record is still rejected when
  scheduler or data-cache transition evidence reaches the manifest threshold
  for a subject in the checked scope.
- Workload diagnostic-scope validation now rejects livelock summaries whose
  aggregate progress-transition count is below the transition evidence stored
  in scoped livelock diagnostic records. Dirty livelock records therefore
  cannot be reported while a weaker aggregate counter hides the same evidence.
- Workload diagnostic-scope validation also rejects aggregate livelock
  diagnostic counts that exceed the aggregate progress-transition count. A
  result cannot claim dirty livelock diagnostics unless it also carries enough
  scoped progress-transition evidence to make those diagnostics possible.
- Workload full-system livelock merge validation rejects explicit merged
  full-system diagnostic evidence that is weaker than the scheduler or
  data-cache scoped livelock evidence it replaces. Empty or under-covered
  merged full-system records can no longer hide dirty scoped diagnostics.
- Workload full-system livelock merge validation also preserves scoped
  diagnostic subjects. Explicit merged full-system records must retain each
  dirty scoped subject's diagnostic count, transition count, and first/last
  transition tick coverage before clean-diagnostic replay can use the merged
  view, so a global scheduler cannot replace a real dirty subject with an
  unrelated aggregate record.
- Workload full-system livelock merge validation also preserves scoped
  transition-kind evidence. Explicit merged full-system records must retain
  each dirty scoped transition kind's diagnostic count, transition count, and
  first/last transition tick coverage, so kind summaries and kind-filtered
  queries cannot be weakened by a global scheduler aggregate.
- Workload full-system progress-transition merge validation now rejects
  explicit full-system transition streams that are weaker than scoped scheduler
  or data-cache transition evidence by aggregate count, subject, transition
  kind, or partition tick window. Clean-diagnostic livelock thresholds therefore
  cannot be satisfied by a global transition stream that drops scoped retry-loop
  evidence.
- Workload progress-transition replay now accepts GPU DMA and accelerator DMA
  scheduler transition records directly, and full-system progress-transition
  evidence derives from those DMA schedulers when no explicit merged record set
  is provided. Explicit merged full-system transition streams must also cover
  DMA scoped transition counts and windows, so heterogeneous device scheduler
  livelock evidence cannot disappear behind a CPU/cache-only summary.
- Workload GPU DMA and accelerator DMA transition evidence now exposes the
  same kind, partition, subject, tick-window, record, and summary queries as
  CPU and data-cache scheduler transitions. Heterogeneous scheduler debugging
  can therefore inspect device-side progress evidence without falling back to
  full-system aggregates.
- Workload DMA scheduler progress-transition reporting also exposes a combined
  GPU-DMA-plus-accelerator-DMA view with the same dimension queries, matching
  the existing combined DMA batch, frontier, and remote-traffic evidence
  surfaces.
- Workload manifests and replay plans now treat `dma-scheduler` as a
  first-class combined scope for scheduler progress, scheduler idle bounds,
  batch worker activity, exact batch timeline checks, partition-set and
  partition-streak checks, and remote endpoint coverage. Combined DMA
  partition-streak evidence is derived from the merged DMA batch timeline, so
  contiguous GPU DMA and accelerator DMA batches are validated as one typed DMA
  scheduler stream instead of two unrelated device-local fragments.
- Kernel parallel epoch planning now exposes the worker-limit-shaped batch plan
  before dispatch. A plan records the configured worker limit, deterministic
  ready-partition chunks, per-worker-count batch summaries, partition-set
  summaries, and maximum planned workers without executing callbacks. The
  scheduler can therefore prove planned multicore occupancy before spawning
  worker threads, avoiding gem5-style reliance on post-hoc global event-queue
  traces to infer parallelism.
- Recorded parallel runs now preserve that planned batch shape alongside the
  actual executed batches. Epoch and run summaries expose planned worker-count
  histograms, partition-set histograms, total planned workers, maximum planned
  workers, and the configured worker limit, so remote wakeups introduced during
  execution cannot overwrite the pre-dispatch multicore occupancy contract.
- System run summaries now lift planned scheduler batches into full-system
  timeline evidence. CPU-scheduler planned timelines and aggregate
  full-system planned timelines are queryable beside actual executed timelines,
  so a remote wakeup that changes the executed batch partition set remains
  distinguishable from the deterministic pre-dispatch plan.
- Workload-result parallel summaries now retain planned CPU-scheduler,
  data-cache scheduler, and merged full-system batch timelines separately from
  actual executed batch timelines. Replay summaries therefore keep the
  pre-dispatch multicore occupancy plan as auditable workload evidence instead
  of collapsing it into post-wakeup execution records.
- Workload manifests and replay plans can now require exact planned
  CPU-scheduler, data-cache scheduler, and merged full-system batch timeline
  records. This turns the deterministic pre-dispatch parallel plan into a
  first-class replay contract, so actual execution records alone cannot satisfy
  planned multicore occupancy requirements.
- Planned scheduler timelines also feed workload max-worker, total-worker,
  batch-worker bucket, duration, sustained-window, and worker-tick contracts.
  These planned scopes let manifests require natural multicore occupancy from
  the pre-dispatch plan, rather than accepting post-execution serial or
  wakeup-mutated evidence as a substitute.
- Workload manifests and replay plans can now require minimum planned
  utilization ratios for CPU-scheduler, data-cache scheduler, GPU DMA,
  accelerator DMA, combined DMA, and merged full-system planned scopes.
  They can also cap planned idle-worker ticks and per-worker-slot planned idle
  ticks for the same scopes while requiring minimum active ticks for each
  declared slot. Planned worker capacity, idle ticks, worker-ticks, and slot
  load distribution are therefore checked as manifest-owned efficiency
  contracts instead of being reconstructed by external scripts after a run.
- Planned scheduler timelines now also feed exact partition-set, sustained
  partition-streak, active-partition, and per-partition activity contracts.
  Manifest checks can therefore require which partitions were naturally planned
  to run together, not just how many workers the post-dispatch run used.
- Planned GPU DMA and accelerator DMA batch timelines are first-class workload
  evidence, with direct, combined DMA, and merged full-system planned scopes.
  Heterogeneous device schedulers can now prove pre-dispatch parallel DMA
  occupancy through exact timeline, worker-bucket, partition-set, and planned
  utilization contracts.
- Explicit planned full-system batch timelines must preserve scoped planned
  scheduler, data-cache, GPU DMA, and accelerator DMA records before replay
  accepts exact timeline or derived worker/partition contracts. A global
  pre-dispatch plan therefore cannot hide device-local planned occupancy behind
  a weaker merged timeline.
- Batch-timeline evidence validation rejects duplicate records at the replay
  boundary. Replaying the same pre-dispatch batch record twice can no longer
  inflate planned full-system worker-bucket evidence.
- Workload resource deadlock merge validation rejects explicit merged resource
  deadlock counts that are weaker than the fabric and DRAM scoped deadlock
  evidence they replace. Resource clean-diagnostic replay therefore cannot hide
  dirty fabric or DRAM deadlocks behind a smaller merged counter.
- Workload full-system deadlock merge validation rejects explicit merged
  full-system deadlock counts that are weaker than resource and data-cache
  scoped evidence. Full-system clean-diagnostic replay therefore cannot hide
  dirty resource or data-cache deadlocks behind a smaller merged counter.
- Workload full-system diagnostic presence checks now treat explicit merged
  full-system deadlock counts as diagnostic evidence even when lower scopes are
  otherwise clean. Full-system replay and reporting APIs therefore cannot lose
  a dirty merged deadlock summary at the presence-check boundary.
- Workload direct batch summaries now apply the same evidence boundary: worker
  histograms require at least two workers, and partition sets or streaks require
  at least two partitions before they can feed derived worker, batch,
  active-partition, or partition-activity contracts. Direct serial summaries no
  longer satisfy natural-parallelism contracts through aggregate counters.
- Workload full-system batch worker-count summaries now keep the strongest
  bucket evidence between scoped worker histograms and explicit full-system
  partition streaks. Reporting APIs therefore cannot understate worker buckets
  that replay contracts already accept through full-system streak evidence.
- Workload full-system batch worker-count summaries now also accept explicit
  merged full-system worker histograms. A global scheduler can report the
  occupancy buckets it actually executed without forcing those buckets through
  exact partition-set or timeline evidence.
- Workload full-system worker-count tick summaries now accept explicit merged
  full-system duration-weighted worker histograms. A global scheduler can
  report occupancy time directly while same-bucket scoped timeline evidence
  remains a lower bound instead of being added again.
- Workload full-system worker-count tick-streak summaries now accept explicit
  merged full-system sustained-occupancy records. A global scheduler can
  report longest continuous occupancy windows directly, while scoped exact
  timelines remain lower-bound evidence for the same minimum-worker queries.
- Workload full-system scheduler-work presence checks now treat explicit
  merged full-system worker-tick bucket and sustained-occupancy summaries as
  full-system work evidence. Result presence APIs therefore expose the same
  direct worker-tick evidence that replay and summary queries already accept.
- Workload scheduler-work presence checks now also treat scoped scheduler and
  data-cache scheduler epoch or empty-epoch counts as work evidence. Idle-only
  scheduler summaries therefore remain visible to scoped and full-system
  presence APIs instead of existing only as raw count queries.
- Workload scheduler-work presence checks now also treat direct GPU DMA and
  accelerator DMA empty-epoch counts as DMA work evidence. Heterogeneous DMA
  idle-bound summaries therefore remain visible to full-system presence APIs
  instead of existing only as raw DMA count queries.
- Workload scoped GPU DMA and accelerator DMA activity checks now use the same
  direct scheduler evidence boundary, including epoch, empty-epoch, dispatch,
  batch, worker-tick, frontier, and remote-traffic records. Device activity
  summaries therefore cannot hide scheduler-visible DMA work.
- Workload resource activity checks now treat fabric byte, occupancy, queue
  delay, contention, and DRAM row, command, turnaround, and latency counters as
  activity evidence. Resource activity presence therefore cannot hide dynamic
  NoC or memory-controller evidence behind transfer/access aggregate counts.
- Workload DRAM QoS activity checks now treat priority and requestor breakdown
  summaries as QoS activity evidence. Per-class memory-controller accounting
  therefore cannot remain queryable while hidden from DRAM and resource
  activity presence.
- Workload DRAM and aggregate resource activity contracts now count the
  strongest DRAM operation evidence across aggregate access counts, read/write
  totals, row outcomes, command counts, QoS aggregate counts, and QoS
  priority/requestor breakdowns. Full-system run summaries and workload replay
  results use the same lower bound, so manifest-declared memory-controller
  activity cannot be underreported just because a run exports class-specific QoS
  summaries instead of a top-level access count.
- Workload fabric, DRAM, and aggregate resource active-count contracts now infer
  a conservative active-resource lower bound from operation, contention,
  diagnostic, and wait-target evidence when explicit active lane or target
  aggregates are absent. Coarse summaries therefore cannot fail active-resource
  contracts merely because the producer omitted a redundant active-count field.
  DRAM active-resource lower bounds now also preserve the strongest explicit
  target, port, or bank count, so memory-channel and bank-level parallelism
  cannot be hidden by collapsing those scopes to one active DRAM class.
- Workload aggregate resource activity contracts now use the same resource
  activity count as workload summaries, including fabric and DRAM wait-for
  diagnostics. Dirty resource queues therefore cannot be visible to summary
  APIs while being ignored by manifest-declared aggregate resource minima.
- Workload scheduler-work presence checks now treat scoped and explicit
  full-system progress-transition records as work evidence. Retry-loop and
  progress-free transition records therefore stay visible at the same presence
  boundary used by summary and replay diagnostics.
- Workload scheduler-work presence checks now treat scoped and merged
  full-system livelock diagnostic counts as work evidence. Dirty scheduler
  diagnostics therefore cannot remain visible only to diagnostic queries while
  disappearing from scheduler-work presence checks.
- Workload full-system batch partition-set summaries now keep the strongest
  per-set evidence between scoped partition-set histograms and explicit
  full-system partition streaks. Reporting APIs therefore expose the same
  full-system partition coverage that replay contracts use for set minima.
- Workload full-system batch worker-count replay now rejects explicit merged
  worker-count buckets that are below scoped CPU, data-cache, GPU DMA, or
  accelerator DMA bucket lower bounds for the same worker count. Exact
  full-system bucket contracts therefore cannot pass only because reporting
  getters take the strongest scoped bucket after a weak global bucket is
  present.
- Workload full-system active-partition replay now rejects explicit merged
  active-partition counts that are below scoped activity, batch-set,
  batch-streak, per-partition activity, or remote-traffic lower-bound evidence.
  A global scheduler cannot publish a weak active-partition total while the
  derived lower-bound evidence proves more active partitions.
- Workload full-system per-partition activity reporting now treats explicit
  full-system partition streaks as worker and dispatch evidence for every
  partition in the streak. Per-partition reporting therefore cannot drop
  merged full-system batch evidence after active-partition summaries accept it.
- Workload full-system per-partition activity reporting now also accepts
  explicit merged full-system partition activity records. A global scheduler
  can report partition-owned worker, dispatch, and remote activity directly
  while scoped and batch-derived evidence remain lower bounds.
- Workload full-system per-partition activity replay now rejects explicit
  merged activity records that are weaker than scoped or batch-derived
  lower-bound evidence for the same partition. Worker, dispatch, remote-send,
  remote-receive, and pending-event counts therefore cannot be hidden by
  lower-bound aggregation after a weak global activity summary is reported.
- Workload full-system remote-traffic reporting now accepts explicit merged
  full-system remote flow and send records. A global scheduler can report
  cross-partition communication directly while same-route scoped flow evidence
  stays a lower bound instead of being added to the merged total again.
- Workload full-system remote-flow merge validation now rejects explicit
  full-system flow records that are weaker than same-route scheduler,
  data-cache, GPU DMA, or accelerator DMA evidence by send count, delivery tick
  window, or delay-bound window. A global remote-flow summary therefore cannot
  narrow away scoped cross-partition traffic while replay checks still pass from
  derived lower-bound evidence.
- Workload full-system exact remote-send merge validation now rejects explicit
  full-system send streams that omit scheduler, data-cache, GPU DMA, or
  accelerator DMA scoped sends. A global exact-send summary therefore cannot
  drop subsystem cross-partition traffic while the merged full-system getter
  still finds the scoped record through lower-bound evidence.
- Workload full-system progress-transition reporting now accepts explicit
  merged full-system transition records. A global scheduler can report exact
  progress-free transition evidence directly without concatenating scoped
  exact-record streams into an already-merged full-system record set.
- Workload full-system exact progress-transition validation now rejects
  explicit full-system transition streams that omit scheduler, data-cache, GPU
  DMA, or accelerator DMA scoped transition records. A global progress-free
  transition stream therefore cannot drop subsystem livelock evidence while
  exact full-system replay expectations still pass from a weaker merged record
  set.
- Workload full-system scheduler-count reporting now accepts explicit merged
  epoch, empty-epoch, and dispatch counts. A global scheduler can report its
  aggregate progress and idle counts directly instead of forcing replay to sum
  scoped scheduler counters that may describe concurrent global epochs, while
  dispatch reporting preserves scoped lower-bound work evidence.
- Workload full-system scheduler-count validation now checks explicit merged
  epoch and empty-epoch counts directly and rejects explicit dispatch counts
  weaker than scoped dispatch lower bounds before replay accepts idle or
  progress contracts. A dirty merged full-system summary can no longer hide
  behind individually valid scoped scheduler summaries, and explicit epoch
  counts must still cover the strongest scoped epoch lower bound, including
  combined DMA scheduler epoch evidence.
- Workload full-system frontier reporting now accepts explicit merged
  full-system initial and final partition frontiers. A global scheduler can
  report conservative safe-time frontiers directly while scoped CPU, cache, and
  DMA frontiers remain conservative lower bounds for the same partition.
- Workload full-system frontier merge validation now rejects explicit
  full-system initial or final partition frontiers that are more optimistic
  than scoped scheduler, data-cache, GPU DMA, or accelerator DMA frontiers by
  now tick, safe-until tick, next pending tick, or pending-event count. A global
  frontier summary therefore cannot hide an earlier scoped safe-time boundary
  while replay contracts still pass through the conservative merged getter.
- Workload full-system dispatch reporting now treats explicit full-system
  partition streaks as aggregate dispatch evidence, using them as a merged
  lower bound instead of adding them to scoped scheduler counts again.
- Workload full-system batch worker-count reporting now derives worker buckets
  from scoped partition-set and partition-streak evidence before merging any
  explicit full-system streaks, so replay checks and result summaries expose
  the same worker-bucket evidence.
- Workload full-system partition-set reporting now derives partition-set
  buckets from scoped partition streaks before merging explicit full-system
  streaks, so per-set queries and summary lists expose the same evidence.
- Workload full-system partition-set reporting now also accepts explicit
  merged full-system partition-set histograms. A run that already records
  global non-consecutive batch sets can feed result summaries and replay
  partition-set contracts without pretending that exact set counts are
  consecutive streaks.
- Workload full-system minimum-worker batch reporting now sums the already
  merged worker buckets, so scoped evidence in one bucket and explicit
  full-system evidence in another bucket are both visible to threshold checks.
- Workload full-system total batch reporting now merges preferred scoped worker
  buckets with explicit full-system buckets while preserving scoped actual
  batch totals, so exact totals do not drop disjoint merged evidence or
  overcount alternative timeline evidence.
- Workload full-system total-worker reporting now uses the same preferred
  bucket merge while preserving scoped actual worker totals, so total-worker
  activity does not drop disjoint scoped and explicit full-system evidence.
- Workload full-system dispatch reporting now uses the same preferred bucket
  merge while preserving scoped actual dispatch totals, so worker-owned
  dispatch evidence remains visible across disjoint merged buckets.
- Workload full-system per-partition activity reporting now derives its batch
  activity lower bound from merged partition-set and streak evidence, so a
  partition does not lose scoped participation when explicit full-system
  participation lands in a different partition-set bucket.
- Workload direct per-partition activity now normalizes dispatch counts without
  worker evidence to zero while preserving remote send and receive evidence.
  A replay artifact can still prove cross-partition communication, but it
  cannot prove scheduler progress from a dispatch that no worker owned.
- Workload parallel remote-flow and remote-flow timing expectations now reject
  same-partition endpoints. A manifest-declared remote-flow contract must cross
  partition boundaries before replay accepts it as remote parallel evidence.
- Workload parallel remote-send expectations now reject same-partition
  endpoints and delivery ticks earlier than source ticks at manifest and replay
  plan insertion. Exact send evidence therefore remains both cross-partition
  and time-causal before replay can validate it.
- Workload parallel remote-delay ceiling expectations now reject zero-delay
  ceilings at manifest and replay plan insertion, matching the scheduler's
  positive remote-delay invariant before replay can accept bounded remote
  traffic evidence.
- Workload parallel remote-delay floor and ceiling expectations now reject
  contradictory same-scope windows at manifest and replay plan insertion. A
  declared minimum remote delay cannot exceed the declared maximum remote delay,
  so impossible timing contracts fail before result replay.
- Workload parallel remote-traffic consistency expectations now reject explicit
  aggregate remote-flow evidence without matching exact remote-send evidence.
  This keeps aggregate flow contracts tied to replayable per-send timing
  records instead of accepting summary-only remote traffic.
- Workload parallel remote-traffic consistency expectations also reject exact
  remote-send routes that are omitted from an explicit same-scope aggregate
  remote-flow set. Exact sends can still provide stronger derived evidence when
  aggregate records are absent, but a present aggregate must be complete for its
  scope.
- Workload parallel remote-traffic consistency now rejects invalid actual
  remote-send evidence as well as invalid manifest declarations. A replayed
  remote send must cross partitions and must not deliver before its source tick,
  preventing local or inverted records from masquerading as parallel traffic.
- Workload parallel remote-traffic consistency now rejects invalid actual
  aggregate remote-flow evidence too. A replayed aggregate remote flow must
  cross partitions, must have an ordered first/last tick window, and must not
  report inverted delay bounds.
- Workload result summaries keep raw same-partition remote traffic available
  for replay validation, but exclude it from derived remote-flow evidence,
  remote-send counts, endpoint sets, active-partition counts, per-partition
  activity, and work flags. Local records therefore remain auditable invalid
  actual evidence without satisfying natural parallelism through secondary
  summaries.
- Workload result summaries apply the same derived-evidence boundary to
  malformed remote timing. Inverted remote sends, inverted aggregate flow
  windows, and inverted aggregate delay bounds remain visible as raw actual
  records for replay validation, but cannot feed remote evidence, endpoint,
  active-partition, activity, or work summaries.
- Workload result summaries also derive remote-send evidence through exact
  record de-duplication. Duplicate raw remote-send records remain visible to
  replay validation, but cannot inflate derived remote-flow counts,
  remote-send counts, endpoint summaries, or per-partition remote activity.
- Workload result summaries preserve explicit remote-flow records as raw
  actual evidence instead of merging same-route records at ingestion. Derived
  remote-flow evidence still aggregates valid same-route records, while replay
  validation can reject a malformed aggregate record even when another record
  on the same route is valid.
- Workload full-system remote-flow validation now scans subsystem raw
  aggregate records before same-route full-system merging. Malformed scheduler,
  data-cache, GPU DMA, or accelerator DMA aggregate records cannot be hidden by
  another valid contribution on the same full-system route.
- Workload full-system remote-traffic consistency validation reuses that raw
  subsystem aggregate scan before comparing aggregate flow records with exact
  remote sends, so a consistency contract cannot pass by merging away a
  malformed subsystem flow record.
- Workload exact remote-send, exact remote-flow, flow-timing, endpoint, and
  remote-delay verifier paths now apply the same actual remote evidence
  structural checks before matching or deriving observations, so malformed
  sends or aggregate flows fail as invalid replay evidence rather than as
  ordinary unexpected traffic.
- Workload partition-use and partition-activity contracts now reuse the same
  actual remote evidence checks before deriving active partition sets or
  per-partition remote send/receive counts, preventing malformed remote records
  from satisfying parallelism expectations through secondary summaries.
- `rem6-cpu` RISC-V cluster scheduler epochs and runs now expose
  kernel-recorded progress-free transition records, transition-kind counts, and
  progress-monitor snapshots at CPU scheduler scope. `rem6-system` consumes
  those CPU APIs for scheduler progress counts and records instead of scanning
  raw batches, while data-cache and merged full-system diagnostics retain their
  existing scoped ordering and tick-window semantics.
- `rem6-virtio` split queues now treat available-ring interrupt suppression as
  device-visible state. Completion paths still write descriptor status, data,
  used-ring element, and used index, but they do not set ISR status or post
  INTx, MSI, or MSI-X delivery when the guest has requested no completion
  interrupt. This keeps full-system device behavior observable through typed
  writeback and interrupt-delivery results instead of relying on implicit logs.
  Split queues can also opt into event-index notification suppression so
  completion interrupts follow the guest's `used_event` threshold instead of
  the legacy available-ring flag.
- `rem6-virtio` split queues can now consume guest-memory indirect descriptor
  tables for block requests while preserving the main queue head as the request
  id and used-ring id. The indirect table is still typed guest memory, not a
  host-side descriptor shortcut, so descriptor data, writeback targets, and
  used-ring completion evidence remain replayable. The main descriptor's
  write-only flag is ignored for indirect-table lookup, matching the VirtIO
  device requirement, and direct descriptor prefixes followed by a terminal
  indirect table expand into one ordered block request while logical descriptor
  length is bounded by queue size before the queue cursor advances.

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
| `src/base` | 199 | `rem6-kernel`, `rem6-stats`, shared crate utilities | partial | Preserve useful statistics, loader, debug, and helper concepts without a large untyped utility layer. Runtime-visible data must remain typed. Stats counter paths are now structured scope/name identities, registry-owned stat groups attach stable group ids plus self-describing group catalogs to snapshots and deltas, stat descriptions preserve checked `Info.desc`-style metadata, nested rate units remain machine-readable in snapshots, stats reset requests reject ticks earlier than the active reset window, typed stats resets and dumps preserve stable reset/dump ids plus reset/snapshot payloads in registry-owned, interleaved, workload-result, and manifest-verifiable histories with exact event-sequence contracts, stats registries reject late counter or group registration after the first dump/reset history record so one history stream has one stable schema, and typed stats deltas reject cross-scope, group-catalog-drifting, description-drifting, schema-drifting, time-regressing, or value-regressing snapshots so reset ordering and stat descriptor drift cannot silently corrupt dump scope. |
| `src/cpu` | 363 | `rem6-cpu`, `rem6-kernel`, `rem6-system` | partial | RISC-V cluster execution exists, RISC-V data access records expose absent memory-route metadata for MMIO accesses as typed optional state instead of panic-only accessors, CPU cluster parallel epochs retain both initial and final partition frontiers through full-system run summaries, and CPU cluster scheduler epochs plus runs expose exact remote-send records, remote-send endpoint counts, progress-free transitions, ordered batch timelines, longest parallel-window evidence, batch worker-count, duration-weighted worker-count tick, worker-tick, partition-set, and same-partition-set streak evidence from the kernel without relying on caller-local batch scans for scheduler-scope counts. Branch prediction has typed state for several gem5-style predictors, and GShare history updates reject stale prediction records before per-thread history or counters can drift. gem5 simple, checker, Minor, O3, KVM-style switching, and traffic testers need typed rem6 equivalents or explicit replacement models. |
| `src/dev` | 418 | `rem6-mmio`, `rem6-uart`, `rem6-timer`, `rem6-interrupt`, `rem6-gpu`, `rem6-accelerator`, `rem6-platform`, `rem6-pci`, `rem6-virtio` | partial | UART, timer, interrupt, an initial typed RISC-V CLINT MMIO model with crate-level snapshot/restore, typed reset policy, platform/topology attachment, typed RISC-V DTS source emission, binary FDT/DTB emission, RISC-V DTB memory/A1 handoff, typed Linux `/chosen` bootargs and initrd DTB metadata, typed DTB and initrd blob installation for store-backed and DRAM-backed memory, GPU, accelerator, initial PCI endpoint, host config-space, 32-bit and 64-bit BAR, BAR MMIO, legacy INTx, MSI, MSI-X, and modern VirtIO PCI common-config, notify-MMIO, ISR-status, device-config, split virtqueue snapshot/restore plus prevalidated system checkpoint capture, regular capability-byte, notify capability-byte, and shared-memory capability paths exist. rem6-pci keeps its root as a facade over focused device modules, including a dedicated typed error module, so PCI breadth does not recreate gem5-style device monoliths. Storage, network, PS/2, QEMU bridge, and broader platform-specific devices remain alignment targets. |
| `src/gpu-compute` | 73 | `rem6-gpu`, `rem6-accelerator`, `rem6-transport` | partial | Preserve command queues, compute-unit scheduling, DMA, and traceability. Current rem6 GPU execution is a smaller typed model. |
| `src/kern` | 18 | `rem6-system`, `rem6-platform`, workload resources | partial | RISC-V Linux boot handoff can install initrd bytes, emit matching `/chosen` DTB metadata, place generated or resolved-resource DTBs in guest memory, and set A1 through typed system APIs. Broader Linux symbols, panic/oops hooks, guest ABI helpers, and other ISA kernels remain open. |
| `src/mem` | 682 | `rem6-memory`, `rem6-transport`, `rem6-cache`, `rem6-directory`, `rem6-coherence`, `rem6-dram`, `rem6-fabric`, protocol crates | partial | rem6 already splits protocol state, topology, NoC, DRAM, replacement state, MSHR resources, prefetch queues, stores, directory state, and coherence harnesses into typed crates. CHI-like line states, a single-line cache controller, a multi-line cache bank, an initial directory decision model, serial plus partitioned multi-cache coherence harnesses, topology-built CHI cache-directory and DRAM routes, CHI recorded run-resource summaries, workload-replay CHI data-cache attribution, manifest-verifiable data-cache run attribution, manifest-verifiable data-cache run accounting consistency, manifest-verifiable MSI/MESI/MOESI/CHI data-cache protocol run counts, manifest-verifiable fabric/DRAM/resource activity counts, manifest-verifiable per-hop, per-link, per-lane, and per-virtual-network fabric activity contracts including hop-index/link/virtual-network minima, per-link, per-lane, and per-virtual-network queue-delay budgets plus per-lane peak queue-delay, per-virtual-network lane fanout, contention budgets, required-link coverage, and per-hop, per-link, per-lane, and per-virtual-network activity-window coverage, identity-backed unique coverage merges for per-link virtual-network counts and per-virtual-network lane counts, direct topology CHI data-cache attach, direct topology store-backed and DRAM-backed CPU fetch/data line-layout derivation from addressed memory regions, per-transfer fabric hop activity with packet, hop index, link, virtual network, queue delay, and timing records, per-link fabric activity, per-lane fabric activity, per-virtual-network fabric activity, and MSHR-backed cache bank QoS metadata, ready arbitration, and typed downstream QoS export exist; broader CHI transactions, prefetcher breadth, cache/DRAM QoS policy breadth, and Ruby-network breadth remain open. |
| `src/python` | 253 | `rem6-workload`, `rem6-platform`, future front ends | partial | Keep gem5's ease of composition while replacing Python object wiring with checked manifests and typed builders. Workload manifests now record typed Linux boot handoff intent, including DTB address, bootargs, device-tree resource identity, initrd address range, and initrd resource identity. RISC-V core fetch and data routes must originate from the declared core partition and source endpoint before replay can build a cluster. RISC-V workload replay derives each core's fetch line layout from the memory target containing the current fetch PC instead of assuming the first target or entry target, and derives replay-injected data request line layouts from the memory target containing each data access address. RISC-V data-cache backing routes must be declared explicitly and originate from the data-cache directory partition and endpoint before replay can attach an external-memory backed cache. GPU and accelerator command routes must target the declared device partition and control endpoint, and GPU and accelerator DMA routes must originate from the declared device partition and DMA endpoint. Resolved resource payloads validate required resource id, digest, device-tree kind, initrd kind, initrd byte length, and manifest identity before workload replay installs DTB and initrd bytes into guest memory. Workload suites now sort manifests by typed workload id, carry deterministic suite identities, derive replay plans from manifest identities, compute deterministic round-robin or least-loaded weighted dispatch records for a checked worker count, carry weighted estimated ticks on dispatch records, summarize planned worker load, makespan, capacity, idle ticks, speedup, and utilization before execution, build planned dispatch timelines with per-workload start/final ticks from the same weighted records, reject load summaries or timelines for dispatch records without estimates, reject planned-load expectations with mismatched suite identity or worker count, reject planned speedup or utilization that falls below declared thresholds, materialize planned timelines as execution summaries, report planned worker occupancy, wall-clock span, occupancy windows, occupancy active/idle/capacity worker ticks, worker-count tick histograms, full and underoccupied tick spans, minimum occupancy worker count, occupancy utilization, per-worker idle ticks, per-worker-slot active/idle ticks, minimum sustained-occupancy checks, minimum full-occupancy checks, and maximum underoccupied-tick checks before a run, verify planned timelines against execution expectations before a run, reject actual execution windows that drift from the planned timeline, reject zero, missing, duplicate, or unexpected dispatch weights, create dispatch-declared suite execution expectations, reject unreachable suite parallelism requirements before execution, verify execution completion records against dispatch order, manifest identity, worker assignment, and optional planned tick windows, derive suite execution summaries from dispatch plans plus per-workload results, record per-workload result start/final windows with invalid-window rejection, compute per-worker suite completion summaries, report runtime per-worker-slot active/idle ticks, report maximum simultaneous workers, enforce minimum simultaneous-worker contracts, summarize suite wall-clock span, serial completion ticks, worker capacity, idle worker ticks, speedup ratios, and utilization ratios, and reject declared speedup, utilization, worker-count occupancy, full-occupancy, or underoccupied-tick thresholds that are not met so overlap and throughput are typed evidence rather than inferred from logs. Workload-result summaries preserve typed guest-host call counts, workload replay outcomes preserve typed guest-host response payloads, and parallel summaries preserve CPU scheduler, data-cache scheduler, direct GPU DMA scheduler, direct accelerator DMA scheduler, and merged full-system remote-send records with source ticks, delivery ticks, delay, and order, remote-flow records as typed partition pairs with counts, first/last ticks, and optional min/max delay bounds plus scheduler epoch, empty-epoch, dispatch counts, exact progress-free transition records, total counts, deterministic dimension lists, per-dimension record slices, counts, tick windows, and compact summaries by kind, partition, and subject, livelock diagnostic records, subject queries, subject summaries, kind summaries with exact kind tick windows, kind-filtered subject and record queries, tick windows, and counts, merged resource and full-system deadlock diagnostics, total worker counts, scoped workload-result batch timeline records that derive batch worker-count, partition-set, and streak evidence, tick-ordered full-system batch worker-count histograms, exact batch partition-set histograms, maximum consecutive batch partition-set streaks including explicit merged full-system streaks, per-partition worker, dispatch, remote-send, and remote-receive activity, data-cache total run counts, attributed and unattributed run counts, data-cache protocol run attribution, fabric/DRAM resource activity, and per-transfer fabric hop activity; workload manifests include exact expected scheduler, data-cache scheduler, direct GPU DMA scheduler, direct accelerator DMA scheduler, or full-system remote-send records, exact expected batch timeline records, exact expected progress-free transition records, remote-flow counts, first/last tick windows, optional min/max delay bounds, endpoint sets, scope-wide minimum delay floors and maximum delay ceilings, plus minimum max-worker use, scheduler epoch and dispatch progress, maximum scheduler idle epochs, total-worker activity, multi-worker batch activity, exact batch partition-set activity, sustained same-batch partition-set streak activity, active partition counts, per-partition activity minima, minimum attributed and maximum unattributed data-cache run counts, data-cache accounting consistency, minimum data-cache protocol run counts, minimum fabric/DRAM/resource activity, minimum per-hop, per-link, per-lane, and per-virtual-network fabric activity, hop-index/link/virtual-network activity windows, per-link, per-lane, and per-virtual-network queue-delay budgets, per-lane peak queue-delay, per-virtual-network lane fanout and contention budgets, per-virtual-network required-link coverage, per-link, per-lane, and per-virtual-network activity-window coverage, clean parallel diagnostic expectations, and minimum scoped wait-for edge-kind count expectations in manifest identity, and replay plans validate wait-for, deadlock, and livelock cleanliness with dirty livelock subject evidence plus kind-specific wait-for count minimums in verification failures. Boot resources, custom guest-host calls, custom guest-host responses, workload suites, suite dispatch plans, weighted dispatch inputs, weighted dispatch load summaries, planned-load expectations, planned dispatch timelines, dispatch-declared execution expectations, execution efficiency summaries, declared suite speedup, utilization, and runtime occupancy thresholds, and dispatch-derived suite execution summaries are reproducible data rather than Python workflow side effects. |
| `src/sim` | 176 | `rem6-kernel`, `rem6-system`, `rem6-checkpoint`, `rem6-stats`, `rem6-power` | partial | Event queues, ticks, objects, exit events, power hooks, probes, checkpoints, and statistics need typed partitioned equivalents. Core scheduling, recorded initial and final parallel-epoch partition frontiers, per-partition parallel activity summaries, remote send records with explicit source tick, delivery tick, persistent source-local order, and lookahead delay carried through system and workload summaries, remote-flow records with optional min/max delay bounds, typed subsystem-local and merged full-system wait-for/deadlock diagnostics, scheduler-recorded progress-free transitions with persistent partition-local order carried into workload summaries, system-run deterministic dimension lists, per-dimension record slices, counts, tick windows, compact summaries by kind, partition, and subject, direct batch worker-count, worker-tick, partition-set, and same-partition-set streak summaries, plus livelock diagnostic subject and transition-kind queries, subject summaries, subject tick windows, transition-kind counts, and kind-window summaries, typed progress-free transition livelock diagnostic records and counts, typed scheduler checkpoint quiescence reports with pending-event tick windows and serial/parallel kind counts, typed probe events, typed power domains with stats-derived expression inputs that reject reset-scope, time, value, group-catalog, descriptor, or description drift through typed errors, monotonic stats reset handling through host actions, typed stats dump outcomes, and checkpoints exist. |
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
| `configs/dram`, `configs/nvm` | 5 | `rem6-dram`, `rem6-memory` | partial | External DDR, HBM, LPDDR, and NVM profiles have typed topology, geometry, bank-group geometry, timing, burst spacing, same-bank-group burst spacing, command-window bandwidth limits, parallel port counts, topology-unit counts, scheduler bank counts, topology bank counts, bank-group capacity summaries, manifest identity, checkpoint encoding, restore-time profile validation, and activity metadata. Runtime activity profiles carry profiled-target counts and profile-derived port, topology-unit, bank, and bank-group capacity denominators separately from active counts. DRAM same-arrival QoS timing batches respect same-agent memory-ordering barriers before priority or turnaround selection. Workload resource summaries preserve the strongest explicit DRAM target, port, or bank active-resource lower bound. NVM media timing can model separate read-media, write-media, send latency, pending-read buffers, and pending-write queue depth. Profile breadth and fuller media behavior remain open. |
| `configs/network` | 2 | `rem6-fabric`, `rem6-transport` | partial | Network configuration must map to NoC lanes, virtual networks, credits, wait-for diagnostics, per-transfer hop activity, per-link activity, per-lane activity, per-virtual-network activity, queue-delay budgets across those scopes, per-virtual-network lane fanout, contention budgets, required-link coverage, and activity-window coverage across those activity scopes. Same-scope link and virtual-network activity-window merges preserve unique resource coverage when lane identities are known. |
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
| `src/cpu/pred` | `rem6-cpu` branch predictor modules | partial | A local two-bit predictor, GShare predictor, BiMode predictor, Tournament predictor, loop predictor, TAGE base predictor, LTAGE predictor, TAGE-SC-L wrapper, standalone multiperspective perceptron predictor, 8KB statistical corrector, branch target buffer, indirect target predictor, and return-address stack have independent typed prediction, lookup, update, target, replacement, speculative history, commit, repair, and snapshot state. GShare keeps gem5's PC xor GHR indexing while replacing opaque history pointers with typed records, per-CPU GHR snapshots, stale-history update rejection, masked squash repair, and restore validation. BiMode keeps gem5's PC-indexed choice table and PC xor GHR direction tables while exposing selected-array training, choice-counter policy, and stale-history update rejection as typed records. Tournament keeps gem5's shared local history table, per-CPU global history, global-history-indexed choice table, disagreement-only choice training, stale-history update rejection, and squash repair while exposing each record as typed state. The loop predictor keeps gem5's set/way indexing, tag matching, confidence threshold, use counter, and optional speculative iteration state while replacing random allocation with deterministic per-set cursors for replayable parallel runs. TAGE base keeps gem5's bimodal table, tagged-table provider selection, folded-history index and tag hashing, alt-on-new counter, useful-bit reset, repairable speculative history, stale-history update rejection, and deterministic allocation records. LTAGE composes TAGE and loop prediction with explicit final provider records, prevalidated loop-before-TAGE conditional training, combined repair, matching thread and instruction-shift validation, and nested snapshot restore. TAGE-SC-L composes LTAGE with the statistical corrector in gem5 order: TAGE and loop predict before SC override, and SC trains before loop and TAGE updates after nested stale-history prevalidation, with explicit repair and nested snapshot records. The standalone multiperspective perceptron keeps gem5's 8KB feature profile shape, transfer tables, filter behavior, low-confidence best-table path, adaptive training threshold, and local/global/path/IMLI/recency histories while making all table and history state typed and per CPU. The 8KB statistical corrector keeps gem5's bias tables, global/backward/local/IMLI GEHL sums, confidence chooser, threshold training, and repairable histories while making histories per CPU for parallel replay instead of hidden shared global state. The indirect target predictor replaces opaque history pointers and random target replacement with typed records, per-CPU history, deterministic LRU, and restore validation. The specialized 8KB/64KB TAGE-SC-L table geometry, 64KB statistical corrector extensions, and MPP_TAGE integration remain open. |
| `src/cpu/checker` | `rem6-cpu` verification harness | planned | Checker behavior should compare architectural commits without hidden simulator state. |
| `src/cpu/kvm` | host-controlled execution modes | partial | rem6 models execution modes and statistics scope; host-assisted native execution is not present yet. |
| `src/cpu/testers`, `src/cpu/trace`, `src/cpu/probes` | tests, trace, stats crates | partial | Traffic generation, trace replay, and probes should feed typed events and summaries. |

### Memory, Cache, Coherence, and NoC

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/mem/cache` | `rem6-cache`, `rem6-coherence`, protocol crates | partial | MSI, MESI, MOESI, and an initial CHI-like state machine, cache controller, cache bank, directory model, and serial plus partitioned multi-cache coherence harnesses exist with tests. The CHI controller can issue ReadShared, ReadUnique, and MakeReadUnique-shaped downstream requests, complete shared and unique fills, preserve pending-miss snapshots, service local slices, and apply snoop downgrade or invalidation to resident data. The CHI cache bank owns multiple line controllers, assigns unique downstream request IDs across lines, records pending fills, can attach typed MSHRs, coalesces same-line read misses without extra downstream traffic, fans coalesced fills out as multiple target outcomes, and restores pending MSHR targets from snapshots. The CHI directory tracks unique clean/dirty ownership, shared clean/dirty sharers, deterministic SnoopShared and SnoopUnique decisions, owner-cache versus backing-memory data sources, MakeReadUnique no-data upgrades, dirty-peer data sourcing, and sorted snapshot restore. The CHI serial harness connects multiple cache controllers to the directory, applies directory snoops before fills, transfers owner-cache data to requesters, updates backing data when a unique dirty owner downgrades to shared clean, records CPU responses and directory decisions, and snapshots directory, cache, backing, response, and decision state. The CHI partitioned harness routes request and response work through `MemoryTransport` and `PartitionedScheduler`, waits for owner snoops before peer fills, preserves dirty owner data when downgrading to shared clean, records directory decisions and CPU responses with scheduler ticks, restores quiescent scheduler, directory, cache, backing, trace, response, and decision state, exposes recorded run summaries, source-target remote-flow records, and parallel run history, and can derive cache-directory and directory-DRAM routes from typed topology components including multi-hop and fabric-backed links. Workload replay and direct topology systems can select CHI as a typed data-cache protocol, route RISC-V data loads through the partitioned CHI harness, merge CHI cache resource activity and remote-flow records back into full-system run summaries, attribute recorded CHI data-cache runs into `RiscvSystemRun` and `WorkloadParallelExecutionSummary`, and require replay-verifiable data-cache protocol run counts for MSI, MESI, MOESI, and CHI workloads. Direct topology MSI, MESI, MOESI, and CHI data-cache responses share a protocol-neutral response harness for request submission, response extraction, run recording, and response-delay conversion while keeping per-protocol snoop invalidation and diagnostics explicit. LRU, FIFO, MRU, LFU, BRRIP, BIP, SHIP, SecondChance, and TreePLRU replacement policies have typed per-set state, victim decisions, invalidation, reset, touch, access-signature training, and snapshot restore. MSHR queues have typed entry allocation, target coalescing, prefetch reserve, ready/service state, snapshot restore, optional per-target QoS class metadata, effective QoS derived from merged targets, QoS-aware ready ordering, typed QoS profiles for target counts, effective-entry counts, requestors, and priorities, and typed conversion of effective cache QoS into downstream transport QoS classes. A typed cache write queue keeps gem5's writeback, write-clean, clean-evict, uncacheable-write, effective-capacity plus reserve, ready tick ordering, pending-conflict, functional-read satisfaction, direct mark-in-service release, and snapshot semantics while replacing Packet pointers, sender-state inheritance, and list iterators with stable handles and explicit request records. Replacement decisions can enqueue typed dirty writebacks, clean evicts, optional clean writebacks, or no entry for invalid victims, with explicit victim-way validation before the queue mutates. The MSI, MESI, MOESI, and CHI cache banks can own an optional typed write queue, enqueue writebacks or uncacheable writes, expose ready handles and conflict queries, issue ready entries as typed downstream requests, reject foreign line layouts, and restore queued entries through bank snapshots. MSI, MESI, MOESI, and CHI cache banks can accept CPU requests with explicit MSHR QoS metadata, expose effective QoS for pending lines, expose live and snapshot MSHR QoS profiles, coalesce same-line read misses without extra downstream traffic, preserve merged QoS through snapshot restore, and fan coalesced fills out as multiple target outcomes. The MSI bank directory harness can submit coalesced and parallel-cycle CPU requests with explicit MSHR QoS, record each target's effective MSHR QoS before fill service, export scheduled misses as typed downstream transport requests that carry effective MSHR QoS into `ParallelMemoryTransaction`, preserve MSHR queue configuration plus target QoS metadata in byte snapshots, aggregate pending cache-bank MSHR QoS profiles across harness snapshots and restored harness state, expose per-cycle effective MSHR QoS diagnostics on recorded parallel runs, and summarize effective QoS by requestor and priority in parallel-cycle history. Typed stride, tagged, DCPT, BOP, SBOOE, SignaturePath, SMS, FDP, PIF, ISB, STeMS, IMP, and initial AMPM access-map prefetchers have deterministic candidate generation, per-request metadata, source addresses, and snapshot restore. The DCPT model keeps gem5's per-PC delta history, signed delta overflow-to-zero rule, masked two-delta partial matching, earliest historical pair scan, and post-match delta replay while adding optional requestor isolation and explicit typed snapshots for parallel replay. The BOP model keeps gem5's best-offset learning over generated smooth offsets, left and right recent-request tables, score and round thresholds, enable-disable policy, optional delayed RR insertion, delay queue capacity/drop behavior, prefetch-fill training hook, and degree candidate generation while making the RR tables, scores, delay queue entries, selected offset, phase state, and last candidates explicit typed snapshot state. The SBOOE model keeps gem5's sandboxed sequential stride candidates, FIFO sandbox entries, score minus late-score policy, score threshold percentage, demand-fill latency tracking, and average latency feedback while making sandbox state, latency buffers, pending demand fills, selected sandbox, and last candidates explicit typed snapshot state. The SignaturePath model keeps gem5's page signature table, shifted xor signature update, pattern table stride counters, low-counter stride replacement with counter aging, confidence-gated prefetches, 0.95 lookahead cap, next-line auxiliary fallback, and page-crossing address generation while replacing opaque cache entries with explicit deterministic LRU and typed snapshots. The SMS model keeps gem5's filter table, active generation table, eviction-committed pattern history table, region offsets, FIFO filter capacity, active LRU capacity, pattern LRU capacity, and trigger-PC plus trigger-offset lookup while avoiding hidden map default insertion and exposing typed snapshots. The AMPM model keeps gem5's previous/current/next hot-zone window, positive and negative stride checks, `s2+1` early match rule, prefetch/useful/raw hit/miss counters, epoch degree adjustment, and candidate marking while making table replacement, integer threshold comparisons, epoch reports, and snapshots explicit typed state. A typed queued prefetch resource models gem5's queued prefetch latency, duplicate filtering with higher-priority duplicate updates, same-line demand squash, page-boundary dropping when no translation path is configured, in-cache or in-miss-queue redundant filtering, optional lowest-priority oldest eviction when full, next-ready-tick visibility, and accuracy throttle state with control percentage, issued/useful counters, max-permitted computation, useful-count invariant checks, and snapshot restore. The queued resource applies that throttle through an explicit enqueue path shared by typed prefetch candidates with ready-tick ordering, same-tick priority ordering, stable order ties, explicit capacity, line size, optional page size, issue width, accepted/duplicate/priority-update/redundant/page-crossing/throttled/full result counts, and full policy before packet creation or cache-controller side effects. A typed multi queued prefetcher preserves gem5's `Multi` earliest-ready query and round-robin source issue behavior while exposing source identity, keeping no-op polls side-effect free, and issuing only one entry from the chosen source. The FDP model keeps gem5's FTQ range expansion, PFQ and translation queue duplicate filtering, fetch-target squash policy, translation success/failure handling, uncacheable and cache-snoop drops, ready latency, issue ordering, queue counters, and snapshot restore while replacing raw CPU, MMU, cache, and packet pointers with explicit typed events. The PIF model keeps gem5's retired-PC training, spatial and temporal compactor records, history buffer, trigger index, stream address buffer continuation, secure-bit lookup behavior, and snapshot restore while replacing probe listeners, cache iterators, and replacement-policy callbacks with explicit typed events and stable history IDs. The ISB model keeps gem5's PC-indexed training unit, physical-to-structural and structural-to-physical address mapping caches, confidence counter update and reassignment policy, chunk-based structural address allocation, secure-bit separation, degree-limited successor prediction, deterministic LRU capacity, and snapshot restore while replacing AssociativeCache entries and raw queue output with typed records. The STeMS model keeps gem5's active generation table, pattern sequence table, region miss order buffer, trigger deltas, confidence-gated sequence reconstruction, duplicate RMOB policy, and cache-residency generation ending while replacing CacheAccessor callbacks and implicit replacement state with typed residency probes, deterministic LRU, secure-bit separation, snapshots, and line-sized reconstruction addresses. The IMP model keeps gem5's prefetch table, indirect-pattern detector, stream fallback, base-plus-index-shift matching, confidence counter, and secure-bit separation while replacing raw PT-entry pointer tags with stable typed keys, handling negative shifts through checked arithmetic, exposing explicit typed index-read events, deterministic LRU capacity, and snapshot restore. rem6-memory now has a typed translation queue with explicit request IDs, access kinds, bounded capacity, latency-derived ready ticks, deterministic ready ordering, duplicate detection, mapped or faulted completion records, snapshot restore, a typed page translation map with page-size validation, aligned virtual-to-physical mappings, overlap rejection, permission checks, page faults, explicit cross-page segment resolution, and snapshot restore, and a typed translation TLB with ASID-keyed deterministic LRU, permission rechecks, fault accounting, bounded inserts, scoped invalidation, evictions, cross-page segment fill into scoped page entries, and snapshot restore; rem6-cpu can bridge queued virtual fetch/load/store translation misses or TLB hits into typed mapped physical memory requests, issue a simple RISC-V data access through translated physical transport addresses on serial or parallel memory transport while preserving the virtual ISA access record, per-segment translated records, or fault records, while full RISC-V core/MMU pipeline integration, fuller cache/DRAM QoS policy integration, and richer cache tags remain open. |
| `src/mem/ruby` | `rem6-coherence`, `rem6-directory`, `rem6-fabric` | partial | rem6 keeps detailed coherence and NoC behavior without a second memory-stack vocabulary. |
| `src/mem/slicc` | protocol crates and typed transition records | partial | rem6 should preserve protocol expressiveness while avoiding generated controllers that hide transient behavior. |
| `src/mem/protocol` | `rem6-protocol-msi`, `rem6-protocol-mesi`, `rem6-protocol-moesi`, `rem6-protocol-chi` | partial | MSI, MESI, and MOESI exist. The CHI-like crate covers typed `I`, shared clean/dirty, unique clean/dirty, ReadShared, ReadUnique, MakeReadUnique upgrade, snoop downgrade, invalidation, busy rejection, transition trace, and directory unique-owner validation. Full CHI request, response, data, DVM, retry, credit, and Ruby-network interactions remain open. |
| `src/mem/qos` | `rem6-fabric`, `rem6-dram`, `rem6-transport`, `rem6-workload` | partial | rem6-fabric has typed QoS requestor IDs, checked priorities, fixed-priority assignment, FIFO/LIFO/LRG queue arbitration, non-mutating empty polls, queue-arbiter snapshots, and QoS-ordered fabric batch transmission that reserves shared links in grant order. rem6-transport can attach a shared QoS arbiter to parallel batch submission so request priority and requestor identity affect first-hop NoC reservation before partition events are scheduled, can order single- and multi-hop direct same-tick target deliveries with the same typed arbiter before invoking target handlers, respects same-agent memory-ordering barriers when direct QoS batches or shared-fabric first-hop reservations choose eligible requests, exposes a typed `TransportQosClass`, and lets cache-originated transactions override QoS requestor separately from the downstream request's cache-agent identity. rem6-coherence can now export MSI bank scheduled misses as typed downstream transport requests, preserving effective MSHR QoS through `TransportQosClass` so same-tick cache-originated memory requests can be batched and ordered by transport QoS without Packet sender-state inheritance. rem6-dram can order same-arrival timing batches through the same typed arbiter before bank, row, and bus timing are computed, filters same-agent acquire/release memory-ordering barriers before QoS priority or turnaround selection, prefers the current read/write bus direction among same-priority candidates, explicitly escalates queued same-requestor candidates to their best assigned batch priority without embedding controller back pointers in the queue policy, accepts memory-controller QoS batches before storage responses are generated, pairs responses with scheduled DRAM grant order, and preserves assigned priority, effective priority, requestor, byte count, and escalation status as typed DRAM activity metadata. Parallel coherence, system, DMA, and workload-result summaries expose DRAM QoS access, byte, escalation, priority, and requestor diagnostics directly from typed activity profiles. Workload manifests declare fixed-priority QoS policy, queue policy, turnaround policy, priority escalation, and per-requestor priority intent as typed replay-plan state; workload replay applies declared fixed-priority and queue policy to shared fabric first-hop reservation, applies declared fixed-priority, queue, turnaround, and escalation policy to direct profiled DRAM accesses so replay summaries carry DRAM priority and requestor metadata, lets same-tick single- and multi-hop direct DRAM deliveries observe manifest QoS before target handling, coalesces same-tick direct QoS deliveries to the same profiled DRAM target into one memory-controller batch, and keeps that batch path active when a data-cache exists by operation-filtering cache-covered data deliveries before batching the remaining DRAM requests. This preserves gem5's fixed-priority, queue-policy, turnaround, escalation, and bandwidth-accounting concepts while avoiding global requestor lookup, memory-controller back pointers, SimObject-name-only setup, and string-only stats. Broader cache/DRAM QoS policy integration remains open. |
| `src/mem/probes` | `rem6-stats`, runtime summaries | partial | Observability should be typed counters, typed probe points/listeners/events, and run summaries, not string-only probes. |
| memory ports, packets, requests in `src/mem` root | `rem6-transport`, `rem6-memory` | partial | Shared request/response transport exists; more gem5 packet semantics need mapping as features are added. |

### DRAM and External Memory

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `configs/dram`, `ext/drampower`, `ext/dramsim2`, `ext/dramsim3`, `ext/dramsys` | `rem6-dram`, adapter crates | partial | rem6 has internal DRAM timing, burst spacing, same-bank-group burst spacing, command-window bandwidth limits, bank-group geometry, activity, and profiles. DRAM snapshot restore rejects profile target, line-layout, geometry, timing, parallel-port, or NVM media-timing drift before rebuilt controller state can expose stale profile evidence. External DRAM simulators should be optional adapters. |
| `configs/nvm`, `src/mem/NVMInterface.py`, `src/mem/nvm_interface.*`, memory profile code | `rem6-memory`, `rem6-dram` | partial | NVM targets have typed controller/media-bank topology and can round-trip through manifests, checkpoints, and DRAM target activity metadata. DRAM activity profiles preserve typed read/write byte counts, and NVM target activity exposes persistent write access, byte counters, max pending NVM reads, max pending persistent writes, profile-level media timing, access-level persistent-ready cycles, checkpointed pending read/write completions, NVM read-buffer/write-queue wait-for diagnostics, manifest identity for NVM media timing, and restore-time rejection of profile media-timing drift without string stats. Richer NVM-specific bandwidth behavior remains open. |
| HBM, LPDDR, DDR class profiles | `rem6-dram` | partial | The profile shape exists for DDR, HBM, LPDDR, and NVM, and checkpoint restore validates that profile metadata still matches the target, memory layout, controller geometry, timing, and parallel-port shape. A broader library of validated profiles is still needed. |

### Heterogeneous Devices

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/gpu-compute` | `rem6-gpu` | partial | rem6 has GPU command submission, workgroup completion, DMA, traces, summaries, checkpoints with typed pending-DMA request metadata, DMA write requests that inherit copy read-request ordering at a coarse level, and workload-result GPU DMA scheduler batch, exact timeline, initial/final frontier, remote-send, remote-flow, worker-count, max-worker, total-worker, worker-tick, partition-set, same-partition-set streak, and per-partition activity evidence captured from recorded read/write scheduler runs, exposed through dedicated batch-timeline, batch-worker, batch-partition, and scheduler-frontier manifest scopes including direct max-worker, total-worker, and batch-activity contracts, and included in full-system scheduler, remote-traffic, and partition aggregates. |
| `src/dev/amdgpu`, `src/dev/hsa` | `rem6-gpu`, future GPU ISA and runtime modules | planned | Full GPU system support needs richer queues, address spaces, interrupts, and ISA-visible state. |
| NPU-style accelerators, not a single gem5 subtree | `rem6-accelerator` | partial | rem6 already models accelerator engines, command lanes, NPU inference commands, DMA, summaries, checkpoints with typed pending-DMA request metadata, DMA write requests that inherit copy read-request ordering, and workload-result accelerator command/completion kind counts for GPU-kernel, NPU-inference, and DMA-command work. Accelerator DMA scheduler batch, exact timeline, initial/final frontier, remote-send, remote-flow, worker-count, max-worker, total-worker, worker-tick, partition-set, same-partition-set streak, and per-partition activity evidence is captured from recorded read/write scheduler runs, exposed through dedicated batch-timeline, batch-worker, batch-partition, and scheduler-frontier manifest scopes including direct max-worker, total-worker, and batch-activity contracts, and included in full-system scheduler, remote-traffic, and partition aggregates. |
| `src/dev/pci`, `src/dev/virtio`, `src/dev/storage`, `src/dev/net` | `rem6-pci`, `rem6-virtio`, `rem6-net`, future device crates | partial | rem6-pci has an initial typed PCI endpoint and host config-space model: function addresses validate bus/device/function ownership, type-0 identity and class header fields are little-endian bytes, common command writes mask reserved bits through shared endpoint and bridge helpers, common cache-line-size, latency-timer, and BIST writes update typed endpoint and bridge config bytes with snapshot restore while rejecting writes that would cross into read-only header fields, common status writes use typed write-one-to-clear semantics while preserving endpoint capability-list state, type-0 Cardbus CIS, subsystem IDs, Expansion ROM, minimum-grant, and maximum-latency fields are explicit typed header data, Expansion ROM writes preserve gem5's 32-bit update and size-probe behavior, BAR sizing writes apply typed memory or I/O masks, 64-bit memory BARs consume checked adjacent config slots and combine lower plus upper dwords into one logical range, fixed legacy I/O BARs preserve a declared address while ignoring config BAR writes, BAR types and host BAR address-space mapping live in a dedicated module instead of the endpoint monolith, command bits gate exposed BAR ranges, an endpoint-owned PCI capability-list registry links multiple installed capabilities in order, installs read-only raw capability byte blocks for device-specific vendor extensions, rejects overlapping regions before mutation, and keeps next-pointer bytes stable while PM, PCIe, MSI, MSI-X, or raw capability control fields are written, PM capabilities expose typed config-space headers, capability words, writable PMCSR state, and snapshot restore, PCIe capabilities expose typed config-space headers, device/link/slot/root/capability2 capability fields, writable control/status fields, snapshot restore, and typed read-only versus width errors instead of gem5's raw PXCAP byte-copy writes, MSI capabilities expose typed config-space headers, preserve clamped vector enable state, mask vectors, restore snapshots, derive typed message address/data pairs, and deliver enabled serial or parallel MSI assertions through rem6-interrupt without direct platform mutation, MSI-X capabilities expose typed config-space table and PBA registers, own BAR-local table/PBA state, mask vectors and functions explicitly, preserve table plus pending bits across snapshots, and deliver enabled serial or parallel MSI-X assertions while recording masked parallel sends as typed pending bits, Type-1 bridge configs expose typed PCI-to-PCI headers, BAR0/BAR1, Expansion ROM, interrupt line/pin, and bridge-control fields, preserve gem5's type-1 BAR update path plus Expansion ROM update and size-probe behavior, validate primary/secondary/subordinate bus ranges, route subordinate config accesses only through declared bridge ranges, filter downstream active BAR host mappings through memory, prefetchable-memory, and I/O windows, and snapshot/restore bridge config plus BAR state with function/identity/class and BAR-shape checks, read-only writes return typed errors, snapshots restore endpoint config and BAR state with function/identity/class checks, host apertures implement CAM-sized and ECAM-sized per-function config slots, host reads route by decoded physical config addresses, missing functions return all-ones reads for guest enumeration, duplicate or out-of-aperture registration is rejected explicitly, active endpoint and bridge BARs map through typed IO, non-prefetchable memory, and prefetchable memory host bases, overlapping host BAR ranges are rejected before system topology consumes them, the config aperture can be attached to the typed MMIO bus with serial or parallel scheduler responses, masked config MMIO writes split byte-enabled runs into typed config writes instead of widening into read-only neighboring bytes, active BAR host ranges can forward serial or parallel runtime MMIO requests into BAR-local device offsets with typed boundary errors plus optional live bridge-window revalidation before each access, and typed PCI legacy INTx direct mapping, explicit platform routing tables, bridge swizzle paths, and endpoint-facing post/clear ports can deliver serial or parallel interrupt assertions through rem6-interrupt without direct platform mutation. rem6-virtio models a modern VirtIO PCI common-config MMIO device with feature-page selection, driver-feature pages, queue selection, queue sizing, queue-enable, queue descriptor/driver/device addresses, device-status reset, typed snapshots, and typed read-only plus invalid queue errors instead of gem5's legacy-only BAR0 header assertions; it also models a modern PCI notify-MMIO device that derives per-queue notify addresses from queue notify offsets and notify-off multipliers, records serial or parallel queue notifications with scheduler ticks, snapshots notification history, and rejects invalid notify layouts or mismatched writes before backend mutation; it models a modern PCI ISR-status MMIO device with separate queue-interrupt and configuration-change bits, serial or parallel read-clear events, snapshot restore, reserved-bit masking, and typed write or width errors instead of gem5's single pending bool plus direct `intrClear` side effect; it models a modern PCI device-specific configuration MMIO container with typed byte mutability, serial or parallel read/write access traces, byte-mask writes, snapshot restore, and typed boundary or read-only-byte errors instead of forwarding raw BAR0 offsets into backend panic paths; it emits standard VirtIO vendor-specific PCI capability bytes for regular structures and notify structures with typed cfg_type, BAR, id, offset, length, cap_len, cap_next, and notify_off_multiplier fields, and converts those bytes into rem6-pci raw capability specs that install into endpoint config space; it now has a typed modern VirtIO PCI transport spec that builds endpoint identity, BAR declarations, common/notify/ISR/device/shared-memory capability chains, binds common/notify/ISR/device-config MMIO devices into BAR-local runtime routers with serial and parallel dispatch, and pre-mutates nothing until missing BARs, out-of-BAR regions, same-BAR overlaps, undersized runtime regions, missing device-config devices, and PCI endpoint shape errors are rejected; it models modern PCI shared-memory capabilities as typed cap64 region descriptors and vendor-specific capability bytes with unique ids, declared BAR containment, 64-bit offset and length splitting, cap-next chaining, config-image export, entry lookup by region id or capability offset, rem6-pci raw capability installation, configuration-space placement validation, and same-BAR overlap rejection before topology consumes the ranges; it emits typed modern VirtIO block device configuration bytes, feature-page records, and writeback mutability masks for capacity, size, segment, geometry, block-size, topology, multiqueue, discard, write-zeroes, and secure-erase fields with shape validation before device-config installation; it executes decoded VirtIO block read, write, flush, get-id, and unsupported requests against a typed 512-byte-sector memory backend through serial and parallel scheduler contexts while recording queue/request/tick/status completions and returning guest-visible error statuses for read-only or out-of-range accesses instead of panic paths; it decodes direct and indirect split virtqueue descriptor chains for VirtIO block into typed requests plus status/data completion metadata while rejecting descriptor loops, missing status descriptors, short headers, and wrong readable/writable descriptor directions before backend execution; it records typed split used-ring completion writeback plans with scatter data writes, status-byte writes, used-ring slot selection, little-endian used elements, and wrapping used indices; it can consume split available-ring block requests from typed guest memory by walking queue descriptor, driver, and buffer addresses through rem6-memory instead of prebuilt host-only descriptor lists; it can snapshot/restore split queue cursor and event-index state through typed queue records and rem6-system checkpoint banks; and it can write block completion data buffers, status bytes, used elements, and used indices back into typed guest memory before raising typed VirtIO PCI ISR queue-interrupt status at the completion tick and posting serial or parallel PCI legacy INTx through rem6-interrupt, sending serial or parallel PCI MSI through the configured typed endpoint and MSI port, or sending serial or parallel PCI MSI-X through the configured typed endpoint and MSI-X port, and returning a typed interrupt-delivery outcome so masked MSI or MSI-X completions remain distinguishable from delivered interrupt events. rem6-net owns typed Ethernet packet payloads, separate wire-length timing metadata, typed Ethernet interface peer binding and event records, typed Ethernet tap stub framing, deterministic retry queues, distributed Ethernet message headers, sync command records, endian-stable payload envelopes, receive scheduling, sync-window checks, missed-packet detection, packet FIFO capacity and reservation state, explicit non-front removal slack, copyout, typed PCAP capture records and byte-image export, fixed full-duplex link serialization and delivery timing, deterministic delay variation, typed shared-bus broadcast timing, direction-local busy state, deterministic ready-delivery drain, typed Ethernet MAC addresses, learning-switch forwarding with TTL expiry, multicast or unknown-destination flood decisions, output queue capacity tail drops, output FIFO timing, ready-output records, typed SINIC registers, FIFO state, RX/TX checksum offload, descriptor-memory DMA, MMIO, PCI BAR binding, and scheduled PCI legacy INTx bridging, and snapshot/restore evidence comparable to gem5 `EthPacketData`, `EtherInt`, `EtherTapBase`, `EtherTapStub`, `DistHeaderPkt`, `DistIface`, `DistIface::RecvScheduler`, `TCPIface`, `PacketFifo`, `EtherDump`, `EtherLink`, `EtherBus`, and `EtherSwitch` while replacing pointer ownership, event callback queues, implicit RNG, raw interface pointers, fd polling side effects, raw C++ header layouts, receiver-thread scheduling side effects, and assert paths with typed errors and explicit records. Broader platform interrupt-controller models remain required for full-system breadth. |
| `src/dev/serial`, `src/dev/riscv`, `src/dev/lupio`, `src/dev/i2c` | `rem6-uart`, `rem6-mmio`, `rem6-interrupt`, `rem6-timer`, future device crates | partial | UART, timer, MMIO, interrupts, and an initial RISC-V CLINT model exist. The CLINT path keeps gem5's `msip`, `mtimecmp`, and read-only `mtime` MMIO layout while replacing direct `System::threads` interrupt mutation with typed interrupt ports and scheduler events, including parallel scheduling. CLINT register, timer-assertion, and RTC-driven `mtime` state can be captured and restored through typed snapshots and a system checkpoint bank, platform declarations now attach CLINT MMIO plus host checkpoints automatically, and reset is explicit through `ClintResetPolicy`: `msip` is cleared, asserted software and timer lines are typed deasserted, stale timer events are invalidated, `mtimecmp` is either preserved or reset to a declared value, and RTC-backed `mtime` resets as explicit device state. The default CLINT timebase remains scheduler ticks for compatibility, while `ClintTimebase::RtcDriven` plus `RiscvRtcSource` models gem5's RTC pulse into CLINT `mtime` without hiding the dependency in global time. Platform declarations can now emit typed RISC-V DTS source nodes and deterministic binary FDT/DTB blobs for CPUs, CPU local interrupt controllers, a `soc` simple bus, CLINT `interrupts-extended`, a generic external interrupt controller, UART interrupt-parent wiring, and Linux `/chosen` bootargs plus `linux,initrd-start` and `linux,initrd-end` metadata without Python object recursion or libfdt mutation. System topology can install the generated RISC-V DTB into store-backed or DRAM-backed guest memory and set each hart's A1 register to the DTB address, replacing gem5's external DTB filename side effect with a typed handoff. Richer wall-clock/BCD RTC behavior and other platform devices remain open. |
| platform-specific device trees under `src/dev/arm`, `src/dev/x86`, `src/dev/mips`, `src/dev/sparc` | future platform crates | planned | These should arrive with the corresponding ISA and platform support. |

### Simulation Kernel, Checkpointing, and Host Control

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| event queue and tick logic in `src/sim` | `rem6-kernel` | covered | Partitioned scheduling, conservative epochs, deterministic order, lookahead, target-clock prevalidation for cross-partition parallel remote messages, scheduler snapshots, recorded initial and final epoch frontiers carried through CPU, data-cache/coherence, full-system, and workload-result summaries with per-partition conservative full-system frontier aggregation, worker-local remote outboxes, persistent per-partition remote and progress-transition order cursors preserved by scheduler snapshots and scheduler checkpoint chunks, batch, epoch, and conservative-run remote-send records ordered by the same deterministic delivery merge key used for cross-partition event insertion, source-target remote-flow records carried through RISC-V cluster, full-system, and workload-result summaries with cross-epoch min/max delay bounds preserved, scheduler epoch, empty-epoch, and dispatch counts, planned and executed kernel batch records with direct start tick, horizon, duration tick, worker-tick occupancy, duration-weighted worker-count bucket, worker-capacity ticks, idle-worker ticks, utilization ratios, per-worker-slot planned and recorded active/idle tick summaries, and worker-tick query accessors, recorded scheduler epoch and conservative-run exact batch worker-count summaries, duration-weighted worker-count tick summaries, exact worker-count batch and tick queries, minimum-worker batch, tick, and batch worker-tick queries, total batch worker-tick queries, and CPU scheduler, data-cache scheduler, and merged full-system progress-free transition records, deterministic dimension lists, per-dimension record slices, counts, tick windows, compact summaries by transition kind, partition, and subject, direct `RiscvSystemRun` and workload-result batch timeline records, GPU DMA and accelerator DMA workload-result batch timeline records, remote-send records, remote-flow evidence, worker-count summaries, duration-weighted worker-count tick summaries, exact worker-count batch and tick queries, planned and recorded worker-capacity ticks, idle-worker ticks, utilization ratios, per-worker-slot recorded active/idle tick summaries, minimum-worker duration-weighted tick queries, minimum-worker batch worker-tick queries, longest minimum-worker tick-streak queries, exact partition-set summaries, same-partition-set streak summaries, per-set counts, exact worker-count bucket counts, and minimum-worker batch counts with full-system batch sequences ordered by worker start tick rather than subsystem concatenation, and declared-threshold livelock diagnostic records, subject queries, subject summaries, subject tick windows, transition-kind queries, transition-kind counts, kind summaries, kind-window summaries, and diagnostic counts exposed on `RiscvSystemRun` and carried into workload summaries, recorded worker-capacity ticks, idle-worker ticks, utilization ratios, and per-worker-slot active/idle summaries retained in workload-result artifacts, batch worker-count histograms, exact batch partition-set histograms, and explicit merged full-system maximum consecutive partition-set streaks carried into workload summaries, manifest-owned and multiset-verifiable expected remote-send, progress-transition, and exact batch timeline records, remote-flow counts, first/last tick windows, delay bounds derivable from exact remote-send records when aggregate flow records are absent or weaker, and scope-wide delay floors and ceilings, batch count summaries derived from the strongest available aggregate, batch-worker, exact partition-set, or streak evidence, minimum scheduler progress contracts backed by aggregate epoch or dispatch counts, batch-worker histograms, exact partition-set histograms, streaks, or worker-owned per-partition dispatch activity, maximum scheduler idle contracts, minimum max-worker contracts backed by aggregate counts, batch-worker histograms, exact partition-set histograms, or streaks, total-worker activity contracts backed by aggregate counts, batch-worker histograms, exact partition-set histograms, streaks, or multi-worker per-partition worker activity, multi-worker batch activity contracts backed by batch-worker histograms, exact partition-set histograms, or streaks, minimum-worker duration-weighted tick activity contracts, minimum thresholded batch worker-tick contracts, and sustained minimum-worker tick-streak contracts backed by exact batch timelines, exact batch partition-set contracts backed by exact histograms or streak counts, sustained same-batch partition-set streak contracts, active-partition contracts backed by aggregate counts, exact batch partition-set unions, streak partition-set unions, activity-derived partition unions, remote-send endpoints, or remote-flow source/target unions, typed work flags backed by partition, frontier, and DMA remote-traffic evidence, per-partition activity contracts backed by explicit activity, exact batch partition-set histograms, streaks, remote-send records, or remote-flow records with same-scope activity projections merged as lower-bound evidence instead of added, per-partition initial/final frontier contracts, clean diagnostic contracts, source and target partition counts in recorded parallel summaries, and typed parallel-worker failure reporting that preserves remaining partition events, keeps executed-time visibility, commits successful callbacks' remote messages, and rolls back local and remote events scheduled by the panicked callback exist. |
| SimObject and Python configuration in `src/sim` and `src/python` | `rem6-platform`, `rem6-workload` | partial | rem6 should keep ease of composition through typed builders and manifests rather than dynamic object graphs. |
| checkpoint support in `src/sim` | `rem6-checkpoint`, `rem6-system` checkpoint banks | partial | Protocol-neutral checkpoint records exist for several subsystems. Scheduler checkpoint capture rejects non-quiescent state with typed quiescence reports for pending-event counts, tick windows, and serial/parallel pending-event kind counts by component and partition, scheduler checkpoint banks preflight quiescence before writing any scheduler chunks, system checkpoint host actions run scheduler quiescence preflight before capturing any attached bank, full-system checkpoint capture stages registry writes until final manifest capture succeeds, execution-mode restore pre-registers internal host checkpoint components only in staged registries, manifest restore clears chunks for registered components absent from the manifest so stale component state cannot satisfy attached-bank validation, checkpoint manifests expose typed component, chunk, and payload-byte summaries for audit, workload replay results preserve those manifest totals and per-component counts for checkpoint and restore outcomes, and workload manifests can require minimum checkpoint coverage totals and named-component coverage during replay verification. More devices and broader pending-state restore coverage remain open. |
| statistics, probes, and power hooks | `rem6-stats`, `rem6-power`, run summaries | partial | Counters, stats snapshots, typed stats dump records, schema-and-reset-scope-checked stats delta records, typed probe registries with checked component, point, and listener identifiers, probe listener state, probe event snapshots with historical listener refs plus cursor-preserving and time-monotonic restore validation, typed power states/domains, power residency snapshots, typed state-weighted dynamic/static power models, typed expression-based dynamic/static power models, typed stat-snapshot metric binding, power metric binding from core stats deltas, typed RC thermal domains, and typed multi-domain thermal-network solving with resistor and capacitor edges exist. Broader power-controller and external-analysis adapter breadth remains open. |
| guest-host events and pseudo instructions | `rem6-system`, `rem6-workload` | partial | ROI, stats, checkpoint, checkpoint restore, stop, execution mode actions, custom guest-host calls, and manifest-declared guest-host response payloads are typed. Broader guest ABI support remains open. |

### External Integration and Tooling

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `ext/systemc`, `src/systemc`, `util/systemc`, `util/tlm` | future adapter crates | external-adapter | Interoperability is useful, but rem6 timing authority must stay in the partitioned runtime. |
| `ext/sst`, `src/sst`, `configs/example/sst` | future SST adapter | external-adapter | SST co-simulation should be explicit and checkpoint-aware. |
| `ext/nomali`, `ext/mcpat`, `ext/dsent` | future optional analysis adapters | external-adapter | Preserve modeling value behind typed records and stable APIs. |
| `ext/libelf`, `ext/libfdt`, `ext/softfloat`, `ext/gdbremote` | `rem6-boot`, `rem6-platform`, future debug and ISA support crates | partial | rem6-boot has typed ELF32 and ELF64 little-endian and big-endian loaders plus an auto-detecting ELF entry point that validates the ELF identity and program-header shape, records typed ELF class, endian, machine, architecture, OS ABI, flags, and operating-system metadata comparable to gem5 `ElfObject::determineArch()` and `ElfObject::determineOpSys()`, including PPC64 ABI v1/v2 selection from ELF flags, endian-sensitive PPC64 default ABI selection when flags omit the ABI, Linux/Solaris/FreeBSD fallback from `.note.ABI-tag`, and Solaris fallback from `.SUNW_version` or `.stab.index` section names, imports physical-address `PT_LOAD` segments into `BootImage`, skips non-loadable segments, preserves `p_memsz > p_filesz` zero-fill, and rejects unsupported class, encoding, version, or overflowing segment memory ranges with typed errors without libelf or unsafe parser state. Its source boundary keeps ELF parsing, boot-image loading, and typed boot errors in focused modules so loader breadth does not grow into a gem5-style untyped utility layer. Workload boot images preserve that ELF metadata through manifest round trips and include it in manifest identity, so ISA or ABI selection cannot silently drift when two images have the same entry and loadable segments but different ELF endian, machine, OS ABI, or flags headers. rem6-platform has an initial typed DTS source tree and deterministic binary FDT/DTB writer for RISC-V platform boot descriptions, including Linux `/chosen` bootargs and initrd start/end metadata, and rem6-system installs initrd bytes plus generated or resolved-resource DTBs into guest memory with the RISC-V A1 register handoff for both store-backed and DRAM-backed memory. rem6 still needs broader ISA loader coverage, kernel image loader breadth, bootloader handoff coverage, soft-float, and debug capability without vendoring unneeded code into the core. |
| `util/gem5art`, resources tooling, disk image tooling | `rem6-workload`, future artifact tooling | partial | rem6 manifests make artifact provenance first-class and reproducible for boot images, declared resources, typed Linux device-tree resources, and typed Linux initrd handoff resources. Resource declarations can now carry typed acquisition provenance with kind, acquisition locator, tool, and revision, and manifest identity hashing includes that provenance so acquisition state cannot silently drift from the workload contract. Disk-image resources can also carry typed construction records with image format, virtual size, tool, operation, input, and arguments; these records are disk-image-only and identity-hashed. Resolved payloads are explicit caller-provided data with manifest identity, id, digest, kind validation, and initrd handoff-size validation; workload replay rejects payload sets resolved for a different manifest, and no hidden download path is part of replay authority. Future tooling still needs richer artifact acquisition execution and construction executors for additional artifact kinds. |
| `tests`, `tests/test-progs`, `util/statetrace` | rem6 tests and trace tooling | partial | gem5 tests are audit input. rem6 acceptance remains Rust tests and typed trace comparison. |

## Evidence Already Present in rem6

- Partitioned scheduler tests cover deterministic event order, lookahead,
  conservative parallel epochs, worker limits, wait-for graphs, and scheduler
  snapshots, including prevalidated scheduler checkpoint-bank restore so
  truncated chunks cannot partially mutate live scheduler frontiers. Wait-for
  graph tests cover checkpoint barrier nodes, barrier wait edges, repeated
  observations, resource-scoped barrier release, kernel-owned edge-kind
  observation windows, kernel-owned blocked-node observation windows, and
  kernel-owned target-node observation windows. Full-system tests preserve the
  barrier edge kind in wait-for summaries and expose target-node windows across
  merged resource and full-system wait-for graphs. Workload-result tests now
  preserve scoped blocked-node and target-node wait-for tick windows, and
  workload replay summary tests carry fabric/data-cache blocked-node and
  target-node windows into merged full-system artifacts. Workload manifest tests
  now bind scoped blocked-node and target-node windows into manifest identity
  and replay
  verification, including missing, mismatched, invalid, and duplicate
  expectation failures.
  Progress-monitor tests cover typed livelock diagnostics for
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
  masked writes, and memory checkpoint-bank tests cover prevalidated
  multi-store and DRAM memory restore so truncated payloads cannot partially
  mutate live memory state. Fabric, coherence, and RISC-V checkpoint-bank tests
  cover decode-first multi-bank and multi-core restore so malformed chunks
  cannot partially rewind another live NoC lane frontier, cache bank,
  architectural PC, or integer register file. Heterogeneous checkpoint-bank
  tests cover decode-first accelerator and GPU restore so a malformed later
  device chunk cannot partially restore an earlier live device. Peripheral
  checkpoint-bank tests cover CLINT, UART, interrupt-controller, and timer
  decode-first restore so malformed later device chunks cannot partially
  restore earlier live platform devices. System action tests cover staged
  manifest restore plus cross-bank prevalidation so a later bad subsystem chunk
  cannot partially restore an earlier CPU or device bank. RISC-V
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
  metadata plus bank-level prevalidation before live device state is mutated.
- CPU branch prediction exposes typed direction prediction, GShare PC-history
  indexing with stale-history rejection, BiMode choice and direction-array
  training with stale-history rejection, Tournament local/global/choice
  training with stale-history rejection, loop trip-count learning, TAGE
  folded-history stale-history rejection, LTAGE loop override integration
  with nested stale-history prevalidation, TAGE-SC-L nested stale-history
  prevalidation, statistical-corrector GEHL override, branch-target lookup,
  indirect-target lookup,
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
  command-window bandwidth limits across row and data commands, plus
  target-local unique active port and bank coverage when DRAM activity windows
  are merged. DRAM profile tests now expose target-sorted parallel resource
  summaries for DDR, HBM, LPDDR, and NVM profiles, including port,
  topology-unit, scheduler-bank, topology-bank, and bank-group capacity, and
  runtime activity profiles preserve profiled-target capacity denominators
  across marker windows even when no target is active. NVM profile tests
  cover typed read/write byte accounting, persistent write counters, NVM media
  timing, pending-read buffer limits, pending-write queue limits, checkpoint
  round-trip of media/pending state, NVM queue wait-for diagnostics, profile
  snapshot drift rejection across target, layout, geometry, timing, parallel
  ports, and NVM media timing, and manifest identity changes for media timing.
  Checkpoint and workload identity tests cover command-window timing, bank-group
  timing, and per-port command history state. Coherence, system, DMA, and
  workload-result summary tests cover direct DRAM QoS diagnostics over those
  typed activity profiles, plus workload-level CPU scheduler, data-cache
  scheduler, merged full-system remote-flow records, scheduler epoch,
  empty-epoch, dispatch counts, exact progress-free transition records, total
  counts, deterministic dimension lists, per-dimension record slices, counts, tick windows, and compact summaries by kind, partition, and subject, system-run and workload-level declared-threshold livelock diagnostic records, subject queries, subject summaries, kind summaries with exact kind tick windows, kind-filtered subject and record queries, diagnostic tick windows, and counts, merged resource and
  full-system deadlock diagnostic counts, batch counts derived from the
  strongest available aggregate, worker-count, exact partition-set, or streak evidence,
  subsystem and full-system wait-for edge-kind counts plus first/last kind
  tick windows carried from system, compute, and DMA wait-for graphs into
  workload summaries,
  exact worker-count bucket counts derived from the strongest available
  worker-count, exact partition-set, or streak evidence, duration-weighted
  worker-count tick bucket counts derived from exact batch timeline evidence,
  manifest-verifiable exact worker-count bucket contracts,
  manifest-verifiable duration-weighted worker-count tick bucket contracts,
  manifest-verifiable minimum-worker duration-weighted tick activity contracts,
  manifest-verifiable sustained minimum-worker tick streak contracts,
  manifest-verifiable minimum thresholded batch worker-tick contracts,
  total-worker counts derived from the
  strongest available aggregate, batch-histogram, exact partition-set, streak, or per-partition worker evidence, exact batch
  partition-set histograms, maximum
  consecutive partition-set streaks, per-partition activity summaries,
  replay-plan
  validation of exact expected remote-flow actual sets and first/last tick windows,
  remote endpoint source/target partition fan-in and fan-out summaries plus
  exact manifest-owned endpoint-set expectations with disjoint source/target
  partition-set validation, manifest-verifiable remote
  delay-floor and delay-ceiling expectations, aggregate-flow to exact-send consistency expectations,
  direct GPU and accelerator DMA scheduler remote-send, remote-flow, timing,
  endpoint, delay-bound, and consistency contracts plus full-system remote
  contracts that include GPU and accelerator DMA scheduler evidence,
  minimum CPU/cache/full-system scheduler epoch progress plus dispatch progress
  from the strongest available aggregate counts, batch-histogram, exact
  partition-set, or per-partition evidence, direct GPU and accelerator DMA
  scheduler epoch and dispatch progress, direct GPU and accelerator DMA
  scheduler max-worker, total-worker activity, batch-activity, frontier, and idle-bound contracts,
  maximum scheduler idle epochs,
  minimum max-worker use derived from the strongest available aggregate,
  worker-count, exact partition-set, or streak evidence, minimum total-worker activity
  derived from the strongest available aggregate, worker-count, exact
  partition-set, streak, or multi-worker per-partition evidence, minimum multi-worker batch activity
  derived from the strongest available worker-count, exact partition-set, or streak
  evidence with explicit full-system threshold totals checked against scoped
  lower bounds, minimum-worker duration-weighted tick and worker-tick activity
  with explicit full-system summaries checked against scoped lower bounds,
  exact batch partition-set activity derived from exact histograms or streak counts
  with explicit full-system summaries checked against scoped lower bounds,
  sustained same-batch partition-set streak activity, minimum active partition
  counts derived from aggregate, exact partition-set, streak, activity,
  dedicated GPU or accelerator DMA timeline scopes, remote-send, or
  remote-flow evidence, GPU and accelerator compute or DMA activity presence
  derived from active device counts as well as command, copy, completion,
  scheduler, frontier, and remote-traffic evidence, remote-flow timing derived from exact
  remote-send records when aggregate flow records are absent or weaker, per-partition
  activity minima backed by explicit activity, exact batch partition-set
  histograms, same-partition-set streaks, dedicated GPU or accelerator DMA
  timeline scopes, remote-send records, or remote-flow records with same-scope
  activity projections merged as lower-bound evidence, data-cache run attribution
  expectations, data-cache run-accounting consistency, data-cache protocol
  run-count expectations, minimum fabric/DRAM/resource activity expectations,
  clean parallel diagnostic expectations including livelock counts, scoped
  wait-for edge-kind count expectations, scoped wait-for edge-kind window
  expectations, scoped wait-for blocked-node and target-node window
  expectations, stats-history reset/dump count, tick-window, and exact
  event-sequence expectations, and
  manifest identity changes for those expected communication contracts.
  Workload replay QoS tests cover same-tick DRAM
  batching while a data-cache is present, including operation filtering so
  instruction fetches are not misclassified as cache-covered data traffic.
- Stats tests cover registry-owned stat groups, self-describing group catalogs
  on snapshots, dumps, and deltas, structured counter scope/name identity, path
  grammar, structured unit and rate grammar, checked counter descriptions,
  counter reset epochs, typed dump history, schema-and-reset-scope-checked
  snapshot deltas, and typed probe point, listener, historical listener-ref,
  event, payload, identifier-grammar, cursor-preserving, and time-monotonic
  snapshot restore records with malformed snapshot rejection.
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
- PCI tests cover typed legacy INTx line mapping, invalid interrupt-pin and
  zero-line rejection, bridge path swizzling across upstream functions,
  explicit root routing-table entries with fallback policies, deterministic
  routing-table ordering, snapshot/restore of platform routing tables, typed
  endpoint config accessors for legacy interrupt line/pin fields, router
  construction of endpoint-facing ports from endpoint config or bridge-swizzled
  paths, host-bridge derivation of nested bridge paths from type-1 bus ranges,
  typed assignment of resolved routing lines back into endpoint config space,
  idempotent interrupt-controller route registration, endpoint identity
  preservation after root-line mapping, serial and parallel endpoint post/clear
  delivery through rem6-interrupt, and observable delivery errors when a
  parallel clear targets a mismatched source. PCI config and host tests cover
  64-bit memory BAR lower
  and upper config dwords, upper-slot reservation, invalid BAR pairing, one
  active logical range per 64-bit BAR pair, and host memory-space mapping of
  full 64-bit PCI BAR bases. Legacy I/O BAR tests cover fixed-address ranges,
  ignored config BAR writes, command-bit gating, and Type-1 I/O-window filtering
  of downstream host mappings. Type-0 header tests cover Cardbus CIS,
  subsystem IDs, common command writes with reserved-bit masking, common
  cache-line-size, latency-timer, and BIST byte writes, status
  write-one-to-clear behavior that preserves capability-list state, Expansion
  ROM reads and writes, Expansion ROM size probing, and minimum-grant plus
  maximum-latency read-only byte behavior. PCI config MMIO tests cover masked
  writes with empty masks, command-halfword byte enables, and non-contiguous
  byte enables that update latency-timer and BIST without widening into
  read-only header bytes, while preserving invalid-width errors for original
  config accesses wider than the typed PCI config model accepts.
  Capability-list tests cover ordered PM plus PCIe plus MSI chaining, MSI plus
  MSI-X chaining, next-pointer preservation across capability control writes,
  raw read-only vendor-specific capability installation, raw capability
  next-pointer ownership, raw snapshot shape checks, raw capability invalid
  offset and size rejection, PMCSR snapshot restore, PCIe
  device/link/slot/root/capability2 control/status snapshot restore, PCIe
  read-only device and slot capability rejection, and overlap rejection without
  mutating existing config bytes. MSI tests cover capability header exposure,
  clamped vector enable state, message-address masking, vector masks, snapshot
  restore, duplicate and invalid capability layouts, and serial plus parallel
  MSI delivery through typed rem6-interrupt routes. MSI-X tests cover typed
  capability table and PBA register exposure, BAR-local table programming,
  vector and function masks, table plus pending-bit snapshot restore, invalid
  and overlapping layout rejection, serial delivery, and masked parallel
  delivery recording into the PBA. PCI source-policy tests keep the crate root
  below the facade budget and all source files below the hard module-size
  budget. Type-1 bridge tests cover typed bridge
  header fields, Expansion ROM reads and writes, Expansion ROM size probing,
  interrupt line/pin bytes, bridge-control writes, common command writes with
  reserved-bit masking, common cache-line-size, latency-timer, and BIST byte
  writes, common status writes that do not create guest-owned bits, bridge
  BAR0/BAR1 install and command-bit-gated host mapping, snapshot restore of
  bridge config and BAR state, writable bus-number and memory-window registers,
  subordinate config routing, and downstream BAR host-range filtering through
  bridge windows. PCI host bridge tests also cover topology-level snapshot
  restore across registered type-1 bridges, downstream endpoints, bridge
  forwarding windows, endpoint BAR mappings, and derived legacy INTx line state,
  with mismatched host topology rejected before live state is replaced. The PCI
  host aperture, config-address decoder, host BAR range mapper, bridge-window
  forwarding, INTx path derivation, and host snapshot/restore live in a focused
  host module, leaving the crate root as a facade instead of letting PCI
  topology code accumulate beside endpoint and capability code. PCI host
  topology snapshots now have a stable byte codec for checkpoint preflight:
  encoded payloads preserve aperture shape, host address bases, sorted bridge
  and endpoint functions, reject malformed or out-of-aperture functions, and
  can be compared against a live host before a broader checkpoint restore
  mutates device state. `rem6-system` now exposes a PCI host checkpoint bank
  that captures this topology payload into host checkpoint manifests and
  prevalidates restore attempts against the live host topology. The bank also
  decodes manifest payloads back into typed audit records in deterministic
  component order, and `SystemActionExecutor` can be constructed directly with
  an attached PCI host checkpoint bank. Full PCI configuration checkpoint bytes
  remain dependent on per-capability state codecs; rem6 does not pretend that
  topology-only payloads restore full PCI configuration state. PM capability
  state now has the first exposed byte codec: endpoint snapshots can emit a PM
  payload and validate a candidate payload against the typed PM spec plus
  current PMCSR, rejecting malformed bytes or live-snapshot mismatches before a
  broader PCI config checkpoint restore mutates device state.
- VirtIO tests cover modern PCI common-config feature-page selection,
  driver-feature writes, queue selection, queue sizing, queue notification
  offsets, queue descriptor/driver/device addresses, queue enable, device-status
  reset, snapshot restore, read-only register rejection, invalid queue-size
  rejection, unavailable-queue write rejection, notify-MMIO address derivation
  from queue notify offsets and notify-off multipliers, serial and parallel
  typed queue notification recording, notify snapshot restore, invalid
  multiplier rejection, write-only notify behavior, and mismatched notify write
  rejection. VirtIO PCI capability tests cover standard vendor-specific
  capability byte layout, cfg_type values, BAR/id/offset/length encoding,
  cap_len, cap_next, notify_off_multiplier extension bytes, zero-length
  rejection, invalid notify-kind rejection, and configuration-space placement
  rejection. They also cover installing generated common, notify, and
  shared-memory capability bytes into rem6-pci raw endpoint capabilities so
  guest enumeration observes the registry-owned chain. VirtIO PCI transport
  tests cover typed modern endpoint construction from identity, BAR, common,
  notify, ISR, device-config, and shared-memory region declarations, generated
  guest-visible capability chains, BAR-local runtime routing for common,
  notify, ISR, and device-config MMIO devices through parallel scheduler
  dispatch, and pre-mutation rejection for undeclared BAR references,
  out-of-BAR regions, same-BAR overlaps, undersized runtime regions, and
  missing device-config runtime devices. VirtIO block configuration tests
  cover modern feature bits, little-endian device-specific configuration
  layout, writeback mutability, read-only field rejection, and shape
  validation for capacity, block size, multiqueue, geometry, topology,
  discard, write-zeroes, and secure-erase limits. VirtIO block device tests
  cover serial and parallel decoded request execution, typed in-memory sector
  backend reads and writes, flush accounting, get-id payload padding, queue and
  request completion traces, unsupported request statuses, read-only write
  protection, out-of-range accesses, backend capacity checks, and 512-byte
  request-shape validation. VirtIO split descriptor-chain tests cover block
  read, write, flush, and get-id decoding into typed requests, status
  descriptor tracking, writable data-byte accounting, loop rejection, short
  header rejection, missing status rejection, wrong readable/writable
  direction rejection, indirect-table write-only flag ignore behavior, direct
  prefix plus terminal indirect-table consumption, logical descriptor-chain
  length rejection, get-id output shape validation, block completion
  scatter-data writeback records, status-byte writeback records, used-ring slot
  selection, wrapping used indices, little-endian used elements, and split
  available-ring walking from typed guest memory into decoded block requests,
  split virtqueue snapshot restore of consumed available-ring cursors and
  event-index mode with shape-mismatch rejection before mutation,
  rem6-system checkpoint-bank capture/restore of split queue state with
  prevalidated multi-queue restore to avoid partial live-state mutation,
  plus guest-memory writeback for block data buffers, status bytes, used
  elements, used indices, legacy available-ring interrupt suppression,
  event-index interrupt suppression, and queue-interrupt ISR status after
  completion writeback, with serial and parallel PCI legacy INTx delivery.
  VirtIO PCI ISR-status tests cover queue and configuration-change bit
  recording, serial and parallel read-clear behavior, snapshot restore,
  reserved-bit masking, read-only write
  rejection, width errors, and boundary errors. VirtIO PCI device-config tests
  cover typed mutable and read-only byte masks, serial and parallel config reads
  and writes, byte-mask writes, snapshot restore, access trace recording,
  invalid layout rejection, read-only byte rejection, and boundary errors.
  VirtIO PCI shared-memory tests cover cap64 offset and length splitting,
  vendor-specific capability byte layout, cap-next chaining, config-image
  export, entry lookup by region id and capability offset, unique region ids,
  declared BAR containment, missing or duplicate BAR rejection, zero-length
  rejection, address-overflow rejection, configuration-space placement
  rejection, short-buffer rejection, same-BAR overlap rejection, and
  BAR-filtered region queries.
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
  schema-and-reset-scope-checked stat-delta binding through the core stats delta API,
  duplicate binding rejection, missing bound-stat rejection, input updates, and snapshot restore. Thermal tests cover RC domain temperature updates from typed
  power estimates, expression input temperature coupling, invalid thermal
  parameter rejection, time-order rejection, update history, and snapshot
  restore. Thermal-network tests cover fixed references, thermal resistors,
  thermal capacitors, multi-domain implicit temperature solving, typed power
  inputs, expression input temperature coupling, invalid topology rejection,
  update history, and snapshot restore.
- Workload manifests record boot images, resources, topology, host events,
  checkpoint lineage, typed QoS policy intent, typed Linux boot handoff intent
  with device-tree and initrd resource validation, explicit required-resource
  payload resolution bound to manifest identity, resource acquisition
  provenance hashing, disk-image construction provenance hashing, RISC-V core
  route source partition and endpoint validation, explicit RISC-V data-cache
  backing-route validation and identity hashing, GPU and accelerator command
  plus DMA endpoint validation,
  manifest-owned parallel remote-flow count, remote-flow timing,
  remote endpoints with disjoint source/target endpoint-set validation, remote
  delay bounds, and exact remote-send contracts for
  CPU/cache/full-system plus direct GPU and accelerator DMA scheduler scopes,
  CPU/cache/full-system scheduler progress, direct GPU and accelerator DMA
  scheduler progress, direct GPU and accelerator DMA scheduler idle bounds,
  direct GPU and accelerator DMA scheduler frontier minima,
  full-system remote-flow and remote-send contracts backed by GPU and
  accelerator DMA scheduler traffic,
  scheduler idle bounds, at-least-two-worker max-worker use, at-least-two-worker total-worker
  activity, batch activity, at-least-two-partition CPU/cache/full-system active-partition, direct GPU and accelerator
  DMA active-partition, active GPU and accelerator compute or DMA device-count
  activity presence, CPU/cache/full-system per-partition activity, direct
  GPU and accelerator DMA per-partition activity, data-cache run attribution
  contracts, data-cache run-accounting consistency contracts, data-cache
  protocol run-count verification contracts, resource activity contracts,
  scoped wait-for edge-kind, blocked-node, and target-node window contracts, and
  clean diagnostic verification contracts, result metadata with
  manifest identity plus start/final ticks, execution mode switches, host
  action summaries, checkpoint restore labels, and statistics snapshots.

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
