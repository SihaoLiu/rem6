# Parallel-First Full-System Kernel

This document records the architectural constraints for rem6 as a
parallel-first, cycle-accurate, heterogeneous full-system simulator. It is a
design gate for future work across the workspace. A new component is aligned
with rem6 only when it moves the implementation toward these constraints.

## Scope

rem6 is a Rust full-system simulation workspace. It targets CPU, GPU, and
accelerator systems with explicit topology, NoC transport, cache hierarchy,
coherence, DRAM-class memory, checkpointing, host control, and statistics. The
goal is gem5-equivalent modeling breadth with a more modern internal runtime.

The key architectural choice is that parallel simulation is not an optional
optimization. It is the default execution model. Serial execution may exist for
small tests and debugging, but it must be a special case of the same partitioned
runtime semantics.

## Research Input

The following public sources shape this design:

- gem5 event-driven programming documentation describes a callback event model
  centered on scheduled events and ticks:
  <https://www.gem5.org/documentation/learning_gem5/part2/events/>
- gem5 memory-system documentation describes ports, packets, requests, and the
  earlier attempt to unify timing and functional access semantics:
  <https://www.gem5.org/documentation/general_docs/memory_system/>
- gem5 Ruby documentation describes Ruby as a detailed coherence and network
  model that is mostly a drop-in replacement for classic memory, while classic
  caches and Ruby are mostly incompatible:
  <https://www.gem5.org/documentation/learning_gem5/part3/MSIintro/>
- gem5 queued prefetcher source limits candidate generation with
  `throttle_control_percentage` and issued/useful prefetch accuracy:
  <https://gem5.googlesource.com/public/gem5/+/c76fa4d39e0c736d333a5759dcd2e69dfb2082c6/src/mem/cache/prefetch/queued.cc>
- gem5 SLICC documentation describes a protocol DSL that generates C++
  controllers and exposes complex transient-state queue handling:
  <https://www.gem5.org/documentation/general_docs/ruby/slicc/>
- Historical Parallel M5 notes describe the long-standing desire to replace
  one global queue with per-object event queues, deterministic parallel
  execution, barriers, slack, and one event queue per simulation thread:
  <https://old.gem5.org/Parallel_M5.html>
- Historical gem5 configuration notes describe existing configurations as hard
  to learn and hard to maintain, and call out asymmetric clusters and DVFS-style
  clocks as difficult under the old setup:
  <https://old.gem5.org/Configuration_musings.html>
- gem5 checkpoint documentation records that Ruby checkpoints require the MOESI
  hammer protocol because only that protocol can flush caches to memory for
  correct checkpoint state:
  <https://www.gem5.org/documentation/general_docs/checkpoints/>
- gem5art documentation states that full-system experiments quickly become
  complicated due to many artifacts, and that the process is difficult even for
  experienced researchers:
  <https://www.gem5.org/documentation/gem5art/main/summary>
- parti-gem5 identifies gem5's single-threaded simulation kernel as a major
  throughput limit, describes thread safety as hard because gem5 was designed
  for sequential execution, and reports speedups with timing deviations:
  <https://arxiv.org/abs/2308.09445>
- The gem5 packaging discussion identifies many build and run-time
  dependencies, numerous compile-time options, and individually managed source
  builds as a reproducibility and onboarding burden:
  <https://www.gem5.org/project/2022/05/23/guix.html>
- Sustainable gem5 Simulations reports large simulation slowdowns and linear
  multi-core time growth from gem5's single-threaded execution:
  <https://www.gem5.org/assets/files/workshop-isca-2023/slides/sustainable-gem5-simulations.pdf>
- The gem5 resources reproducibility work identifies complex disk image
  creation, limited guest-host communication, and external scripting for
  multiple workloads as reproducibility problems:
  <https://arxiv.org/abs/2512.13479>
- Recent gem5 call-stack profiling work identifies layered runtime complexity
  and hard-to-pinpoint coherence deadlock and livelock:
  <https://arxiv.org/abs/2605.01419>
- gem5 branch predictor discussion identifies incomplete speculative history
  support and history unwinding as a source of misleading predictor results:
  <https://github.com/orgs/gem5/discussions/1341>

## Debt Map

The following table is the working debt map. Each rem6 countermeasure must be
backed by tests, traces, or explicit runtime records.

| gem5 pressure | rem6 countermeasure | Required evidence |
| --- | --- | --- |
| Single-threaded simulation kernel limits multi-core throughput. | Partitioned conservative runtime is the default scheduler. | Tests show independent partitions execute in parallel epochs with deterministic tick order, and workload manifests can require batch count, dispatch progress, and worker activity derived from the strongest available aggregate, batch histogram, exact partition-set histogram, same-partition-set streak, or per-partition activity evidence, per-partition remote activity derived from remote-flow records, initial or final frontier minima, and worker-use evidence. |
| Parallel extensions are added around an older serial core. | Every core, cache, directory bank, NoC tile, memory channel, GPU unit, and accelerator engine has partition ownership. | Topology tests reject components without a partition, and run summaries report active partitions from aggregate counts, exact batch partition-set unions, activity-derived partition unions, or remote-flow source/target unions. Summary work flags are also driven by typed partition evidence and frontier records rather than by worker aggregates alone. |
| Classic cache and Ruby coherence stacks are split. | Memory, cache, coherence, NoC, and DRAM use one transaction and message vocabulary. | Cross-crate tests move CPU, GPU, and DMA traffic through the same transport path, and workload replay manifests can require attributed data-cache runs with no unattributed bridge activity, internally consistent data-cache run accounting, and recorded MSI/MESI/MOESI/CHI data-cache protocol runs. |
| Ruby protocols encode topology and protocol behavior together. | Protocol crates own state machines; topology and transport crates own placement and routing. | Protocol tests run without topology, and topology tests swap protocol backends without changing routes. |
| SLICC-generated controllers hide transient-state behavior behind generated code. | Rust protocol engines expose transitions, pending transactions, stalls, and causal messages as typed records. | State-machine tests assert transition records and stalled-request wakeups. |
| Replacement policy state can be hidden behind cache tags. | Cache replacement policies are typed per-set records with explicit invalidate, reset, touch, victim, signature training, and snapshot operations. | Replacement tests assert LRU, FIFO, MRU, LFU, BRRIP, BIP, SHIP, SecondChance, and TreePLRU decisions and restore behavior. |
| MSHR resources can become implicit queue state inside a cache. | Cache MSHR queues are typed records with explicit entry allocation, target coalescing, prefetch reserve, ready state, service state, retry state, completion, and snapshot operations. MSI cache banks can attach these queues, replay coalesced same-line read targets from a single fill, and expose the fan-out through harness and transport CPU response records. MESI and MOESI cache banks use the same typed queue shape for same-line read miss coalescing and target-outcome fan-out. | MSHR, MSI bank, MESI bank, and MOESI bank tests assert target merging, target limits, demand reserve behavior, ready ordering, service transitions, completion, restore behavior, same-line read miss coalescing, restored coalesced targets, harness-level coalesced response fan-out, shared response-outcome fan-out helpers for MSI transport paths, and MESI/MOESI bank target-outcome fan-out. |
| Prefetcher state can be hidden behind cache timing side effects. | Stride prefetch state is an explicit per-requestor PC table with confidence, stride, deterministic replacement, and snapshot records. Queued prefetch state is a typed resource with latency, capacity, line size, duplicate filtering, higher-priority duplicate updates, same-line demand squash, explicit cache and miss-queue residency filtering, full-queue policy, ready-tick ordering, same-tick priority ordering, stable order ties, next-ready-tick visibility, issue width, accepted/duplicate/priority-update/redundant/throttled/full result counts, accuracy throttle control percentage, issued/useful counters, useful-count invariants, and snapshot restore before packet creation or cache-controller side effects. | Prefetcher tests assert context isolation, confidence-gated degree candidates, queued latency, duplicate filtering, duplicate priority updates, same-line demand squash, redundant residency filtering, lowest-priority oldest full-queue eviction, next-ready-tick visibility, priority-aware ready issue order, issue-width limiting, accuracy-based max-permitted throttling, throttled enqueue counts, useful-count rejection above issued count, and snapshot restore. |
| QoS scheduling can depend on memory-controller pointers and global requestor lookup. | QoS arbitration is explicit typed state with requestor IDs, checked priorities, queue policy state, snapshots, fabric batch reservation in grant order, transport first-hop batch scheduling through a shared arbiter, DRAM same-arrival timing batches ordered before bank, row, and bus timing are computed, typed read/write direction preference among same-priority timing candidates, and explicit same-requestor priority escalation inside DRAM timing batches. Fixed-priority assignment, FIFO/LIFO/LRG queue arbitration, NoC link reservation, transport event timing, DRAM timing order, DRAM turnaround choice, and escalation do not require a global object graph. | QoS tests assert fixed requestor priority assignment, default priority fallback, priority validation, highest-priority selection, FIFO/LIFO ordering, LRG requestor rotation, non-mutating empty polls, snapshot replay, QoS-driven shared-link reservation order, transport-level first-hop scheduling order, DRAM-level timing batch order, DRAM read/write direction preference, and same-requestor priority escalation. |
| Deadlock and livelock can look like a normal long run. | Runtime-level progress monitors and protocol-level wait-for graphs are required for blocking resources. | Tests inject wait-for cycles and progress-free transition windows, then assert bounded diagnostics rather than silent hangs. |
| Checkpoint correctness depends on a specific protocol flush path. | Checkpointing snapshots partition state, pending events, stores, directories, caches, and devices through protocol-neutral traits. | Checkpoint tests cover MSI, MESI, MOESI, CPU, GPU, accelerator, fabric, and memory state. |
| KVM, fast-forwarding, and model switching are external workflow choices. | Execution modes are modeled as host-controlled runtime actions with explicit statistics scope. | Host-control tests show ROI, switch, and statistics actions as traceable events. |
| Front-end speculation can hide predictor state and unwind behavior. | Branch predictors, GShare predictors, BiMode predictors, Tournament predictors, loop predictors, TAGE predictors, LTAGE predictors, TAGE-SC-L predictors, standalone multiperspective perceptron predictors, statistical correctors, branch-target buffers, indirect target predictors, and return stacks are per-model typed state with explicit prediction, lookup, update, replacement, speculative history, repair, and snapshot records. | Predictor tests assert counter training, GShare PC-history indexing, BiMode choice and selected-array training, Tournament local/global/choice training, loop trip-count learning, LTAGE loop override and repair, TAGE-SC-L SC override and ordered training, multiperspective perceptron 8KB profile shape, filter transitions, per-CPU histories, adaptive training, statistical-corrector GEHL override and repair, TAGE folded-history indexing and provider selection, target lookup, deterministic target replacement, indirect path history, return-stack operations, speculation commit, speculation repair, restore behavior, and incompatible snapshot rejection. |
| Full-system experiments need external scripts and fragile artifacts. | Workload manifests, resources, host events, checkpoints, and result metadata are first-class rem6 data. | Manifest tests reconstruct runs from recorded metadata and reject missing inputs. |
| Profiling often observes the simulator indirectly. | Run summaries expose scheduler, fabric, DRAM, coherence, device, host, and trace activity from the runtime. | System tests assert resource profiles match per-component activity counts, and workload manifests can require fabric, DRAM, or aggregate resource activity minima. |

## Non-Negotiable Invariants

The invariants below apply to production components in the workspace.

1. Partition ownership is explicit.
   A component that can schedule work or hold timing state must have a
   `PartitionId` or an equivalent typed owner that maps to a scheduler
   partition. The owner cannot be inferred from insertion order.

2. Cross-partition communication is a message.
   A partition must not mutate another partition's timing state directly. It
   must emit a typed message with source, target, send tick, delivery tick, and
   deterministic ordering identity.

3. Remote messages require lookahead.
   A remote message must respect the scheduler's minimum remote delay. A model
   that needs zero-delay coupling must be modeled inside one partition or must
   use a specialized synchronizing primitive with explicit tests.

4. Local events and remote messages remain distinct.
   A local pipeline event can occur below the remote lookahead. A remote message
   cannot bypass the lookahead by masquerading as a local callback.

5. Event ordering is deterministic.
   Equal-tick work must have a stable tie-break tuple. The tuple must be
   visible enough for tests and trace comparison.

6. Parallel execution is conservative by default.
   The scheduler may execute independent partitions concurrently only up to a
   horizon that cannot be invalidated by legal remote messages.

7. Serial execution is a compatibility view.
   A serial run should use the same event semantics as a parallel run. It may
   reduce scheduling overhead, but it must not allow behavior that the parallel
   runtime would reject.

8. Protocol transitions are typed data.
   Coherence state transitions, snoops, invalidations, writebacks, ownership
   transfers, retries, and stalls must be observable as typed records. They must
   not be buried in string traces.

9. Time and data movement are separate records.
   A packet can carry data, but the timing event, route, protocol transition,
   memory service, and data update must be independently attributable.

10. Checkpoints are protocol neutral.
    A snapshot cannot depend on a particular coherence protocol's ability to
    flush dirty lines to backing memory. Dirty ownership, pending invalidations,
    transient transaction buffers, fabric queues, and scheduler frontiers must
    be checkpointable state.

11. Host control is part of the simulated system boundary.
    ROI markers, exit events, statistics resets, checkpoint requests, CPU mode
    switches, device launches, and guest traps must pass through a typed
    host-control channel.

12. Observability is a runtime feature.
    Each run summary must expose enough information to explain where time went:
    scheduler epochs, empty epochs, active partitions, fabric transfers, DRAM
    accesses, coherence transactions, device work, host actions, and stop
    reason.

13. Backpressure is modeled explicitly.
    Queues, credits, MSHRs, transaction buffers, DRAM banks, NoC virtual
    networks, and device command queues must expose occupancy and stall causes.

14. Blocking requires a wait-for edge.
    A component that blocks on another component, resource, or protocol event
    must record that dependency in a form that can be inspected by tests and
    diagnostics.

15. Model switching preserves authority.
    When a CPU or device changes execution mode, the authoritative architectural
    state and pending timing state must be transferred through a typed snapshot
    or handoff protocol.

16. External references are never dependencies.
    Code under `temp` can inform design choices, but production crates must not
    import it, invoke it, depend on its build outputs, or mention it as a
    runtime prerequisite.

## Wait-For Graph Policy

Parallel full-system simulation cannot treat a blocked model as an opaque lack
of ready events. A blocked cache, directory bank, fabric lane, DRAM bank, device
queue, interrupt target, or host-control action must name what it is waiting
for. The runtime can then distinguish valid contention from a cycle that will
never resolve.

The wait-for graph is not a string log. It is typed diagnostic state with nodes,
edge kinds, observed ticks, and repeated-observation counts. The graph must be
small enough to keep during normal tests and structured enough to expose through
run summaries.

Node categories:

- transactions, such as memory requests, DMA commands, interrupt deliveries, or
  host-control actions;
- resources, such as cache lines, MSHRs, directory entries, virtual networks,
  DRAM banks, device queues, or checkpoint barriers;
- components, such as cores, cache controllers, directory banks, memory
  controllers, fabric routers, and device engines.

Edge kinds:

- queue wait, used when a request cannot enter or advance in a bounded queue;
- resource wait, used when a request depends on ownership, credits, bank state,
  or an in-flight fill;
- message wait, used when a component is waiting for a response, snoop,
  invalidation acknowledgement, interrupt acknowledgement, or host reply;
- barrier wait, used when a partition must wait for a checkpoint,
  synchronization, mode switch, or externally requested quiescence.

A blocking site must record a wait-for edge at the boundary where the model
returns a structured busy result. It should not wait until a scheduler timeout,
because the scheduler may be making valid progress elsewhere. The edge source
should be the blocked transaction when one exists. The target should be the
resource or component that must become ready before the transaction can
continue.

Repeated blocking on the same source, target, and edge kind updates the existing
edge instead of appending duplicate records. The first-observed tick preserves
when the stall became visible. The last-observed tick and observation count show
whether the same dependency keeps recurring.

A resolving event must clear the matching dependency at the same semantic
boundary that makes the blocked work retryable. Examples:

- a cache fill clears waits targeting the cache line for that requester;
- a snoop acknowledgement clears waits targeting the acknowledgement resource;
- a DRAM bank completion clears waits targeting that bank request slot;
- a fabric credit return clears waits targeting the credit resource;
- a checkpoint barrier release clears waits targeting the barrier resource.

The clear operation should be scoped. Clearing every wait in the graph hides
unrelated problems. Clearing only by transaction can miss multiple requests that
were blocked by one shared resource. Resource-scoped clearing is preferred when
one fill, credit, or bank completion can unblock several transactions.

The graph is diagnostic state, not a scheduling mechanism. The scheduler should
not use it to decide which event is legal to execute. Scheduling legality still
comes from conservative frontiers, lookahead, event ticks, and message delays.
The graph explains why work is blocked and gives deadlock detection a bounded
input.

Deadlock detection runs over the graph and reports a cycle as typed data. A
diagnostic should include:

- nodes in the cycle;
- edge kinds in traversal order;
- first and last observed ticks;
- observation counts;
- partition or component identity when available.

Normal contention must not be reported as deadlock. A miss waiting on a cache
line while an earlier fill is in flight is a valid queue wait. It becomes
suspicious only when the dependency graph cycles or when a policy-specific
livelock monitor observes repeated transitions without useful work.

The livelock monitor is also typed state. It records the subject being watched,
the transition kind, the active progress-free transition window, the last useful
work tick, and a deterministic diagnostic once a declared transition threshold
is reached. Parallel scheduler workers can emit typed progress-free transition
records during callback execution; batch, epoch, and run summaries aggregate
those records deterministically and can replay them into a monitor snapshot.
Workload clean-diagnostic expectations may declare the transition threshold; a
replay summary uses the lowest declared threshold so a stricter diagnostic scope
cannot be hidden by a looser one. The system-run object exposes CPU-scheduler,
data-cache scheduler, and merged full-system progress counts plus
threshold-driven livelock diagnostic counts before workload replay translates
them into manifest-verifiable result summaries. Useful work resets the active
window so retry-heavy but productive models do not look like livelock.

Tests for each integration layer should cover both outcomes:

- a normal blocked request records a wait-for edge and clears it after the
  resolving event;
- repeated blocking on the same dependency updates observation counts;
- an injected dependency cycle reports a deadlock diagnostic;
- repeated progress-free transitions report a livelock diagnostic;
- parallel worker progress records aggregate into run-level monitor snapshots;
- useful work clears the active progress-free transition window;
- unrelated dependencies remain in the graph when one resource resolves.

Run summaries should include wait-for, deadlock, and livelock diagnostics only
when useful. A normal completed run can report zero remaining edges and zero
livelock diagnostics. A stopped, timed-out, or diagnostic run should expose the
graph snapshot or the bounded diagnostic that explains why forward movement
stopped.

The policy applies to CPU, GPU, accelerator, DMA, interrupt, timer, and host
traffic. Heterogeneous models must not bypass it by reporting only device-local
busy flags. If a device engine blocks on memory, credits, command queue space,
or host action, that dependency belongs in the same wait-for vocabulary as CPU
coherence traffic. System-run diagnostics merge fabric, DRAM, and data-cache
wait-for edges before checking full-system deadlocks, so a cycle that spans two
subsystems is not hidden by clean per-subsystem graphs. Workload-result
summaries carry the merged resource and full-system deadlock counts forward so
manifest clean-diagnostic checks see the same cross-subsystem cycles as the
system run.

## Workspace Responsibilities

The workspace keeps one crate per subsystem. The boundary rule is that each
crate owns one reason to change and exposes typed data to adjacent crates.

| Crate family | Responsibility |
| --- | --- |
| `rem6-kernel` | Ticks, clock domains, partitioned scheduling, conservative epochs, snapshots of scheduler state, deterministic event identity. |
| `rem6-topology` | Components, ports, partitions, clock domains, endpoint validation, and static topology metadata. |
| `rem6-transport` | Memory and control transactions over topology endpoints, route validation, batch submission, and transport traces. |
| `rem6-fabric` | NoC or fabric timing, virtual networks, lane activity, route resource use, and link-level backpressure. |
| `rem6-memory` | Addresses, access sizes, request identities, line layout, partitioned stores, and generic memory transactions. |
| `rem6-dram` | DRAM-class timing, geometry, bank or channel activity, target profiles, and memory service latency. |
| `rem6-cache` | Cache controller behavior independent from system topology, including hits, misses, replacement, and controller resources. |
| `rem6-directory` | Directory state and sharer or owner bookkeeping that does not require a full-system harness. |
| `rem6-protocol-*` | Coherence protocol state machines and transition rules. |
| `rem6-coherence` | Protocol harnesses, partitioned directory execution, coherence summaries, and integration of protocol engines with memory and fabric records. |
| `rem6-cpu` | CPU front ends, architectural state, core clusters, instruction events, and data or instruction memory requests. |
| `rem6-gpu` | GPU command submission, compute units, workgroups, DMA, traces, and checkpointable device state. |
| `rem6-accelerator` | Accelerator command engines, NPU-style jobs, DMA, traces, and checkpointable device state. |
| `rem6-mmio` | MMIO address decoding, register banks, device register semantics, and access errors. |
| `rem6-interrupt` | Interrupt controller state, routing, pending delivery, and checkpointable interrupt metadata. |
| `rem6-timer` | Timer MMIO, programmed events, interrupt emission, and checkpointable timer state. |
| `rem6-boot` | Boot images, segments, initial memory population, and workload input metadata. |
| `rem6-checkpoint` | Manifest format, component chunks, restore validation, and protocol-neutral checkpoint assembly. |
| `rem6-stats` | Typed counters, registry-owned stat groups, self-describing snapshot group catalogs, checked stat descriptions, structured scope/name paths, typed unit registration, structured units and rate units, registry ownership, statistics reset, typed dump history, schema-and-reset-scope-checked snapshot deltas, and run metadata. |
| `rem6-power` | Power domains, residency, typed expression inputs, core stats-delta bindings, and thermal models. |
| `rem6-platform` | Platform assembly helpers that remain thin wrappers over topology, memory, devices, and host control. |
| `rem6-system` | Full-system composition. It wires crates together but must not hide timing, protocol, fabric, or device behavior behind untyped glue. |

## Scheduler Model

The runtime uses partitioned discrete-event simulation.

Each partition has local time, pending local events, and a frontier. The global
tick is a derived conservative frontier, not the owner of all work. A scheduler
epoch dispatches safe local work whose execution cannot be invalidated by any
legal remote message.

The scheduler records:

- epoch count and empty-epoch count;
- dispatch count and batch count;
- worker count per epoch;
- active partitions;
- recorded partition frontiers before and after each parallel epoch;
- ready partitions selected for dispatch;
- progress-free transition count before livelock thresholding;
- event kind, tick, partition, and deterministic local identity;
- errors for serial events inside a parallel epoch;
- errors for remote delays below lookahead.

The scheduler must support a debug serial view, but new full-system execution
paths should call the parallel APIs and record the parallel run summary. CPU
cluster and full-system run records must preserve both the initial and final
frontiers for each recorded parallel epoch, so higher layers can verify how far
each partition advanced instead of inferring it from aggregate counts.
Data-cache and coherence run summaries follow the same rule, and workload
result summaries must retain those frontiers separately from aggregate worker
and batch counts. Workload result summaries also expose aggregate full-system
frontier views so verification can reason about the combined CPU and
data-cache/coherence conservative horizon without discarding per-subsystem
records. Those full-system frontier views merge same-partition CPU and
data-cache scheduler records conservatively instead of reporting whichever
subsystem progressed further. Workload result summaries treat non-empty typed
frontier and partition evidence as parallel work even when worker and batch
aggregates are not present. Exact batch partition-set histograms also imply
active partition counts, exact partition-set batch counts, minimum max-worker
use, total-worker activity, and multi-worker batch activity for any worker
threshold not larger than the recorded partition set. Maximum
same-partition-set streak records imply the same lower-bound metrics from their
consecutive batch counts. Both exact batch sets and streak records also imply
per-partition worker and dispatch activity
for every partition contained in each recorded batch set, making same-batch
participation replay-verifiable without duplicating explicit activity records.
Within one scheduler scope, explicit per-partition activity,
remote-flow-derived activity, and batch-derived activity are alternative
lower-bound evidence and merge by the strongest recorded field instead of by
addition. Full-system activity only adds after the CPU scheduler and
data-cache/coherence scheduler have been summarized separately. Workload
result summary accessors also choose the strongest available aggregate or
fine-grained evidence for batch count, dispatch progress, max-worker use,
thresholded multi-worker batch activity, and total-worker activity, so a lower
aggregate or worker-count counter cannot hide detailed partition-set execution
records.
Workload manifests may declare required initial or final frontier minima for
specific partitions and scopes, turning
conservative-frontier progress into a replay contract rather than an informal
trace inspection.

## Message Model

A message is the only legal cross-partition timing edge. All cross-partition
messages carry:

- source partition;
- target partition;
- source component or endpoint;
- target component or endpoint;
- source tick;
- delivery tick;
- message class;
- request or transaction identity;
- optional payload;
- deterministic tie-break identity.

The message class must be typed. Examples include memory request, memory
response, coherence request, coherence response, snoop, invalidation, DMA
transfer, interrupt, timer signal, host event, GPU command, accelerator command,
and statistics action.

## Memory and Coherence Model

rem6 must avoid a split between a simple memory system and a high-fidelity
coherence system. All memory traffic should flow through a shared set of
transaction concepts:

- architectural request identity;
- requester agent;
- address and line address;
- access size;
- access intent;
- permission requested;
- source endpoint;
- target endpoint;
- fabric route;
- coherence transaction identity when coherence applies;
- data movement identity when bytes move;
- service response.

Cache and coherence crates must stay separable. A protocol engine should not
know the full-system topology. A topology route should not know protocol state.
The integration layer can combine them, but it must preserve typed records from
both sides.

Coherence protocol support should be proven at three levels:

- pure protocol transition tests;
- partitioned harness tests with memory and fabric activity;
- full-system CPU, GPU, accelerator, and DMA tests that use the same transport
  and scheduler.

MSI, MESI, and MOESI are required baseline protocols. CHI-like modeling should
be built from the same primitives rather than through a separate Ruby-like
engine.

## NoC and Memory Resources

NoC and memory are timing resources, not passive helper functions. A model of a
fabric, DRAM channel, HBM stack, LPDDR channel, or DDR controller must expose:

- partition ownership;
- route or target identity;
- queue occupancy;
- bandwidth or service capacity;
- request start tick;
- response tick;
- contention source;
- virtual network or traffic class;
- service profile;
- checkpoint state.

The system run summary must aggregate resource activity without discarding the
per-resource records.

## Heterogeneous Devices

CPU cores, GPU compute units, and accelerators are peers in the runtime. They
may differ in execution model, but they share these obligations:

- partition ownership;
- command or instruction trace;
- memory and DMA traffic through shared transport;
- scheduler activity records;
- checkpointable architectural and timing state;
- host-control integration;
- resource activity in system summaries.

Device-specific shortcuts are allowed only when they preserve the same external
transaction semantics and report the shortcut in run metadata.

## Host Control and Workloads

Full-system control must be typed and replayable. The host boundary covers:

- boot image loading;
- guest events;
- ROI begin and end;
- statistics reset and typed dump records;
- checkpoint request;
- stop request;
- CPU or device mode switch;
- GPU or accelerator launch;
- workload metadata;
- result metadata.

The workload representation should eventually include all inputs needed for
replay: boot image segments, disk or resource identifiers, kernel arguments,
device configuration, host actions, and checkpoint lineage.

## Checkpoint Model

Checkpointing is a coordinated snapshot of partitioned state. A component can
be checkpointed only if it can expose enough typed state to restore behavior
without relying on hidden global runtime data.

Checkpoint manifests must validate:

- component identity;
- component kind;
- checkpoint label;
- chunk names;
- duplicate chunks;
- missing chunks;
- scheduler partition count;
- scheduler lookahead;
- scheduler worker limit;
- topology compatibility;
- pending events or an explicit quiescence marker;
- protocol state compatibility;
- memory target compatibility.

A checkpoint restore must reject state that would silently discard pending
events, resource queues, dirty ownership, or in-flight messages.

## Observability Model

Every production full-system run must make these facts inspectable:

- stop reason;
- final tick;
- executed instruction or command counts;
- scheduler profile;
- active partition count;
- fabric profile;
- DRAM profile;
- coherence profile;
- cache or directory profile;
- device traces;
- host events;
- checkpoint or restore metadata when relevant;
- errors and diagnostic context.

String logs are useful for humans, but they are not sufficient evidence for
tests. Tests must assert typed records.

## Error Handling

The runtime should prefer explicit, typed errors over panics for user-visible
model failures. Panics are acceptable for internal invariants that cannot be
recovered without corrupting state, but model-level invalid input should return
structured errors.

Important error classes include:

- unknown partition;
- invalid endpoint;
- impossible route;
- zero remote delay;
- remote delay below lookahead;
- unsupported access size;
- unmapped memory region;
- protocol unexpected event;
- protocol busy state;
- deadlock or livelock diagnostic;
- checkpoint manifest mismatch;
- restore would discard state;
- unsupported device operation;
- host-control mismatch.

## Proof Obligations

Each capability should be added with tests that make the intended behavior fail
before implementation. The named obligations below are the baseline for future
work.

| Capability | Evidence before implementation is accepted |
| --- | --- |
| Partitioned scheduling | Tests for deterministic equal-tick order, local events below lookahead, remote delay rejection, recorded parallel epochs, and scheduler snapshots. |
| CPU cluster execution | Tests for instruction fetch, data access, traps, host stop, per-core partition activity, and checkpoint restore. |
| GPU execution | Tests for command submission over topology, workgroup scheduling, DMA traffic, traces, run summaries, and checkpoint restore. |
| Accelerator execution | Tests for command submission, NPU-style job completion, DMA traffic, traces, run summaries, and checkpoint restore. |
| Fabric transport | Tests for route validation, virtual network activity, bandwidth contention, trace records, and checkpoint restore. |
| DRAM timing | Tests for target mapping, read and write timing, geometry effects, activity records, and checkpoint restore. |
| MSI coherence | Pure transition tests, partitioned directory tests, full-system CPU data tests, resource activity tests, and checkpoint tests. |
| MESI coherence | Pure transition tests, partitioned directory tests, full-system CPU data tests, resource activity tests, and checkpoint tests. |
| MOESI coherence | Pure transition tests, partitioned directory tests, full-system dirty-owner tests, resource activity tests, and checkpoint tests. |
| Multi-protocol integration | Tests that swap protocol backend without changing topology definitions. |
| Host events | Tests for guest event delivery, stop requests, statistics actions, and deterministic replay metadata. |
| Checkpointing | Tests for manifest validation, scheduler restore, memory restore, device restore, coherence restore, and restore rejection for incompatible state. |
| Statistics | Tests for registry-owned stat groups, self-describing group catalogs on snapshots, dumps, and deltas, checked counter descriptions, structured counter scope/name identity, path grammar, structured unit and rate grammar, monotonic reset behavior, typed dump records, schema-and-reset-scope-checked deltas, and aggregation into system summaries. |
| Power | Tests for power domains, expression inputs, stat snapshot and core stats-delta bindings, thermal coupling, and invalid scope or schema rejection. |
| Deadlock diagnostics | Tests that create a wait-for cycle and assert a bounded diagnostic. |
| Livelock diagnostics | Tests that create repeated progress-free transitions and assert a bounded diagnostic. |

## Disallowed Patterns

The following patterns recreate the debt this project is meant to avoid:

- global mutable event queues used as the only timing owner;
- direct cross-partition mutation;
- untyped closures as the only record of model behavior;
- string-only traces for protocol correctness;
- protocol code that embeds topology layout;
- topology code that embeds protocol state;
- memory shortcuts that bypass transport without activity records;
- checkpoint code that flushes all dirty state into memory and loses ownership;
- device models that bypass host control and scheduler records;
- external scripts as the only way to reproduce full-system experiments;
- tests that only prove a helper function rather than a full-system behavior;
- production dependencies on files under `temp`.

## Current Alignment

The current workspace already contains aligned building blocks:

- `rem6-kernel` has partitioned scheduling with conservative epochs, lookahead,
  deterministic dispatch, recorded summaries, and scheduler snapshots.
- `rem6-topology` assigns components to partitions and validates ports and
  endpoint connections.
- `rem6-transport`, `rem6-fabric`, `rem6-memory`, and `rem6-dram` carry memory
  transactions and resource activity.
- `rem6-coherence` contains partitioned MSI, MESI, and MOESI harnesses with
  activity summaries.
- `rem6-system` wires RISC-V, host events, fabric, DRAM, GPU, accelerator, and
  coherence paths into full-system tests.
- `rem6-checkpoint` provides manifest and chunk validation.

The largest current risk is integration drift: separate features can pass local
tests while the full-system path keeps temporary protocol-specific bridges. New
work should tighten the shared transaction model and reduce bridge-specific
logic.

## Near-Term Implementation Slices

The near-term work should favor slices that strengthen the shared runtime
rather than merely add isolated features.

### MSI Full-System Data Backend

Add an MSI full-system data-cache backend using the same bridge shape as the
MESI and MOESI backends, then identify the shared response conversion that can
be collapsed into a protocol-neutral adapter. Required proof:

- failing full-system MSI data test first;
- green test through partitioned coherence harness;
- system run reports coherence fabric and DRAM activity;
- cache state and data value assertions prove the protocol path is used;
- no changes to topology definitions are required to swap the backend.

### Protocol-Neutral Data Adapter

Refactor MESI, MOESI, and MSI data-cache response handling behind a common
adapter trait or typed enum only after the MSI path is green. Required proof:

- existing MESI and MOESI full-system tests stay green;
- MSI full-system test stays green;
- error variants remain protocol-specific enough for diagnosis;
- no protocol engine imports `rem6-system`.

### Coherence Checkpoint Coverage

Add checkpoint coverage for partitioned coherence state before expanding to
larger protocols. Required proof:

- dirty owner or sharer state survives snapshot and restore;
- pending transient state is either rejected or restored explicitly;
- restore validates protocol, line layout, agent count, and scheduler
  compatibility.

### Wait-For Diagnostics

Introduce a typed wait-for graph for blocking coherence and transport
resources. Required proof:

- an injected cycle yields a bounded diagnostic;
- normal contention does not report a cycle;
- run summaries can expose the diagnostic context.

### Shared Workload Manifest

Add a first manifest type that records boot image, topology, host-control
events, and checkpoint lineage. Required proof:

- a manifest can reconstruct a small RISC-V full-system run;
- missing resource metadata is rejected;
- result metadata links to the manifest identity.

## Completion Bar

rem6 is not complete until the following broad requirements are proven by
current-state evidence:

- gem5-equivalent component categories exist for CPU, GPU, accelerator,
  topology, NoC, cache, directory, coherence, memory, device, host control,
  checkpoint, statistics, and workloads;
- these categories run through the partitioned runtime;
- representative full-system runs use parallel execution by default;
- MSI, MESI, MOESI, and CHI-like coherence behavior are covered by protocol,
  harness, and full-system tests;
- CPU, GPU, accelerator, DMA, interrupt, timer, UART, fabric, DRAM, and host
  control are checkpointable or explicitly rejected with typed reasons;
- run summaries expose typed evidence for scheduler, resource, coherence, and
  device activity;
- deadlock and livelock diagnostics are testable;
- production code has no dependency on `temp`;
- all changed production files satisfy workspace lint, format, test, and
  repository policy scans.
