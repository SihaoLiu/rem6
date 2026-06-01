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
  worker records, private worker-limit state, or an external profiler. The
  standalone CLI artifact now exposes the recorded worker-slot active and idle
  ticks for RISC-V runs too, so host-core utilization is visible at the same
  stats boundary as per-core instruction counts and per-partition scheduler
  activity. It also exposes actual worker-lane plus partition active ticks, so
  the standalone artifact can show which simulated partition occupied each
  host worker lane without relying on worker-slot totals alone. The main
  `rem6 run` JSON artifact now carries the same scheduler counts,
  worker-slot active and idle ticks, worker-lane partition ticks, partition
  activity, ready-partition order, and initial plus final partition frontiers as
  a structured `parallel.scheduler` object, while hierarchical stats expose the
  same frontier and ready-partition records with stable indexes, so scripted
  checks can parse typed parallel evidence without scraping terminal output or
  losing per-epoch records.
  The same artifact now also carries structured fetch and
  data memory transport summaries with per-route source counters, request and
  response counts, round-trip tick totals, and maximum round-trip ticks, so
  NoC and external-memory latency evidence can be consumed without rebuilding
  route identities from flat stats paths. CLI RISC-V data-access stats also
  carry total and per-core
  load/store/atomic byte counts from completed access records, including AMO
  execution, so cache, NoC, and external-memory bandwidth accounting can be
  validated from the same simulator artifact instead of reconstructed from
  instruction traces. The CLI artifact also carries fetch and data memory
  transport request, request-arrival, response, response-arrival, total
  round-trip tick, and maximum round-trip tick counters derived from
  `MemoryTrace`, and repeats those counters by route id and source endpoint,
  giving NoC and external-memory latency checks an explicit stats surface
  without collapsing independent core ports into one aggregate. Planned batches
  now also expose stable host worker-lane
  records with lane, partition, start tick, horizon, and duration, so the
  simulator can audit which host worker would own each ready partition before
  callbacks run instead of proving multicore use only from aggregate occupancy.
  Executed kernel batches now retain the matching actual worker-lane identity
  with lane, partition, partition-clock start, safe-until tick, next event tick,
  pending-event count, and duration-weighted lane tick summaries, so recorded
  parallel runs can prove which host lane executed each partition instead of
  inferring that binding from worker vector order or an external profiler.
  `RiscvSystemRun` now carries those recorded lane bindings across CPU
  scheduler, data-cache scheduler, and merged full-system scopes with lane tick
  and lane-partition tick queries, so system-level artifacts can audit actual
  host-lane occupancy without reopening kernel-private batch records.
  The CLI can also write the full JSON run artifact to a requested path and
  write the hierarchical stats array to its own requested path while returning
  a small machine-readable output envelope on stdout, so scripted runs can keep
  durable stats artifacts without scraping terminal streams.
  Workload replay summaries carry planned
  capacity totals and planned worker-lane records into result artifacts, derive
  planned worker-ticks, idle ticks, utilization ratios, and per-lane ticks from
  preserved planned evidence, and let replay contracts require a specific
  planned lane to cover a specific partition for a minimum tick budget. Replay
  diagnostics therefore keep the same pre-dispatch authority. Workload results
  also carry recorded worker-capacity ticks, idle-worker ticks, utilization
  ratios, and per-worker-slot active/idle summaries separately from
  multi-worker parallel-evidence contracts. Kernel recorded
  epoch and run summaries expose the same actual worker-capacity, idle-worker,
  utilization, and worker-slot occupancy evidence after callbacks and remote
  wakeups run, and `RiscvSystemRun` carries that recorded capacity evidence for
  CPU-scheduler, data-cache scheduler, and merged full-system scopes. Workload
  manifests and replay plans can now require those planned worker-slot active
  and idle tick budgets, plus planned worker-lane partition coverage, across
  CPU-scheduler, data-cache scheduler, GPU DMA, accelerator DMA, combined DMA,
  and merged full-system planned scopes.
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
  Kernel remote scheduling also reports absolute lookahead-boundary violations
  with source tick, requested delivery tick, and required minimum delivery tick
  from both serial and parallel scheduler contexts, so a cross-partition event
  that would violate deterministic remote-delivery slack fails as typed
  replayable state instead of becoming an event-queue merge side effect.
  Parallel epoch plans now also expose typed remote-delivery windows before any
  worker callback runs: for each planned source partition and remote target,
  the plan records the source tick, target clock, minimum legal delivery tick,
  first target-accepted delivery tick, epoch horizon, and whether the message
  could be delivered inside the current conservative epoch. This turns the
  cross-queue timing window into auditable data rather than leaving it as an
  implicit async insertion or quantum-merge side effect.
  System-level serial and parallel trap event preflight now uses the same
  absolute delivery-boundary error before a host event is queued, and GPU plus
  accelerator submission preflight now checks the source partition clock
  against the same boundary before device work is enqueued, so guest-host
  notifications and device launches cannot bypass the kernel contract through
  a higher-level helper.
  Transport route preflight also walks request and earliest-response hops with
  absolute source and delivery ticks before scheduling memory traffic, so
  invalid NoC or memory route latencies fail as typed boundary evidence rather
  than as callback panics inside transport event delivery.
  CPU MMIO data issue applies the same request and earliest-response
  preflight before dispatching a parallel worker, including translated data
  accesses after virtual-to-physical resolution, so invalid device response
  timing remains a typed CPU/MMIO scheduler error and cannot disappear into a
  worker callback or delayed response-error side channel.
  Interrupt line ports also prevalidate their controller route before serial or
  parallel signal delivery is scheduled, so static line-target mismatches fail
  at the device/interrupt boundary while delivery-time state conflicts remain
  explicit recorded errors. Programmable timer arm paths use that route
  preflight before committing armed state, and they commit timer generations
  only after the deadline event is accepted by the scheduler. Invalid timer
  interrupt wiring and deadline-delivery lookahead violations therefore return
  typed errors at the MMIO/device call site instead of persisting ghost timer
  state or deferring the failure to a future callback. CLINT hart construction
  now validates both software and timer interrupt routes before runtime state
  exists, so bad platform wiring cannot be deferred to `msip` writes,
  `mtimecmp` callbacks, reset deassertions, or RTC pulse callbacks. CLINT
  `msip`, immediate `mtimecmp`, and RTC-driven timer assertion paths also
  commit register or asserted-line state only after the required interrupt
  signal has been accepted by the scheduler, preventing remote-boundary
  failures from leaving partially committed platform state. MC146818 RTC
  periodic interrupt startup now prevalidates the typed interrupt route before
  scheduling the first serial or parallel pulse, records scheduler or delivery
  errors as RTC-owned typed data, and invalidates stale periodic events through
  a device-local generation when PIE is cleared. Alarm and update-ended RTC
  second ticks expose typed interrupt flags, set read-clear status-C bits, and
  deliver serial or parallel assert/deassert pulses only after the RTC route has
  been checked. UART RX injection and final-byte RX MMIO reads follow the same
  rule: failed serial or parallel interrupt assertion keeps bytes out of the
  pending RX FIFO, while failed deassertion returns a typed MMIO device error
  before consuming the byte.
  RISC-V PLIC MMIO now uses the standard 32-bit priority, pending, enable,
  threshold, and claim/complete windows for platform interrupt controllers, so
  device-tree `riscv,plic0` declarations no longer point at a rem6-only claim
  register shape. PLIC claims are filtered by explicit enable and threshold
  state, context-indexed enable and threshold/claim windows are routed through
  explicit context-to-target records, and invalid completion writes return
  typed errors while preserving the outstanding claim. The PLIC device model
  also lives in `rem6-interrupt/src/plic.rs` instead of the interrupt crate root,
  so RISC-V interrupt-controller growth stays behind a focused module boundary
  rather than reproducing gem5-style device and register-bank accumulation in a
  single broad file. Platform PLIC declarations now carry explicit context
  records from guest-visible context index to hart external-interrupt specifier,
  interrupt target, and target partition; the same records drive both the MMIO
  device and `interrupts-extended` DTS emission, avoiding gem5's split between
  Python `hart_config` strings and C++ context register construction. PLIC
  local state now has typed snapshots for context enable bits and threshold
  registers, plus checkpoint ports and banks that prevalidate all target PLIC
  devices before any restore mutation. Controller-wide interrupt routes,
  pending interrupts, priorities, claims, and history remain in the generic
  interrupt-controller snapshot, so rem6 does not recreate gem5-style broad
  device serialization where register-local state, event scheduling, and
  controller state are restored through one fragile object boundary. Platform
  construction now retains each PLIC MMIO device as an enumerable platform
  device, and topology host setup attaches those PLIC devices to the full-system
  checkpoint path automatically.
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
- Unsupported MMIO regions that gem5 models with panic-on-touch devices are
  represented as typed trap devices in rem6: serial and parallel accesses return
  `MmioError::UnsupportedDeviceAccess` and leave an access log with device name,
  tick, request id, operation, range, and write payload evidence.
- Classic cache checkpointing in gem5 still treats dirty cache state as a bad
  checkpoint for the classic cache path: `BaseCache` records the dirty condition
  during serialization and rejects restore of that checkpoint. rem6 cache-bank
  snapshots instead preserve dirty line state and data as ordinary typed
  snapshot state, and expose dirty-line count plus sorted line-address audit
  views for MSI, MESI, MOESI, and CHI banks so checkpoint validation can prove
  what would be restored before mutating live cache state.
- Replacement policy ownership is a fragile Ruby and CHI boundary in gem5. A
  public CHI issue reports that changing a `RubyCache` replacement policy from
  Python can reach a SHiP reset path without the access information needed for
  predictor training, while local reference code warns that replacement updates
  moved into SLICC protocol state machines. rem6 therefore keeps replacement
  set state and set/way residency in protocol-neutral typed structures: the
  protocol controller can ask for a victim or touch a resident line, but the
  policy state, line ownership, snapshot restore, and validation are not hidden
  in generated protocol code or callback side effects. CHI cache banks now use
  that boundary for clean-line capacity eviction, preserve replacement state in
  bank snapshots, and reject dirty capacity victims with typed state instead of
  silently dropping ownership before writeback integration exists.
- `AddrRange` is a central gem5 memory primitive, but public issue #2855
  identifies two full-system limits that have leaked into many call sites:
  non-power-of-two memory or CHI SNF channel counts need modulo interleaving,
  and x86-style physical holes need sparse ranges whose backing-store offsets
  are packed after excluded intervals. gem5's design discussion also notes that
  `AddrRangeMap::contains` can be hot under Ruby. rem6 keeps the simple
  `AddressRange` as a small interval and moves optional mapping behavior into
  typed `AddressMapRegion` records. The decoder stores sparse holes, modulo
  interleave granularity, stripe count, match index, and packed local offsets as
  data that snapshots can validate, so a full-system memory map can express
  three, six, or twelve channels and I/O holes without forcing every consumer to
  reinterpret raw start/end pairs. Partitioned stores also expose mapped-range
  validation that checks decoder ownership and resident cache lines without
  issuing synthetic memory requests, so device preflight paths can prove
  guest-memory writeback targets before mutating guest-visible state.
- Public issue #2816 reports that a full-system `MESI_Three_Level` plus O3 CPU
  run can hit Ruby `MessageBuffer`'s strict-FIFO panic when a newly computed
  arrival tick is earlier than the last recorded arrival tick. The upstream
  response treats this as a likely protocol bug that is hard to reproduce and
  not safely ignorable. rem6 moves that boundary into a typed transport message
  buffer: strict FIFO admission computes the arrival tick before mutation,
  rejects arrival regressions and disallowed zero-latency sends as typed errors,
  preserves the previous queue state on rejection, records explicit bypass
  intent, orders ready messages by arrival plus stable sequence, and preserves
  the FIFO guard through snapshots. Protocol controllers therefore get a
  replayable admission contract instead of discovering ordering drift through a
  late simulator panic.
- Public issue #1129 reports a `MOESI_CMP_directory` transition path where a
  later unblock action deallocates a TBE that was not allocated by the earlier
  transition into the busy state. The workaround discussion points at adding a
  missing SLICC action, but the deeper debt is that resource lifetimes live in
  hand-maintained transition action lists and are only checked by assertions
  after a rare path executes. rem6-protocol-moesi now has a typed
  transition-resource contract: protocol states declare required resources,
  transitions declare allocate or release effects, duplicate transition keys are
  rejected before execution, entering a resource-owning state without allocation
  is a typed error, leaving such a state without release is a typed error, and
  release without ownership is rejected. This keeps protocol expressiveness
  while making SLICC-style missing action bugs verifiable before a full-system
  run reaches them.
- Ruby functional reads in gem5 are scattered across controller access
  permission probes, controller buffers, network buffers, and backing-store
  fallback, with the selected line data written through a mutable `Packet`.
  rem6 is moving that behavior into typed coherence audit records: the MSI
  bank harness now reports read-only, read-write, busy, maybe-stale, backing,
  and invalid counts, selects modified cache data before shared or backing
  data, and reports busy or in-transit lines without fabricating a line.
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
  records when aggregate flow records are absent or weaker. Remote debugging
  now starts from a typed GDB remote packet boundary: `rem6-debug` decodes
  acknowledgements, interrupts, packet payloads, payload length limits,
  checksums, malformed checksums, trailing bytes, and skipped prefix noise
  before any ISA register cache or socket loop can mutate simulator state.
  That keeps remote debugger transport errors as ordinary data instead of
  coupling packet parsing to thread-context callbacks. Workload manifests and replay
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
- Branch predictor configuration should fail before allocation or opaque host
  allocator failures. A public gem5 SimpleBTB issue reports `malloc(): bad
  size` when large BTB entry and associativity combinations are configured.
  rem6 now bounds default BTB entries and associativity with typed
  `BranchTargetBufferError` variants while retaining an explicit `with_limits`
  constructor for experiments that deliberately raise those budgets.
- Observability and statistics need stronger contracts. Public gem5 issue
  #1644 about stats reset explicitly calls out missing reset tests and user
  confusion from inconsistent stats. rem6 therefore treats statistics,
  activity, wait-for
  graphs, and run summaries as typed data with tests rather than string-only
  logs or ad hoc probes. Stats reset windows are monotonic: a reset request
  before the previous reset tick is rejected without changing the active epoch
  or counter values, successful reset records are retained in typed
  registry-owned history with stable reset ids, and dump/reset events are also
  kept in one typed interleaved history. Each counter also declares an explicit
  reset policy: resettable counters are zeroed, constant counters are retained,
  and monotonic lifetime counters are retained. Reset records preserve a typed
  per-counter audit with the policy, previous value, and post-reset value, and
  snapshots plus deltas carry the same policy so reset behavior cannot be
  inferred from naming convention or stale documentation. That interleaved
  history is globally
  monotonic: a dump or reset earlier than the previous history record is
  rejected before ids, epochs, counters, or record vectors change. Workload
  replay carries that typed reset/dump history into workload results, so replay
  artifacts preserve both the final snapshot and the ordered stats-control
  record stream. Manifests can require minimum reset/dump counts, exact
  first/last stats-history ticks, and exact reset/dump event sequences with
  stable ids, epochs, and dump reset windows, so stats-control behavior becomes
  replay-verifiable workload identity rather than a log-only side effect. Once
  a registry has emitted any dump or reset history record, counter and group
  registration is locked so the history stream cannot silently mix multiple
  schemas without a typed schema event. Counter paths are structured
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
- The gem5 output path convention writes run artifacts such as configuration
  dumps and `stats.txt` under `m5out`, with different file formats carrying
  different slices of run identity. rem6 now has a workspace-owned `rem6`
  binary crate that begins the final standalone-simulator path explicitly:
  `rem6 run --isa riscv --binary <ELF> --max-tick <tick> --stats-format json`
  reads a real ELF through `rem6-boot`, rejects ISA/header mismatches before
  emitting stats, and writes one machine-readable run artifact containing ELF
  identity, load-segment count, status, and hierarchical typed stats. The
  same stats snapshot can now also be emitted as a gem5-style text statistics
  block with begin/end markers plus row-level unit and reset-policy metadata,
  so `stats.txt`-style consumers do not need to parse the JSON artifact. The
  initial status is deliberately `loaded` rather than a false execution claim;
  instruction execution is now exposed only when requested with `--execute`.
  That execution path instantiates real RISC-V cores on independent scheduler
  partitions, drives them through the existing parallel system-run driver, stops
  at guest traps through the host event path, at the configured `--max-tick`
  execution budget, or at an explicit `--max-instructions <count>`
  committed-instruction budget, and records
  per-core architectural state plus hierarchical committed-instruction,
  stop-reason, and parallel-scheduler stats.
  The CLI artifact also emits scheduler dispatch, batch, total-worker,
  active-partition, remote-send, batch worker-tick, worker-capacity tick, and
  idle-worker tick stats from the recorded parallel run, so standalone
  simulator output exposes the same multicore runtime evidence that subsystem
  tests inspect directly. The CLI now also accepts an explicit
  `--parallel-workers <count>` host-worker budget, defaults it to the requested
  core count, passes it into the partitioned scheduler, and emits the configured
  limit as a constant stat. Parallel capacity is therefore a typed simulator
  input and artifact field rather than an implicit host-side runtime choice. The
  artifact now also emits per-partition scheduler worker, dispatch,
  remote-send, remote-receive, and max-pending-event stats, so standalone runs
  can explain which core, memory, or host partition contributed to aggregate
  parallel activity.
  The CLI execution path also provisions per-core data endpoints, so ELF-backed
  RISC-V programs can execute load/store instructions through the same memory
  transport and `PartitionedMemoryStore` path used by subsystem tests; data load,
  store, and atomic counts are emitted as hierarchy-preserving stats. Requested
  post-run memory dumps read back bytes from the same memory store, which makes
  store effects part of the machine-readable artifact instead of an implicit log
  side effect. RISC-V CLI execution also binds each architectural hart id to the
  core's typed `CpuId` and supports the `mhartid` CSR read path, so one ELF can
  naturally branch per hart while running on independent parallel scheduler
  partitions. Device boot, richer runtime controls, and full-system run plumbing
  remain separate acceptance targets.
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
  Recent gem5 issue #3092 reports incorrect `SerialLink` latency when the
  system clock is not 1GHz. rem6 fabric serial-link hops therefore bind fixed
  latency and serialization bandwidth to an explicit `ClockDomain` and typed
  lane bit-rate before converting to ticks. Serial-link bandwidth can be
  declared either as bits per link cycle or as gem5-style bits per nanosecond
  with an explicit ticks-per-nanosecond timebase; the latter path converts
  nanoseconds through `ClockDomain::ticks_to_cycles_ceil` before scheduling
  ticks, so non-1GHz links cannot silently reuse a 1GHz serialization
  assumption.
  Public gem5 issue #2493 reports a DRAM `MemCtrl` state where the write queue
  remains nonempty while QoS turnaround selection repeatedly chooses the other
  direction and no request drains. The local reference keeps read and write
  queues plus a turnaround-selected bus state as separate mutable counters.
  rem6-dram instead orders a same-arrival QoS batch from typed request records:
  current-direction preference can have an explicit same-direction burst limit,
  but that limit only forces a switch when an opposite-direction request is
  actually eligible. A write-only batch therefore keeps draining writes, while
  mixed batches can bound read/write starvation without selecting an empty
  direction.
- Power equations should not depend on late string lookup into global
  statistics. gem5's MathExprPowerModel accepts equations that reference stat
  names plus automatic variables. rem6 keeps the equation idea, but binds
  metric inputs, temperature, voltage, and clock period through typed records
  before evaluation. Power metric inputs can also be derived from two stats
  snapshots only when they share the same reset epoch and reset tick, the second
  snapshot is not earlier in simulated time, and every bound counter is
  monotonic within that scope. Stat group-catalog, path/unit descriptor, and
  description drift are surfaced as typed power errors rather than falling
  through a panic boundary. Recent gem5 issue #3192 reports thermal-model
  intermediate nodes initialized to 0K, causing nonphysical cooling. rem6
  thermal networks therefore model passive junctions as explicit typed nodes
  with required initial temperature and heat capacity, reject absolute-zero or
  lower temperatures at construction and restore, and include those junctions
  in the same RC solve as powered domains. Thermal snapshots also carry typed
  node-initialization records for domains, references, and passive junctions;
  restore rejects partial or type-mismatched initialization evidence, so a
  multi-node RC network cannot silently lose the original nonzero junction
  seed after checkpointing.
- Compatibility bugs cluster around cross-subsystem seams. Recent public gem5
  issues include syscall-emulation gaps for modern libc behavior, RISC-V vector
  tracing crashes, and a three-level CHI LR/SC race in multicore RISC-V
  workloads. rem6 therefore keeps ISA, guest ABI, coherence, memory ordering,
  and device behavior behind typed crate boundaries with focused regression
  tests before broad parity claims. Public gem5 issue #3098 reports RISC-V
  `mcycle` and `minstret` writes being ignored. rem6 therefore keeps cycle and
  retired-instruction CSRs in a typed counter bank where machine aliases are
  writable, user aliases are read-only, increments wrap explicitly, and RV32
  `mcycleh`/`minstreth` plus `cycleh`/`instreth` aliases project the high word
  of the same 64-bit counter instead of becoming separate pseudo-registers.
  `RiscvHartState` now owns that counter bank for instruction execution:
  `csrrs rd, cycle, x0`, `csrrs rd, instret, x0`, and the corresponding machine
  aliases read the live counters from the same path used by ELF execution, and
  normal in-order instructions advance both counters. Snapshot restore
  preserves both counters before full privileged CSR decoding is widened.
  Public gem5 issue #2742 reports MinorCPU branch prediction copying RISC-V
  dynamic-instruction vector configuration so an older `vl` can overwrite the
  current loop's vector length. The local reference keeps `_vl` and `_vtype` in
  `PCState`, and Minor `BranchData` clones `PCStateBase` targets for predicted
  streams. rem6-isa-riscv instead makes vector configuration explicit hart
  state and models control-flow changes as typed updates: branch-prediction
  targets carry only the target PC, copied dynamic snapshots are normalized to
  PC-only targets, and only `RiscvVectorConfigUpdate` can mutate `vl` or
  `vtype`.
  Public gem5 issue #2907 reports `vcompress.vm` overwriting destination tail
  elements under tail-undisturbed policy. The local reference fills a static
  compress buffer and also writes `Vd_vu[i]` while scanning selected source
  elements, so destination indices beyond the final compressed count can be
  clobbered before tail policy is applied. rem6-isa-riscv instead models vector
  element images, vector length, and tail policy as explicit data. The
  `RiscvVectorCompressPlan` first forms the compacted prefix, writes only that
  prefix, and then applies a deterministic tail policy: undisturbed tails retain
  the incoming destination image, while agnostic tails become all ones for the
  selected element width.
  Public gem5 issue #2885 reports RISC-V vector macro-instruction flags not
  applying to generated micro-operations. The local reference vector templates
  build `StaticInstPtr microop` records and then set micro-op-local properties
  such as delayed commit and first/last markers, which leaves macro-op
  scheduling flags dependent on generator plumbing. rem6-isa-riscv instead
  models instruction flags as typed bit records and expands vector micro-ops by
  merging macro flags with micro-op-local flags before any `RiscvVectorMicroOp`
  record exists. Every generated micro-op therefore carries serialize-after,
  non-speculative, delayed-commit, and future execution constraints through one
  explicit contract.
  Public gem5 issue #2677 reports RISC-V vector fixed-point instructions using
  default rounding in places that should read `vxrm`, plus missing `vxsat`
  updates at narrowing saturation sites. The local reference has integer vector
  decode bodies that call rounding with `0` where a typed `vxrm` value belongs,
  leaves `vxsat` clipping sites marked as unfinished, and commits saturation
  through a separate serialized `VxsatMicroInst`. rem6-isa-riscv instead models
  `vxrm`, `vxsat`, and the `vcsr` alias as typed fixed-point vector state,
  models all four fixed-point rounding modes as an enum, and returns saturation
  evidence from narrow-clip execution so each operation can merge `vxsat`
  directly into architectural state.
  The RISC-V privileged ISA specifies `mhartid` as the hardware-thread id CSR.
  gem5's open RISC-V full-system synchronization issues make hart identity a
  prerequisite for testing multicore boot and lock paths with one shared image
  rather than host-side per-core entry hacks. rem6-isa-riscv now decodes a
  read-only `mhartid` CSR read, `RiscvHartState` stores an explicit hart id, and
  rem6-cpu initializes that id from `CpuId` when constructing a RISC-V core.
  This keeps per-hart behavior in architectural state and leaves CLI execution,
  topology execution, and future Linux/SBI boot using the same mechanism.
  Public gem5 issue #2688 reports a RISC-V full-system mutex failure in a
  three-level CHI hierarchy when LR/SC pairs race with contending atomic writes.
  The local reference keeps the LL/SC monitor in Ruby `Sequencer` methods over a
  local `AbstractCacheEntry::m_locked` field, while CHI SLICC actions add
  store-miss timeout and eviction-side callbacks. rem6-protocol-chi instead
  exposes a line-global `ChiReservationTable`: LR records are protocol state,
  a successful SC consumes its own reservation and invalidates overlapping peer
  reservations, failed SC consumes any stale local reservation, and coherence
  invalidations or evictions produce typed invalidation records. That gives
  multicore CHI replay a deterministic reservation authority instead of relying
  on per-cache-entry side effects.
  Public gem5 issue #3075 reports a related RISC-V Ruby CHI full-system hang
  where multithreaded workloads can accumulate repeated SC failures and RCU
  stalls under TimingSimpleCPU and O3CPU instead of terminating. The local
  reference increments per-thread store-conditional failure counters and emits
  periodic string warnings, while CHI separately grows an SC lock latency
  multiplier. rem6-cpu now keeps `RiscvStoreConditionalProgress` as typed CPU
  state: SC failures are grouped by CPU, address, and size, threshold crossings
  materialize `RiscvStoreConditionalFailureDiagnostic` records with first and
  last failure ticks, successful SC completion resets the per-CPU streak, and
  snapshot/restore preserves streaks and diagnostics. `RiscvClusterRun` now
  preserves those diagnostics from its cores and exposes run-level counts plus
  per-CPU queries. `RiscvSystemRun` carries the same typed diagnostic records
  from serial, parallel, MMIO, and translated full-system drivers, so
  parallel full-system replay can keep LR/SC starvation evidence attached to
  the run result instead of hiding it in warnings and protocol-local latency
  side effects.
  Public gem5 issue #3013 reports a Ruby CHI `Evict`/`Snp*Invalid` hazard where
  `RestoreFromHazard` can rebuild `dir_sharers` from a snoop-mutated directory
  state and then hit an Evict acknowledgement assertion that expects the
  requester to remain listed as a sharer. The local reference copies
  `dir_sharers` through `copyCacheAndDir` and later `Send_CompI` asserts the
  Evict requester is still present. rem6-directory therefore keeps a typed
  `ChiEvictHazard` snapshot for the pending Evict requester and pre-hazard line
  state, and `ChiEvictHazardRestore` reports whether the request became stale
  while preserving the acknowledgement target independently from the current
  post-snoop sharer set.
  Public gem5 issue #2754 reports sim-se `wait4` returning incorrect status
  for abnormal child exits. The local reference implements `wait4Func` in
  `src/sim/syscall_emul.hh` by consuming a `SIGCHLD` signal and writing a fixed
  exited status, while x86 and RISC-V syscall tables route guest `wait4` to that
  shared helper. rem6-system therefore keeps guest wait results as typed
  `GuestWaitStatus` values before lowering to POSIX wait-status integers:
  normal exit encodes `code << 8`, signal termination preserves the low
  seven-bit signal with an optional core-dump bit, stopped children encode
  `(signal << 8) | 0x7f`, continued children encode `0xffff`, and invalid guest
  signals are typed errors.
  Public gem5 issue #1320 reports RISC-V GAP benchmark hangs with timing and
  minor multicore CPUs where the final barrier reaches a `futex` wait with only
  one core still awake, while atomic multicore reaches the same barrier with
  all cores completing the synchronization. The local reference routes RISC-V
  syscall number 98 to the shared `futexFunc`, and `FutexMap` stores
  `ThreadContext*` waiters plus a side `waitingTcs` set that is mutated by
  suspend, wake, and requeue callbacks. rem6-system therefore keeps guest
  futex state as a typed `GuestFutexTable`: wait operations compare expected
  and observed values before queueing, waiters carry guest thread id,
  thread-group id, partition, enqueue tick, and bitset, wake and requeue
  operations emit deterministic FIFO records, duplicate waiters and empty
  wait bitsets are typed errors, and the waiting index is explicitly cleared
  or moved as wake and requeue records are produced.
  Public gem5 issue #2750 reports sim-se `dup2` allocating the next available
  guest file descriptor like `dup` instead of occupying the requested
  destination descriptor. The local reference clones the source host-backed fd,
  calls host `dup2`, closes any existing target guest entry, and then calls
  `allocFD`, which loses the explicit `newfd` request. rem6-system therefore
  keeps guest fd allocation as a typed `GuestFdTable`: `dup` allocates the
  lowest free guest fd, while `dup2` first validates the source, returns the
  same fd as a no-op for `oldfd == newfd`, and otherwise replaces exactly the
  requested destination fd with a duplicated guest file-description mapping.
  Public gem5 issue #3132 reports O3 data prefetches tying up LQ/ROB retirement
  resources until memory responses arrive. rem6 therefore records O3 prefetches
  as typed dependency-trace memory records with an explicit retire-after-issue
  completion policy, so future O3 replay and pipeline models cannot silently
  treat prefetch as a register-writing load.
  Public gem5 issue #1050 reports the Indirect Memory Prefetcher repeating the
  same indirect prefetch address when `max_prefetch_distance` is greater than
  one because generated candidates reuse the current index value. rem6's IMP
  path therefore separates the observed index from typed future lookahead
  indexes: multi-distance indirect prefetches consume those lookahead values by
  degree, and a missing lookahead source cannot inflate the candidate list with
  duplicate current-index addresses.
  Public gem5 issue #621 reports a classic-cache MSHR deferred-target hazard:
  after a read fills a line, gem5 can move a deferred write and deferred
  `CleanSharedReq` into the active target list, satisfy every target locally,
  clear dirty state, and deallocate the MSHR without sending the clean
  downstream. rem6 therefore gives each MSHR target a typed post-fill action.
  Demand reads, writes, upgrades, atomics, and prefetches remain local
  fill-service targets, while writeback, clean-evict, and invalidation
  maintenance requests are exported as explicit post-fill downstream requests
  from cache-bank fill results. That keeps maintenance traffic observable to
  NoC, directory, and memory scheduling instead of hiding it behind a local
  cache-block side effect.
  Public gem5 issue #2955 reports a classic-cache block move hazard: the local
  reference `CacheBlk` move assignment calls `insert` with the source tag after
  `TaggedEntry::insert` changed to expect a full address and extract the tag
  internally. Any `BaseTags::moveBlock` user can therefore build a target block
  whose tag no longer matches the source block. rem6-cache therefore stores
  resident cache-line identity as a canonical full `Address`, exposes typed
  resident-line relocation on the replacement directory, rejects tag-shaped
  values that are not resident full-line addresses, validates the destination
  set and way before mutation, and relocates replacement state without asking
  SHiP-like policies for a synthetic new signature.
  Public gem5 issue #3096 reports `IEW::instToCommit` overflowing an
  `IEWStruct` TimeBuffer future window when many LSQ completions become ready on
  the same tick. The local reference increments `wbCycle` after each `wbWidth`
  occupied slot but relies on `TimeBuffer::valid` to catch out-of-window
  accesses. rem6-cpu therefore models the IEW writeback-to-commit transfer as a
  typed `O3WritebackTransferPolicy`: a same-tick ready set is admitted only up
  to `(future_cycles + 1) * writeback_width`, every admitted item carries an
  explicit cycle offset and slot inside the representable window, and remaining
  completions are reported as deferred work rather than indexing past the
  buffer, dropping completions, or silently bypassing bandwidth.
  Public gem5 issue #2956 reports distributed O3 issue queues underutilizing
  same-OpClass function units because `InstructionQueue::listOrder` is keyed by
  OpClass rather than by the physical issue queue that owns the ready
  instruction. The local reference stores one `readyInsts[OpClass]` heap and a
  `listOrder` entry per OpClass, then advances past a busy FU result for that
  whole OpClass. rem6-cpu therefore exposes an
  `O3DistributedIssueScheduler` whose resource key is `(issue_queue,
  op_class)`: once one queue's IntAlu capacity is exhausted, younger ready
  IntAlu work in another queue can still issue in the same cycle if that
  queue has available capacity.
  Public gem5 issue #2953 reports RISC-V unordered vector reductions losing O3
  throughput because correctness is protected by putting `IsSerializeAfter` on
  the first unordered reduction micro-op. In the local O3 rename path,
  serialize-after marks the next instruction serialize-before, and
  serialize-before waits for an empty ROB. rem6-cpu instead exposes scoped O3
  dependency records: unordered reduction partial micro-ops produce
  reduction-local scopes, the publish micro-op waits for those scopes and
  produces the architectural result scope, and unrelated younger work remains
  issueable without a whole-pipeline serialization barrier.
  Public gem5 issue #2211 reports RISC-V O3 full-system failure when both the
  return-address stack is disabled and `SimpleIndirectPredictor` stops hashing
  path-history targets through `indirectHashTargets = False`. The local
  reference exposes both switches independently in `BranchPredictor.py`, and
  `BPredUnit` later relies on RAS, BTB, and indirect predictor target providers
  during early boot control flow. rem6-cpu therefore makes that risky
  cross-parameter combination a typed `BranchTargetSafetyConfig` error for
  RISC-V O3 full-system construction unless either RAS is enabled or indirect
  path targets are hashed.
  Public gem5 issue #3157 reports ArmO3 SE startup faults when a younger
  AArch64 TLS read observes stale `TPIDR_EL0`: the older misc-register write is
  architecturally applied only at commit, but generic O3 dependency readiness
  can release dependents at writeback. rem6-cpu therefore models destination
  register visibility explicitly. Normal destinations publish at writeback,
  commit-visible misc destinations defer dependency wakeup and scoreboard
  readiness until commit, always-ready fixed mappings remain dependency-free,
  and commit-time release never reruns memory-dependence completion.
  Public gem5 issue #3041 reports an O3 rename segfault when an instruction
  carries an `InvalidRegClass` source. The local reference maps the source to
  `invalidPhysRegId`, but its `InvalidRegClass` branch only exits the class
  switch and then still reaches the scoreboard lookup. rem6-cpu therefore
  separates source-rename decisions from scoreboard state: invalid source
  classes map to a typed invalid physical register, are marked ready in their
  source slot, and do not emit a scoreboard lookup, while mapped sources still
  consult scoreboard readiness explicitly.
  Public gem5 issue #3010 reports AArch64 TLBI ASIDE1 invalidating final-level
  global entries for a matching ASID even though the architectural operation
  should retain those entries. The local reference matches ASID, regime, VMID,
  and security state in `TLBIASID::matchEntry` before `TLB::flush` invalidates
  entries, but that predicate has no global-entry check. rem6-memory therefore
  carries explicit `TranslationTlbEntryScope` on every TLB entry and exposes
  `flush_non_global_address_space` for Arm-shaped ASID TLBI, while
  `flush_address_space` remains the force-clear operation for callers that
  intentionally need all entries for an ASID removed.
  Public gem5 issue #2962 reports x86-64 REX prefixes in invalid positions
  extending the destination register even though only a REX prefix immediately
  before the opcode or `0x0f` escape should apply. The local reference records
  `emi.rex` whenever `doPrefixState` sees a REX prefix, and a later legacy
  prefix updates `emi.legacy` without clearing that pending REX state. rem6
  starts x86 support with a typed prefix scan: legacy prefixes, active REX, and
  ignored REX prefixes are separate records; a legacy prefix after REX records
  the REX as ignored, and contiguous REX prefixes keep only the last one before
  opcode decode.
  Public gem5 issue #2912 reports x86 `STI` and `CLI` computing IOPL from the
  wrong RFLAGS bits, letting a user-mode `cli` escape the expected #GP fault
  when carry plus the reserved bit make the low two RFLAGS bits look like
  IOPL=3. The local reference has `RFLAGS.iopl` at bits 13:12, but the
  generated microcode shifts into a temporary and then masks the original
  RFLAGS value. rem6-isa-x86 therefore gives RFLAGS, CPL, CR4.PVI, and
  interrupt-flag operations typed decision records: protected-mode `STI` and
  `CLI` compare CPL only against bits 13:12, report general-protection faults
  explicitly, and keep PVI/VIF behavior separate from IF mutation.
  Public gem5 issue #3190 reports O3 block-to-unblock transitions that wait
  until a downstream skid buffer is empty, causing a bubble after the backwards
  unblock signal propagates. rem6-cpu therefore exposes an `O3UnblockPolicy`
  that signals unblock when the remaining skid entries are within
  `backward_signal_delay_cycles * downstream_width`, preserving gem5's skid
  buffering concept while making the delay-compensated threshold typed and
  testable before a full O3 pipeline lands.
  Public gem5 issue #3185 reports Ruby checkpoint restore crashing when warmup
  code schedules at tick 0 while the live event queue still has a later current
  tick. gem5's Ruby warmup path temporarily resets global time and relies on
  restore ordering. rem6-kernel instead exposes a checkpoint restore event plan
  with an isolated warmup replay clock and a live-event lower bound at the
  restored tick. The plan preserves capture order for audit but exports warmup
  events in replay-clock order and live events in scheduler order, so subsystem
  warmup events cannot be mistaken for live scheduler events after checkpoint
  restore. The warmup boundary is now stateful: once warmup is finished, the
  plan rejects any later warmup event while still allowing live events at or
  after the restored tick, so a late subsystem callback cannot recreate gem5's
  schedule-in-past path after live handoff has begun.
  Public gem5 issue #910 reports x86 full-system KVM-to-O3 switching that can
  segfault or jump unexpectedly after `m5_switch_cpu_addr`, with local logs
  showing large unsupported-MSR skip sets, x86 special-register warnings, and a
  post-switch O3 `mwait` path dereferencing incomplete request state. The local
  reference requires KVM CPUs to be drained and idle before `switchOut` or
  `takeOverFrom`, then performs architecture-specific register and MSR transfer
  through KVM ioctl paths that panic on failed set/get operations while
  unsupported MSRs are only warned and skipped. rem6-system therefore exposes a
  typed host-assisted switch admission plan before native execution support
  lands: a host-assisted target must declare complete architecture state,
  no pending MMIO/PIO/hypercall/halt/mwait service, matching memory mode, and
  no unsupported host registers before a detailed or timing target can take
  over. The resulting plan lists quiesce, pending-service validation,
  deterministic state capture, target install, and resume actions rather than
  relying on hidden host vCPU state.

Research anchors refreshed through 2026-05-30:

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
- Public gem5 issue anchor refreshed on 2026-05-30: open sim-se `wait4`
  status bug where abnormal child exits can lose the terminating-signal status
  expected by libc `wait`.
- Public gem5 issue anchor refreshed on 2026-05-30: open RISC-V GAP timing and
  minor multicore hang where final futex-barrier synchronization reaches only
  one awake core instead of all participating cores.
- Public gem5 issue anchor refreshed on 2026-05-30: open sim-se `dup2`
  bug where the requested destination fd is ignored and the next available fd
  is allocated instead.
- Public gem5 issue anchor refreshed on 2026-05-30: open SerialLink latency
  bug when system clock frequency is not 1GHz.
- Public gem5 issue anchor refreshed on 2026-05-30: open DRAM `MemCtrl`
  write-queue drain bug where QoS turnaround can repeatedly select a direction
  that does not drain the nonempty queue.
- Public gem5 issue anchor refreshed on 2026-05-30: open ThermalModel
  intermediate-node initialization bug that can drive nodes to 0K.
- Public gem5 issue anchor refreshed on 2026-05-30: open SimpleBTB oversized
  configuration bug where large entry and associativity combinations can crash
  in host allocation instead of producing a typed configuration error.
- Public gem5 issue anchor refreshed on 2026-05-30: open O3
  `InvalidRegClass` source rename bug where an invalid source can fall through
  to a scoreboard lookup and segfault.
- Public gem5 issue anchor refreshed on 2026-05-30: open RISC-V
  `mcycle` and `minstret` writes ignored bug.
- Public gem5 issue anchor refreshed on 2026-05-30: open MinorCPU RISC-V
  branch-prediction vector-length bug where copied dynamic-instruction state can
  move a stale `vl` into the predicted stream.
- Public gem5 issue anchor refreshed on 2026-05-30: open RISC-V
  `vcompress.vm` tail-undisturbed bug where destination elements after the
  compressed prefix can be overwritten instead of preserved.
- Public gem5 issue anchor refreshed on 2026-05-30: open RISC-V vector
  micro-operation flag propagation bug where macro-instruction scheduling flags
  are not visible on generated vector micro-ops.
- Public gem5 issue anchor refreshed on 2026-05-30: open RISC-V vector
  fixed-point rounding and saturation bug where `vxrm` can be replaced by a
  default mode and narrowing saturation can miss `vxsat`.
- RISC-V privileged ISA anchor refreshed on 2026-05-30: `mcycle` and
  `minstret` are 64-bit machine counters, with RV32 exposing high-half CSR
  aliases rather than independent counters. rem6 now exposes executable
  `cycle` and `instret` CSR reads through hart state instead of limiting counter
  coverage to standalone bank tests.
- RISC-V privileged ISA anchor refreshed on 2026-05-30: `mhartid` is the
  machine hardware-thread id. rem6 binds this CSR to the typed RISC-V core id
  instead of treating per-core identity as a CLI-only launch option.
- Public gem5 issue anchor refreshed on 2026-05-30: RISC-V checkpoint restore
  has needed explicit PMP table serialization, with issue #3001 tied to a
  `v25.1.0.1` hotfix. rem6 keeps PMP entries as typed raw-address/config/range
  records with snapshot and decode-first restore. RISC-V cores now own a
  16-entry PMP table by default, and core checkpoint ports encode an explicit
  `pmp` chunk with entry-count and range prevalidation before mutating live
  state. RISC-V fetch and data issue also consult the core PMP table before
  instruction fetches, plain data, translated data, or MMIO data requests are
  submitted, matching gem5's TLB/PMP placement while keeping denials as typed
  CPU errors.
- RISC-V PMA anchor refreshed on 2026-05-30: gem5's local `PMAChecker` keeps
  uncacheable ranges and misaligned-load/store support ranges beside the TLB
  path, then applies PMA after PMP rather than letting memory targets discover
  an illegal request late. rem6 now owns a typed `RiscvPmaTable` with explicit
  misaligned-support and uncacheable ranges. RISC-V plain data, translated
  data, and MMIO data issue check PMA alignment after PMP and before request
  submission, and denials remain `RiscvCpuError::DataPmaAccess` instead of
  callback panics or post-dispatch memory errors. RISC-V fetch and data memory
  requests that hit an uncacheable PMA range carry explicit
  uncacheable-plus-strict-order flags in `MemoryRequest`, so the cache,
  transport, and memory layers can observe device-like ordering through a
  typed request contract instead of relying on hidden request flags. Cache
  miss-generated downstream requests now preserve those flags, and MSHR
  entries reject coalescing whenever the existing targets or the incoming
  request are uncacheable or strict-order. That preserves gem5's useful
  no-merge rule without hiding it behind packet flags. Cache-bank uncacheable
  reads now bypass clean resident lines for MSI/MESI/MOESI/CHI, forward the
  original typed request downstream, and convert the fill response into a CPU
  response without installing the returned bytes. CHI banks also remove the
  bypassed resident way from the replacement directory, so replacement
  metadata cannot report a line that the bank no longer owns. Dirty resident
  uncacheable reads now use the same typed write-queue eviction surface when a
  queue is attached: MSI/MESI/MOESI/CHI banks enqueue a full-line dirty
  writeback, remove the resident line, retain the original uncacheable read as
  pending-fill state, and expose that read only as a post-issue downstream
  request on the writeback issue record. That gives callers one typed ordering
  point: send the dirty writeback first, then the original uncacheable read,
  and still complete the read through the no-install fill path. Bank snapshots
  retain those pending uncacheable reads with their blocking writeback handle,
  so restore keeps the same post-issue downstream-read ordering after the
  dirty writeback is issued without confusing already-issued clean bypass
  reads on the same line. Restore rejects pending uncacheable reads whose
  blocking handle no longer names a same-line dirty writeback, and also
  rejects malformed pending entries that are cacheable, do not require a
  response, belong on the write queue, target another cache bank's agent, or
  carry a mismatched line layout. Banks without a typed write queue still
  reject that path before mutating the dirty line. gem5
  also routes uncacheable writes through
  `BaseCache::allocateWriteBuffer` instead of an MSHR; rem6 cache banks with a
  typed write queue now do the same for MSI/MESI/MOESI/CHI by enqueuing the
  original write request, preserving its uncacheable-plus-strict flags,
  exposing it as a ready write-queue issue, and avoiding MSHR or line
  installation. If the target line is dirty, MSI/MESI/MOESI/CHI banks enqueue
  a full-line dirty writeback before the uncacheable write and then remove the
  resident line, so external memory observes the dirty data before the
  device-like write. Same-line cacheable reads that arrive before the queue is
  drained are satisfied from the typed write queue by overlaying later masked
  uncacheable bytes over the earlier full-line writeback bytes when present,
  while same-line writes, atomics, and later uncacheable reads are rejected as
  explicit write-queue conflicts instead of allocating a stale miss or a stale
  uncached read. When the uncacheable queue entry is issued, those
  banks retain a typed in-flight uncacheable-write record across
  snapshot/restore and match the later memory write response back into a CPU
  response without touching the fill path. Restore rejects malformed
  in-flight write entries that are cacheable, are not writes, or do not require
  a response, and applies the same agent and line-layout checks as live
  write-queue enqueue paths, so malformed checkpoint data cannot reroute reads,
  evictions, or foreign-bank writes through the write-response path. This keeps
  gem5's useful
  `handleUncacheableWriteResp` timing shape while replacing Packet
  sender-state identity with an auditable request-id map. rem6 transport now
  treats same-agent strict-order requests as full ordering edges for direct QoS
  batches and shared-fabric first-hop reservation, so uncacheable PMA traffic
  can constrain NoC priority arbitration instead of only carrying an inert flag.
  Broader response-latency modeling remains an alignment target.
- Public gem5 issue anchor refreshed on 2026-05-30: open three-level CHI
  LR/SC race where contending RISC-V mutex paths can violate lock ownership
  under Ruby CHI.
- Public gem5 issue anchor refreshed on 2026-05-30: open RISC-V Ruby CHI
  full-system multithreaded application hang where TimingSimpleCPU and O3CPU
  can accumulate repeated SC failures and RCU stalls.
- Public gem5 issue anchor refreshed on 2026-05-30: open Ruby CHI
  `Evict`/`Snp*Invalid` hazard where restore can clear sharers needed by an
  Evict acknowledgement assertion.
- Public gem5 issue anchor refreshed on 2026-05-30: open O3 data prefetch
  retirement bug where prefetches can hold LQ/ROB resources until memory
  responses arrive.
- Public gem5 issue anchor refreshed on 2026-05-30: open Indirect Memory
  Prefetcher bug where distance greater than one repeats the same indirect
  address instead of using future index values.
- Public gem5 issue anchor refreshed on 2026-05-30: open classic-cache MSHR
  deferred clean bug where a read fill can locally consume and drop a deferred
  clean request instead of forwarding it downstream after the deferred write.
- Public gem5 issue anchor refreshed on 2026-05-30: open classic-cache
  `CacheBlk` move bug where a moved block can pass a tag to an insert path that
  now expects a full address: <https://github.com/gem5/gem5/issues/2955>.
- Public gem5 issue anchor refreshed on 2026-05-30: open O3 TimeBuffer
  assertion bug where same-tick LSQ completions can overflow
  `IEW::instToCommit` future slots.
- Public gem5 issue anchor refreshed on 2026-05-30: open O3 distributed issue
  queue underutilization bug where a busy queue can block same-OpClass ready
  work in a different issue queue.
- Public gem5 issue anchor refreshed on 2026-05-30: open RISC-V unordered
  vector reduction over-serialization issue where a serialize-after first
  micro-op can force unrelated younger work behind a ROB-empty barrier.
- Public gem5 issue anchor refreshed on 2026-05-30: open RISC-V O3
  branch-predictor configuration bug where disabling both RAS and indirect
  target hashing can lead to an invalid early full-system memory address.
- Public gem5 issue anchor refreshed on 2026-05-30: open ArmO3 SE
  misc-register RAW hazard where non-always-ready misc-reg dependents can wake
  before the producer commits architectural state.
- Public gem5 issue anchor refreshed on 2026-05-30: open AArch64 TLBI ASIDE1
  bug where ASID invalidation can evict final-level global entries because the
  global bit is not part of the match predicate.
- Public gem5 issue anchor refreshed on 2026-05-30: open x86 REX prefix
  placement bug where a REX byte before a later legacy prefix can remain active
  instead of being ignored before opcode decode.
- Public gem5 issue anchor refreshed on 2026-05-30: open x86 STI/CLI IOPL
  bug where protected-mode interrupt-flag microcode can read low RFLAGS bits
  instead of the architectural IOPL field at bits 13:12.
- Public gem5 issue anchor refreshed on 2026-05-30: open O3 inter-stage
  unblock bug where empty-only skid-buffer unblocks introduce a delay bubble.
- Public gem5 issue anchor refreshed on 2026-05-30: open CHI replacement-policy
  bug where a Python-selected `RubyCache` policy can reach replacement state
  without required access metadata.
- Public gem5 issue anchor refreshed on 2026-05-30: open `AddrRange`
  refactor request for sparse physical ranges and non-power-of-two modulo
  interleaving: <https://github.com/gem5/gem5/issues/2855>.
- Public gem5 issue anchor refreshed on 2026-05-30: open Ruby checkpoint
  restore bug where warmup scheduling can try to insert an event before the
  live event queue's current tick.
- Public gem5 issue anchor refreshed on 2026-05-30: open x86 KVM-to-O3
  full-system switch bug where skipped host registers, special-register
  mismatches, and pending wait-state transfer can crash or misdirect execution
  after `m5_switch_cpu_addr`.

Implementation evidence through 2026-06-01:

- `rem6-system` has typed guest wait-status encoding for future syscall
  emulation handoff. Tests cover normal exits, signal termination, the
  core-dump bit, stopped children, continued children, and invalid signal
  rejection, so guest ABI status values cannot collapse every child result into
  a successful exit before being copied to guest memory. It also has a typed
  guest futex table for future syscall emulation handoff. Tests cover public
  gem5 issue #1320 by requiring multicore barrier waiters to remain visible
  until a wake, mismatch waits to return would-block without mutation, zero
  wait bitsets and duplicate waiters to fail without mutation, and requeue to
  preserve FIFO order while updating the waiting index. It also has a typed
  guest fd table for future syscall emulation handoff. Tests cover public gem5
  issue #2750 by requiring `dup2` to return and install the requested
  destination fd, replace an existing destination without allocating another
  fd, clear close-on-exec only on newly duplicated descriptors, and preserve
  same-fd no-op behavior after source validation.
- `rem6-memory` has typed sparse and modulo-interleaved address map regions for
  future full-system memory maps. Tests cover the gem5 issue #2855 shape by
  routing one base physical range across three modulo stripes, rejecting
  ambiguous interleave policies, excluding sparse I/O holes, packing offsets
  after holes, rejecting requests that cross holes or stripe ownership, and
  preserving interleaved mapping policy through partitioned-store snapshots.
- `rem6-timer` has an initial typed ARM Cortex-A9 CPU local timer/watchdog
  model aligned with gem5 `src/dev/arm/timer_cpulocal`: each declared CPU owns
  a local timer and watchdog register bank, MMIO dispatch selects the CPU from
  the scheduler partition instead of an untyped packet context id, serial and
  parallel scheduler paths share the same state machine, interrupt routes are
  prevalidated typed ports, and watchdog mode records reset assertions in
  snapshots rather than terminating the simulator with a fatal true-watchdog
  path. The implementation keeps gem5's load, counter, control, interrupt
  status, watchdog reset status, watchdog disable sequence, auto-reload, and
  `clock << (4 * prescalar)` timing surface while turning unknown registers,
  write-only reads, invalid CPU mappings, invalid prescalar shifts, deadline
  overflow, and stale callbacks into typed errors or generation-filtered
  no-ops. A zero load in auto-reload mode advances by at least one decrement
  tick, preventing the same-tick idle-drain loop that can emerge from gem5's
  direct zero-delay event scheduling shape. `rem6-system` also has a dedicated
  CPU local timer checkpoint bank that preserves each CPU's timer and watchdog
  snapshot through decode-first restore validation without partially mutating
  target devices. Platform-owned CPU local timers are now attached to topology
  host checkpoints automatically, so full-system checkpoint capture preserves
  CPU-local timer/watchdog state without script-side object discovery or
  object-local serialization hooks.
- `rem6-platform` can now declare ARM CPU local timer MMIO regions with explicit
  CPU partition records, timer interrupt routes, and watchdog interrupt routes.
  The builder validates each CPU partition, constructs the typed timer bank,
  registers per-CPU interrupt ports against the platform interrupt controller,
  attaches the same local MMIO wrapper as one source-partition view per CPU, and
  retains devices by base address for host integration. The MMIO bus therefore
  allows same-address device windows only when their source partitions differ,
  while preserving overlap rejection within one source partition. This keeps
  gem5's per-thread local-timer concept while avoiding packet-context and Python
  board-wiring drift: the CPU identity, MMIO route, interrupt target, and source
  id are checked data before the simulator can run.
- `rem6-platform` now keeps RISC-V device-tree modeling in a focused
  `device_tree` module and leaves the crate root as a facade over platform
  configuration, builder, topology, and error APIs. Source-policy tests enforce
  both the facade budget and the hard per-source-file size budget, so typed
  platform growth cannot quietly recreate the monolithic Python/C++ wiring
  surface that makes gem5 board descriptions hard to audit.
- `rem6-cpu` now keeps RISC-V data issue, MMIO data issue, conditional-store
  failure recording, data completion, and request materialization in a focused
  `riscv_data_issue` module while preserving the existing core, cluster, and
  translated-data APIs. CPU source-policy tests keep the crate root under the
  facade budget, require data issue to stay out of the root, and enforce the
  hard per-source-file size budget, so CPU timing breadth can grow without
  recreating gem5-style CPU monoliths.
- `rem6-memory` now keeps `MemoryRequest`, `MemoryResponse`, response status,
  atomic request payload validation, and atomic read-modify-write byte
  materialization in a focused `request` module. Memory source-policy tests keep
  the crate root under the facade budget, require request and response state to
  stay out of the root, and enforce the hard per-source-file size budget, so
  cache, DRAM, transport, and CPU integrations share one typed request contract
  without regrowing a gem5-style packet utility monolith.
- `rem6-dram` now keeps public DRAM error formatting in a focused `error`
  module and target-backed external-memory controller state in
  `memory_controller`, leaving the crate root focused on timing, command, bank,
  port, and controller scheduling primitives. DRAM source-policy tests keep the
  crate root under the facade budget, require error and memory-controller state
  to stay out of the root, and enforce the hard per-source-file size budget, so
  DDR, HBM, LPDDR, NVM, QoS, and target-backed storage growth do not collapse
  into a gem5-style memory-controller monolith.
- `rem6-kernel` parallel epoch plan tests now cover pre-dispatch
  remote-delivery windows. Each window records the source partition, target
  partition, source event tick, target partition clock, minimum lookahead-safe
  delivery tick, first target-accepted delivery tick, and epoch horizon. The
  test asserts that a source at the beginning of an epoch can have a current
  epoch remote window while a later source in the same epoch must defer remote
  delivery beyond that horizon, making cross-partition timing slack visible
  before callbacks and remote outboxes execute.
- `rem6-interrupt` now keeps generic interrupt error reporting, route
  identifiers, delivery and history records, and controller snapshots in
  focused `error`, `route`, `event`, and `snapshot` modules. Snapshot restore
  rejects missing priorities, duplicate line, priority, pending, and
  claimed-target records, references to unknown lines, pending/claimed overlap,
  and pending, claimed, or history route mismatches before controller state
  mutates, leaving the crate root centered on typed delivery channels,
  controller state machines, and the generic MMIO window. Interrupt
  source-policy tests keep the crate root under the facade budget, require those
  reusable contracts to stay out of the root, and enforce the hard
  per-source-file size budget, so generic controller
  growth does not recreate gem5-style platform interrupt wiring spread across
  callbacks, platform helpers, and device-local assertion paths.
- `rem6-coherence` now keeps harness error reporting, submit/result response
  records, and partitioned route/cache/memory configuration in focused modules,
  leaving the crate root centered on serial and partitioned MSI harness
  orchestration. Coherence source-policy tests keep the root under the facade
  budget, require those API surfaces to stay out of the root, and enforce the
  hard per-source-file size budget, so MSI, MESI, MOESI, CHI, topology, QoS,
  and DRAM-backed coherence growth cannot recreate a gem5 Ruby-style harness
  monolith. The MSI bank harness now exposes functional-read audit records with
  explicit access-permission counts, selected source identity, and optional line
  data, so Ruby-style inspection paths are typed diagnostics instead of mutable
  packet side effects.
- `rem6-cache` now keeps MSI and MESI controller state, snapshots, errors,
  pending-miss records, CPU-event decoding, and response materialization in
  focused `msi` and `mesi` modules. Cache source-policy tests keep the crate
  root under the facade budget, require those controller states to stay out of
  the root, and enforce the hard per-source-file size budget, so cache
  controller growth does not recreate gem5-style mixed protocol controller
  monoliths. The same crate now also exposes a protocol-neutral replacement
  directory for set/way line ownership, victim replacement, hit touch, resident
  lookup, and snapshot restore validation. CHI cache banks now attach that
  directory for clean-line capacity eviction, deterministic snapshot replay,
  and typed dirty-victim rejection before writeback integration, giving
  protocol banks one typed replacement boundary instead of Ruby/CHI-specific
  callback state.
- `rem6-transport` now keeps endpoint ids, route ids, route hops, route
  topology derivation, route errors, and transport QoS class state in a focused
  `route` module, and strict-FIFO message admission, bypass metadata, ready
  ordering, and snapshot state in a focused `message_buffer` module while
  leaving the crate root centered on serial, parallel, and batched delivery.
  Transport source-policy tests keep the crate root under the facade budget,
  require route and message-buffer contracts to stay out of the root, and
  enforce the hard per-source-file size budget, so NoC, QoS, Ruby-style
  message buffers, and scheduler integration growth does not recreate
  gem5-style event-queue and packet-routing monoliths.
- `rem6-gpu` now keeps GPU device/kernel/workgroup/DMA ids, wait-for markers,
  compute configuration, kernel launches, trace events, workgroup completion
  records, GPU device/slot/queued-workgroup snapshots, and GPU error reporting
  in focused `command`, `trace`, `snapshot`, and `error` modules while leaving
  the crate root centered on execution, DMA issue/completion, wait-for
  diagnostics, and parallel run summaries. GPU source-policy tests keep the
  crate root under the facade budget, require those command, trace, snapshot,
  and error contracts to stay out of the root, and enforce the hard
  per-source-file size budget, so future `src/gpu-compute` parity work can grow
  queues, compute-unit scheduling, and DMA paths without recreating gem5-style
  command-processor and pipeline monoliths.
- `rem6-accelerator` now keeps command identifiers, command kinds, wait-for
  markers, engine configuration, accelerator trace events, and accelerator
  engine/queued-command snapshots, and accelerator error reporting in focused
  `command`, `trace`, `snapshot`, and `error` modules while leaving the crate
  root centered on execution, DMA issue/completion, wait-for diagnostics, and
  parallel run summaries. Accelerator source-policy tests keep the crate root
  under the facade budget, require command, snapshot, and error contracts to
  stay out of the root, and enforce the hard per-source-file size budget, so
  GPU-kernel, NPU-inference, and DMA accelerator growth does not recreate
  gem5-style HSA/AMDGPU device-command and queue-state monoliths.
- `rem6-virtio` now keeps queue indexes, queue specs, notify specs, queue
  notifications, VirtIO error variants, error formatting, and device-error
  conversion helpers in focused `queue` and `error` modules while leaving the
  crate root centered on legacy MMIO, modern PCI common-config, and notify-MMIO state
  machines. VirtIO source-policy tests keep the crate root under the facade
  budget, require queue and error contracts to stay out of the root, and
  enforce the hard per-source-file size budget, so block, queue, rng, console,
  MMIO transport, PCI transport, shared-memory, and future network VirtIO work
  does not recreate gem5-style mixed device, queue, and transport
  monoliths. The 9P device path now keeps attach, statfs, walk, open, create,
  path, metadata, read, write, clunk, remove, xattr, directory-read, fsync,
  advisory-lock, and namespace-mutation operation handlers in focused
  operation-family modules while the main device file remains responsible for
  dispatch, transport-facing state, and session state, so future 9P parity
  work can grow without reintroducing a gem5-style proxy/device/protocol
  monolith. Modern
  VirtIO PCI common-config construction now always exposes the required
  version-1 reserved feature bit on feature page 1 so device builders cannot
  accidentally advertise modern
  PCI capabilities without the corresponding negotiation bit. VirtIO 9P now
  preserves gem5's device id 9, mount-tag feature,
  default 32-entry request queue, and little-endian `len + tag` device-config
  surface, while validating tag length and installing the config as a typed
  read-only PCI device-config region through the same modern PCI transport used
  by block, console, and RNG. The 9P request queue now decodes the
  little-endian 9P message header and payload from readable split descriptors,
  validates declared message length plus reply-descriptor direction, preserves
  request tag and type as typed data, scatters response bytes into writable
  descriptors, writes used-ring entries, raises typed ISR queue interrupts, and
  leaves the available cursor unchanged when descriptor decoding fails. The 9P
  device execution path now handles the protocol handshake messages needed
  before filesystem traffic: `Tversion` replies with bounded `Rversion`
  negotiation data, clamps client `msize` to the device limit, floors undersized
  requests to a valid `Rversion` envelope, carries the negotiated value into
  later I/O-unit replies, and keeps counted `Rread` and `Rreaddir` data replies
  inside the negotiated message budget. A parsed
  `Tversion` also resets live fid and attach state while retaining completion
  history for diagnostics, `Tauth` parses authentication requests before returning an
  explicit no-auth-backend errno, `Tattach` records no-auth attached fids plus
  user metadata, returns a deterministic root qid, rejects unsupported auth
  fids, and rejects occupied attach fids before replacing existing fid state,
  unsupported requests return typed
  `Rlerror` payloads, and malformed protocol payloads fail as typed errors
  before completion state mutates. A typed in-memory 9P namespace now covers the
  first filesystem operations after attach: `Twalk` resolves named files and
  directories into new fid state, handles `.` and `..` directory components
  with root-clamped parent traversal, returns partial qid vectors without
  binding the destination fid when later components miss, rejects occupied
  destination fids, rejects opened non-directory source fids without binding
  the destination fid, rejects overlong element vectors and path elements longer
  than the advertised `statfs` `namelen` before state mutation, rejects
  non-empty same-fid rebinding, and preserves empty same-fid walk replies,
  `Tmkdir` creates deterministic directory qids and rejects duplicate names with
  errno payloads, path-name namespace mutations reject empty names, reserved dot
  or dot-dot components, path separators, and names longer than the advertised
  `statfs` `namelen` before state mutation,
  `Tlcreate` creates named root or child-directory files, rejects occupied names
  without replacing existing nodes, rejects already-open fids before namespace
  mutation, and retargets the directory fid to the opened file,
  `Tgetattr` reports deterministic root, directory, and file metadata,
  `Tstatfs` reports deterministic namespace capacity metadata, legacy `Tstat`
  emits deterministic 9P2000 stat metadata for existing fids and rejects stale
  fids with errno payloads, legacy `Twstat` parses stat write requests, applies
  supported mode, uid, gid, mtime, atime, and length updates, supports
  same-parent name updates while preserving open fid access and walked hard-link
  fid path identity, treats same-file hard-link target names as no-ops, rejects
  stale hard-link rename fids before mutation, and rejects stale fids before
  mutation, `Tlopen` and legacy
  `Topen` mark file and allowed-mode directory fids open, report qid plus
  I/O-unit data,
  and enforce read-only, write-only, read-write, and execute-only access bits,
  reject write-mode directory opens before changing fid state,
  reject attempts to reopen already-open fids before access-mode mutation,
  `Tlopen` honors RISC-V/Linux-compatible truncate and append flag values,
  legacy `Topen` honors 9P2000 truncate, remove-on-close, and append mode bits,
  legacy `Tcreate` shares the same checked namespace creation path as
  `Tlcreate`, including duplicate-file rejection and opened-fid access-mode
  propagation plus remove-on-close and append-mode propagation, `Treaddir`
  returns stable `.`/`..` plus sorted file, symlink, or directory dirents with
  resumable byte offsets and count-bounded whole-entry replies, `Tsymlink`
  creates deterministic symlink qids, `Treadlink` returns counted symlink
  targets, `Tmknod` creates deterministic special-node qids for character and
  block device metadata with stable Linux device-number encoding, `Tsetattr`
  handles mode, uid, gid, explicit atime/mtime, and size-valid metadata paths,
  truncates or extends file data for size updates, and exposes the new metadata
  through `Tgetattr`, `Tread` returns counted byte ranges,
  `Twrite` mutates and extends byte ranges, `Tlink` creates hard links with
  shared qid identity, shared file contents, updated link counts, and unlink
  behavior that keeps surviving linked fids live, `Trename` moves non-directory
  fid-backed nodes into target directories while preserving moved qids, open
  fid access, and walked-fid path identity, and renames directory fids across
  same and different parents while updating descendant fid path identity,
  permits empty-directory target replacement, rejects non-empty directory
  targets, and rejects descendant targets, `Trenameat`
  renames and moves non-directory nodes across directory fids while preserving
  the moved file qid, open fid access, and moved hard-link fid path identity,
  treating target hard links to the same file as no-ops and replacing other
  target files with explicit target-fid invalidation, and renames directories
  across same and different parents while updating descendant fid path identity,
  permitting empty-directory target replacement, rejecting non-empty directory
  targets, and rejecting descendant targets,
  `Tunlinkat` removes named root or child-directory files and invalidates fids
  only when no linked directory entry remains, removes empty directories only
  when `AT_REMOVEDIR` is present, and rejects non-empty directory removal with
  `ENOTEMPTY`, `Tremove` removes the walked hard-link directory entry for file
  fids, including after that hard-link entry is renamed, rejects already-unlinked
  walked hard-link entries without deleting surviving names for the same qid,
  removes empty-directory fids plus their namespace entries, rejects root and
  non-empty-directory removals while clunking valid remove fids, clunks xattr
  read/write fids on remove errors without committing pending xattr writes,
  drops pending xattr-write fids for deleted backing nodes, and releases
  byte-range locks owned by the removed fid even when a surviving hard link
  keeps the namespace node live, `Tclunk`
  drops ordinary fid state, releases byte-range locks owned by the clunked fid,
  and commits pending xattr-write fids, `Tflush` acknowledges old tags without
  mutating synchronous fid or namespace state, `Tfsync` validates fids before
  acknowledging writeback intent, `Tlock` accepts advisory lock requests on open
  file fids, records byte-range read and write locks per namespace node and
  client owner, reports blocked status for incompatible overlapping locks,
  preserves unreleased byte ranges when an unlock request covers only the middle
  of an existing lock, rejects unknown lock flag bits with `EINVAL`, `Tgetlock`
  returns the first conflicting lock or a deterministic unlock payload when no
  conflict exists, rejects unknown lock flag bits with `EINVAL`, `Txattrcreate`
  converts a target fid into an xattr-write fid with bounded byte writes,
  validates non-empty xattr names against `statfs` `namelen` without treating
  slash as a path separator, honors `XATTR_CREATE` and `XATTR_REPLACE` semantics,
  rejects invalid flag combinations with `EINVAL`,
  `Tclunk` persists the value in the deterministic namespace, and `Txattrwalk`
  returns either a named xattr read fid or a sorted NUL-delimited xattr-name
  list while rejecting occupied destination fids and returning `ENODATA` for
  missing names. The 9P
  device entry point delegates
  typed request payload parsing, protocol string payload construction, and
  per-message request structs plus wire constants to a focused protocol module.
  It also delegates namespace tree state, qid encoding, readdir payload assembly,
  and fid-open state to a focused namespace module, so protocol dispatch stays
  separate from mutable filesystem state. Missing names, duplicate directory
  names, stale fids, and deleted-fid access return `Rlerror` errno payloads
  instead of panicking or depending on an external proxy. This keeps the useful
  gem5 VirtIO framing model while avoiding gem5's broad 9P proxy boundary, state-loss
  warning path, and external 9P server dependency for deterministic tests.
  Unsupported `Tsetattr`
  ctime-style mask bits are rejected as unsupported namespace metadata breadth
  rather than silently reported as modeled behavior.
  VirtIO RNG now exposes gem5's device id 4 and zero-length config
  surface, uses an explicit deterministic entropy source for reproducible
  tests and deterministic replay, decodes writable split descriptor chains into typed
  RNG requests, scatters generated bytes back to guest memory, writes used-ring
  entries, and raises typed ISR queue interrupts while rejecting readable or
  empty RNG descriptor chains as typed errors instead of panicking in descriptor
  helpers. RNG device assembly now preserves gem5's single 16-entry request
  queue default, exposes empty common-config features with one queue, creates a
  typed notify device for queue 0, and attaches common, notify, and ISR devices
  to the modern PCI BAR runtime without declaring a zero-length device-config
  capability. VirtIO console now exposes gem5's device id 3, size feature, and
  80-by-24 little-endian config surface, keeps guest-to-host terminal output
  and host-to-guest pending input as explicit typed buffers, decodes transmit
  and receive split descriptor chains with direction validation, and records
  used-ring completion plans with gem5-compatible zero-length transmit
  completions and actual receive byte counts. It can now consume console
  receive and transmit requests directly from typed guest memory, scatter
  receive bytes back to guest buffers, write used elements and indices, and
  raise typed ISR queue interrupts after completion writeback while leaving the
  available cursor unchanged when descriptor decoding fails. Console device
  assembly now builds the read-only PCI device-config bytes, exposes common-config
  feature and queue state for the receive and transmit queues, creates typed
  notify devices for both queue offsets, and attaches those config, common,
  notify, and ISR devices to the modern PCI BAR runtime used by other VirtIO
  devices. VirtIO MMIO transport now
  exposes gem5-visible magic, version,
  device/vendor id, feature-page selection, queue selection, queue size, queue
  PFN, interrupt status/ack, device-status, and device-config windows as typed
  MMIO state, records queue notifications, derives split virtqueue addresses
  from PFN/page/align state, and returns typed errors for unsupported features,
  unsupported page size or queue alignment, invalid queue sizing, unavailable
  queue notifications, short register access, and read-only register writes
  instead of gem5 panic or warn-only paths. VirtIO PCI
  common-config snapshots now expose stable byte payloads for feature maps,
  selected registers, queue state, and admin-queue fields. VirtIO PCI notify
  snapshots now expose stable byte payloads for notification history.
  VirtIO PCI device-config snapshots now expose stable byte payloads for config
  bytes, writable masks, and access history. VirtIO PCI ISR snapshots now expose
  stable byte payloads for interrupt status and event history. `rem6-system`
  now consumes common-config, notify, device-config, and ISR payloads through
  VirtIO PCI checkpoint ports and banks that stage malformed-payload rejection
  before full-system restore mutates live common registers, notification
  history, config bytes, or interrupt status.
- `rem6-storage` now owns gem5 `DiskImage`, `RawDiskImage`, and `CowDiskImage`
  level 512-byte sector contracts in a dedicated crate. Raw images validate
  sector-multiple capacity, read-only writes, range checks, flush accounting,
  and snapshot restore through typed errors; COW images support nested layers,
  sorted dirty-sector snapshots, explicit writeback, and child-preserving
  overlay reads. File-backed images expose explicit read-write or read-only
  open modes, typed host-file IO errors, sector-shape validation before use,
  explicit flush accounting through `sync_all`, and byte-exact snapshots that
  restore host-file contents through typed capacity validation instead of hidden
  exit callbacks. `rem6-virtio` can now execute block requests directly against
  those storage layers, so VirtIO, IDE, simple-disk, and future storage
  controllers can share one typed image substrate instead of each embedding a
  private sector backend. VirtIO block assembly now preserves gem5's single
  128-entry request queue default, exposes optional multiqueue common-config
  and notify layouts from the typed block configuration, builds PCI
  device-config, common, notify, and ISR devices from the block device, and
  attaches them through the same modern PCI BAR runtime used by other VirtIO
  devices. Storage checkpoint banks now encode raw, file-backed, and COW image
  snapshots as deterministic chunks, prevalidate every chunk before restoring
  any image, and reject malformed payloads without partial mutation.
  The storage crate also owns a typed `SimpleDisk` copy primitive aligned with
  gem5 `src/dev/storage/simple_disk.*`: reads stage complete 512-byte-sector
  payloads before writing guest memory, writes stage complete guest payloads
  before mutating storage, transfer records preserve guest address, start
  sector, sector count, and byte count, byte counts must be nonzero sector
  multiples, storage range is checked before any guest-memory side effect, and
  gem5's unimplemented write path is replaced with typed read-only, range,
  guest-address, and guest-memory errors.
  A typed `IdeDisk` PIO core now covers the gem5 `src/dev/storage/ide_disk.*`
  task-file offsets, status and error bits, software reset, ATA identify data,
  read-native-max register update, LBA-only PIO read and write commands, sector
  count zero as 256 sectors, interrupt-pending visibility, and storage-backed
  sector transfers. The rem6 path prevalidates CHS rejection and storage ranges
  before setting busy state, stages PIO writes until the full payload has been
  received, and returns typed unsupported-command, register-offset,
  data-direction, and storage errors instead of gem5 panic paths or raw
  `SimObject` state mutation. IDE disk snapshots now preserve task-file,
  status, control, pending-interrupt, explicit PIO transfer payload and cursor
  state, pending timed media commands, and active DMA requests, so delayed
  commands, partial reads, writes, and started DMA transfers restore without
  relying on hidden chunk-generator or event-local state.
  A typed `IdeController` core now covers gem5 `src/dev/storage/ide_ctrl.*`
  channel device selection, command and control register forwarding, absent
  selected-device reads as zero without panic, shared interrupt visibility
  across primary and secondary channels, BMI command and status registers, BMI
  interrupt write-one-to-clear behavior, PRD table alignment masking, and typed
  DMA readiness and direction errors. It also has typed BAR dispatch policy for gem5's
  primary command, primary control, secondary command, secondary control, and
  bus-master BAR windows, including `io_shift`, control-offset adjustment,
  command data-port word transfers, BMI primary/secondary window splitting,
  and bus-master-disabled write ignores. IDE DMA execution now decodes PRD
  tables into checked transfer plans, prevalidates guest-memory reads and
  writes before mutating disk or guest state, executes READ DMA and WRITE DMA
  against the shared storage image substrate, and updates BMI active,
  interrupt, and command bits through typed state transitions. IDE controller
  snapshots now preserve channel identity, selected device, pending interrupt,
  BMI command/status/PRD state, and attached disk snapshots with decode-first
  shape checks before live mutation. IDE controller checkpoint banks now encode
  those snapshots as deterministic chunks, validate all controller chunks with
  clone-restore preflight, and reject malformed chunks or repaired BMI
  command/status/PRD snapshots without partial live controller mutation. `rem6-system` host checkpoint actions now stage those
  controller banks with the rest of the system, validate malformed IDE chunks
  before any live restore, and let topology systems register IDE checkpoint
  ports before or after host-controller creation. This replaces gem5's
  callback-heavy IDE DMA event chain and fragile object-local
  serialize/unserialize pattern with explicit transfer plans, guest-memory
  boundaries, register records, and decode-first checkpoint chunks. IDE media
  timing now uses a typed timing port: media commands enter BSY immediately,
  retain the pending command as snapshot state, complete on the owning
  scheduler partition after the declared delay, and only then expose DRQ,
  transfer payloads, DMA readiness, and optional PCI INTx delivery. Timed PIO
  reads and writes also preserve the sector cursor across a typed inter-sector
  delay: after a sector boundary the disk returns to BSY with DRQ hidden, the
  next sector becomes visible only after the scheduler-owned media delay, and
  direct data-port reads or writes during the gap are rejected instead of
  consuming buffered bytes early. Timed PIO writes commit the completed sector
  before entering the inter-sector delay, so the storage image reflects the
  same sector granularity as gem5's `writeDisk(curSector++, dataBuffer)` path
  without exposing the next DRQ early. This replaces gem5's immediate
  `updateState(ACT_DATA_READY)` paths around its "scheduled event" TODOs with
  an explicit partition-local event and typed completion-error capture. Timed
  DMA execution now also runs behind the same partition-local timing port:
  the DMA start request leaves BMI active while guest memory or disk contents
  remain unchanged until the declared disk delay plus sector-count delay has
  elapsed. The port validates the DMA request, PRD table, and guest access
  before scheduling the event, then executes the PRD-described transfer and
  records any completion error in typed storage state instead of gem5's
  callback-owned wait events.
  `rem6-system` host checkpoint actions can attach storage image and IDE
  controller banks, stage their chunk capture with the rest of the system, and
  restore storage state only after decode-first validation has accepted every
  attached bank. RISC-V topology systems can now register storage image and IDE
  controller checkpoint ports before or after host-controller creation, and the
  topology layer attaches them to the host checkpoint executor automatically.
  IDE PCI endpoint specs now preserve the gem5 PIIX4 identity, PCI class and
  programming-interface bytes, initial status, INTA line, five I/O BAR windows,
  and the explicit `io_shift`/control-offset dispatch policy used by the
  controller BAR paths. That keeps board identity and guest enumeration data in
  typed Rust structs instead of script-side PCI parameters with implicit
  device-specific config state. IDE PCI legacy INTx ports now bind the endpoint
  function and pin to a typed PCI interrupt route, synchronize the controller's
  shared primary/secondary pending state into serial or parallel scheduler
  interrupt delivery, suppress duplicate assertions when the shared line is
  already in the requested state, and preserve delivery errors as typed
  interrupt-port evidence.
  Storage source-policy tests keep the
  crate under the facade and hard per-source-file budgets, avoiding gem5-style
  panic paths, process-exit save callbacks, and mutable SimObject disk state.
- `rem6-uart` now keeps UART ids, TX/RX byte records, interrupt-delivery error
  records, UART/PL011 snapshot records, and UART/PL011 error reporting in
  focused `event`, `snapshot`, and `error` modules while leaving the crate root
  centered on 8250-style UART and PL011 MMIO state machines, interrupt route
  prevalidation, and PrimeCell register handling. UART and PL011 checkpoint
  decode also validates the RX ledger before restore by requiring consumed bytes
  to match the injection-history prefix and pending bytes to match the remaining
  suffix. UART source-policy tests keep the crate root under the facade budget,
  require event, snapshot, and error contracts to stay out of the root, and
  enforce the hard per-source-file size budget, so broader serial, terminal, and
  PL011 platform work does not recreate gem5-style mixed device, stream, and
  object-local serialization monoliths.
- `rem6-system` workload replay now keeps data-cache line harness ownership,
  protocol-specific response routing, final-line extraction, and profiled DRAM
  line fallback in a focused `workload_replay::data_cache_backend` module.
  System source-policy tests require that backend to stay out of the replay
  root, preserving the manifest-driven full-system replay path without letting
  cache protocol wiring, DRAM backing, and workload orchestration collapse into
  one gem5-style integration file.
- `rem6-net` now keeps network error variants, formatting, and trait
  implementation in a focused `error` module while leaving the crate root
  centered on packet payloads, link timing, full-duplex delivery, and FIFO
  state. Network source-policy tests keep the crate root under the facade
  budget, require error reporting to stay out of the root, and enforce the hard
  per-source-file size budget, so broader Ethernet, distributed-link, switch,
  tap, and NIC-device work does not recreate gem5-style mixed packet,
  interface, event, and error-path monoliths.
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
- Scheduler quiescent restore now also verifies each partition snapshot id
  against its target scheduler slot before mutating clocks or event-order
  counters, rejects snapshots whose global tick is earlier than any partition
  clock, and rejects snapshots whose partition clock is earlier than the global
  tick. A malformed checkpoint cannot swap per-partition scheduler state or
  restore a scheduler-wide clock that later lets callbacks discover a
  schedule-in-past failure after partial state has already changed.
- Checkpoint restore event plans now retain original warmup and live-event
  capture order for audit, while exposing warmup events sorted by replay clock
  and live events sorted by scheduler tick, partition, and restore order. This
  keeps Ruby-style warmup replay deterministic without letting a restore-time
  event captured before a later live scheduler event force schedule-in-past
  insertion.
- The serial scheduler drain now advances idle partition clocks to the final
  tick before returning, so the debug serial view cannot leave a stale
  partition frontier that later accepts a parallel event in scheduler-global
  past time.
- Scheduler event control now exposes typed cancel and reschedule APIs keyed by
  `PartitionEventId`. Missing or already-dispatched ids return
  `EventNotPending`, rescheduling before the owning partition clock returns
  `InThePast` without removing the event, and callbacks retain identity across
  reschedule.
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
- PLIC checkpoint banks now round-trip context-local enable and threshold state
  through typed chunks and are attached to host checkpoint actions with the same
  staged capture and decode-first restore discipline as other device banks. A
  bad PLIC chunk or mismatched PLIC context map therefore fails before any live
  PLIC state is changed. Platform-owned PLIC devices are now retained as
  checkpointable topology devices, so full-system host checkpoints capture both
  the generic interrupt controller state and the PLIC-local context state
  without requiring script-side registration.
- RTC checkpoint banks now round-trip the MC146818 selected address, CMOS byte
  array, RTC core registers, and read-clear status-C interrupt flags through
  typed chunks with decode-first restore. Platform-owned RTC MMIO devices are
  retained as checkpointable topology devices, so host checkpoints can preserve
  guest-visible CMOS/RTC state without script-side registration or gem5-style
  object-local serialization hooks. RTC periodic interrupt scheduling remains
  runtime-owned and explicit, keeping pending event capture behind the scheduler
  checkpoint quiescence contract instead of hiding it inside the RTC payload.
- PL031 checkpoint banks now round-trip ARM RTC time base, last write tick,
  load value, match value, raw/masked interrupt status, tick rate, and match
  generation through dedicated typed chunks. Host checkpoint actions include
  PL031 banks in the same staged capture and decode-first restore path as other
  devices, so malformed PL031 chunks cannot partially mutate live RTC state.
  Platform-owned PL031 devices are declared through typed configs, retained as
  checkpointable topology devices, and attached to host checkpoints
  automatically instead of relying on RealView-style Python object wiring.
- SP804 checkpoint banks now round-trip both ARM dual-timer snapshots through
  dedicated typed chunks, including load, background-load, current countdown
  base, last update tick, control bits, raw/masked interrupt state, clock tick,
  and generation for each timer. Platform-owned SP804 devices are declared
  through typed configs, retained as checkpointable topology devices, and
  attached to host checkpoints automatically, so full-system checkpoint capture
  does not rely on Python object discovery or object-local serialization hooks.
- SP805 checkpoint banks now round-trip watchdog load/value base state, control
  bits, lock state, integration-test state, raw interrupt state, clock tick,
  generation, and reset-assertion records through dedicated typed chunks.
  Platform-owned SP805 watchdogs are declared through typed configs, retained
  as checkpointable topology devices, and attached to host checkpoints
  automatically, so RealView-style watchdog declarations no longer require
  script-side object discovery or object-local serialization hooks.
- Checkpoint manifests now expose typed summaries with component count, chunk
  count, total payload bytes, and per-component chunk and byte counts. This
  gives checkpoint artifacts machine-checkable coverage evidence without
  relying on logs or ad hoc manifest scans.
- Checkpoint manifest summaries now retain sorted chunk-level payload evidence
  for each component. Audit code can verify exact checkpoint chunk coverage
  from typed manifest data instead of rescanning payloads or relying on
  subsystem-specific logging.
- Workload checkpoint component summaries canonicalize duplicate chunk names by
  keeping the strongest payload evidence per name, so duplicate summary entries
  cannot inflate component chunk counts or total payload coverage.
- Workload checkpoint manifest summaries canonicalize duplicate component names
  before computing totals, merging chunk evidence and preserving stronger
  aggregate counts so repeated component entries cannot inflate manifest
  coverage.
- Workload checkpoint and checkpoint-restore labels are verified as occurrence
  counts, not just set membership, so repeated same-label host events must be
  recorded exactly and cannot hide missing or extra rollback points.
- Workload checkpoint and checkpoint-restore manifest summaries reject summary
  ticks after the replay final tick, keeping checkpoint coverage evidence
  causally inside the recorded run rather than accepting late artifacts.
- Workload checkpoint manifest summaries also require their summary tick to
  match a planned checkpoint host event with the same label, so capture
  coverage evidence cannot drift away from the actual rollback point.
- Repeated same-label checkpoint summaries consume planned checkpoint ticks as
  a multiset, so duplicate summary records cannot repeatedly claim one
  rollback point while leaving another same-label checkpoint uncovered.
- Checkpoint-restore manifest summaries now require their summary tick to
  match and consume a planned restore host event with the same label, so
  restore coverage cannot drift away from the replayed rollback point or reuse
  one restore event while another remains uncovered.
- Workload replay results now carry checkpoint and checkpoint-restore manifest
  summaries with label, manifest tick, component count, chunk count, and total
  payload bytes, plus per-component chunk-level payload evidence, so replay
  artifacts retain checkpoint coverage evidence without rereading host action
  logs or payload blobs.
- Workload manifests can now declare minimum checkpoint and checkpoint-restore
  manifest component, chunk, and payload-byte totals. Replay-plan verification
  rejects missing or under-covered checkpoint summaries, which turns
  full-system checkpoint coverage into a checked manifest contract.
- Workload checkpoint summaries now preserve per-component chunk and
  payload-byte counts in replay results, and workload manifests can require
  minimum coverage for named checkpoint components during capture or restore.
- Workload manifests can now require exact checkpoint chunk names for named
  components during capture or restore. These requirements are normalized into
  the workload identity and verified against replay result chunk summaries, so
  a checkpoint can prove that state such as registers, pages, or device queues
  was captured instead of only proving aggregate byte counts.
- Required checkpoint chunks can also carry per-chunk minimum payload-byte
  contracts. Replay verification rejects present-but-undercovered chunks during
  capture or restore, and the minimums are hashed into the workload identity,
  so a required state block cannot be satisfied by an empty placeholder chunk.
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
- Explicit full-system wait-for window validation now preserves raw global
  kind, blocked-node, and target-node records before merged query summaries are
  built. Replay rejects duplicate global window records before clean diagnostic
  checks, so a parallel full-system run cannot inflate typed wait-for evidence
  by replaying the same global window twice.
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
  ready-partition chunks, stable planned worker-lane records, per-lane planned
  ticks, per-worker-count batch summaries, partition-set summaries, and maximum
  planned workers without executing callbacks. The
  scheduler can therefore prove planned multicore occupancy before spawning
  worker threads, avoiding gem5-style reliance on post-hoc global event-queue
  traces to infer parallelism.
- Workload planned-batch summaries now preserve scoped planned worker-lane
  records for CPU scheduler, data-cache scheduler, GPU DMA, accelerator DMA,
  combined DMA, and merged full-system views. Replay plans can require a
  particular host lane to own a particular partition for a minimum number of
  planned ticks, so pre-dispatch multicore evidence survives into workload
  artifacts instead of disappearing after kernel planning.
- System run summaries now lift CPU-scheduler and data-cache planned
  worker-lane records from kernel plans into `RiscvSystemRun` and workload
  result artifacts. The same run object can report scoped and merged
  full-system lane ownership, per-lane planned ticks, and lane/partition tick
  totals, so real workload replay results preserve pre-dispatch host-lane
  ownership instead of requiring a hand-authored summary.
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
- GPU DMA and accelerator DMA replay now carry scheduler planned worker-lane
  records from recorded read/write runs into workload summaries. Direct GPU,
  direct accelerator, and combined DMA queries can report lane, partition,
  start tick, and horizon ownership, so device-side DMA parallelism is audited
  from the scheduler's pre-dispatch plan instead of inferred from executed
  batch counters.
- Merged full-system planned worker-lane summaries now include direct GPU DMA
  and accelerator DMA lane records in addition to CPU-scheduler and data-cache
  lanes. Full-system replay evidence therefore cannot satisfy pre-dispatch
  lane-ownership contracts while hiding device-side DMA worker ownership behind
  a CPU/cache-only aggregate.
- GPU DMA and accelerator DMA replay also carry planned DMA batch timelines
  and planned worker-capacity ticks from the kernel scheduler plans. Merged
  full-system planned worker ticks, capacity ticks, idle ticks, and utilization
  ratios now include those DMA planned records, so full-system efficiency
  checks cannot be satisfied by a CPU/cache-only planned-capacity aggregate.
- Explicit planned full-system worker-capacity summaries are treated as lower
  bounds against the scoped CPU-scheduler, data-cache, GPU DMA, and accelerator
  DMA planned capacities. Replay idle-budget checks therefore keep the strongest
  available pre-dispatch capacity evidence instead of trusting a weaker global
  aggregate.
- Explicit recorded full-system worker-capacity summaries follow the same
  lower-bound rule against scoped CPU-scheduler, data-cache, GPU DMA, and
  accelerator DMA recorded capacities. Runtime utilization and idle summaries
  therefore cannot hide executed device-side worker slack behind a weaker
  global capacity counter.
- Explicit recorded full-system worker-slot summaries are also merged with
  scoped CPU-scheduler, data-cache, GPU DMA, and accelerator DMA slot summaries
  by retaining the strongest active and idle tick evidence per slot. A runtime
  full-system summary therefore cannot hide device-side worker slots merely by
  publishing a shorter global slot list.
- Explicit recorded full-system worker-count buckets retain their raw replay
  evidence separately from the normalized reporting view. Replay rejects empty
  or duplicate global worker-count buckets before merge lower-bound checks, so
  a convenient aggregate cannot silently collapse malformed full-system
  evidence.
- Explicit recorded full-system worker-count tick and tick-streak summaries
  follow the same raw evidence rule. Replay rejects empty or duplicate global
  worker-count duration summaries before tick-bucket, tick-activity,
  tick-streak, or worker-tick lower-bound checks, preventing normalized
  duration sums from hiding contradictory multicore occupancy records.
- Explicit recorded full-system partition-set and partition-streak summaries
  also retain raw replay evidence. Replay rejects empty, single-partition, or
  duplicate global partition summaries before strongest-evidence merge checks,
  so normalized partition-set aggregation cannot erase contradictory multicore
  placement evidence.
- Explicit recorded full-system batch timelines must also cover scoped runtime
  scheduler, data-cache, GPU DMA, and accelerator DMA records before reporting
  APIs use them as the full-system timeline. Runtime timeline queries therefore
  expose scoped device-side batches directly instead of hiding them until replay
  validation rejects the weak merge.
- Replay validation still audits raw explicit recorded full-system timeline
  records before applying that reporting fallback. Duplicate or malformed
  global timeline records are therefore rejected at the evidence boundary even
  when the explicit global timeline is too weak to cover scoped device records.
- Explicit planned full-system batch timelines must preserve scoped planned
  scheduler, data-cache, GPU DMA, and accelerator DMA records before replay
  accepts exact timeline or derived worker/partition contracts. A global
  pre-dispatch plan therefore cannot hide device-local planned occupancy behind
  a weaker merged timeline.
- Planned full-system worker-slot summaries retain scoped CPU-scheduler,
  data-cache, GPU DMA, and accelerator DMA planned slot evidence directly, and
  merge explicit full-system planned slot evidence only when the explicit
  timeline covers the scoped planned records. Slot-level pre-dispatch occupancy
  therefore cannot be weakened by publishing a partial global timeline with a
  larger capacity counter.
- Planned full-system worker-lane records follow the same evidence boundary:
  scoped CPU-scheduler, data-cache, GPU DMA, and accelerator DMA lane records
  remain authoritative unless the explicit full-system lane list covers those
  scoped records. A global pre-dispatch lane summary therefore cannot erase
  device-side worker ownership or shrink a scoped lane window.
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
- Explicit recorded full-system per-partition activity records also retain raw
  replay evidence separately from the normalized reporting view. Replay audits
  each global partition activity record before duplicate same-partition
  activity can be merged, so contradictory weak records cannot combine into a
  passing full-system summary.
- Workload full-system remote-traffic reporting now accepts explicit merged
  full-system remote flow and send records. A global scheduler can report
  cross-partition communication directly while same-route scoped flow evidence
  stays a lower bound instead of being added to the merged total again.
- Workload full-system remote-flow merge validation now rejects explicit
  full-system flow records that are weaker than same-route scheduler,
  data-cache, GPU DMA, or accelerator DMA evidence by send count, delivery tick
  window, or delay-bound window. A global remote-flow summary therefore cannot
  narrow away scoped cross-partition traffic while replay checks still pass from
  derived lower-bound evidence. The detailed mismatch payload is boxed behind a
  constructor, preserving typed diagnostics without making every workload
  replay `Result` carry the largest remote-flow error inline.
- Explicit recorded full-system remote-flow records now also retain raw replay
  evidence separately from the filtered reporting view. Replay rejects
  zero-send global flow records before they can be dropped by derived
  remote-flow evidence, so empty full-system traffic aggregates cannot hide
  behind scoped cross-partition traffic.
- Full-system remote-traffic consistency now includes explicit global
  remote-flow records in its aggregate-vs-exact-send audit. A merged global
  flow therefore cannot bypass consistency checks just because scoped
  scheduler flows are internally matched.
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
- Full-system progress-transition summary validation now also rejects duplicate
  explicit global transition records before clean-diagnostic livelock
  thresholds are applied. A replay run cannot satisfy global progress evidence
  by repeating the same transition record and letting subject, kind, or
  partition summaries count it as independent work.
- Full-system livelock diagnostic validation now rejects duplicate explicit
  global diagnostic records before merged diagnostic counts or transition
  evidence are checked. A global clean-diagnostics replay cannot inflate
  diagnostic evidence by repeating the same livelock record.
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
- Explicit recorded full-system frontier validation also audits each raw
  global frontier before same-partition frontiers are conservatively merged for
  reporting. Duplicate weak frontiers therefore cannot combine one record's
  earlier safe-time boundary with another record's pending-event count to pass
  replay.
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
| `src/arch` | 1187 | `rem6-isa-riscv`, future ISA crates | partial | Keep per-ISA decoding and architectural state as isolated crates. RISC-V exists with typed counters, vector state, atomic memory operations, and `mhartid` CSR reads backed by explicit hart identity; ARM, x86, Power, SPARC, MIPS, and AMDGPU ISA support need equivalent crate ownership before claiming parity. |
| `src/base` | 199 | `rem6-kernel`, `rem6-stats`, `rem6-debug`, shared crate utilities | partial | Preserve useful statistics, loader, debug, and helper concepts without a large untyped utility layer. Runtime-visible data must remain typed. Stats counter paths are now structured scope/name identities, registry-owned stat groups attach stable group ids plus self-describing group catalogs to snapshots and deltas, stat descriptions preserve checked `Info.desc`-style metadata, nested rate units remain machine-readable in snapshots, stats reset requests reject ticks earlier than the active reset window, typed stats resets and dumps preserve stable reset/dump ids plus reset/snapshot payloads in registry-owned, interleaved, workload-result, and manifest-verifiable histories with exact event-sequence contracts, per-counter reset policies distinguish zeroed, constant, and monotonic lifetime counters with typed before/after reset audits, stats registries reject late counter or group registration after the first dump/reset history record so one history stream has one stable schema, typed stats deltas reject cross-scope, group-catalog-drifting, description-drifting, reset-policy-drifting, schema-drifting, time-regressing, or value-regressing snapshots so reset ordering and stat descriptor drift cannot silently corrupt dump scope, and GDB remote packet framing is decoded as typed packets, acknowledgements, interrupts, checksum errors, payload-limit errors, and skipped-prefix records before any socket or thread-context layer can observe it. |
| `src/cpu` | 363 | `rem6-cpu`, `rem6-kernel`, `rem6-system` | partial | RISC-V cluster execution exists, RISC-V data access records expose absent memory-route metadata for MMIO accesses as typed optional state instead of panic-only accessors, CPU cluster parallel epochs retain both initial and final partition frontiers through full-system run summaries, and CPU cluster scheduler epochs plus runs expose exact remote-send records, remote-send endpoint counts, progress-free transitions, ordered batch timelines, longest parallel-window evidence, batch worker-count, duration-weighted worker-count tick, worker-tick, partition-set, and same-partition-set streak evidence from the kernel without relying on caller-local batch scans for scheduler-scope counts. RISC-V store-conditional progress now records per-CPU failure streaks, typed threshold diagnostics, success resets, snapshot/restore state, cluster-run aggregation, and full-system run aggregation for LR/SC starvation evidence instead of depending on periodic warnings. Branch prediction has typed state for several gem5-style predictors, and GShare history updates reject stale prediction records before per-thread history or counters can drift. RISC-V branch-prediction control-flow targets now cannot carry vector configuration copied from older dynamic instruction state. RISC-V O3 full-system branch-target safety now rejects the gem5 #2211-shaped combination where RAS is disabled and indirect target path hashing is also disabled. rem6-cpu also has an initial typed in-order pipeline scheduler that validates per-stage widths, advances only the oldest ready prefix, records structural stalls, keeps younger ready work blocked behind the first older stall, validates branch redirect sequence and stage ownership before recording younger pipeline flush evidence, applies cycle plans into the next in-flight pipeline state with retirement and flush removal, emits before/plan/after cycle records, compact cycle summaries, and disjoint-mergeable run summaries with overlap rejection, round-trips typed in-flight snapshots with a cycle cursor, duplicate-sequence restore rejection, and cycle-overflow rejection, and encodes external checkpoint payloads with decode-first validation of magic, version, widths, stage codes, payload size, and duplicate sequences. It also has typed O3 inter-stage unblock, dependency-release, issue-queue, and writeback-transfer policies that compensate for backward signal delay, defer commit-visible misc-register publication until architectural commit, keep distributed issue capacity keyed by queue plus OpClass, and report overfull IEW-to-commit completion sets as deferred work before any TimeBuffer-like index can exceed its declared future window. gem5 simple, checker, fuller Minor, O3, KVM-style switching, and traffic testers need typed rem6 equivalents or explicit replacement models. |
| `src/dev` | 418 | `rem6-mmio`, `rem6-uart`, `rem6-timer`, `rem6-interrupt`, `rem6-gpu`, `rem6-accelerator`, `rem6-platform`, `rem6-pci`, `rem6-virtio` | partial | UART, timer, interrupt, an initial typed RISC-V CLINT MMIO model with crate-level snapshot/restore, a typed RISC-V PLIC-compatible MMIO model for priority, pending, enable, threshold, and claim/complete windows with context-local snapshot/restore, platform device retention, and prevalidated system checkpoint capture, typed MC146818 RTC register and CMOS MMIO models with platform device retention, periodic, alarm, and update-ended interrupt flag and pulse delivery, plus host checkpoint capture, an ARM PL031 RTC MMIO model with raw/masked interrupt latching, serial/parallel interrupt delivery, platform retention, and host checkpoint capture, an ARM SP804 dual-timer core/MMIO model with dual 0x20-byte register windows, prescaled 16-bit or 32-bit countdown, raw/masked interrupt latching, one-shot and periodic reload behavior, and serial/parallel interrupt delivery, an ARM SP805 watchdog core/MMIO model with gem5-visible registers, PrimeCell ID, lock handling, raw/masked interrupt state, serial/parallel interrupt delivery, typed platform declaration, retained device enumeration, host checkpoint capture, typed reset-assertion evidence, generation-based stale-event invalidation, and typed unsupported-integration-test errors, an ARM Cortex-A9 CPU local timer/watchdog model with partition-selected serial/parallel MMIO, per-CPU typed interrupt routes, typed platform declaration, and retained device enumeration, and an ARM PL011 UART MMIO model with PrimeCell ID, typed platform declaration, retained device enumeration, and host checkpoint capture. Typed reset policy, platform/topology attachment, typed RISC-V DTS source emission, binary FDT/DTB emission, RISC-V DTB memory/A1 handoff, typed Linux `/chosen` bootargs and initrd DTB metadata, typed DTB and initrd blob installation for store-backed and DRAM-backed memory, GPU, accelerator, initial PCI endpoint, host config-space, 32-bit and 64-bit BAR, BAR MMIO, legacy INTx, MSI, MSI-X, and legacy VirtIO MMIO register transport, VirtIO RNG and console device execution, modern VirtIO PCI common-config, notify-MMIO, ISR-status, device-config, split virtqueue snapshot/restore plus prevalidated system checkpoint capture, regular capability-byte, notify capability-byte, and shared-memory capability paths exist. rem6-pci keeps its root as a facade over focused device modules, including a dedicated typed error module, so PCI breadth does not recreate gem5-style device monoliths. Storage, network, PS/2, QEMU bridge, and broader platform-specific devices remain alignment targets. |
| `src/gpu-compute` | 73 | `rem6-gpu`, `rem6-accelerator`, `rem6-transport` | partial | Preserve command queues, compute-unit scheduling, DMA, and traceability. Current rem6 GPU execution is a smaller typed model. |
| `src/kern` | 18 | `rem6-system`, `rem6-platform`, workload resources | partial | RISC-V Linux boot handoff can install initrd bytes, emit matching `/chosen` DTB metadata, place generated or resolved-resource DTBs in guest memory, and set A1 through typed system APIs. Broader Linux symbols, panic/oops hooks, guest ABI helpers, and other ISA kernels remain open. |
| `src/mem` | 682 | `rem6-memory`, `rem6-transport`, `rem6-cache`, `rem6-directory`, `rem6-coherence`, `rem6-dram`, `rem6-fabric`, protocol crates | partial | rem6 already splits protocol state, topology, NoC, DRAM, replacement state, MSHR resources, prefetch queues, stores, directory state, and coherence harnesses into typed crates. CHI-like line states, a single-line cache controller, a multi-line cache bank, an initial directory decision model, serial plus partitioned multi-cache coherence harnesses, topology-built CHI cache-directory and DRAM routes, CHI recorded run-resource summaries, workload-replay CHI data-cache attribution, manifest-verifiable data-cache run attribution, manifest-verifiable data-cache run accounting consistency, manifest-verifiable MSI/MESI/MOESI/CHI data-cache protocol run counts, manifest-verifiable fabric/DRAM/resource activity counts, manifest-verifiable per-hop, per-link, per-lane, and per-virtual-network fabric activity contracts including hop-index/link/virtual-network minima, per-link, per-lane, and per-virtual-network queue-delay budgets plus per-lane peak queue-delay, per-virtual-network lane fanout, contention budgets, required-link coverage, and per-hop, per-link, per-lane, and per-virtual-network activity-window coverage, identity-backed unique coverage merges for per-link virtual-network counts and per-virtual-network lane counts, direct topology CHI data-cache attach, direct topology store-backed and DRAM-backed CPU fetch/data line-layout derivation from addressed memory regions, per-transfer fabric hop activity with packet, hop index, link, virtual network, queue delay, and timing records, per-link fabric activity, per-lane fabric activity, per-virtual-network fabric activity, MSHR-backed cache bank QoS metadata, ready arbitration, typed downstream QoS export, cache downstream preservation of uncacheable-plus-strict request attributes, MSHR no-merge handling for uncacheable or strict-order requests, clean-resident uncacheable cache-bank read bypass without fill installation, CHI replacement-directory cleanup for bypassed resident ways, dirty-resident uncacheable read-bypass rejection that preserves dirty state when no write queue is attached, dirty-resident uncacheable-read and uncacheable-write dirty writeback ordering when a typed write queue is attached, typed write-queue routing for direct uncacheable writes without MSHR allocation, typed in-flight response matching for queued uncacheable writes, and NoC QoS scheduling consumption of same-agent uncacheable-plus-strict ordering exist; broader CHI transactions, prefetcher breadth, cache/DRAM QoS policy breadth, and Ruby-network breadth remain open. |
| `src/python` | 253 | `rem6-workload`, `rem6-platform`, future front ends | partial | Keep gem5's ease of composition while replacing Python object wiring with checked manifests and typed builders. Workload manifests now record typed Linux boot handoff intent, including DTB address, bootargs, device-tree resource identity, initrd address range, and initrd resource identity. RISC-V core fetch and data routes must originate from the declared core partition and source endpoint before replay can build a cluster. RISC-V workload replay derives each core's fetch line layout from the memory target containing the current fetch PC instead of assuming the first target or entry target, and derives replay-injected data request line layouts from the memory target containing each data access address. RISC-V data-cache backing routes must be declared explicitly and originate from the data-cache directory partition and endpoint before replay can attach an external-memory backed cache. GPU and accelerator command routes must target the declared device partition and control endpoint, and GPU and accelerator DMA routes must originate from the declared device partition and DMA endpoint. Resolved resource payloads validate required resource id, digest, device-tree kind, initrd kind, initrd byte length, and manifest identity before workload replay installs DTB and initrd bytes into guest memory. Workload suites now sort manifests by typed workload id, carry deterministic suite identities, derive replay plans from manifest identities, compute deterministic round-robin or least-loaded weighted dispatch records for a checked worker count, carry weighted estimated ticks on dispatch records, summarize planned worker load, makespan, capacity, idle ticks, speedup, and utilization before execution, build planned dispatch timelines with per-workload start/final ticks from the same weighted records, reject load summaries or timelines for dispatch records without estimates, reject planned-load expectations with mismatched suite identity or worker count, reject planned speedup or utilization that falls below declared thresholds, materialize planned timelines as execution summaries, report planned worker occupancy, wall-clock span, occupancy windows, occupancy active/idle/capacity worker ticks, worker-count tick histograms, full and underoccupied tick spans, minimum occupancy worker count, occupancy utilization, per-worker idle ticks, per-worker-slot active/idle ticks, minimum sustained-occupancy checks, minimum full-occupancy checks, and maximum underoccupied-tick checks before a run, verify planned timelines against execution expectations before a run, reject actual execution windows that drift from the planned timeline, reject zero, missing, duplicate, or unexpected dispatch weights, create dispatch-declared suite execution expectations, reject unreachable suite parallelism requirements before execution, verify execution completion records against dispatch order, manifest identity, worker assignment, and optional planned tick windows, derive suite execution summaries from dispatch plans plus per-workload results, record per-workload result start/final windows with invalid-window rejection, compute per-worker suite completion summaries, report runtime per-worker-slot active/idle ticks, report maximum simultaneous workers, enforce minimum simultaneous-worker contracts, summarize suite wall-clock span, serial completion ticks, worker capacity, idle worker ticks, speedup ratios, and utilization ratios, and reject declared speedup, utilization, worker-count occupancy, full-occupancy, or underoccupied-tick thresholds that are not met so overlap and throughput are typed evidence rather than inferred from logs. Workload-result summaries preserve typed guest-host call counts, workload replay outcomes preserve typed guest-host response payloads, and parallel summaries preserve CPU scheduler, data-cache scheduler, direct GPU DMA scheduler, direct accelerator DMA scheduler, and merged full-system remote-send records with source ticks, delivery ticks, delay, and order, remote-flow records as typed partition pairs with counts, first/last ticks, and optional min/max delay bounds plus scheduler epoch, empty-epoch, dispatch counts, exact progress-free transition records, total counts, deterministic dimension lists, per-dimension record slices, counts, tick windows, and compact summaries by kind, partition, and subject, livelock diagnostic records, subject queries, subject summaries, kind summaries with exact kind tick windows, kind-filtered subject and record queries, tick windows, and counts, merged resource and full-system deadlock diagnostics, total worker counts, scoped workload-result batch timeline records that derive batch worker-count, partition-set, and streak evidence, tick-ordered full-system batch worker-count histograms, exact batch partition-set histograms, maximum consecutive batch partition-set streaks including explicit merged full-system streaks, per-partition worker, dispatch, remote-send, and remote-receive activity, data-cache total run counts, attributed and unattributed run counts, data-cache protocol run attribution, fabric/DRAM resource activity, and per-transfer fabric hop activity; workload manifests include exact expected scheduler, data-cache scheduler, direct GPU DMA scheduler, direct accelerator DMA scheduler, or full-system remote-send records, exact expected batch timeline records, exact expected progress-free transition records, remote-flow counts, first/last tick windows, optional min/max delay bounds, endpoint sets, scope-wide minimum delay floors and maximum delay ceilings, plus minimum max-worker use, scheduler epoch and dispatch progress, maximum scheduler idle epochs, total-worker activity, multi-worker batch activity, exact batch partition-set activity, sustained same-batch partition-set streak activity, active partition counts, per-partition activity minima, minimum attributed and maximum unattributed data-cache run counts, data-cache accounting consistency, minimum data-cache protocol run counts, minimum fabric/DRAM/resource activity, minimum per-hop, per-link, per-lane, and per-virtual-network fabric activity, hop-index/link/virtual-network activity windows, per-link, per-lane, and per-virtual-network queue-delay budgets, per-lane peak queue-delay, per-virtual-network lane fanout and contention budgets, per-virtual-network required-link coverage, per-link, per-lane, and per-virtual-network activity-window coverage, clean parallel diagnostic expectations, and minimum scoped wait-for edge-kind count expectations in manifest identity, and replay plans validate wait-for, deadlock, and livelock cleanliness with dirty livelock subject evidence plus kind-specific wait-for count minimums in verification failures. Boot resources, custom guest-host calls, custom guest-host responses, workload suites, suite dispatch plans, weighted dispatch inputs, weighted dispatch load summaries, planned-load expectations, planned dispatch timelines, dispatch-declared execution expectations, execution efficiency summaries, declared suite speedup, utilization, and runtime occupancy thresholds, and dispatch-derived suite execution summaries are reproducible data rather than Python workflow side effects. |
| `src/sim` | 176 | `rem6-kernel`, `rem6-system`, `rem6-checkpoint`, `rem6-stats`, `rem6-power` | partial | Event queues, ticks, objects, exit events, power hooks, probes, checkpoints, and statistics need typed partitioned equivalents. Core scheduling, recorded initial and final parallel-epoch partition frontiers, per-partition parallel activity summaries, remote send records with explicit source tick, delivery tick, persistent source-local order, and lookahead delay carried through system and workload summaries, remote-flow records with optional min/max delay bounds, typed subsystem-local and merged full-system wait-for/deadlock diagnostics, scheduler-recorded progress-free transitions with persistent partition-local order carried into workload summaries, system-run deterministic dimension lists, per-dimension record slices, counts, tick windows, compact summaries by kind, partition, and subject, direct batch worker-count, worker-tick, partition-set, and same-partition-set streak summaries, plus livelock diagnostic subject and transition-kind queries, subject summaries, subject tick windows, transition-kind counts, and kind-window summaries, typed progress-free transition livelock diagnostic records and counts, typed scheduler checkpoint quiescence reports with pending-event tick windows and serial/parallel kind counts, checkpoint restore event plans with isolated warmup replay clocks and live-event restored-tick lower bounds, typed probe events, typed power domains with stats-derived expression inputs that reject reset-scope, time, value, group-catalog, descriptor, or description drift through typed errors, thermal-network junction nodes with explicit initial temperature and capacitance plus snapshot-retained initialization evidence, monotonic stats reset handling through host actions, typed stats dump outcomes, and checkpoints exist. |
| `src/systemc` | 3911 | future `rem6-systemc` or adapter crate | external-adapter | Preserve interoperability only through an adapter boundary. Core rem6 timing must not depend on SystemC. |
| `src/sst` | 6 | future SST adapter crate | external-adapter | Preserve co-simulation value behind a typed boundary that cannot bypass rem6 partition ownership. |
| `src/proto` | 9 | `rem6-proto`, future adapters | partial | Protobuf-like exchange must produce typed rem6 data before entering simulation. rem6-proto has typed instruction, packet, and O3 dependency trace records, validation, canonical maps, window-checked dependencies, stable identity, typed prefetch memory completion policy that marks O3 prefetch records as retire-after-issue rather than response-blocking loads, checked binary frame envelopes, length-delimited frame streams with stream magic, version, varint32 record lengths, embedded-frame validation, a resettable cursor that exposes record indexes and byte offsets, a validated stream index with kind counts, payload byte totals, identities, and byte ranges, deterministic shard plans over contiguous records, shard-local cursors that support independent out-of-order ingestion while preserving global record indexes, deterministic worker assignment plans that separate parallel shard ownership from merge order, worker-local cursors over non-contiguous assigned shards, a merge buffer that turns out-of-order worker records back into global record order, a parallel reader that hides worker poll order from output order, and a stream-bound parallel ingestion plan that derives the validated index, shard plan, worker plan, reader, threaded worker decode path, and per-worker decode summary from one stream; concrete protobuf and gzip adapters remain open. |
| `src/learning_gem5`, `src/doc`, `src/doxygen`, `src/test_objects` | 39 | docs and tests | partial | Keep useful examples as audit input, but rem6 acceptance is through Rust tests and architecture docs. |

## Configuration and Experiment Surface

| gem5 source anchor | Local files | rem6 owner | Coverage | Alignment target |
| --- | ---: | --- | --- | --- |
| `configs/common` | 25 | `rem6-platform`, `rem6-workload` | partial | Common system assembly should become typed platform builders and manifests with validation. |
| `configs/example` | 81 | `rem6-workload`, `rem6-system` tests | partial | Preserve easy examples, but every example should be reconstructable from a manifest and tested where practical. |
| `configs/ruby` | 17 | `rem6-coherence`, protocol crates, `rem6-system` | partial | Keep multi-protocol examples while avoiding a separate Ruby-like engine. |
| `configs/topologies` | 10 | `rem6-topology`, `rem6-fabric`, `rem6-transport` | partial | Topology definitions should be protocol-neutral and reusable across CPU, GPU, DMA, and accelerator traffic. |
| `configs/dram`, `configs/nvm` | 5 | `rem6-dram`, `rem6-memory` | partial | External DDR, HBM, LPDDR, and NVM profiles have typed topology, geometry, bank-group geometry, timing, burst spacing, same-bank-group burst spacing, command-window bandwidth limits, parallel port counts, topology-unit counts, scheduler bank counts, topology bank counts, bank-group capacity summaries, manifest identity, checkpoint encoding, restore-time profile validation, and activity metadata. Runtime activity profiles carry profiled-target counts and profile-derived port, topology-unit, bank, and bank-group capacity denominators separately from active counts. DRAM same-arrival QoS timing batches respect same-agent memory-ordering barriers before priority or turnaround selection, and current-direction turnaround can enforce a bounded same-direction burst while refusing to switch to an empty or ordering-blocked direction. Workload resource summaries preserve the strongest explicit DRAM target, port, or bank active-resource lower bound. NVM media timing can model separate read-media, write-media, send latency, pending-read buffers, and pending-write queue depth. Profile breadth and fuller media behavior remain open. |
| `configs/network` | 2 | `rem6-fabric`, `rem6-transport` | partial | Network configuration must map to NoC lanes, virtual networks, credits, wait-for diagnostics, per-transfer hop activity, per-link activity, per-lane activity, per-virtual-network activity, queue-delay budgets across those scopes, per-virtual-network lane fanout, contention budgets, required-link coverage, and activity-window coverage across those activity scopes. Serial-link timing is bound to an explicit `ClockDomain`, latency cycles, lane count, and either bits-per-cycle or bits-per-nanosecond lane bit-rate before tick conversion. Same-scope link and virtual-network activity-window merges preserve unique resource coverage when lane identities are known. |
| `configs/boot`, `configs/dist`, `configs/splash2`, `configs/learning_gem5`, `configs/deprecated` | 27 | `rem6-boot`, `rem6-workload`, tests | partial | Boot and benchmark examples should become manifest resources, not external scripts. Linux boot handoff manifests now make device-tree and initrd resources explicit, require matching resource definitions, validate resource kind, validate resolved payload digest and initrd size, include bootargs plus DTB/initrd placement in manifest identity, bind resolved payload sets to that manifest identity, and let replay install resolved DTB/initrd bytes without a script side effect. Deprecated examples are audit input only. |

## Detailed Module Map

### ISA and Architecture

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/arch/riscv` | `rem6-isa-riscv`, `rem6-cpu` | partial | RV64I decode and execution exist, with RV64A `LR.W`/`LR.D` and `SC.W`/`SC.D` paths that record a typed load-reservation address and size only after the read completes, submit matching store-conditionals as atomic memory requests, write architectural success or failure codes into `rd`, clear reservations on both outcomes, record local conditional failures without mutating memory, use cluster-level data-event tracking to invalidate peer reservations after completed overlapping writes, and let system data-cache response paths apply MSI, MESI, MOESI, and CHI invalidating snoops to matching core reservations before the requesting CPU response is delivered. All non-LR/SC RV64A word and doubleword AMOs now decode and execute as typed atomic memory accesses whose responses write the old memory value to `rd`, with word-width old values sign-extended to XLEN and word-width stores updating only four bytes. `MemoryRequest` carries an explicit atomic operation so the memory store and MSI, MESI, MOESI, and CHI cache-controller hit paths compute read-modify-write bytes only after capturing the old response bytes. `FENCE` and `FENCE.I` now decode as typed barrier instructions; execution records preserve fence predecessor/successor sets or instruction-cache barrier identity while advancing in-order without issuing a memory request. RV64A aq/rl flags now map to typed memory-ordering metadata on ISA memory accesses and CPU data-access records: release records a read/write fence before the access, acquire records a read/write fence after the access, and acquire-release records both. Machine `mcycle` and `minstret` counter writes are typed and observable through their user read aliases, user counter aliases reject writes, snapshot restore round-trips both counters, and hart execution can read live `cycle` and `instret` values through the CSR instruction path. RISC-V PMP tables now keep raw `pmpaddr`, typed `pmpcfg`, decoded TOR/NA4/NAPOT ranges, active-rule counts, locked-entry behavior, lowest-entry match order, default S/U denial when implemented entries are inactive, and decode-first snapshots so checkpoint restore cannot silently drop protection state or partially mutate a live table. RISC-V vector configuration is typed hart state with `vl` and `vtype` records; branch-prediction targets carry only the target PC, while explicit vector-configuration updates are the only control-flow updates that can mutate that state. RISC-V vector element images and `vcompress.vm` tail policy are also typed: compress writes only the selected prefix and then applies undisturbed or deterministic all-ones agnostic tail handling by element width, so generated micro-op ordering cannot overwrite destination tail elements before the architectural tail rule is applied. RISC-V vector micro-op expansion merges macro instruction flags with micro-op-local flags before emitting records, so serialize-after, non-speculative, delayed-commit, and future scheduling constraints cannot disappear while lowering a vector macro-op. Generic `MemoryRequest`s now carry typed before/after read-write ordering metadata, and RISC-V data issue maps aq/rl metadata into the requests submitted through serial or parallel transport. rem6-cpu has an initial typed CPU translation frontend that queues virtual fetch/load/store/atomic translation misses, can attach the typed rem6-memory TLB for immediate ASID-scoped hits, fills that TLB only when queued misses complete through the page resolver, preserves CPU memory request identity separately from translation request identity, materializes mapped physical `MemoryRequest`s, records typed translation faults, snapshots pending translation metadata plus optional TLB state, and can consume the typed rem6-memory page resolver for mapped or faulted completions without gem5-style `RequestPtr` mutation or delayed callback state hidden inside packets. rem6-memory also has a typed translation TLB with bounded capacity, ASID-keyed entries, deterministic LRU, permission rechecks on hits, hit/miss/fault/insert/eviction counters, explicit all-address-space and scoped invalidation, and snapshot restore. The page resolver can emit explicit cross-page translation segments with first-failed-segment faults, and rem6-cpu can turn queued CPU translations into per-segment translated records with sliced store payloads and masks. Segmented completion can also fill ASID-scoped TLB page entries for each mapped segment so later single-page accesses hit without re-running the page resolver. A simple RISC-V data issue path can now own an optional data translation frontend, translate a load or store effective address through the page resolver and TLB before transport submission, keep the original ISA memory access in the data event, and record the physical request address explicitly. The translated RISC-V driver path polls typed translation readiness without issuing the next fetch early; the core, cluster, system run driver, and topology-built core paths can submit translated data requests through the parallel memory transport; and the MMIO-aware system path can route translated physical data addresses to either platform MMIO or memory without using the virtual address for device selection. The plain data issue path rejects a configured translation frontend when no page map is supplied. This replaces gem5's hidden split translation callback state with typed data that future RISC-V timing paths can schedule or replay deterministically. Privileged ISA, concrete page tables, fuller CPU pipeline TLB wiring, walker memory accesses, cache and memory enforcement of AQ/RL timing barriers, fuller vector instruction execution, and richer traps are alignment targets. |
| `src/arch/x86` | `rem6-isa-x86` | partial | Initial x86 support has a typed prefix scan for long-mode and protected-mode instruction bytes. It keeps legacy prefixes, active REX, opcode map, opcode byte, and ignored REX prefixes as explicit records, covering the REX placement rule. It also has typed RFLAGS, CPL, CR4.PVI, and protected-mode `STI`/`CLI` decision records that extract IOPL from bits 13:12 and emit explicit general-protection errors or IF/VIF mutations. Fuller decode, execution, segmentation, paging, and system-register behavior remain open. |
| `src/arch/arm`, `src/arch/power`, `src/arch/sparc`, `src/arch/mips` | future ISA crates | planned | rem6 should add each ISA as a crate with isolated decode, architectural state, and tests. A shared rem6-memory contract already exposes global/non-global TLB entry scope and non-global ASID flushes for future Arm TLBI handling. |
| `src/arch/amdgpu` | `rem6-gpu`, future GPU ISA crate | planned | Current GPU model is command-level. ISA-level GPU execution remains open. |
| `src/arch/generic`, `src/arch/null`, `src/arch/isa_parser` | shared ISA traits and build tooling | planned | rem6 should prefer Rust traits and generated tables only when generated artifacts are checked and reviewable. |

### CPU Models

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/cpu/simple` | `rem6-cpu`, `rem6-system` | partial | rem6 has simple RISC-V core and cluster tests with fetch, data access, traps, and host stop. |
| `src/cpu/minor` | `rem6-cpu` in-order pipeline module | partial | rem6 now has typed Fetch1, Fetch2, Decode, Execute, and Commit stage widths with an in-order scheduler that advances only the oldest ready prefix, records the first structural stall, blocks younger ready work behind that stall, treats Commit advancement as retirement, validates resolved branch redirect sequence and stage ownership before recording target PCs plus flushed younger pipeline work, applies cycle plans into the next in-flight state by advancing destinations, retaining stalls, retiring commits, and removing flushed work, emits before/plan/after cycle records, per-cycle count summaries, and disjoint-mergeable aggregate run summaries for audit, snapshots in-flight pipeline work with a cycle cursor, restore-time duplicate sequence rejection, and checked cycle advancement, and serializes external checkpoint payloads with explicit decode validation before restore. Predictor coupling and full cycle-visible MinorCPU state remain open. Its future RISC-V prediction path must use PC-only branch targets plus explicit vector-configuration updates so predicted streams cannot inherit stale `vl` or `vtype` from copied dynamic instruction state. |
| `src/cpu/o3` | future out-of-order CPU crate plus `rem6-cpu` and `rem6-proto` contracts | planned | Needs rename, issue, reorder, load/store queue, speculation, squash, and executable typed traces. The trace contract already distinguishes prefetch records from response-blocking loads with retire-after-issue completion policy. The CPU contract already models delay-compensated inter-stage unblock decisions so decode, rename, and IEW do not have to wait for empty skid buffers before signalling upstream stages. It also models destination-register dependency release by architectural visibility, so commit-visible misc-register writes wake dependents only after commit updates architectural state while memory-dependence completion remains a writeback-only action. O3 source rename decisions now model invalid source registers as typed ready operands that skip scoreboard lookup, preventing gem5 issue #3041-shaped invalid physical-register dereferences. Distributed issue scheduling is keyed by physical issue queue plus OpClass, so a busy FU in one queue cannot hide ready same-OpClass work in another queue. IEW writeback-transfer contracts now admit ready completions only inside the declared future window and surface extra completions as deferred work, so a future O3 implementation does not inherit gem5's `instToCommit` TimeBuffer assertion path. |
| `src/cpu/pred` | `rem6-cpu` branch predictor modules | partial | A local two-bit predictor, GShare predictor, BiMode predictor, Tournament predictor, loop predictor, TAGE base predictor, LTAGE predictor, TAGE-SC-L wrapper, standalone multiperspective perceptron predictor, 8KB statistical corrector, branch target buffer, indirect target predictor, and return-address stack have independent typed prediction, lookup, update, target, replacement, speculative history, commit, repair, and snapshot state. GShare keeps gem5's PC xor GHR indexing while replacing opaque history pointers with typed records, per-CPU GHR snapshots, stale-history update rejection, masked squash repair, and restore validation. BiMode keeps gem5's PC-indexed choice table and PC xor GHR direction tables while exposing selected-array training, choice-counter policy, and stale-history update rejection as typed records. Tournament keeps gem5's shared local history table, per-CPU global history, global-history-indexed choice table, disagreement-only choice training, stale-history update rejection, and squash repair while exposing each record as typed state. The loop predictor keeps gem5's set/way indexing, tag matching, confidence threshold, use counter, and optional speculative iteration state while replacing random allocation with deterministic per-set cursors for replayable parallel runs. TAGE base keeps gem5's bimodal table, tagged-table provider selection, folded-history index and tag hashing, alt-on-new counter, useful-bit reset, repairable speculative history, stale-history update rejection, and deterministic allocation records. LTAGE composes TAGE and loop prediction with explicit final provider records, prevalidated loop-before-TAGE conditional training, combined repair, matching thread and instruction-shift validation, and nested snapshot restore. TAGE-SC-L composes LTAGE with the statistical corrector in gem5 order: TAGE and loop predict before SC override, and SC trains before loop and TAGE updates after nested stale-history prevalidation, with explicit repair and nested snapshot records. The standalone multiperspective perceptron keeps gem5's 8KB feature profile shape, transfer tables, filter behavior, low-confidence best-table path, adaptive training threshold, and local/global/path/IMLI/recency histories while making all table and history state typed and per CPU. The 8KB statistical corrector keeps gem5's bias tables, global/backward/local/IMLI GEHL sums, confidence chooser, threshold training, and repairable histories while making histories per CPU for parallel replay instead of hidden shared global state. The indirect target predictor replaces opaque history pointers and random target replacement with typed records, per-CPU history, deterministic LRU, and restore validation. A typed branch-target safety profile binds RISC-V O3 full-system return prediction to at least one return-target stabilizer, either RAS or indirect target hashing, before a full-system run can be constructed. The branch target buffer rejects gem5 issue #3188-sized default configurations before allocation, while explicit limit overrides keep larger experiments reviewable. The specialized 8KB/64KB TAGE-SC-L table geometry, 64KB statistical corrector extensions, and MPP_TAGE integration remain open. |
| `src/cpu/checker` | `rem6-cpu` verification harness | planned | Checker behavior should compare architectural commits without hidden simulator state. |
| `src/cpu/kvm` | host-controlled execution modes | partial | rem6 models execution modes and statistics scope, and host-assisted switch admission now requires complete architecture state, matching memory mode, no unsupported host registers, and no pending host service before a detailed or timing target can take over. Native host-assisted execution is not present yet. |
| `src/cpu/testers`, `src/cpu/trace`, `src/cpu/probes` | tests, trace, stats crates | partial | Traffic generation, trace replay, and probes should feed typed events and summaries. |

### Memory, Cache, Coherence, and NoC

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/mem/cache` | `rem6-cache`, `rem6-coherence`, protocol crates | partial | MSI, MESI, MOESI, and an initial CHI-like state machine, cache controller, cache bank, directory model, and serial plus partitioned multi-cache coherence harnesses exist with tests. The CHI controller can issue ReadShared, ReadUnique, and MakeReadUnique-shaped downstream requests, complete shared and unique fills, preserve pending-miss snapshots, service local slices, and apply snoop downgrade or invalidation to resident data. The CHI cache bank owns multiple line controllers, assigns unique downstream request IDs across lines, records pending fills, can attach typed MSHRs, coalesces same-line read misses without extra downstream traffic, fans coalesced fills out as multiple target outcomes, and restores pending MSHR targets from snapshots. The CHI directory tracks unique clean/dirty ownership, shared clean/dirty sharers, deterministic SnoopShared and SnoopUnique decisions, owner-cache versus backing-memory data sources, MakeReadUnique no-data upgrades, dirty-peer data sourcing, sorted snapshot restore, and typed Evict-hazard restore records that retain the pending Evict requester and pre-hazard line state separately from the current post-snoop sharer set. The CHI serial harness connects multiple cache controllers to the directory, applies directory snoops before fills, transfers owner-cache data to requesters, updates backing data when a unique dirty owner downgrades to shared clean, records CPU responses and directory decisions, and snapshots directory, cache, backing, response, and decision state. The CHI partitioned harness routes request and response work through `MemoryTransport` and `PartitionedScheduler`, waits for owner snoops before peer fills, preserves dirty owner data when downgrading to shared clean, records directory decisions and CPU responses with scheduler ticks, restores quiescent scheduler, directory, cache, backing, trace, response, and decision state, exposes recorded run summaries, source-target remote-flow records, and parallel run history, and can derive cache-directory and directory-DRAM routes from typed topology components including multi-hop and fabric-backed links. Workload replay and direct topology systems can select CHI as a typed data-cache protocol, route RISC-V data loads through the partitioned CHI harness, merge CHI cache resource activity and remote-flow records back into full-system run summaries, attribute recorded CHI data-cache runs into `RiscvSystemRun` and `WorkloadParallelExecutionSummary`, and require replay-verifiable data-cache protocol run counts for MSI, MESI, MOESI, and CHI workloads. Direct topology MSI, MESI, MOESI, and CHI data-cache responses share a protocol-neutral response harness for request submission, response extraction, run recording, and response-delay conversion while keeping per-protocol snoop invalidation and diagnostics explicit. LRU, FIFO, MRU, LFU, BRRIP, BIP, SHIP, SecondChance, and TreePLRU replacement policies have typed per-set state, victim decisions, invalidation, reset, touch, access-signature training, and snapshot restore. A protocol-neutral replacement directory binds those per-set policies to cache line residency with explicit set/way ownership, deterministic victim replacement, hit touch, resident-line lookup, typed resident-line relocation, and snapshot restore validation, so CHI, MSI, MESI, and MOESI cache banks can share replacement state without moving policy updates into protocol-specific generated behavior. MSHR queues have typed entry allocation, target coalescing, prefetch reserve, ready/service state, snapshot restore, optional per-target QoS class metadata, effective QoS derived from merged targets, QoS-aware ready ordering, typed QoS profiles for target counts, effective-entry counts, requestors, and priorities, and typed conversion of effective cache QoS into downstream transport QoS classes. A typed cache write queue keeps gem5's writeback, write-clean, clean-evict, uncacheable-write, effective-capacity plus reserve, ready tick ordering, pending-conflict, functional-read satisfaction, direct mark-in-service release, and snapshot semantics while replacing Packet pointers, sender-state inheritance, and list iterators with stable handles and explicit request records. Replacement decisions can enqueue typed dirty writebacks, clean evicts, optional clean writebacks, or no entry for invalid victims, with explicit victim-way validation before the queue mutates. The MSI, MESI, MOESI, and CHI cache banks can own an optional typed write queue, enqueue writebacks or uncacheable writes, expose ready handles and conflict queries, issue ready entries as typed downstream requests, reject foreign line layouts, and restore queued entries through bank snapshots. MSI, MESI, MOESI, and CHI cache banks can accept CPU requests with explicit MSHR QoS metadata, expose effective QoS for pending lines, expose live and snapshot MSHR QoS profiles, coalesce same-line read misses without extra downstream traffic, preserve merged QoS through snapshot restore, and fan coalesced fills out as multiple target outcomes. The MSI bank directory harness can submit coalesced and parallel-cycle CPU requests with explicit MSHR QoS, record each target's effective MSHR QoS before fill service, export scheduled misses as typed downstream transport requests that carry effective MSHR QoS into `ParallelMemoryTransaction`, preserve MSHR queue configuration plus target QoS metadata in byte snapshots, aggregate pending cache-bank MSHR QoS profiles across harness snapshots and restored harness state, expose per-cycle effective MSHR QoS diagnostics on recorded parallel runs, and summarize effective QoS by requestor and priority in parallel-cycle history. Typed stride, tagged, DCPT, BOP, SBOOE, SignaturePath, SMS, FDP, PIF, ISB, STeMS, IMP, and initial AMPM access-map prefetchers have deterministic candidate generation, per-request metadata, source addresses, and snapshot restore. The DCPT model keeps gem5's per-PC delta history, signed delta overflow-to-zero rule, masked two-delta partial matching, earliest historical pair scan, and post-match delta replay while adding optional requestor isolation and explicit typed snapshots for parallel replay. The BOP model keeps gem5's best-offset learning over generated smooth offsets, left and right recent-request tables, score and round thresholds, enable-disable policy, optional delayed RR insertion, delay queue capacity/drop behavior, prefetch-fill training hook, and degree candidate generation while making the RR tables, scores, delay queue entries, selected offset, phase state, and last candidates explicit typed snapshot state. The SBOOE model keeps gem5's sandboxed sequential stride candidates, FIFO sandbox entries, score minus late-score policy, score threshold percentage, demand-fill latency tracking, and average latency feedback while making sandbox state, latency buffers, pending demand fills, selected sandbox, and last candidates explicit typed snapshot state. The SignaturePath model keeps gem5's page signature table, shifted xor signature update, pattern table stride counters, low-counter stride replacement with counter aging, confidence-gated prefetches, 0.95 lookahead cap, next-line auxiliary fallback, and page-crossing address generation while replacing opaque cache entries with explicit deterministic LRU and typed snapshots. The SMS model keeps gem5's filter table, active generation table, eviction-committed pattern history table, region offsets, FIFO filter capacity, active LRU capacity, pattern LRU capacity, and trigger-PC plus trigger-offset lookup while avoiding hidden map default insertion and exposing typed snapshots. The AMPM model keeps gem5's previous/current/next hot-zone window, positive and negative stride checks, `s2+1` early match rule, prefetch/useful/raw hit/miss counters, epoch degree adjustment, and candidate marking while making table replacement, integer threshold comparisons, epoch reports, and snapshots explicit typed state. A typed queued prefetch resource models gem5's queued prefetch latency, duplicate filtering with higher-priority duplicate updates, same-line demand squash, page-boundary dropping when no translation path is configured, in-cache or in-miss-queue redundant filtering, optional lowest-priority oldest eviction when full, next-ready-tick visibility, and accuracy throttle state with control percentage, issued/useful counters, max-permitted computation, useful-count invariant checks, and snapshot restore. The queued resource applies that throttle through an explicit enqueue path shared by typed prefetch candidates with ready-tick ordering, same-tick priority ordering, stable order ties, explicit capacity, line size, optional page size, issue width, accepted/duplicate/priority-update/redundant/page-crossing/throttled/full result counts, and full policy before packet creation or cache-controller side effects. A typed multi queued prefetcher preserves gem5's `Multi` earliest-ready query and round-robin source issue behavior while exposing source identity, keeping no-op polls side-effect free, and issuing only one entry from the chosen source. The FDP model keeps gem5's FTQ range expansion, PFQ and translation queue duplicate filtering, fetch-target squash policy, translation success/failure handling, uncacheable and cache-snoop drops, ready latency, issue ordering, queue counters, and snapshot restore while replacing raw CPU, MMU, cache, and packet pointers with explicit typed events. The PIF model keeps gem5's retired-PC training, spatial and temporal compactor records, history buffer, trigger index, stream address buffer continuation, secure-bit lookup behavior, and snapshot restore while replacing probe listeners, cache iterators, and replacement-policy callbacks with explicit typed events and stable history IDs. The ISB model keeps gem5's PC-indexed training unit, physical-to-structural and structural-to-physical address mapping caches, confidence counter update and reassignment policy, chunk-based structural address allocation, secure-bit separation, degree-limited successor prediction, deterministic LRU capacity, and snapshot restore while replacing AssociativeCache entries and raw queue output with typed records. The STeMS model keeps gem5's active generation table, pattern sequence table, region miss order buffer, trigger deltas, confidence-gated sequence reconstruction, duplicate RMOB policy, and cache-residency generation ending while replacing CacheAccessor callbacks and implicit replacement state with typed residency probes, deterministic LRU, secure-bit separation, snapshots, and line-sized reconstruction addresses. The IMP model keeps gem5's prefetch table, indirect-pattern detector, stream fallback, base-plus-index-shift matching, confidence counter, and secure-bit separation while replacing raw PT-entry pointer tags with stable typed keys, handling negative shifts through checked arithmetic, exposing explicit typed index-read events plus future-index lookahead for multi-distance indirect prefetches, deterministic LRU capacity, and snapshot restore. rem6-memory now has a typed translation queue with explicit request IDs, access kinds, bounded capacity, latency-derived ready ticks, deterministic ready ordering, duplicate detection, mapped or faulted completion records, snapshot restore, a typed page translation map with page-size validation, aligned virtual-to-physical mappings, overlap rejection, permission checks, page faults, explicit cross-page segment resolution, and snapshot restore, and a typed translation TLB with ASID-keyed deterministic LRU, permission rechecks, fault accounting, bounded inserts, scoped invalidation, evictions, cross-page segment fill into scoped page entries, and snapshot restore; rem6-cpu can bridge queued virtual fetch/load/store translation misses or TLB hits into typed mapped physical memory requests, issue a simple RISC-V data access through translated physical transport addresses on serial or parallel memory transport while preserving the virtual ISA access record, per-segment translated records, or fault records, while full RISC-V core/MMU pipeline integration, fuller cache/DRAM QoS policy integration, and richer cache tags remain open. |
| `src/mem/ruby` | `rem6-coherence`, `rem6-directory`, `rem6-fabric` | partial | rem6 keeps detailed coherence and NoC behavior without a second memory-stack vocabulary. |
| `src/mem/slicc` | protocol crates and typed transition records | partial | rem6 should preserve protocol expressiveness while avoiding generated controllers that hide transient behavior. MOESI transition-resource contracts now make SLICC-style resource lifetime effects explicit typed data instead of hidden action-list side effects. |
| `src/mem/protocol` | `rem6-protocol-msi`, `rem6-protocol-mesi`, `rem6-protocol-moesi`, `rem6-protocol-chi` | partial | MSI, MESI, and MOESI exist. rem6-protocol-moesi now includes typed protocol state, event, resource, and transition-resource validation records that reject duplicate transition keys, missing busy-state resource allocation, missing resource release, and release without ownership before a generated or hand-written transition can execute. The CHI-like crate covers typed `I`, shared clean/dirty, unique clean/dirty, ReadShared, ReadUnique, MakeReadUnique upgrade, snoop downgrade, invalidation, busy rejection, transition trace, directory unique-owner validation, and a line-global LR/SC reservation table that serializes overlapping store-conditionals and records coherence invalidations before three-level CHI races can leave stale per-cache monitors. Full CHI request, response, data, DVM, retry, credit, and Ruby-network interactions remain open. |
| `src/mem/qos` | `rem6-fabric`, `rem6-dram`, `rem6-transport`, `rem6-workload` | partial | rem6-fabric has typed QoS requestor IDs, checked priorities, fixed-priority assignment, FIFO/LIFO/LRG queue arbitration, non-mutating empty polls, queue-arbiter snapshots, and QoS-ordered fabric batch transmission that reserves shared links in grant order. rem6-transport can attach a shared QoS arbiter to parallel batch submission so request priority and requestor identity affect first-hop NoC reservation before partition events are scheduled, can order single- and multi-hop direct same-tick target deliveries with the same typed arbiter before invoking target handlers, respects same-agent memory-ordering barriers and same-agent strict-order requests when direct QoS batches or shared-fabric first-hop reservations choose eligible requests, exposes a typed `TransportQosClass`, and lets cache-originated transactions override QoS requestor separately from the downstream request's cache-agent identity. rem6-coherence can now export MSI bank scheduled misses as typed downstream transport requests, preserving effective MSHR QoS through `TransportQosClass` so same-tick cache-originated memory requests can be batched and ordered by transport QoS without Packet sender-state inheritance. rem6-dram can order same-arrival timing batches through the same typed arbiter before bank, row, and bus timing are computed, filters same-agent acquire/release memory-ordering barriers before QoS priority or turnaround selection, prefers the current read/write bus direction among same-priority candidates, explicitly escalates queued same-requestor candidates to their best assigned batch priority without embedding controller back pointers in the queue policy, accepts memory-controller QoS batches before storage responses are generated, pairs responses with scheduled DRAM grant order, and preserves assigned priority, effective priority, requestor, byte count, and escalation status as typed DRAM activity metadata. Parallel coherence, system, DMA, and workload-result summaries expose DRAM QoS access, byte, escalation, priority, and requestor diagnostics directly from typed activity profiles. Workload manifests declare fixed-priority QoS policy, queue policy, turnaround policy, priority escalation, and per-requestor priority intent as typed replay-plan state; workload replay applies declared fixed-priority and queue policy to shared fabric first-hop reservation, applies declared fixed-priority, queue, turnaround, and escalation policy to direct profiled DRAM accesses so replay summaries carry DRAM priority and requestor metadata, lets same-tick single- and multi-hop direct DRAM deliveries observe manifest QoS before target handling, coalesces same-tick direct QoS deliveries to the same profiled DRAM target into one memory-controller batch, and keeps that batch path active when a data-cache exists by operation-filtering cache-covered data deliveries before batching the remaining DRAM requests. This preserves gem5's fixed-priority, queue-policy, turnaround, escalation, and bandwidth-accounting concepts while avoiding global requestor lookup, memory-controller back pointers, SimObject-name-only setup, and string-only stats. Broader cache/DRAM QoS policy integration remains open. |
| `src/mem/probes` | `rem6-stats`, runtime summaries | partial | Observability should be typed counters, typed probe points/listeners/events, and run summaries, not string-only probes. |
| memory ports, packets, requests in `src/mem` root | `rem6-transport`, `rem6-memory` | partial | Shared request/response transport exists; more gem5 packet semantics need mapping as features are added. |

### DRAM and External Memory

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `configs/dram`, `ext/drampower`, `ext/dramsim2`, `ext/dramsim3`, `ext/dramsys` | `rem6-dram`, adapter crates | partial | rem6 has internal DRAM timing, burst spacing, same-bank-group burst spacing, command-window bandwidth limits, bank-group geometry, activity, and profiles. DRAM snapshot restore rejects profile target, line-layout, geometry, timing, parallel-port, or NVM media-timing drift before rebuilt controller state can expose stale profile evidence, while boxing the large mismatch payload so rich diagnostics do not bloat every `Result` error path. External DRAM simulators should be optional adapters. |
| `configs/nvm`, `src/mem/NVMInterface.py`, `src/mem/nvm_interface.*`, memory profile code | `rem6-memory`, `rem6-dram` | partial | NVM targets have typed controller/media-bank topology and can round-trip through manifests, checkpoints, and DRAM target activity metadata. DRAM activity profiles preserve typed read/write byte counts, and NVM target activity exposes persistent write access, byte counters, max pending NVM reads, max pending persistent writes, profile-level media timing, access-level persistent-ready cycles, checkpointed pending read/write completions, NVM read-buffer/write-queue wait-for diagnostics, manifest identity for NVM media timing, and restore-time rejection of profile media-timing drift without string stats. Richer NVM-specific bandwidth behavior remains open. |
| HBM, LPDDR, DDR class profiles | `rem6-dram` | partial | The profile shape exists for DDR, HBM, LPDDR, and NVM, and checkpoint restore validates that profile metadata still matches the target, memory layout, controller geometry, timing, and parallel-port shape. A broader library of validated profiles is still needed. |

### Heterogeneous Devices

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `src/gpu-compute` | `rem6-gpu` | partial | rem6 has GPU command submission, workgroup completion, DMA, traces, summaries, checkpoints with typed pending-DMA request metadata, snapshot restore slot-count validation, DMA write requests that inherit copy read-request ordering at a coarse level, and workload-result GPU DMA scheduler batch, exact timeline, initial/final frontier, remote-send, remote-flow, worker-count, max-worker, total-worker, worker-tick, partition-set, same-partition-set streak, and per-partition activity evidence captured from recorded read/write scheduler runs, exposed through dedicated batch-timeline, batch-worker, batch-partition, and scheduler-frontier manifest scopes including direct max-worker, total-worker, and batch-activity contracts, and included in full-system scheduler, remote-traffic, and partition aggregates. |
| `src/dev/amdgpu`, `src/dev/hsa` | `rem6-gpu`, future GPU ISA and runtime modules | planned | Full GPU system support needs richer queues, address spaces, interrupts, and ISA-visible state. |
| NPU-style accelerators, not a single gem5 subtree | `rem6-accelerator` | partial | rem6 already models accelerator engines, command lanes, NPU inference commands, DMA, summaries, checkpoints with typed pending-DMA request metadata, snapshot restore lane-count validation, DMA write requests that inherit copy read-request ordering, and workload-result accelerator command/completion kind counts for GPU-kernel, NPU-inference, and DMA-command work. Accelerator DMA scheduler batch, exact timeline, initial/final frontier, remote-send, remote-flow, worker-count, max-worker, total-worker, worker-tick, partition-set, same-partition-set streak, and per-partition activity evidence is captured from recorded read/write scheduler runs, exposed through dedicated batch-timeline, batch-worker, batch-partition, and scheduler-frontier manifest scopes including direct max-worker, total-worker, and batch-activity contracts, and included in full-system scheduler, remote-traffic, and partition aggregates. |
| `src/dev/storage/simple_disk.*` | `rem6-storage` | partial | rem6 keeps gem5's console-facing sector copy value while replacing raw `System::physProxy` writes, heap-buffer lifetime management, panic-only validation, and the unimplemented write path with `SimpleDisk`, `SimpleDiskGuestMemory`, typed transfer records, sector-shape validation, storage-range preflight, complete-payload staging, explicit flush, and typed guest-memory or storage errors. IDE controller integration remains open. |
| `src/dev/storage/ide_disk.*` | `rem6-storage` | partial | rem6 now has a typed IDE disk core with gem5 task-file offsets, status/error bits, identify payload geometry/capacity/model fields, read-native-max, software reset, LBA PIO read/write, sector-count-zero handling, staged write payloads, DMA request state, timed media-command state, snapshot restore, and typed errors for CHS access, unsupported commands, invalid offsets, wrong data direction, and storage failures. Fuller media timing policy remains open. |
| `src/dev/storage/ide_ctrl.*` | `rem6-storage`, `rem6-system`, `rem6-pci` | partial | rem6 now has a typed IDE controller core with primary and secondary channels, device0/device1 selection, command and control forwarding into selected disks, missing selected-device no-op writes and zero reads instead of panic, shared interrupt state, BMI status capability bits, BMI interrupt write-one-to-clear behavior, PRD table low-bit masking, DMA PRD execution with checked guest-memory transfer plans, timed media-command scheduling, deterministic snapshot/restore, controller checkpoint banks with decode-first BMI snapshot validation, topology/system host checkpoint registration, PCI endpoint identity, BAR layout specs, legacy INTx ports, and typed BAR dispatch for primary command, primary control, secondary command, secondary control, and bus-master windows with `io_shift`, control-offset adjustment, BMI primary/secondary window splitting, and bus-master-disabled write ignores. Config-space timing registers and broader board integration remain open. |
| `src/dev/pci`, `src/dev/virtio`, `src/dev/storage`, `src/dev/net` | `rem6-pci`, `rem6-virtio`, `rem6-storage`, `rem6-net`, future device crates | partial | rem6-pci has an initial typed PCI endpoint and host config-space model: function addresses validate bus/device/function ownership, type-0 identity and class header fields are little-endian bytes, common command writes mask reserved bits through shared endpoint and bridge helpers, common cache-line-size, latency-timer, and BIST writes update typed endpoint and bridge config bytes with snapshot restore while rejecting writes that would cross into read-only header fields, common status writes use typed write-one-to-clear semantics while preserving endpoint capability-list state, type-0 Cardbus CIS, subsystem IDs, Expansion ROM, minimum-grant, and maximum-latency fields are explicit typed header data, Expansion ROM writes preserve gem5's 32-bit update and size-probe behavior, BAR sizing writes apply typed memory or I/O masks, 64-bit memory BARs consume checked adjacent config slots and combine lower plus upper dwords into one logical range, fixed legacy I/O BARs preserve a declared address while ignoring config BAR writes, BAR types and host BAR address-space mapping live in a dedicated module instead of the endpoint monolith, command bits gate exposed BAR ranges, an endpoint-owned PCI capability-list registry links multiple installed capabilities in order, installs read-only raw capability byte blocks for device-specific vendor extensions, rejects overlapping regions before mutation, and keeps next-pointer bytes stable while PM, PCIe, MSI, MSI-X, or raw capability control fields are written, PM capabilities expose typed config-space headers, capability words, writable PMCSR state, and snapshot restore, PCIe capabilities expose typed config-space headers, device/link/slot/root/capability2 capability fields, writable control/status fields, snapshot restore, and typed read-only versus width errors instead of gem5's raw PXCAP byte-copy writes, MSI capabilities expose typed config-space headers, preserve clamped vector enable state, mask vectors, restore snapshots, derive typed message address/data pairs, and deliver enabled serial or parallel MSI assertions through rem6-interrupt without direct platform mutation, MSI-X capabilities expose typed config-space table and PBA registers, own BAR-local table/PBA state, mask vectors and functions explicitly, preserve table plus pending bits across snapshots, and deliver enabled serial or parallel MSI-X assertions while recording masked parallel sends as typed pending bits, Type-1 bridge configs expose typed PCI-to-PCI headers, BAR0/BAR1, Expansion ROM, interrupt line/pin, and bridge-control fields, preserve gem5's type-1 BAR update path plus Expansion ROM update and size-probe behavior, validate primary/secondary/subordinate bus ranges, route subordinate config accesses only through declared bridge ranges, filter downstream active BAR host mappings through memory, prefetchable-memory, and I/O windows, and snapshot/restore bridge config plus BAR state with function/identity/class and BAR-shape checks, read-only writes return typed errors, snapshots restore endpoint config and BAR state with function/identity/class checks, host apertures implement CAM-sized and ECAM-sized per-function config slots, host reads route by decoded physical config addresses, missing functions return all-ones reads for guest enumeration, duplicate or out-of-aperture registration is rejected explicitly, active endpoint and bridge BARs map through typed IO, non-prefetchable memory, and prefetchable memory host bases, overlapping host BAR ranges are rejected before system topology consumes them, the config aperture can be attached to the typed MMIO bus with serial or parallel scheduler responses, masked config MMIO writes split byte-enabled runs into typed config writes instead of widening into read-only neighboring bytes, active BAR host ranges can forward serial or parallel runtime MMIO requests into BAR-local device offsets with typed boundary errors plus optional live bridge-window revalidation before each access, and typed PCI legacy INTx direct mapping, explicit platform routing tables, bridge swizzle paths, and endpoint-facing post/clear ports can deliver serial or parallel interrupt assertions through rem6-interrupt without direct platform mutation. rem6-virtio models a modern VirtIO PCI common-config MMIO device with feature-page selection, driver-feature pages, queue selection, queue sizing, queue-enable, queue descriptor/driver/device addresses, device-status reset, typed snapshots, and typed read-only plus invalid queue errors instead of gem5's legacy-only BAR0 header assertions; it also models a modern PCI notify-MMIO device that derives per-queue notify addresses from queue notify offsets and notify-off multipliers, records serial or parallel queue notifications with scheduler ticks, snapshots notification history, and rejects invalid notify layouts or mismatched writes before backend mutation; it models a modern PCI ISR-status MMIO device with separate queue-interrupt and configuration-change bits, serial or parallel read-clear events, snapshot restore, reserved-bit masking, and typed write or width errors instead of gem5's single pending bool plus direct `intrClear` side effect; it models a modern PCI device-specific configuration MMIO container with typed byte mutability, serial or parallel read/write access traces, byte-mask writes, snapshot restore, and typed boundary or read-only-byte errors instead of forwarding raw BAR0 offsets into backend panic paths; it emits standard VirtIO vendor-specific PCI capability bytes for regular structures and notify structures with typed cfg_type, BAR, id, offset, length, cap_len, cap_next, and notify_off_multiplier fields, and converts those bytes into rem6-pci raw capability specs that install into endpoint config space; it now has a typed modern VirtIO PCI transport spec that builds endpoint identity, BAR declarations, common/notify/ISR/device/shared-memory capability chains, binds common/notify/ISR/device-config MMIO devices into BAR-local runtime routers with serial and parallel dispatch, and pre-mutates nothing until missing BARs, out-of-BAR regions, same-BAR overlaps, undersized runtime regions, missing device-config devices, and PCI endpoint shape errors are rejected; it models modern PCI shared-memory capabilities as typed cap64 region descriptors and vendor-specific capability bytes with unique ids, declared BAR containment, 64-bit offset and length splitting, cap-next chaining, config-image export, entry lookup by region id or capability offset, rem6-pci raw capability installation, configuration-space placement validation, and same-BAR overlap rejection before topology consumes the ranges; it emits typed modern VirtIO block device configuration bytes, feature-page records, and writeback mutability masks for capacity, size, segment, geometry, block-size, topology, multiqueue, discard, write-zeroes, and secure-erase fields with shape validation before device-config installation; it executes decoded VirtIO block read, write, flush, get-id, and unsupported requests against typed 512-byte-sector memory or rem6-storage image backends through serial and parallel scheduler contexts while recording queue/request/tick/status completions and returning guest-visible error statuses for read-only or out-of-range accesses instead of panic paths; it decodes direct and indirect split virtqueue descriptor chains for VirtIO block into typed requests plus status/data completion metadata while rejecting descriptor loops, missing status descriptors, short headers, and wrong readable/writable descriptor directions before backend execution; it records typed split used-ring completion writeback plans with scatter data writes, status-byte writes, used-ring slot selection, little-endian used elements, and wrapping used indices; it can consume split available-ring block requests from typed guest memory by walking queue descriptor, driver, and buffer addresses through rem6-memory instead of prebuilt host-only descriptor lists; it can snapshot/restore split queue cursor and event-index state through typed queue records and rem6-system checkpoint banks; and it can write block completion data buffers, status bytes, used elements, and used indices back into typed guest memory before raising typed VirtIO PCI ISR queue-interrupt status at the completion tick and posting serial or parallel PCI legacy INTx through rem6-interrupt, sending serial or parallel PCI MSI through the configured typed endpoint and MSI port, or sending serial or parallel PCI MSI-X through the configured typed endpoint and MSI-X port, and returning a typed interrupt-delivery outcome so masked MSI or MSI-X completions remain distinguishable from delivered interrupt events. rem6-storage owns typed raw, file-backed, and copy-on-write sector-image layers comparable to gem5 `DiskImage`, `RawDiskImage`, and `CowDiskImage`: 512-byte sector reads and writes are range-checked, read-only raw or file-backed images reject writes before mutation, file-backed images surface typed host-file errors and explicit sync flush accounting, COW layers can nest over raw, file-backed, or COW children, dirty-sector snapshots are deterministic, restore prevalidates capacity before mutation, writeback is explicit instead of hidden behind process-exit callbacks, and storage image checkpoint banks encode raw, file-backed, or COW snapshots as deterministic chunks with decode-first restore validation. `rem6-system` host actions can attach those storage checkpoint banks, capture their chunks in staged manifests, and reject malformed restore payloads before mutating any live storage image. RISC-V topology systems can register storage image plus VirtIO split virtqueue, PCI common-config, notify-MMIO, ISR, and device-config checkpoint ports before or after host-controller creation and attach them automatically to the host checkpoint executor. IDE controller DMA, timing, PCI endpoint/INTx, and checkpoint paths now share the typed storage and PCI substrate; broader IDE board integration remains an alignment target. rem6-net owns typed Ethernet packet payloads, separate wire-length timing metadata, typed Ethernet interface peer binding and event records, typed Ethernet tap stub framing, deterministic retry queues, distributed Ethernet message headers, sync command records, endian-stable payload envelopes, receive scheduling, sync-window checks, missed-packet detection, packet FIFO capacity and reservation state, explicit non-front removal slack, copyout, typed PCAP capture records and byte-image export, fixed full-duplex link serialization and delivery timing, deterministic delay variation, typed shared-bus broadcast timing, direction-local busy state, deterministic ready-delivery drain, typed Ethernet MAC addresses, learning-switch forwarding with TTL expiry, multicast or unknown-destination flood decisions, output queue capacity tail drops, output FIFO timing, ready-output records, typed SINIC registers, FIFO state, RX/TX checksum offload, descriptor-memory DMA, MMIO, PCI BAR binding, and scheduled PCI legacy INTx bridging, and snapshot/restore evidence comparable to gem5 `EthPacketData`, `EtherInt`, `EtherTapBase`, `EtherTapStub`, `DistHeaderPkt`, `DistIface`, `DistIface::RecvScheduler`, `TCPIface`, `PacketFifo`, `EtherDump`, `EtherLink`, `EtherBus`, and `EtherSwitch` while replacing pointer ownership, event callback queues, implicit RNG, raw interface pointers, fd polling side effects, raw C++ header layouts, receiver-thread scheduling side effects, and assert paths with typed errors and explicit records. Broader platform interrupt-controller models remain required for full-system breadth. |
| `src/dev/baddev` | `rem6-mmio` | partial | gem5's `BadDevice` intentionally panics when an unsupported MMIO region is touched. rem6 replaces that with `UnsupportedMmioDevice`, a named typed trap region that can be installed on the serial or parallel MMIO bus, returns `MmioError::UnsupportedDeviceAccess`, validates direct range crossings, and records tick, request id, operation, range, and write payload evidence for replayable diagnostics. |
| `src/dev/arm/amba_device` | `rem6-amba`, device crates | partial | rem6 now owns ARM PrimeCell/AMBA ID decoding in a small shared Rust crate rather than copying gem5's `readId` callback pattern into every device. PL031, SP804, SP805, and PL011 consume the same typed helper for the `0xfe0..0xffc` ID window, keeping device ids auditable while avoiding a timer-owned utility or per-device byte-shift clones. |
| `src/dev/arm/pl011` and RealView PL011 platform declarations | `rem6-uart`, `rem6-amba`, `rem6-interrupt`, `rem6-platform`, `rem6-system`, future ARM platform crates | partial | rem6 now models the ARM PL011 UART register surface as a typed MMIO device: data reads return queued RX bytes or zero, data writes record TX bytes and raise TX raw status, flag reads expose gem5-style CTS, RX-empty or RX-full, and TX-empty bits, baud, line-control, control, interrupt FIFO level, interrupt mask, raw interrupt, masked interrupt, interrupt-clear, and DMA-control registers keep gem5-visible offsets, and the PrimeCell ID bytes come from `rem6-amba`. RX injection raises raw RX and timeout status, interrupt masks drive serial or parallel typed interrupt assertions, final-byte data reads clear raw RX state and deassert masked interrupts only after route validation, and DMA-enable writes return a typed device error instead of gem5's panic path. Platform builders can now declare PL011 MMIO regions, route and optional interrupt wiring as typed data, retain the resulting devices by base address, enumerate them for topology host setup, and attach device-specific checkpoint banks automatically. PL011 checkpoint chunks preserve TX history, RX injection, pending and consumed RX bytes, interrupt delivery errors, control, baud, line-control, FIFO-level, mask, and raw interrupt registers through decode-first host restore with RX ledger-consistency validation, so a malformed later PL011 chunk cannot partially mutate an earlier live device. This keeps the useful gem5-visible PL011 register surface while replacing RealView Python object wiring and object-local serialization with typed platform data and system-level checkpoint validation. |
| `src/dev/arm/rtc_pl031` and RealView PL031 platform declarations | `rem6-timer`, `rem6-platform`, `rem6-system`, future ARM platform crates | partial | rem6 now models the ARM PL031 RTC as a typed core plus MMIO wrapper: the data, match, load, control, interrupt mask, raw interrupt, masked interrupt, and clear registers keep gem5-visible offsets and read/write behavior, the counter derives elapsed seconds from explicit scheduler ticks and a declared tick rate, match events latch raw status independently of masking, masked status follows the interrupt mask, and serial or parallel typed interrupt ports emit assert/deassert pulses only after route validation. The MMIO wrapper also exposes gem5-compatible PrimeCell ID bytes through the shared typed ARM AMBA ID helper instead of duplicating `readId` callback logic inside each device. Snapshots preserve the time base, last write tick, load value, match value, raw/masked state, tick rate, and generation, while generation checks discard stale callbacks and the MMIO wrapper avoids enqueueing default wrap-distance match events after load writes so parallel idle drains cannot be dominated by a far-future artifact. Platform builders can now declare PL031 MMIO regions, initial time, tick rate, and optional interrupt routes as typed data, retain the resulting devices, and attach their checkpoint banks automatically to topology host controllers. Dedicated PL031 checkpoint banks now encode those fields in a device-specific chunk and restore them through host actions with decode-first validation instead of gem5-style object-local serialization or RealView Python object wiring. |
| `src/dev/arm/timer_sp804` and RealView SP804 platform declarations | `rem6-timer`, `rem6-platform`, `rem6-system`, future ARM platform crates | partial | rem6 now models the ARM SP804 dual timer as a typed core plus MMIO wrapper. The core keeps gem5's two-timer layout, register offsets, reset interrupt-enable bit, `LOAD`, read-only `CURRENT`, `CONTROL`, `INTCLEAR`, raw interrupt, masked interrupt, and `BGLOAD` behavior surface, including one-shot expiry, free-running reload, periodic reload, 16-bit versus 32-bit counter width, and the gem5 prescale formula `clock << (4 * prescale)`. rem6 separates each timer into explicit snapshot state with generation-based stale-event invalidation, validates clock ticks, widths, routes, timer indices, and reserved prescale values with typed errors, supports both serial and parallel scheduler MMIO response paths, exposes gem5-compatible PrimeCell ID bytes through the shared typed ARM AMBA ID helper, and delivers assert/deassert pulses through typed interrupt ports only after route validation. The background-load register is represented as explicit state rather than gem5's single shared `loadValue`, so periodic reload intent can be audited separately from immediate load writes. Platform builders can now declare SP804 MMIO regions, timer clocks, and paired interrupt routes as typed data, retain the resulting MMIO devices, and attach their checkpoint banks automatically to topology host controllers. Broader RealView wiring remains a follow-up alignment target. |
| `src/dev/arm/timer_cpulocal` | `rem6-timer`, `rem6-platform`, `rem6-system`, future ARM platform crates | partial | rem6 now has an initial typed Cortex-A9 CPU local timer/watchdog core plus MMIO wrapper. Each CPU owns the gem5-visible local timer and watchdog register windows for `LOAD`, `COUNTER`, `CONTROL`, interrupt status, watchdog reset status, and the watchdog disable sequence; MMIO dispatch selects the CPU from the scheduler partition instead of a packet context id; serial and parallel MMIO response paths share the same generation-filtered timer state; and timer/watchdog interrupt delivery uses typed, prevalidated interrupt ports. Platform builders can declare the MMIO region, clock tick, explicit CPU partition list, per-CPU MMIO routes, timer interrupt routes, and watchdog interrupt routes, then retain the resulting device by base address. The MMIO bus supports source-partition-local views of the same address range, so CPU-local registers can be accessed naturally from multiple CPU partitions without aliasing all requests through one global device route. rem6 preserves gem5's auto-reload, interrupt-enable, watchdog-mode stickiness, disable sequence, and `clock << (4 * prescalar)` timing surface for valid prescalar values, but rejects invalid prescalar shifts and deadline overflow with typed errors instead of relying on shift behavior. Zero-load auto-reload advances by at least one decrement tick instead of requeueing at the same scheduler tick. True watchdog mode records reset assertions as typed snapshot evidence instead of calling gem5's fatal true-watchdog path. Topology host controllers attach platform-owned CPU local timer checkpoint banks automatically, and those banks preserve per-CPU timer/watchdog snapshots through decode-first host capture and restore validation. Broader ARM board wiring remains an alignment target. |
| `src/dev/arm/watchdog_sp805` and RealView SP805 platform declarations | `rem6-timer`, `rem6-amba`, `rem6-interrupt`, `rem6-platform`, `rem6-system`, future ARM platform crates | partial | rem6 now models the ARM SP805 watchdog as a typed core plus MMIO wrapper. The core keeps gem5-visible `LOAD`, read-only `VALUE`, `CONTROL`, `INTCLR`, raw interrupt, masked interrupt, lock, and integration-test register offsets, lock magic, enable and reset-enable bits, and PrimeCell ID `0x00141805`. rem6 deliberately makes the countdown value current-tick-derived from the declared watchdog clock instead of deriving it from the scheduled timeout event, records reset assertions as typed snapshot evidence, invalidates stale timeout callbacks through generations, supports serial and parallel MMIO response paths, and returns typed errors for unknown registers and unsupported integration-test harness writes instead of gem5-style warn or panic paths. Platform builders can now declare SP805 MMIO regions, watchdog clocks, and optional interrupt routes as typed data, retain the resulting devices by base address, enumerate them for topology host setup, and attach dedicated checkpoint banks automatically. SP805 checkpoint chunks preserve timeout interval, current counter base, enable and reset-enable bits, lock state, integration-test state, raw interrupt state, clock tick, generation, and reset-assertion records through decode-first host restore instead of gem5-style object-local serialization or RealView Python object wiring. Broader ARM board wiring remains a follow-up alignment target. |
| `src/dev/serial`, `src/dev/riscv`, `src/dev/lupio`, `src/dev/i2c` | `rem6-uart`, `rem6-mmio`, `rem6-interrupt`, `rem6-timer`, future device crates | partial | UART, timer, MMIO, interrupts, and initial RISC-V CLINT plus PLIC-compatible models exist. The CLINT path keeps gem5's `msip`, `mtimecmp`, and read-only `mtime` MMIO layout while replacing direct `System::threads` interrupt mutation with typed interrupt ports and scheduler events, including parallel scheduling. CLINT register, timer-assertion, and RTC-driven `mtime` state can be captured and restored through typed snapshots and a system checkpoint bank, platform declarations now attach CLINT MMIO plus host checkpoints automatically, and reset is explicit through `ClintResetPolicy`: `msip` is cleared, asserted software and timer lines are typed deasserted, stale timer events are invalidated, `mtimecmp` is either preserved or reset to a declared value, and RTC-backed `mtime` resets as explicit device state. The default CLINT timebase remains scheduler ticks for compatibility, while `ClintTimebase::RtcDriven` plus `RiscvRtcSource` models gem5's RTC pulse into CLINT `mtime` without hiding the dependency in global time. A typed MC146818 RTC register core now covers binary and BCD calendar registers, status A/B policy, status C/D read behavior, read-clear status-C interrupt flags, second ticks, SET-bit clock freeze, alarm and update-ended interrupt flag generation, alarm wildcard matching, and raw snapshot restore while replacing gem5 panic paths for unsupported status writes and invalid BCD values with typed errors. A typed MC146818 RTC MMIO wrapper now exposes gem5-style CMOS address and data ports, preserves the NMI mask bit on address reads while selecting low 7-bit registers for data access, routes RTC registers through the typed RTC core, preserves the remaining CMOS byte array, supports serial and parallel MMIO responses, and snapshots/restores the selected address, CMOS bytes, and RTC core together. RTC interrupt delivery now mirrors gem5's `MC146818::RTCEvent` plus `RiscvRTC` raise/lower wrapper through typed interrupt ports: startup validates the route before scheduling, serial and parallel scheduler paths emit assert/deassert pulses for periodic, alarm, and update-ended events, PIE clearing invalidates stale periodic events through a generation counter, and platform declarations register the RTC line through the same shared interrupt controller as timers and UARTs. Platform declarations can now attach that RTC MMIO wrapper to the typed MMIO bus, retain it as a topology device, and attach it automatically to host checkpoint actions through a decode-first RTC checkpoint bank. The PLIC path keeps gem5's memory-map shape for priority, pending, enable, threshold, and claim/complete registers, including context-indexed enable windows and threshold/claim windows aligned with gem5 `PlicRegisters` context construction, while replacing gem5's register callbacks, per-context output queues, and assertion-style completion path with typed context routes, typed filtered claims, and typed completion errors. Platform PLIC configs now emit gem5-style `interrupts-extended` entries from typed context records and feed those same records into the MMIO device, so a multi-hart platform cannot advertise one context mapping in the device tree while simulating another. Platform PLIC configs also carry an explicit source count for `riscv,ndev`, with registered external devices still allowed to raise the emitted count, replacing gem5's post-hoc Python `attachPlic` maximum-source scan with a deterministic typed declaration. The PLIC implementation is split into `rem6-interrupt/src/plic.rs`, keeping the interrupt crate root focused on generic controller state instead of letting platform-specific MMIO logic grow into a monolith. Platform declarations can now emit typed RISC-V DTS source nodes and deterministic binary FDT/DTB blobs for CPUs, CPU local interrupt controllers, a `soc` simple bus, CLINT `interrupts-extended`, PLIC `interrupts-extended`, a PLIC-compatible external interrupt controller, UART interrupt-parent wiring, and Linux `/chosen` bootargs plus `linux,initrd-start` and `linux,initrd-end` metadata without Python object recursion or libfdt mutation. System topology can install the generated RISC-V DTB into store-backed or DRAM-backed guest memory and set each hart's A1 register to the DTB address, replacing gem5's external DTB filename side effect with a typed handoff. Other platform devices remain open. |
| platform-specific device trees under `src/dev/arm`, `src/dev/x86`, `src/dev/mips`, `src/dev/sparc` | future platform crates | planned | These should arrive with the corresponding ISA and platform support. |

PLIC source-count declarations feed both the emitted `riscv,ndev` property and the live `PlicMmioDevice`. Priority, pending, enable, snapshot restore, and claim filtering therefore share one bounded source view instead of letting the device tree and runtime register model drift apart.

### Simulation Kernel, Checkpointing, and Host Control

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| event queue and tick logic in `src/sim` | `rem6-kernel` | covered | Partitioned scheduling, conservative epochs, deterministic order, lookahead, target-clock prevalidation for cross-partition parallel remote messages, pre-dispatch remote-delivery windows, scheduler snapshots, recorded initial and final epoch frontiers carried through CPU, data-cache/coherence, full-system, and workload-result summaries with per-partition conservative full-system frontier aggregation, worker-local remote outboxes, persistent per-partition remote and progress-transition order cursors preserved by scheduler snapshots and scheduler checkpoint chunks, batch, epoch, and conservative-run remote-send records ordered by the same deterministic delivery merge key used for cross-partition event insertion, source-target remote-flow records carried through RISC-V cluster, full-system, and workload-result summaries with cross-epoch min/max delay bounds preserved, scheduler epoch, empty-epoch, and dispatch counts, planned and executed kernel batch records with direct start tick, horizon, duration tick, worker-tick occupancy, duration-weighted worker-count bucket, worker-capacity ticks, idle-worker ticks, utilization ratios, per-worker-slot planned and recorded active/idle tick summaries, and worker-tick query accessors, recorded scheduler epoch and conservative-run exact batch worker-count summaries, duration-weighted worker-count tick summaries, exact worker-count batch and tick queries, minimum-worker batch, tick, and batch worker-tick queries, total batch worker-tick queries, and CPU scheduler, data-cache scheduler, and merged full-system progress-free transition records, deterministic dimension lists, per-dimension record slices, counts, tick windows, compact summaries by transition kind, partition, and subject, direct `RiscvSystemRun` and workload-result batch timeline records, GPU DMA and accelerator DMA workload-result batch timeline records, remote-send records, remote-flow evidence, worker-count summaries, duration-weighted worker-count tick summaries, exact worker-count batch and tick queries, planned and recorded worker-capacity ticks, idle-worker ticks, utilization ratios, per-worker-slot recorded active/idle tick summaries, minimum-worker duration-weighted tick queries, minimum-worker batch worker-tick queries, longest minimum-worker tick-streak queries, exact partition-set summaries, same-partition-set streak summaries, per-set counts, exact worker-count bucket counts, and minimum-worker batch counts with full-system batch sequences ordered by worker start tick rather than subsystem concatenation, and declared-threshold livelock diagnostic records, subject queries, subject summaries, subject tick windows, transition-kind queries, transition-kind counts, kind summaries, kind-window summaries, and diagnostic counts exposed on `RiscvSystemRun` and carried into workload summaries, recorded worker-capacity ticks, idle-worker ticks, utilization ratios, and per-worker-slot active/idle summaries retained in workload-result artifacts, batch worker-count histograms, exact batch partition-set histograms, and explicit merged full-system maximum consecutive partition-set streaks carried into workload summaries, manifest-owned and multiset-verifiable expected remote-send, progress-transition, and exact batch timeline records, remote-flow counts, first/last tick windows, delay bounds derivable from exact remote-send records when aggregate flow records are absent or weaker, and scope-wide delay floors and ceilings, batch count summaries derived from the strongest available aggregate, batch-worker, exact partition-set, or streak evidence, minimum scheduler progress contracts backed by aggregate epoch or dispatch counts, batch-worker histograms, exact partition-set histograms, streaks, or worker-owned per-partition dispatch activity, maximum scheduler idle contracts, minimum max-worker contracts backed by aggregate counts, batch-worker histograms, exact partition-set histograms, or streaks, total-worker activity contracts backed by aggregate counts, batch-worker histograms, exact partition-set histograms, streaks, or multi-worker per-partition worker activity, multi-worker batch activity contracts backed by batch-worker histograms, exact partition-set histograms, or streaks, minimum-worker duration-weighted tick activity contracts, minimum thresholded batch worker-tick contracts, and sustained minimum-worker tick-streak contracts backed by exact batch timelines, exact batch partition-set contracts backed by exact histograms or streak counts, sustained same-batch partition-set streak contracts, active-partition contracts backed by aggregate counts, exact batch partition-set unions, streak partition-set unions, activity-derived partition unions, remote-send endpoints, or remote-flow source/target unions, typed work flags backed by partition, frontier, and DMA remote-traffic evidence, per-partition activity contracts backed by explicit activity, exact batch partition-set histograms, streaks, remote-send records, or remote-flow records with same-scope activity projections merged as lower-bound evidence instead of added, per-partition initial/final frontier contracts, clean diagnostic contracts, source and target partition counts in recorded parallel summaries, and typed parallel-worker failure reporting that preserves remaining partition events, keeps executed-time visibility, commits successful callbacks' remote messages, and rolls back local and remote events scheduled by the panicked callback exist. |
| standalone simulator entry point and output artifacts | `rem6` | partial | The `rem6` binary crate now provides an initial tested `run` command for ELF-backed inputs. It validates requested ISA against ELF architecture, rejects mismatches before producing stats, emits a single JSON artifact with schema, input identity, ELF metadata, load status, structured `parallel.scheduler` evidence, structured fetch/data transport evidence, and hierarchical typed stats, and can run RISC-V ELF inputs with `--execute --cores <n>` through real per-core fetch/data endpoints, per-core partitions, the memory transport, and the parallel system driver until a guest trap reaches the host, the configured `--max-tick` budget stops the run, or an explicit `--max-instructions <count>` budget stops committed-instruction execution first. RISC-V CLI runs expose distinct `mhartid` values per core, so a shared ELF can branch by hart while still using the same parallel execution path. `--start-address <addr>` can override the effective reset PC while the artifact still preserves the original ELF entry, and both loaded and executed artifacts plus CLI stats report the effective start address so bootloader, kernel handoff, and later platform flows do not have to conflate image metadata with runtime reset state. `--riscv-boot-a0 <value>` and `--riscv-boot-a1 <value>` seed the RISC-V boot argument registers, are applied before execution, and are recorded in both artifacts and stats so full-system handoff values are explicit runtime inputs rather than hidden platform-side register initialization. `--load-blob <addr>:<path>` reads an external binary blob, maps it into the CLI memory store after ELF load, preserves adjacent cache-line contents, and records blob address, byte count, and source path in artifacts plus numeric blob stats so DTB, initrd, and firmware handoff inputs are explicit rather than side effects. The CLI also accepts `--min-remote-delay <ticks>` as a scheduler lookahead runtime option and records the value in both the structured scheduler artifact and hierarchical stats. `--memory-route-delay <ticks>` separately controls the simple CLI memory-route request and response latency floor, must be at least the configured scheduler lookahead, and is emitted in the simulation artifact and hierarchical stats so transport timing is explicit instead of inferred from trace deltas alone. `--host-event-delay <ticks>` separately controls the guest-trap host-event delivery latency floor, defaults to the scheduler lookahead, must be at least the configured scheduler lookahead, and is emitted in the simulation artifact and hierarchical stats so trap delivery timing is explicit rather than tied implicitly to scheduler lookahead. The main artifact and CLI stats now include the configured parallel worker limit plus recorded parallel scheduler dispatch, batch, total-worker, active-partition, remote-send, worker-tick, capacity, idle-worker, per-worker-slot active/idle ticks, actual worker-lane partition active ticks, scheduler ready partitions, indexed initial and final partition frontiers, and per-partition worker, dispatch, remote-send, remote-receive, and pending-event evidence rather than only the coarse epoch and max-worker counters. The main artifact also exposes fetch/data transport totals and per-route source counters, while CLI stats include per-core and aggregate data load/store/atomic bytes, aggregate and route/source fetch/data memory transport request and response counts, round-trip tick totals, maximums, the configured instruction limit, and a typed stop-reason counter. Requested `--dump-memory <addr>:<bytes>` ranges are read from the post-run store and emitted as hex in the same artifact, `--output <path>` writes the selected run output format to a file while stdout reports the artifact path, and `--stats-output <path>` writes the selected stats payload as a separate file for direct stats consumers; when both output paths are requested, stdout reports both durable artifact paths and the selected format in one machine-readable envelope. The selected stats output can be the typed JSON array or a gem5-style text statistics block with begin/end markers plus row-level unit and reset-policy metadata. Platform construction, checkpoint options, additional runtime options, and broader ISA execution remain open. |
| SimObject and Python configuration in `src/sim` and `src/python` | `rem6-platform`, `rem6-workload` | partial | rem6 should keep ease of composition through typed builders and manifests rather than dynamic object graphs. |
| checkpoint support in `src/sim` | `rem6-checkpoint`, `rem6-kernel`, `rem6-system` checkpoint banks | partial | Protocol-neutral checkpoint records exist for several subsystems. Scheduler checkpoint capture rejects non-quiescent state with typed quiescence reports for pending-event counts, tick windows, and serial/parallel pending-event kind counts by component and partition, scheduler checkpoint banks preflight quiescence before writing any scheduler chunks, system checkpoint host actions run scheduler quiescence preflight before capturing any attached bank, full-system checkpoint capture stages registry writes until final manifest capture succeeds, execution-mode restore pre-registers internal host checkpoint components only in staged registries, manifest restore clears chunks for registered components absent from the manifest so stale component state cannot satisfy attached-bank validation, checkpoint restore event plans separate isolated warmup replay events from live scheduler events, reject live events before the restored tick, and seal the warmup boundary before live handoff, PLIC checkpoint banks preserve context-local enable and threshold state with restore-time base and context-route validation, RTC checkpoint banks preserve selected CMOS address, CMOS bytes, and RTC core registers with decode-first validation, PL031 checkpoint banks preserve ARM RTC counter, match, raw/masked interrupt, tick-rate, and generation state with decode-first validation, SP804 checkpoint banks preserve both ARM dual-timer snapshots with decode-first validation, SP805 checkpoint banks preserve ARM watchdog state and reset-assertion records with decode-first validation, CPU local timer checkpoint banks preserve per-CPU ARM local timer/watchdog snapshots with decode-first validation, storage image checkpoint banks preserve raw and COW sector-image snapshots through host checkpoint actions with decode-first validation, VirtIO PCI common-config checkpoint banks preserve selected registers and queue state with decode-first validation, VirtIO PCI notify checkpoint banks preserve notification history with decode-first validation, VirtIO PCI ISR checkpoint banks preserve interrupt status and event history with decode-first validation, VirtIO PCI device-config checkpoint banks preserve device bytes, writable masks, and access history with decode-first validation, checkpoint manifests expose typed component, chunk, and payload-byte summaries for audit, workload replay results preserve those manifest totals and per-component counts for checkpoint and restore outcomes, and workload manifests can require minimum checkpoint coverage totals and named-component coverage during replay verification. More devices and broader pending-state restore coverage remain open. |
| statistics, probes, and power hooks | `rem6-stats`, `rem6-power`, run summaries | partial | Counters, hierarchical stat groups, per-counter reset policies, stats snapshots, typed stats dump records, typed reset records with before/after policy audits, schema-and-reset-scope-checked stats delta records, typed probe registries with checked component, point, and listener identifiers, probe listener state, probe event snapshots with historical listener refs plus cursor-preserving and time-monotonic restore validation, typed power states/domains, power residency snapshots, typed state-weighted dynamic/static power models, typed expression-based dynamic/static power models, typed stat-snapshot metric binding, power metric binding from core stats deltas, typed RC thermal domains, typed multi-domain thermal-network solving with resistor and capacitor edges, and initialized passive thermal junction nodes exist. Broader power-controller and external-analysis adapter breadth remains open. |
| guest-host events and pseudo instructions | `rem6-system`, `rem6-workload` | partial | ROI, stats, checkpoint, checkpoint restore, stop, execution mode actions, custom guest-host calls, manifest-declared guest-host response payloads, focused system/trap event port modules, and absolute parallel trap delivery-boundary preflight are typed. Broader guest ABI support remains open. |

### External Integration and Tooling

| gem5 source anchor | rem6 owner | Coverage | Notes |
| --- | --- | --- | --- |
| `ext/systemc`, `src/systemc`, `util/systemc`, `util/tlm` | future adapter crates | external-adapter | Interoperability is useful, but rem6 timing authority must stay in the partitioned runtime. |
| `ext/sst`, `src/sst`, `configs/example/sst` | future SST adapter | external-adapter | SST co-simulation should be explicit and checkpoint-aware. |
| `ext/nomali`, `ext/mcpat`, `ext/dsent` | future optional analysis adapters | external-adapter | Preserve modeling value behind typed records and stable APIs. |
| `ext/libelf`, `ext/libfdt`, `ext/softfloat`, `ext/gdbremote` | `rem6-boot`, `rem6-platform`, `rem6-debug`, future ISA support crates | partial | rem6-boot has typed ELF32 and ELF64 little-endian and big-endian loaders plus an auto-detecting ELF entry point that validates the ELF identity and program-header shape, records typed ELF class, endian, machine, architecture, OS ABI, flags, and operating-system metadata comparable to gem5 `ElfObject::determineArch()` and `ElfObject::determineOpSys()`, including PPC64 ABI v1/v2 selection from ELF flags, endian-sensitive PPC64 default ABI selection when flags omit the ABI, Linux/Solaris/FreeBSD fallback from `.note.ABI-tag`, and Solaris fallback from `.SUNW_version` or `.stab.index` section names, imports physical-address `PT_LOAD` segments into `BootImage`, skips non-loadable segments, preserves `p_memsz > p_filesz` zero-fill, and rejects unsupported class, encoding, version, or overflowing segment memory ranges with typed errors without libelf or unsafe parser state. Its source boundary keeps ELF parsing, boot-image loading, and typed boot errors in focused modules so loader breadth does not grow into a gem5-style untyped utility layer. Workload boot images preserve that ELF metadata through manifest round trips and include it in manifest identity, so ISA or ABI selection cannot silently drift when two images have the same entry and loadable segments but different ELF endian, machine, OS ABI, or flags headers. rem6-platform has an initial typed DTS source tree and deterministic binary FDT/DTB writer for RISC-V platform boot descriptions, including Linux `/chosen` bootargs and initrd start/end metadata, and rem6-system installs initrd bytes plus generated or resolved-resource DTBs into guest memory with the RISC-V A1 register handoff for both store-backed and DRAM-backed memory. rem6-debug now owns GDB remote packet framing, checksum validation, acknowledgement and interrupt frame parsing, bounded payload configuration, and skipped-prefix reporting as typed data before architecture-specific register caches are added. rem6 still needs broader ISA loader coverage, kernel image loader breadth, bootloader handoff coverage, soft-float, and ISA register-cache/debug-session capability without vendoring unneeded code into the core. |
| `util/gem5art`, resources tooling, disk image tooling | `rem6-workload`, future artifact tooling | partial | rem6 manifests make artifact provenance first-class and reproducible for boot images, declared resources, typed Linux device-tree resources, and typed Linux initrd handoff resources. Resource declarations can now carry typed acquisition provenance with kind, acquisition locator, tool, and revision, and manifest identity hashing includes that provenance so acquisition state cannot silently drift from the workload contract. Disk-image resources can also carry typed construction records with image format, virtual size, tool, operation, input, and arguments; these records are disk-image-only and identity-hashed. Resolved payloads are explicit caller-provided data with manifest identity, id, digest, kind validation, and initrd handoff-size validation; workload replay rejects payload sets resolved for a different manifest, and no hidden download path is part of replay authority. Future tooling still needs richer artifact acquisition execution and construction executors for additional artifact kinds. |
| `tests`, `tests/test-progs`, `util/statetrace` | rem6 tests and trace tooling | partial | gem5 tests are audit input. rem6 acceptance remains Rust tests and typed trace comparison. |

## Evidence Already Present in rem6

- Partitioned scheduler tests cover deterministic event order, lookahead,
  conservative parallel epochs, worker limits, wait-for graphs, and scheduler
  snapshots, including prevalidated scheduler checkpoint-bank restore so
  truncated chunks cannot partially mutate live scheduler frontiers, and
  malformed global ticks that precede partition clocks or partition clocks
  that precede the global tick are rejected before restore mutates the target
  scheduler. Serial drain tests require idle partition clocks to advance to the
  final tick before any later parallel scheduling can occur. Wait-for graph
  tests cover checkpoint barrier nodes, barrier wait edges, repeated
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
- Kernel checkpoint restore schedule tests cover the separation between
  isolated warmup replay events and live scheduler events, typed rejection of
  live events before the restored tick, typed rejection of warmup events before
  the replay clock or after the restored tick, warmup slack summaries, and
  one-way warmup sealing before live handoff.
- CPU in-order pipeline tests cover typed per-stage width validation, oldest
  ready-prefix advancement, structural stall reporting, younger ready-work
  ordering blocks behind the first older stall, Commit-stage retirement, and
  branch-redirect flushing of younger pipeline work with explicit target-PC
  evidence after validating redirect sequence and stage ownership. Snapshot
  tests cover in-flight pipeline state round trips and duplicate in-flight
  sequence rejection during restore. Cycle-advance tests cover next-state
  mutation for advanced work, retained stalls, retired commits, and redirect
  flush removal. Cycle cursor tests cover snapshot/restore preservation,
  increment on advance, and overflow rejection without state mutation. Cycle
  record tests cover before snapshot, plan, and after snapshot preservation for
  audit and trace consumers. Cycle summary tests cover advanced, retired,
  flushed, structural-block, ordering-block, state-change, and redirect-target
  reporting. Run summary tests cover empty windows, positive redirect, flush,
  structural-block, ordering-block, state-change, and count totals across
  records, plus disjoint partial-summary merges and overlap rejection.
  Checkpoint payload tests cover binary round trips, cycle/config/in-flight
  preservation, duplicate-sequence rejection before payload creation, truncated
  payload rejection, invalid magic rejection, unsupported-version rejection,
  invalid stage-code rejection, payload-decoded zero-width rejection through the
  normal configuration validator, and explicit rejection of values too large for
  the payload field width.
- CPU O3 dependency release tests cover writeback-visible destinations,
  commit-visible misc-register destinations, always-ready fixed mappings, and
  the rule that commit-time dependency publication does not rerun
  memory-dependence completion.
- CPU O3 rename tests cover public gem5 issue #3041 by requiring
  `InvalidRegClass`-shaped source operands to map to a typed invalid physical
  register, become ready without a scoreboard lookup, and leave ordinary mapped
  operands under explicit scoreboard readiness.
- CPU O3 pipeline tests cover the reusable delay-compensated unblock policy
  used by fetch/decode/rename/IEW-style stage pairs: a downstream stage signals
  unblock before its skid buffer is empty when the remaining entries can drain
  within the backward signal delay times downstream width. They also cover
  zero-delay empty-only behavior and typed rejection of zero downstream width.
  O3 writeback transfer tests cover same-tick completion sets that exceed the
  IEW-to-commit future window, exact-fit completion sets, explicit deferred
  counts, in-window cycle/slot assignments, zero writeback width rejection, and
  writeback-window overflow rejection.
  O3 distributed issue tests cover public gem5 issue #2956 by requiring a busy
  issue queue to block only its own `(queue, OpClass)` capacity while a peer
  issue queue with the same OpClass and remaining capacity can still issue
  younger ready work in the same cycle. They also cover zero issue width and
  zero queue-capacity rejection. O3 scoped dependency tests cover public gem5
  issue #2953 by requiring unordered RISC-V vector reduction partial
  micro-ops, publish micro-ops, and unrelated younger work to be represented by
  reduction-local dependency scopes instead of serialize-after barriers; they
  also cover ordered reduction chains and duplicate producer rejection.
- CPU branch-target safety tests cover the RISC-V O3 full-system predictor
  configuration from public gem5 issue #2211 by rejecting simultaneous RAS
  disablement and disabled indirect target path hashing while allowing either
  mechanism to protect return targets.
- System host-assisted switch admission tests cover public gem5 issue #910 by
  rejecting KVM-shaped detailed takeover when x86 host state is incomplete,
  host MSRs or special registers are unsupported, memory modes differ, or a
  pending `mwait` service still needs a materialized request. They also require
  a successful takeover plan to expose ordered quiesce, validation, state
  capture, target-install, and resume actions.
- Memory line-store tests cover independent line reads, masked writes, AMO
  read-modify-write responses, writeback replacement, request shape rejection,
  duplicate line-snapshot restore rejection, and partitioned-memory restore
  propagation of line-snapshot validation.
- Memory translation TLB tests cover public gem5 issue #3010 by preserving
  global same-ASID entries during non-global ASID flush, removing same-ASID
  non-global entries, preserving other ASIDs, snapshotting entry scope, and
  rejecting nonmonotonic restore LRU counters so malformed checkpoints cannot
  reuse an existing stable replacement value.
- Memory translation queue tests cover pending-request ready ordering, snapshot
  restore, duplicate rejection, and nonmonotonic restore order-counter rejection
  so a malformed checkpoint cannot reuse an existing stable ordering value.
- x86 ISA prefix scan tests cover public gem5 issue #2962 by requiring a REX
  prefix before a later legacy prefix to be recorded as ignored, requiring a
  REX immediately before a one-byte opcode or `0x0f` escape to stay active,
  requiring contiguous REX prefixes to keep only the last candidate, and
  requiring non-64-bit mode to treat REX-shaped bytes as opcodes.
- x86 interrupt-flag tests cover public gem5 issue #2912 by requiring
  user-mode `CLI` with architectural IOPL zero to fault even when carry and the
  reserved RFLAGS bit are set, while allowing IOPL=3 IF mutation and
  modeling CR4.PVI-mediated VIF set/clear plus VIP-triggered #GP behavior.
- CHI protocol reservation tests cover the RISC-V three-level CHI LR/SC race
  from public gem5 issue #2688 by requiring overlapping store-conditionals on
  one line to serialize through a line-global reservation table, and by
  requiring coherence invalidations or evictions to produce typed reservation
  invalidation records.
- CHI directory Evict-hazard tests cover public gem5 issue #3013 by retaining
  the pending Evict requester and pre-hazard line state across a snoop-invalid
  restore, even when the current post-snoop directory state no longer lists the
  requester as a sharer.
- Cache replacement-directory tests cover public gem5 issue #2955 by requiring
  resident-line relocation to preserve canonical full line addresses, reject
  tag-shaped values, reject wrong-set moves, reject occupied destinations, and
  relocate SHiP-trained entries without requesting a synthetic new signature.
- Full-system RISC-V tests drive multicore fetch, data access, traps, host stop,
  statistics, run summaries, and system-level store-conditional failure
  diagnostics through the partitioned scheduler; workload
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
  aq/rl ordering propagation into parallel transport atomic requests. RISC-V
  ISA tests cover typed `mcycle` and `minstret` machine writes, executable
  `cycle` and `instret` CSR reads from hart state, user read aliases, user write
  rejection, wrapping increments, CSR address decoding, and counter snapshot
  restore. Dedicated CSR counter tests cover RV32 low/high word address
  decoding, high-half user read aliases, machine low/high word writes that
  preserve the other half, and read-only errors for user high-half writes.
  RISC-V PMP tests cover TOR, NA4, and NAPOT range decoding, lowest
  matching-entry priority, locked-entry write rejection, locked TOR lower-bound
  protection, configuration-before-address materialization, default inactive
  table denial for S/U modes, snapshot round trips, and entry-count mismatch
  restore rejection without partial live mutation. RISC-V frontend tests cover
  locked PMP rejection for instruction fetch, physical data load, and
  translated data load before any memory request is issued. RISC-V PMA tests
  cover default misaligned data rejection, explicit misaligned-support ranges,
  aligned-access bypass, and CPU frontend rejection of physical and translated
  misaligned data loads before memory issue, plus successful issue when the
  physical range explicitly supports misalignment. Memory request tests cover
  typed uncacheable-plus-strict-order flags independent of barrier ordering.
  RISC-V PMA and frontend tests cover uncacheable range matching plus fetch and
  data request delivery with strict-order flags set before transport response.
  RISC-V vector-config prediction tests cover branch-prediction targets
  that drop copied dynamic `vl`/`vtype` state while preserving the current hart
  vector configuration, and explicit vector-configuration updates that mutate
  `vl`/`vtype` only through the typed update path.
  RISC-V vector-compress tests cover tail-undisturbed preservation of
  destination elements after the compressed prefix and deterministic all-ones
  tail-agnostic handling. RISC-V vector micro-op tests cover macro flag
  propagation for serialize-after and non-speculative constraints, merging with
  delayed-commit micro-op-local flags, and first/last micro-op identity.
  RISC-V vector fixed-point tests cover `vcsr` alias masking for `vxrm` and
  `vxsat`, all four fixed-point rounding modes in narrow-clip execution, and
  direct saturation evidence merging without an extra CSR micro-op.
  Memory and cache-controller tests cover atomic responses that capture old
  bytes before masked writes, and memory checkpoint-bank tests cover prevalidated
  multi-store and DRAM memory restore so truncated payloads cannot partially
  mutate live memory state. Fabric, coherence, and RISC-V checkpoint-bank tests
  cover decode-first multi-bank and multi-core restore so malformed chunks
  cannot partially rewind another live NoC lane frontier, cache bank,
  architectural PC, integer register file, or PMP snapshot chunk.
  Heterogeneous checkpoint-bank tests cover decode-first accelerator and GPU
  restore so a malformed later device chunk cannot partially restore an earlier
  live device. Peripheral
  checkpoint-bank tests cover CLINT, UART, PL011, PLIC, RTC,
  interrupt-controller, VirtIO PCI common-config, notify-MMIO, ISR,
  device-config, and timer decode-first restore so
  malformed later device chunks cannot partially restore earlier live platform
  devices. System action tests cover staged
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
- Coherence harness restore paths now prevalidate snapshot backing-line and
  directory-state line identity for serial MSI/MESI/MOESI/CHI and partitioned
  MSI/MESI/MOESI/CHI before scheduler, directory, cache, or backing-store
  mutation. Malformed snapshots return typed wrong-line errors and preserve the
  previous live snapshot, replacing gem5 Ruby-style restore failures that can
  surface later as schedule-in-past or partially restored state.
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
  queue configuration plus target QoS and ordering state. They also cover the
  public gem5 issue #621 hazard by requiring MSHR completion to split local
  fill-service targets from writeback or clean downstream requests, and by
  requiring MSI bank fill results to expose deferred writeback traffic instead
  of returning it as a local no-response target. The same cache-bank coverage
  includes bank-level dirty-line count and address audit plus dirty data restore
  for MSI, MESI, MOESI dirty-owner, and CHI dirty states. It also covers
  clean-resident uncacheable read bypass for MSI and CHI banks, including
  original downstream request forwarding, response conversion without line
  installation, and CHI replacement-directory resident-way cleanup. MSI,
  MESI, MOESI, and CHI tests also require dirty resident lines to reject this
  read bypass while preserving dirty snapshot evidence when no typed write
  queue is attached, and require direct uncacheable writes to enter the typed
  write queue without allocating MSHR entries, pending fills, or resident cache
  lines. Dirty-resident uncacheable-read tests require `accept_cpu_request` to
  withhold the original read until the full-line dirty writeback is issued, at
  which point the writeback issue record exposes the read as an ordered
  post-issue downstream request for MSI, MESI, MOESI, and CHI. They also cover
  MOESI Owned and CHI SharedDirty resident lines, clearing the resident line
  and preserving the no-install fill behavior. Snapshot/restore tests require
  those pending uncacheable reads to survive restore and remain attached to the
  later dirty-writeback issue record, while already-issued clean pending reads
  on the same line remain independent. Malformed snapshot tests also reject
  cacheable pending entries, uncacheable writes, and pending uncacheable reads
  whose blocking writeback handle is missing, target another cache bank's
  agent, or carry a mismatched line layout. The
  bank-level same-line conflict tests also keep any in-flight uncacheable
  atomic request as a typed reservation until its downstream response returns,
  so follow-up cacheable or uncacheable requests on that line fail with a
  pending-uncacheable conflict instead of being reordered ahead of the
  outstanding atomic request.
  Dirty-resident
  uncacheable-write tests require a full-line dirty writeback queue entry
  before the uncacheable write queue entry for MSI, MESI, MOESI, and CHI, then
  clearing the resident line. These tests also require same-line cacheable reads
  to observe dirty writeback data overlaid with later uncacheable-write bytes
  when present, and same-line writes plus later uncacheable reads to stop at a
  typed write-queue conflict while those entries are pending. The same
  protocols now cover queued uncacheable write response matching after
  snapshot restore: issuing the write queue creates one in-flight typed write
  record, a memory write response becomes the original CPU response, repeated
  responses are rejected as unknown, and malformed in-flight write snapshot
  entries are rejected before restore mutates the bank, including cacheable
  writes, uncacheable atomics, no-response evictions, foreign-agent writes, and
  wrong-layout writes.
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
  typed key LRU capacity, future-index lookahead, duplicate-current-index
  suppression without lookahead, snapshot restore, cache write queue ready ordering,
  reserve handling, functional-read satisfaction, snapshot restore with
  monotonic order-counter validation, and replacement-triggered dirty
  writeback, clean evict, clean writeback, invalid victim suppression, and
  victim-way validation, plus MSI/MESI/MOESI/CHI bank write queue attachment,
  ready issue, conflict lookup, uncacheable-match filtering, functional-read
  delegation, and bank snapshot restore,
  AMPM cross-hot-zone access-map
  matching, AMPM useful-prefetch accounting, AMPM epoch degree increase and
  decrease with snapshot restore, page-boundary candidate dropping, multi-source
  queued prefetch earliest-ready
  reporting, deterministic round-robin issue, non-mutating no-op polls, and
  single-entry issue from the selected source.
- Fabric QoS tests cover explicit fixed-priority requestor mapping,
  highest-priority queue selection, FIFO/LIFO same-priority ordering, LRG
  requestor rotation, non-mutating empty polls, snapshot replay, and
  QoS-driven fabric batch reservation order on a shared link. Fabric timing
  tests cover public gem5 issue #3092 by requiring bits-per-nanosecond
  serial-link bandwidth to convert through the declared clock domain instead
  of treating nanoseconds as cycles. Transport tests cover QoS-driven
  first-hop fabric reservation before parallel batch events are scheduled,
  explicit QoS requestor override for shared-fabric LRG
  arbitration, direct target batch priority assignment from an explicit
  QoS requestor, and same-agent acquire/release ordering barriers that constrain
  direct QoS batch and shared-fabric first-hop reservation eligibility. They
  also cover same-agent uncacheable-plus-strict requests as full ordering
  edges that can block higher-priority direct QoS and shared-fabric requests.
  Memory tests cover shared read/write barrier matching, same-agent request
  ordering edges, and strict-order serialization around neighboring same-agent
  requests. DRAM tests cover same-agent acquire/release ordering barriers that
  constrain same-arrival QoS timing-batch eligibility, QoS-driven same-arrival
  request ordering before bank and bus timing are computed, typed read/write
  direction preference among same-priority timing candidates, burst-limited
  current-direction turnaround that switches only to an eligible waiting
  opposite direction, explicit same-requestor priority escalation inside timing
  batches, per-priority/per-requestor QoS access and byte accounting in DRAM
  activity profiles, gem5-like burst spacing across same-direction commands on
  a shared port, gem5-like
  same-bank-group burst spacing for bank-group memories, and gem5-like
  command-window bandwidth limits across row and data commands, plus
  target-local unique active port and bank coverage when DRAM activity windows
  are merged. DRAM memory-controller tests also keep the public memory error
  type within a bounded `Result` error-size budget, so rich typed profile drift
  diagnostics do not make strict workspace linting fail. DRAM profile tests now
  expose target-sorted parallel resource summaries for DDR, HBM, LPDDR, and NVM
  profiles, including port,
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
  counter reset epochs, typed reset policy audits for resettable, constant, and
  monotonic counters, typed dump history, schema-and-reset-scope-checked
  snapshot deltas including reset-policy drift rejection, and typed probe
  point, listener, historical listener-ref, event, payload, identifier-grammar,
  cursor-preserving, and time-monotonic snapshot restore records with malformed
  snapshot rejection.
- CLI tests cover the initial standalone `rem6 run` binary path: RISC-V ELF
  inputs produce one JSON run artifact with ELF identity and hierarchical typed
  stats, ISA/header mismatches fail before any stats are emitted, and explicit
  RISC-V execution on two parallel cores reaches an environment-call trap while
  preserving per-core architectural state, committed-instruction stats, and
  parallel-scheduler worker stats in the run artifact. The same two-core CLI
  test now requires detailed recorded parallel-scheduler stats for dispatches,
  batches, total workers, active partitions, remote sends, worker-ticks,
  worker-capacity ticks, idle-worker ticks, and per-partition scheduler
  activity for CPU, memory, and host partitions. Another CLI test sets
  `--parallel-workers 1` on a two-core run and verifies both the configured
  worker-limit stat and the resulting one-worker scheduler max. CLI tests also
  cover ELF-backed RISC-V load/store execution through the data endpoint path
  and require per-core and aggregate data-access stats plus a post-store memory
  dump in the artifact. A two-core CLI test now reads `mhartid` from the same ELF,
  branches per hart, writes distinct per-hart memory slots, and verifies the
  post-run memory dump plus per-core store stats. CLI tests also execute
  `rdcycle` and `rdinstret` from an ELF and verify the resulting architectural
  registers in the JSON artifact. Instruction-limit CLI tests require
  `--max-instructions <count>` to stop before a later guest trap, report the
  `instruction_limit` stop reason in the artifact and stats, and reject the
  limit when execution is not enabled or the count is zero. A tick-limit CLI
  test runs a non-trapping branch loop and requires the configured `--max-tick`
  budget to stop with a `tick_limit` artifact and stats stop reason instead of
  a CLI error. CLI source-policy tests now keep the binary crate root under the
  facade budget while `config.rs` owns argument parsing and `guest_memory.rs`
  owns ELF plus external-blob store construction, so standalone simulator
  growth does not accumulate in one gem5-style launch file.
- UART/MMIO tests cover transmitted byte logging, RX FIFO status, snapshot
  restore, serial and parallel RX interrupt assertion, serial interrupt
  deassertion after the final byte is read, and rejection of serial or parallel
  RX interrupt delivery before RX FIFO or consumed-byte state is mutated. PL011
  UART tests cover gem5-visible register reads and writes, RX/TX data behavior,
  PrimeCell ID reads through `rem6-amba`, raw and masked RX/TX interrupt state,
  final-byte serial interrupt deassertion, parallel masked RX assertions, and
  typed DMA-enable rejection instead of gem5's panic path. Topology checkpoint
  tests cover PL011 platform retention, automatic host checkpoint bank
  attachment, checkpoint chunk capture, and restore through host actions.
- Timer/MMIO tests cover typed RISC-V CLINT `msip` software interrupts,
  `mtimecmp` timer interrupt scheduling, future-deadline timer deassertion,
  read-only `mtime` from scheduler ticks, the same `mtimecmp` path under the
  parallel scheduler, RTC-driven `mtime` advancement from typed serial and
  parallel RTC pulse sources, MTIP delivery on `mtimecmp` reach, and CLINT
  snapshot/restore of per-hart `msip`, `mtimecmp`, timer assertion, and
  RTC-backed `mtime` state. CLINT reset tests cover `msip` clearing,
  `mtimecmp` reset policy, timer-assertion clearing, serial and parallel typed
  interrupt deassertion, and stale timer-event invalidation through generation
  changes. Programmable timer tests also cover serial and parallel rejection of
  invalid interrupt routes before arm state is committed, plus serial and
  parallel rejection of deadline-event delivery that violates remote lookahead
  before any timer generation is persisted. CLINT tests cover constructor-time
  rejection of invalid hart interrupt routes before any MMIO, reset, or RTC
  callback can observe partial device state. They also cover `msip`,
  immediate `mtimecmp`, and RTC-driven timer assertion failures that reject
  remote delivery before changing the guest-visible CLINT register or
  asserted-line state.
- RTC core tests cover MC146818-compatible binary and BCD calendar registers,
  status A/B defaults, status C/D reads, read-clear status-C flags, leap-day
  rollover, SET-bit clock freeze, alarm and update-ended flags, alarm wildcard
  matching, raw register snapshots, snapshot restore, and typed rejection of
  unsupported status writes, read-only status writes, unknown registers, and
  invalid BCD data instead of gem5-style panic paths. RTC MMIO tests cover
  CMOS address/data port routing, NMI-mask preservation with low-7-bit register
  selection, RTC data routing, nonvolatile CMOS bytes, wrapper snapshot/restore,
  serial and parallel MMIO response paths, serial and parallel periodic
  interrupt pulse delivery, serial and parallel alarm/update tick interrupt
  delivery, status-C read-clear behavior after periodic pulses, PIE-clear
  stale-event invalidation, and typed rejection of bad widths, unmapped offsets,
  and read-only RTC writes. Platform tests cover RTC MMIO bus attachment,
  retained device lookup, and periodic interrupt route wiring, while system
  checkpoint tests cover RTC bank decode-first restore, status-C flag
  preservation, host checkpoint-action round trips through manifests, and
  automatic host checkpoint attachment for platform-owned RTC devices. PL031
  tests cover elapsed data-register reads, load and match writes, ignored
  control writes, PrimeCell ID reads, ignored PrimeCell ID writes, raw and masked interrupt latching, interrupt clearing,
  snapshot restore, no-route match status latching, serial and parallel MMIO
  access, serial and parallel match interrupt pulses, typed unknown-register
  errors, avoidance of default wrap-distance idle drains after load writes,
  checkpoint bank decode-first restore without partial mutation, and host
  checkpoint-action capture/restore through manifests. Platform and topology
  tests cover PL031 MMIO bus attachment, interrupt routing, retained device
  lookup, and automatic topology host checkpoint attachment. SP804 tests cover
  dual-timer core countdown, one-shot interrupt latching and clearing,
  background-load periodic reload, gem5-compatible prescale timing, dual MMIO
  timer windows, PrimeCell ID reads, ignored PrimeCell ID writes, serial and parallel response paths, serial and parallel
  interrupt pulses, typed width or unknown-register errors, platform MMIO bus
  attachment with paired interrupt routes, retained device lookup, checkpoint
  bank decode-first restore without partial mutation, host checkpoint-action
  capture/restore through manifests, and automatic topology host checkpoint
  attachment. CPU-local timer tests cover per-CPU timer countdown,
  auto-reload, zero-load minimum-decrement scheduling, interrupt latching and
  clearing, watchdog timer-mode countdown, watchdog-mode reset assertion
  evidence without simulator termination, watchdog disable sequencing,
  partition-selected MMIO dispatch, typed interrupt deassertion on clear, and
  both serial and parallel MMIO scheduler paths, plus platform builder
  attachment with retained device lookup, per-CPU MMIO route coverage
  validation, source-partition-local bus views, and per-CPU timer/watchdog
  interrupt routes, plus checkpoint bank decode-first restore without partial
  mutation, CPU-count validation, host checkpoint-action capture/restore
  through manifests, and automatic topology host checkpoint attachment. SP805
  tests cover watchdog countdown, zero-load minimum-clock
  scheduling, lock behavior, raw and masked interrupt state, reset-assertion
  records, serial interrupt assert and clear delivery, parallel MMIO response
  paths, PrimeCell ID reads, typed width errors, typed unknown-register errors,
  typed unsupported integration-test harness errors, platform MMIO bus
  attachment with optional interrupt routing, retained device lookup,
  checkpoint bank decode-first restore without partial mutation,
  host checkpoint-action capture/restore through manifests, and automatic
  topology host checkpoint attachment.
- PLIC/MMIO tests cover PLIC-compatible 32-bit priority, pending, enable,
  threshold, and claim/complete windows, including serial enable and threshold
  filtering before claim, parallel repeated-claim behavior before matching
  completion, context-indexed enable and threshold/claim routing for distinct
  target partitions, typed wrong-completion errors that preserve claimed state,
  and platform interrupt-controller routing through the same PLIC register map
  advertised by the RISC-V device tree. Platform tests also cover typed PLIC
  context records driving both gem5-style `interrupts-extended` DTB metadata and
  context-specific MMIO claim routing for a second hart target. PLIC tests now
  cover context-local snapshot and restore of enable and threshold state, while
  system checkpoint tests cover PLIC bank decode-first restore and host
  checkpoint-action round trips through manifests. System topology tests also
  cover automatic host checkpoint attachment for platform-owned PLIC devices.
- MMIO tests cover typed unsupported-device trap regions for gem5 `BadDevice`
  alignment, including serial access logs, parallel bus completion errors, name
  validation, and direct range-crossing rejection without simulator-wide panic.
- System action tests cover CLINT checkpoint-bank capture and restore through
  host checkpoint manifests for per-hart `msip`, `mtimecmp`, timer assertion,
  and RTC-backed `mtime` state.
- Platform and topology tests cover declared CLINT hart interrupt routes, CLINT
  MMIO bus routing, declared CLINT reset policy plumbing, and automatic host
  checkpoint-bank attachment for platform CLINT devices.
- PCI tests cover typed legacy INTx line mapping, invalid interrupt-pin and
  zero-line rejection, bridge path swizzling across upstream functions,
  explicit root routing-table entries with fallback policies, stable byte
  codecs for routing-table and router snapshots, deterministic routing-table
  ordering, snapshot/restore of platform routing tables, typed
  endpoint config accessors for legacy interrupt line/pin fields, router
  construction of endpoint-facing ports from endpoint config or bridge-swizzled
  paths, host-bridge derivation of nested bridge paths from type-1 bus ranges,
  typed assignment of resolved routing lines back into endpoint config space,
  idempotent interrupt-controller route registration, endpoint identity
  preservation after root-line mapping, serial and parallel endpoint post/clear
  delivery through rem6-interrupt, and observable delivery errors when a
  parallel clear targets a mismatched source. System checkpoint tests cover PCI
  legacy INTx router checkpoint banks with staged malformed-payload rejection
  before live router or registry mutation. PCI config and host tests cover
  64-bit memory BAR lower
  and upper config dwords, upper-slot reservation, invalid BAR pairing, one
  active logical range per 64-bit BAR pair, and host memory-space mapping of
  full 64-bit PCI BAR bases. They also cover stable BAR payload encoding for
  endpoint checkpoint audit, including BAR shape, lower and upper raw register
  state, and malformed payload rejection. Legacy I/O BAR tests cover
  fixed-address ranges, ignored config BAR writes, command-bit gating, and
  Type-1 I/O-window filtering of downstream host mappings. Type-0 header
  tests cover Cardbus CIS, subsystem IDs, common command writes with
  reserved-bit masking, common cache-line-size, latency-timer, and BIST byte
  writes, status
  write-one-to-clear behavior that preserves capability-list state, Expansion
  ROM reads and writes, Expansion ROM size probing, and minimum-grant plus
  maximum-latency read-only byte behavior. PCI config MMIO tests cover masked
  writes with empty masks, command-halfword byte enables, and non-contiguous
  byte enables that update latency-timer and BIST without widening into
  read-only header bytes, while preserving invalid-width errors for original
  config accesses wider than the typed PCI config model accepts. Endpoint,
  type-1 bridge, and host snapshot tests also cover byte-exact config-space
  checkpoint audit payloads with function-set mismatch, byte mismatch, and
  truncated-payload rejection.
  Capability-list tests cover ordered PM plus PCIe plus MSI chaining, MSI plus
  MSI-X chaining, next-pointer preservation across capability control writes,
  raw read-only vendor-specific capability installation, raw capability
  next-pointer ownership, raw snapshot shape checks, raw stable payload
  encoding for endpoint checkpoint audit, raw capability invalid offset and
  size rejection, PMCSR snapshot restore, PCIe
  device/link/slot/root/capability2 control/status snapshot restore, PCIe
  stable payload encoding for endpoint checkpoint audit, PCIe read-only device
  and slot capability rejection, and overlap rejection without mutating
  existing config bytes. MSI tests cover capability header exposure,
  clamped vector enable state, message-address masking, vector masks, snapshot
  restore, stable payload encoding for endpoint checkpoint audit, duplicate and
  invalid capability layouts, and serial plus parallel MSI delivery through
  typed rem6-interrupt routes. MSI-X tests cover typed
  capability table and PBA register exposure, BAR-local table programming,
  vector and function masks, table plus pending-bit snapshot restore, stable
  payload encoding for endpoint checkpoint audit, invalid and overlapping
  layout rejection, serial delivery, and masked parallel delivery recording
  into the PBA. CLI source-policy tests keep standalone run configuration and
  guest-memory loading in focused modules while enforcing the facade and hard
  module-size budgets across the binary crate. PCI source-policy tests keep the
  crate root below the facade budget and all source files below the hard
  module-size budget. CPU source-policy tests keep the CPU crate root below
  the facade budget, keep RISC-V data issue in a focused module, and enforce
  the hard module-size budget across CPU sources. Platform source-policy tests
  keep the platform crate root below the facade budget, keep RISC-V
  device-tree code in a focused module, and enforce the hard module-size budget
  across platform sources. Memory source-policy tests keep request and response
  state in a focused module while enforcing the hard module-size budget across
  memory sources. DRAM source-policy tests keep public error state and target-backed
  memory-controller state in focused modules while enforcing the hard
  module-size budget across DRAM sources. Interrupt source-policy tests keep
  generic interrupt errors, route contracts, event records, and snapshots in
  focused modules while enforcing the facade and hard module-size budgets
  across interrupt sources. Storage source-policy tests keep the initial
  storage crate within the facade and hard module-size budgets. Coherence
  source-policy tests keep harness errors, CPU response records, and
  partitioned configuration in focused modules while enforcing the hard
  module-size budget across coherence sources.
  Cache source-policy tests keep MSI and MESI controller state in focused
  modules while enforcing the hard module-size budget across cache sources.
  Transport source-policy tests keep route and message-buffer contracts in
  focused modules while enforcing the facade and hard module-size budgets
  across transport sources.
  GPU source-policy tests keep command, trace, snapshot, and error contracts in
  focused modules while enforcing the facade and hard module-size budgets
  across GPU sources.
  Accelerator source-policy tests keep command contracts, snapshot contracts,
  and error reporting in focused modules while enforcing the facade and hard
  module-size budgets across accelerator sources.
  VirtIO source-policy tests keep queue contracts and error reporting in focused
  modules while enforcing the facade and hard module-size budgets across VirtIO
  sources.
  UART source-policy tests keep event, snapshot, and error contracts in focused
  modules while enforcing the facade and hard module-size budgets across UART
  sources.
  Network source-policy tests keep error reporting in a focused module while
  enforcing the facade and hard module-size budgets across network sources.
  System source-policy tests keep workload replay data-cache backend state in a
  focused module while enforcing the hard module-size budget across system
  sources. Type-1 bridge tests cover typed bridge header fields, Expansion ROM
  reads and writes, Expansion ROM size probing, interrupt line/pin bytes,
  bridge-control writes, common command writes with reserved-bit masking,
  common cache-line-size, latency-timer, and BIST byte writes, common status
  writes that do not create guest-owned bits, bridge
  BAR0/BAR1 install and command-bit-gated host mapping, snapshot restore of
  bridge config and BAR state, stable bridge BAR payload encoding for
  checkpoint audit, writable bus-number and memory-window registers,
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
  mutates device state. Endpoint and type-1 bridge snapshots now also expose
  byte-exact config-space payloads for checkpoint audit, and host snapshots
  aggregate those payloads in function-sorted bridge and endpoint maps.
  `rem6-system` now exposes a PCI host checkpoint bank that captures the
  topology payload plus bridge and endpoint config-space and BAR payload maps
  into host checkpoint manifests, decodes manifest payloads back into typed
  audit records in deterministic component order, and prevalidates restore
  attempts against the live host topology, config-space bytes, and BAR payloads
  when the corresponding chunk families are present. Topology-only legacy PCI
  host manifests still restore through topology validation, and config-only
  manifests still restore through topology plus config-space validation. BAR
  chunks without config-space chunks, complete config-and-BAR manifests without
  endpoint capability chunks, and fully populated endpoint capability chunk
  families are handled as distinct manifest generations. Partially missing
  config, BAR, or endpoint capability chunks, malformed payload maps,
  truncated function payloads, function-set mismatches, and live-snapshot byte
  mismatches are rejected before broader PCI config restore mutates device
  state. `SystemActionExecutor`
  can be constructed directly with an attached PCI host checkpoint bank. rem6
  still does not pretend that PCI host checkpoint validation alone restores full
  PCI configuration state.
  BAR state now has a stable byte codec for endpoint checkpoint audit,
  preserving endpoint shape, raw lower and upper register state, size-probe
  masks, and 64-bit upper-slot ownership while rejecting malformed or
  ambiguous payloads before broader PCI config restore mutates device state.
  Type-1 bridge snapshots expose the same BAR payload audit API with the
  bridge's two-slot BAR shape, and host snapshots aggregate bridge and
  endpoint BAR payload maps in function order. Bridge or endpoint function-set
  mismatches, malformed BAR payloads, and BAR byte mismatches are rejected
  before restore applies broader bridge or endpoint configuration bytes. Host
  snapshots also aggregate endpoint raw, PM, PCIe, MSI, and MSI-X capability
  payload maps in function order, with key-set validation and capability
  byte validation delegated to each endpoint snapshot.
  Raw capability state now has a stable byte codec for vendor capability audit,
  preserving the canonical capability bytes while rejecting nonzero
  next-pointer bytes because the endpoint capability registry owns chain
  reconstruction. PM capability
  state exposes the same byte-codec pattern: endpoint snapshots can emit a PM
  payload and validate a candidate payload against the typed PM spec plus
  current PMCSR, rejecting malformed bytes or live-snapshot mismatches before a
  broader PCI config checkpoint restore mutates device state. PCIe capability
  state follows the same audit pattern for capability identity,
  device/link/slot/root/capability2 declared fields, and all writable
  control/status registers. MSI capability state follows the same audit pattern
  for vector count, 64-bit and per-vector-mask support, enabled vectors,
  programmed address and data, and mask and pending bits. MSI-X capability
  state does the same for table and PBA BAR placement, enable and function-mask
  bits, table entries, and PBA pending words while rejecting malformed table or
  out-of-range pending bits.
- VirtIO tests cover VirtIO 9P device id, mount-tag feature bit,
  length-prefixed read-only tag config bytes, default request queue sizing,
  notify layout, modern PCI endpoint and capability assembly, config MMIO
  reads, notify MMIO writes through the BAR runtime, and oversize mount-tag
  rejection, 9P split-queue header and payload decoding, reply writable-byte
  accounting, malformed header length, declared-length mismatch, missing or
  readable reply descriptor rejection, guest-memory available-ring consumption,
  guest-memory response scatter writes, used-ring writeback, ISR queue
  interrupts, decode-error cursor preservation, `Tversion` to `Rversion`
  negotiation, client `msize` clamping plus valid-envelope flooring,
  negotiated I/O-unit propagation, negotiated `Rread` and `Rreaddir`
  data-budget enforcement, `Tversion` fid and
  attach-state reset, `Tattach` to
  root-qid response generation, attached-fid metadata
  recording, unsupported-auth-fid rejection, occupied-fid rejection without
  state replacement,
  unsupported-request `Rlerror` replies, and malformed 9P payload
  rejection without completion mutation, explicit `Tauth` no-auth-backend
  rejection with malformed auth parsing errors, in-memory namespace file installation,
  `Twalk` qid-vector replies, missing-name `Rlerror` handling, partial qid
  replies without destination-fid binding, occupied-newfid rejection, `.` and
  `..` directory component traversal, opened non-directory source-fid rejection
  without destination-fid binding, maximum element-count acceptance plus
  over-limit vector rejection and statfs-namelen element rejection before
  completion mutation, same-fid non-empty walk rejection, empty same-fid walk replies,
  `Tmkdir` directory creation, duplicate-name errno replies, directory qid preservation,
  and directory walk/listing behavior, `Tlcreate` root and child-directory file
  creation, reserved-name rejection, duplicate-file rejection without clobbering,
  plus opened-fid retargeting, `Tgetattr` root, directory, and file
  metadata replies, `Tstatfs` deterministic filesystem-capacity replies,
  legacy `Tstat` file stat payloads, stale-fid errno replies, and malformed
  payload rejection, legacy `Twstat` mode, uid, gid, mtime, atime, length, and
  same-parent name updates including walked hard-link fid path selection, file
  shrink visibility through reads, open-fid read survival after rename,
  same-file hard-link target no-ops, old-name rejection, new-name walk qid
  preservation,
  stale-fid errno replies, and malformed
  stat-blob rejection, `Tlopen` and legacy `Topen` file and allowed-mode directory qid plus
  I/O-unit replies, access-mode checks for read-only, write-only, read-write,
  and execute-only opened fids, root and child-directory write-mode open
  rejection without changing fid state, reopen rejection before access-mode mutation, `Tlopen`
  truncate and append flag handling,
  legacy `Topen` truncate, remove-on-close, hard-link remove-on-close, and
  append mode handling, legacy
  `Tcreate` and `Tlcreate` opened-fid access-mode and append-mode propagation,
  legacy `Tcreate` remove-on-close handling, create-on-open-fid rejection without
  namespace mutation, legacy `Tcreate` checked file creation, duplicate-file
  rejection without clobbering, statfs-namelen create-name rejection before
  completion mutation, plus created-fid retargeting, `Treaddir` sorted
  root and child-directory dirents, resumable offsets, count-bounded replies,
  and directory-only error handling, counted `Tread` ranges, `Twrite` counted
  replies plus overwrite mutation, `Tlink` hard-link qid reuse, shared write
  visibility, link-count metadata, and surviving-fid reads after one name is
  unlinked, `Trename` fid-backed cross-directory file
  moves with preserved open-fid access and qid identity plus directory rename
  across same and different parents with descendant fid path updates,
  empty-directory target replacement, non-empty target rejection, and
  descendant-target rejection, `Trenameat`
  same-directory renames, cross-directory file moves including same-name moves,
  preserved moved qids, open-fid access, same-file hardlink target no-ops,
  stale fid-backed rename source rejection, replacement-target fid
  invalidation, directory rename across same and different parents with
  descendant fid path updates, empty-directory target replacement, non-empty
  target rejection, descendant-target rejection, post-rename directory entries,
  and old-name walk rejection,
  `Tunlinkat` root and
  child-directory file removal with post-delete directory and walk checks,
  `Tunlinkat` empty-directory removal through `AT_REMOVEDIR` plus non-empty
  directory `ENOTEMPTY` rejection, `Tremove` fid-backed file removal using the
  walked hard-link entry after normal walk or rename, already-unlinked walked
  hard-link entry rejection without deleting surviving names for the same qid,
  empty-directory removal with stale-fid rejection, root rejection with fid
  clunking, and non-empty directory `ENOTEMPTY` rejection that clunks the remove
  fid,
  `Tsymlink` creation with symlink qids, symlink walk and sorted dirent
  exposure, `Treadlink` target replies, non-symlink and stale readlink
  rejection, `Tmknod` character-device creation, walk, dirent dtype, metadata
  mode, `rdev`, read rejection, stale-parent rejection, duplicate-name
  rejection, file-parent rejection, `Tsetattr` mode, uid, gid, explicit
  atime/mtime, size-valid file shrink, zero-filled growth, and metadata
  visibility, stale setattr rejection, directory size-mutation rejection,
  unsupported ctime-mask rejection, advisory `Tlock` success for open file fids,
  blocked status for overlapping incompatible byte-range locks, full and
  partial unlock requests, unknown lock flag rejection, `Tgetlock` conflict
  payloads and no-conflict unlock-payload reporting, unknown getlock flag
  rejection, `Txattrcreate` value writes, slash-bearing xattr names, and
  commit-on-clunk persistence with `XATTR_CREATE`/`XATTR_REPLACE` flag handling,
  `Txattrwalk` named xattr reads, xattr-list read fids,
  occupied-newfid rejection, missing-xattr `ENODATA`, invalid-flag `EINVAL`,
  and stale-fid errors,
  `Tclunk` fid removal and lock release, `Tremove` lock release while linked
  namespace entries survive plus xattr-fid clunking without pending-write
  commits and pending xattr-write cleanup for deleted backing nodes, `Tflush`
  no-op acknowledgement without fid mutation, `Tfsync`
  acknowledgement for existing fids, and stale metadata,
  directory, create, fsync, write, remove, unlink, and read `Rlerror` handling,
  source-policy coverage for keeping 9P typed payload parsing, wire constants,
  and attach, statfs, walk, open, create, path, metadata, read, write, clunk,
  remove, xattr, directory-read, fsync, lock, and namespace-mutation operation
  handlers out of the main device file and out of the operation-root module
  while keeping protocol dispatch below the focused-device line budget,
  9P completion writeback prevalidation that rejects later response sinks before
  mutating earlier guest data, used-ring state, or ISR state,
  modern PCI version-1 feature exposure for 9P,
  block, console, and RNG, legacy RNG device id and zero-config behavior,
  reproducible entropy generation, writable split descriptor-chain decoding,
  RNG used-ring writeback, guest-memory scatter writes, ISR queue interrupts,
  RNG completion writeback prevalidation before guest-data or used-ring
  mutation,
  readable/empty RNG descriptor rejection, RNG common-config queue exposure,
  notify layout, modern PCI endpoint and capability assembly without a
  device-config capability, common-config MMIO reads, notify MMIO writes through
  the BAR runtime, console device id, size feature,
  80-by-24 config bytes, transmit descriptor decoding into terminal output,
  receive descriptor decoding into scattered host input bytes, console
  used-ring writeback, typed guest-memory available-ring consumption for
  receive and transmit queues, guest-memory receive scatter writes,
  guest-memory transmit input reads, queue-interrupt ISR status after console
  completion writeback, console completion writeback prevalidation before
  guest-data or used-ring mutation, console descriptor direction rejection, read-only console
  PCI device-config bytes, common-config feature and queue exposure, queue
  notify layout, modern PCI endpoint and capability assembly, config MMIO reads,
  and notify MMIO writes through the BAR runtime, legacy MMIO
  magic/version/device/vendor id registers,
  feature-page selection, driver feature validation, queue selection, queue
  sizing, PFN-derived split-ring address layout, device-config window
  forwarding, queue notifications, interrupt status/ack, typed unsupported
  feature, page-size, queue-align, invalid-queue-size, and unavailable-queue
  errors, short-register access errors, read-only register write errors, and
  modern PCI common-config feature-page selection,
  driver-feature writes, queue selection, queue sizing, queue notification
  offsets, queue descriptor/driver/device addresses, queue enable, device-status
  reset, snapshot restore, stable common-config snapshot payload encoding with
  malformed-payload rejection, read-only register rejection, invalid queue-size
  rejection, unavailable-queue write rejection, notify-MMIO address derivation
  from queue notify offsets and notify-off multipliers, serial and parallel
  typed queue notification recording, notify snapshot restore, stable notify
  snapshot payload encoding with malformed-payload rejection, rem6-system
  checkpoint-bank capture/restore through full-system host actions, topology
  host-controller attachment before or after host creation, malformed
  checkpoint payload rejection without live common-register, notification, or
  registry mutation, invalid multiplier rejection, write-only notify behavior,
  and mismatched notify write rejection.
  VirtIO PCI capability tests cover standard vendor-specific
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
  discard, write-zeroes, and secure-erase limits. VirtIO block transport tests
  cover gem5's 128-entry request-queue default, block device-config capacity
  reads, optional multiqueue common-config and notify offsets, modern PCI
  endpoint and capability assembly, config MMIO reads, and notify MMIO writes
  through the BAR runtime. VirtIO block device tests
  cover serial and parallel decoded request execution, typed in-memory sector
  and rem6-storage image backend reads and writes, flush accounting, get-id
  payload padding, queue and request completion traces, unsupported request
  statuses, read-only write protection, out-of-range accesses, backend
  capacity checks, and 512-byte request-shape validation.
  Storage image tests cover raw and file-backed sector reads and writes,
  explicit flush accounting, byte-exact file-backed snapshot restore,
  read-only rejection before mutation, typed range, host-file, and image shape
  errors, nested
  copy-on-write overlays, deterministic dirty-sector snapshots, explicit
  writeback to the child image, and storage checkpoint banks that capture raw,
  file-backed, and COW snapshots and reject malformed chunks without partial
  restore.
  Simple disk tests cover image-to-guest and guest-to-image sector copies,
  transfer records, explicit flush accounting, sector-multiple rejection,
  out-of-range storage preflight without guest-memory mutation, and read-only
  image rejection without storage mutation.
  IDE disk tests cover identify payload geometry, capacity, model, feature,
  ATA-version, and DMA-mode fields, LBA PIO read and write sector transfers,
  CHS rejection before busy state is exposed, range rejection before mutation,
  ATAPI identify abort status, data-direction rejection, software reset, and
  read-native-max task-file updates. IDE disk snapshot tests cover restore of
  task-file state and a partial PIO write transfer, plus mismatch rejection
  before mutation.
  IDE controller tests cover selected primary-channel device PIO routing,
  shared primary/secondary interrupt visibility and clearing, missing selected
  device behavior, BMI capability and interrupt status, PRD table alignment,
  typed DMA readiness and direction rejection, command/control BAR dispatch with
  shifted command offsets and control offset adjustment, data-port word reads,
  secondary BMI window selection, bus-master-disabled write ignores, snapshot
  restore of channel selection, BMI PRD state, disk-transfer cursor state, and
  shape mismatch rejection before mutation. IDE DMA tests cover READ DMA from
  disk to guest memory, WRITE DMA from guest memory to disk, active DMA
  snapshot restore before execution, and malformed PRD rejection without disk
  or guest-memory mutation. IDE checkpoint tests cover active DMA snapshot
  capture/restore through deterministic controller chunks, malformed
  controller chunk rejection, and repaired BMI snapshot rejection without
  partial restore. IDE PCI tests cover the
  PIIX4 vendor/device identity, mass-storage class and programming-interface
  bytes, status and interrupt-line config bytes, five I/O BAR shapes, active
  BAR ranges, dispatch-policy derivation from explicit layout parameters, and
  typed legacy INTx delivery from shared controller interrupt state through the
  parallel scheduler. IDE timing tests cover delayed media-read readiness,
  BSY-before-DRQ state, deferred PCI INTx delivery, multi-sector PIO read
  inter-sector delay before the next DRQ, data-read rejection while the
  inter-sector delay is pending, multi-sector PIO write inter-sector delay
  with sector-granular storage commit, data-write rejection while the
  inter-sector delay is pending, delayed READ DMA guest-memory mutation after
  disk plus sector latency, WRITE DMA PRD rejection before event scheduling,
  and completion-error capture on the parallel scheduler path.
  System checkpoint action tests cover storage image and IDE controller bank
  attachment, staged capture into host manifests, and malformed storage or IDE
  restore rejection without partial live-state mutation. Topology checkpoint
  tests cover storage image and IDE controller port registration before and
  after host-controller creation with automatic host executor attachment.
  VirtIO split descriptor-chain tests cover block
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
  prevalidated multi-queue restore plus topology host-controller attachment
  before or after host creation to avoid partial live-state mutation,
  plus guest-memory writeback for block data buffers, status bytes, used
  elements, used indices, legacy available-ring interrupt suppression,
  event-index interrupt suppression, block completion writeback prevalidation
  before guest-data, status-byte, or used-ring mutation, and queue-interrupt ISR status after
  completion writeback, with serial and parallel PCI legacy INTx delivery.
  VirtIO PCI ISR-status tests cover queue and configuration-change bit
  recording, serial and parallel read-clear behavior, snapshot restore,
  stable checkpoint payload encoding, rem6-system checkpoint-bank
  capture/restore through full-system host actions, malformed payload rejection
  without live ISR or registry mutation, reserved-bit masking, read-only write
  rejection, width errors, and boundary errors. VirtIO PCI device-config tests
  cover typed mutable and read-only byte masks, serial and parallel config reads
  and writes, byte-mask writes, snapshot restore, stable snapshot payload
  encoding with malformed-payload rejection, rem6-system checkpoint-bank
  capture/restore through full-system host actions, malformed checkpoint payload
  rejection without live config or registry mutation, access trace recording,
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
  in the installed DTB. They also cover automatic host checkpoint-bank
  attachment for platform PLIC, CLINT, timer, UART, and generic interrupt
  controller state. The typed RISC-V Linux boot handoff also installs
  initrd bytes and matching DTB metadata into both store-backed and DRAM-backed
  memory. Workload replay tests cover resolved Linux device-tree and initrd
  payload installation into guest memory snapshots from typed manifest handoff
  state, and reject resolved payload sets bound to a different manifest
  identity.
- Proto-boundary tests cover typed instruction, packet, and O3 dependency trace
  records, one-of instruction encoding, memory-access and packet-size
  validation, duplicate id-string rejection, canonical id-string ordering,
  dependency sequence/window validation, duplicate dependency-record rejection,
  stable trace identity, O3 prefetch records with retire-after-issue memory
  completion policy, binary frame round-trip, frame kind validation,
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
  snapshot restore, including duplicate component-id rejection. Power-model
  tests cover residency-weighted dynamic/static watt aggregation,
  static/dynamic-only modes, temperature updates, missing state-model rejection,
  and snapshot restore. Power-expression tests cover
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
  inputs, expression input temperature coupling, initialized passive junction
  nodes, absolute-zero temperature rejection, passive-only network rejection,
  invalid topology rejection, typed node-initialization evidence, restore
  rejection for partial or mismatched initialization evidence, update history,
  and snapshot restore.
- Debug packet tests cover GDB remote packet checksum encoding and decoding,
  malformed frame rejection, bounded payload enforcement, acknowledgement,
  negative acknowledgement, interrupt, and skipped-prefix stream parsing before
  socket or ISA register-cache integration.
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
- Complete predictor coupling, external checkpoint payloads, and richer
  cycle-visible state for the in-order pipeline, add fuller out-of-order
  pipeline execution, checker, richer branch predictors, and host-assisted
  execution models with checkpointable state.
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
