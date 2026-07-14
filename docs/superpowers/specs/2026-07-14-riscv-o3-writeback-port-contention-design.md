# RISC-V O3 Writeback-Port Contention Design

## Status

Approved for specification on 2026-07-14 under the active
`temp/improve-rem6-0.md` continuation contract.

This document defines one bounded execution increment. The migration progress
authority remains `docs/architecture/gem5-to-rem6-migration.md`.

## Goal

Make the existing `O3WritebackTransferPolicy` and
`O3WritebackTransferBuffer` plan cycle-visible register writeback for the
bounded detailed RISC-V O3 live window, with one explicit transient reservation
calendar owning occupied future slots.

Register-writing scalar FU/ALU completions and completed scalar loads must
share one configurable writeback-port width. Port admission must change real
ROB readiness, dependency wakeup, O3 trace timing, and ordered retirement
through `rem6 run --execute`.

This increment advances the CPU ledger's explicit writeback-port-contention
boundary. It does not claim a persistent general issue queue, arbitrary
wakeup/select, broad FP/vector writeback, an unbounded ROB, arbitrary mixed
memory/control windows, restorable live transport, or a general O3 engine.
The CPU score remains at 74% representative.

## Current Boundary

The repository already contains a generic writeback transfer planning model:

- `O3WritebackTransferPolicy` owns width and bounded future capacity;
- `O3WritebackTransferBuffer` gives deferred completions priority over newly
  ready rows;
- `O3WritebackTransferSnapshot` and its checkpoint payload preserve the
  configured policy and deferred sequence order;
- `O3PendingStateSnapshot` already embeds that writeback snapshot in the O3
  runtime checkpoint graph.

The model does not retain occupied future slots. `plan_cycle` returns future
offsets, but the buffer snapshot contains only policy plus deferred rows. A live
runtime that accepts fixed FU reservations before later load responses therefore
needs an explicit transient occupancy authority; pretending that the current
snapshot is already that authority would permit overbooking.

Those types are publicly exported and covered by isolated tests, but the live
RISC-V O3 path does not use them. Current live timing instead derives
writeback independently in several places:

- speculative FU readiness is `issue_tick + execute_wait_cycles`;
- scalar-load forwarding readiness is `response_tick + 1`;
- `O3RuntimeTraceRecord::writeback_tick` recomputes a maximum from issue,
  latency, and response fields;
- live ROB readiness is marked by the retire or memory-response path without
  consulting a shared port.

The generic writeback model is therefore dormant scaffolding while real
execution bypasses it. Extending the existing ad hoc tick arithmetic would
preserve duplicate authorities and violate the active goal's prohibition on
orphan APIs and disconnected state.

## Considered Approaches

### 1. FU-only bounded writeback contention

Apply port width only to fixed-latency scalar FU/ALU completions.

Advantages:

- fixed completion ticks are known at issue;
- the implementation avoids variable memory responses;
- top-level collision fixtures are straightforward.

Cost: scalar loads would still publish through a separate writeback rule. The
migration ledger could record bounded FU contention, but it could not honestly
remove the broader writeback-port-contention gap.

### 2. Shared scalar FU/load writeback port

Use one live writeback authority for register-writing scalar FU/ALU rows and
completed scalar loads.

Advantages:

- activates the existing generic writeback model on a real runtime path;
- makes dependency readiness consume post-contention timing;
- covers both fixed- and variable-latency register completion;
- supports direct and hierarchy-backed top-level evidence;
- establishes the timing foundation needed before persistent IQ/wakeup/select.

Cost: variable load responses must join deterministic reservations without
invalidating already issued dependent FU rows.

### 3. Persistent IQ, wakeup/select, and writeback together

Promote the current pending-state issue metadata into a persistent IQ while
also adding writeback contention.

Advantages:

- approaches a general O3 execution core;
- would centralize issue and completion timing in one large step.

Cost: this crosses ROB projection, fetch identity, wakeup, select, rollback,
checkpoint, transfer, and writeback boundaries simultaneously. It risks a
second serialized queue beside the live ROB and would make writeback timing a
moving dependency during the IQ implementation.

Approach 2 is selected. Persistent IQ/wakeup/select follows this increment.

## Configuration

Add `--riscv-o3-writeback-width <1..=4>` and the matching TOML field
`riscv_o3_writeback_width`.

The default is one. The existing default O3 pending-state snapshot already
encodes one IEW writeback slot and zero future cycles. Activating that policy
preserves the repository's declared default rather than silently replacing it
with a second configuration source.

The valid range is owned by `rem6-cpu` and reused by the CLI parser and direct
`RiscvCore` setter. Zero and five fail before execution. The option requires
`--execute` and `--isa riscv`, matching the existing issue-width and
scalar-memory-depth boundaries.

This increment does not add per-operation-class writeback knobs. One shared
register-writeback width is sufficient for the bounded domain.

## Completion Domain

The shared port covers rows that publish a scalar integer register value:

- scalar integer ALU descendants admitted by the current live window;
- scalar integer multiply/divide heads and descendants;
- cacheable scalar loads after their data response is complete.

The following do not consume this port in this increment:

- stores and store-conditionals without a load value;
- direct conditional branches and other destinationless rows;
- system events, traps, interrupts, `JAL`, and `JALR` descendants;
- floating-point and vector descendants, which are outside the current live
  speculative window;
- MMIO completion, which remains single-outstanding and device-owned.

Atomic and other non-window memory events continue through the existing
retirement/stat path. Broad operation-class writeback remains part of the
general O3 boundary.

## Runtime Ownership

A focused `o3_runtime_writeback.rs` module will own live completion
classification, writeback-port arbitration, admitted ticks, reservation
cleanup, and plan-derived counters.

The module adds one `O3WritebackReservationCalendar` to `O3RuntimeState`. It is
a transient, non-serialized map from absolute tick to occupied slots and owning
ROB sequences. It is the sole authority for capacity already reserved at a
future or current tick. It is not a second ROB, IQ, or deferred FIFO.

`O3RuntimeSnapshot.pending_state.writeback` remains the configuration and
planner-buffer authority. `O3WritebackTransferBuffer` is reconstructed from
that snapshot for an atomic reservation operation, gives its local deferred
rows priority, and is drained before the operation returns. Stable live runtime
state must not leave a second deferred `VecDeque` in the pending snapshot.

Each live completion owner records the immutable result it needs after a
calendar entry is consumed or pruned:

- `O3LiveSpeculativeExecution` records raw FU-ready and admitted writeback
  ticks;
- `O3LiveScalarMemory` records response, raw load-ready, and admitted
  writeback ticks;
- `O3LiveRetiredInstruction` carries the admitted tick into trace/stat
  composition.

An admitted tick stored on a live row is historical timing, not capacity
authority. While a slot can still affect another reservation, the calendar
entry is authoritative. Calendar and owner records must agree on sequence,
tick, and slot; disagreement is an invariant error.

## Arbitration Semantics

The generic buffer gains an occupancy-aware cycle-planning entry point. It
accepts the set of slots already occupied in the cycle, preserves its deferred
FIFO before newly ready rows, and assigns the lowest free slots. Existing
`plan_cycle` delegates with an empty occupied set, so isolated callers retain
their current behavior. No RISC-V instruction classification enters
`o3_pipeline.rs`.

The live runtime reserves a completion as follows:

1. reject a duplicate sequence or return its identical existing reservation;
2. collect completions whose raw-ready tick is newly known, but keep rows with
   `raw_ready_tick > base_tick` outside the transfer buffer;
3. order eligible rows by raw-ready tick and then ROB sequence;
4. reconstruct the buffer from the pending-state writeback snapshot;
5. derive occupied slots for the absolute cycle from the reservation calendar;
6. append only rows with `raw_ready_tick <= base_tick` after the buffer's
   deferred rows and call the occupancy-aware planner;
7. record admissions in the calendar and on the owning live rows;
8. advance one absolute cycle with the buffer's deferred rows and repeat until
   the local queue is empty; when it empties before the next raw-ready tick,
   jump `base_tick` directly to that next tick;
9. persist the drained buffer snapshot and record the plan-derived counters.

A future-raw-ready row is neither ready nor deferred and contributes no
ready/deferred counter before its raw-ready tick. It enters the local buffer
exactly once when the base cycle reaches that tick.

The configured policy keeps `future_cycles == 0`; the runtime advances the
absolute base tick one cycle at a time. This avoids a hidden second future
window and makes every delayed cycle observable. Tick overflow is an explicit
runtime error rather than a saturated duplicate reservation.

Fixed-latency FU rows reserve once their issue and raw-ready ticks are known. A
later variable-latency load response cannot displace an existing calendar
entry. It enters at `response_tick + 1` and takes the earliest free slot at or
after that tick. This deterministic reservation rule prevents a late response
from retroactively changing issue timing for an already executed dependent FU
row. Among rows discovered together, raw-ready tick and ROB sequence determine
order; an already reserved slot outranks every later-discovered row, including
an older variable-latency load.

Partial live-window re-entry consults both the owner's recorded result and the
calendar. Repeated callbacks cannot allocate more than the configured width in
one cycle, admit the same sequence twice, or count the same planning decision
twice. Each `O3WritebackReservation` carries a `decision_counted` marker that
is set with the first counter update and is not cleared by stats reset.

## Raw-Ready Timing

Raw readiness remains owned by the existing execution models:

- fixed-latency FU/ALU raw readiness is issue tick plus the existing pipeline
  latency;
- zero-latency ALU rows are raw-ready in their issue cycle;
- scalar loads are raw-ready at `response_tick + 1`, preserving the current
  CPU-to-O3 response handoff boundary.

Raw readiness is not writeback. Speculative planning may carry a value together
with its admitted availability tick, but no consumer may issue before that tick
and no ROB or trace surface may report an earlier writeback.

The existing FU latency counters continue to measure execution latency. Port
delay is recorded separately and must not inflate FU latency-cycle stats.

## Wake Scheduling And Publication

Writeback reservation occurs before scheduler wake ownership is committed.

For a fixed-latency FU head, live-window preparation is split from the current
gate action:

1. stage or find the live ROB row;
2. record the head execution and reserve its writeback slot;
3. schedule speculative younger rows, reserving each fixed completion as it is
   recorded;
4. pass the head's admitted tick, not its raw-ready tick, to
   `RiscvLiveRetireGate::before_retire_at_known_ready_tick`;
5. arm the gate wake directly at that admitted tick.

Re-entry reuses the reservation. The implementation does not first arm a raw
FU wake and then cancel or rearm it. The admitted gate callback is the point
that marks the fixed-FU ROB row ready and feeds ordered retirement.

When a speculative fixed-FU descendant later reaches its normal retirement
callback, the callback marks the ROB entry ready with the stored admitted tick,
not the callback's current tick. The callback may publish architectural state
later, but it cannot rewrite the already established O3 writeback timing.

A completed scalar load records response data and reserves its slot before it
marks the ROB row ready. If the admitted tick is later than the response tick,
one deduplicated per-core writeback wake keeps the scheduler live until the
earliest unpublished load admission.

A focused `RiscvO3WritebackWakeState` in `RiscvCoreState` owns this bridge. It
contains the desired earliest unpublished scalar-load tick, at most one
scheduled wake snapshot, and detached stale wakes awaiting their tick. The
memory-response path updates only the desired tick. After every serial or
parallel cluster turn,
`RiscvSystemRunDriver::schedule_riscv_system_events_from_turn{,_parallel}`
queries each detailed core and schedules the requested no-op partition event,
then returns the event snapshot to the core for ownership tracking. The event
callback marks that wake fired; the following run-stats drain publishes rows at
the scheduler's current tick.

An identical desired/scheduled pair is a no-op. An earlier desired tick detaches
the stale later wake and schedules the earlier one. Load publication, retry,
failure, rollback, redirect, reset, and mode cleanup recompute or clear the
desired tick; detached wakes are harmless no-ops and are pruned after their
scheduled tick, matching live-retire-gate wake ownership. Live checkpoint
capture rejects an owned or detached writeback wake, and mode transfer keeps it
with the resident core.

The system drive passes its current tick to scalar-memory publication;
`record_ready_o3_scalar_memory_event_with_trace` cannot expose the event, apply
the architectural register write, or mark the ROB row ready before
`current_tick >= admitted_tick`.

## Dependency And Publication Semantics

Register dependencies consume admitted writeback timing.

- speculative source forwarding uses the producer's admitted writeback tick;
- load-dependent younger rows cannot issue before the load's admitted port
  tick, although the scheduler may pre-plan them after the reservation exists;
- a zero-latency consumer issued in its producer's writeback cycle competes
  for another slot in that cycle;
- a consumer whose own completion is delayed can execute speculatively but
  cannot publish its result to another dependent before its admitted tick.

No speculative value becomes architectural at writeback. The committed rename
map and architectural register file remain retirement-owned. Oldest-first ROB
commit remains unchanged.

Completed scalar-load architectural writeback must not occur before the
admitted tick is reached. A load response and reservation can be present in the
live memory record while its ROB row and dependent consumers remain unready.

## Trace Semantics

`O3RuntimeTraceRecord` gains an explicit admitted writeback tick for live rows.
The trace must preserve separate values for:

- issue tick;
- raw FU or memory completion timing;
- admitted writeback tick;
- ordered commit tick.

`issue_to_writeback_ticks` includes both execution latency and port delay.
Existing FU latency fields continue to report only execution latency.
`writeback_to_commit_ticks` continues to describe ordered retirement delay.

Non-live legacy trace construction retains its existing derived behavior until
that path is moved under a broader O3 engine.

## Cleanup And Re-entry

Every live-window cleanup path must remove writeback authority for discarded
sequences:

- branch rollback and control-descendant truncation;
- scalar-memory retry, failure, and suffix cancellation;
- PC redirect, interrupt, reset, HTM abort, and detailed-mode cleanup;
- restore and checkpoint rollback;
- live-window rebind after split-fetch replacement.

Cleanup removes future calendar entries owned by discarded sequences and clears
their owner-side results. A slot whose cycle has not occurred may then be reused
by a later reservation; a consumed historical slot is never replayed. The
pending-state planner buffer must still be empty at the stable boundary.

Calendar entries earlier than the current scheduler tick are pruned after their
owners have published or been discarded. Entries at the current tick are kept
until a later tick is observed, so separate same-tick callbacks see identical
occupancy. Quiescent drain/checkpoint finalization may prune consumed current-
tick entries after the scheduler has finished dispatching that tick.

Stats reset clears cumulative counters and the auxiliary deduplication history,
but preserves the calendar, admitted ticks, and configured width. It does not
replay planning decisions made before reset and cannot make a live row write
back twice. Existing reservations keep `decision_counted == true`; reservations
created after reset contribute to the new epoch normally.

## Checkpoint Compatibility

The O3 runtime checkpoint payload advances from version 22 to version 23 for
the new cumulative writeback-port counters.

Version 23 round-trips the counters and the existing pending-state writeback
policy. Versions 1 through 22 remain byte-decodable and initialize the new
counters to zero.

`O3RuntimeCheckpointPayload` gains a private decode-origin marker containing
either the runtime payload version or `LegacyPendingOnly`. Constructors mark
new payloads as version 23 and `decode` preserves the input version.

Because `runtime_from_legacy_pending` lives in `rem6-system`, `rem6-cpu`
provides a public
`O3RuntimeCheckpointPayload::from_legacy_pending_state(O3PendingStateSnapshot)`
constructor. It builds the default empty ROB/LSQ/rename snapshot internally and
marks `LegacyPendingOnly` without exposing the origin enum. The system path
uses this constructor instead of rebuilding a current-origin payload.

Historical pending-state deferred completions were inert generic scaffolding,
not live RISC-V runtime authority. `snapshot()` preserves them for inspection,
but `restore_checkpoint_payload` consumes an origin-aware restore snapshot:
versions 1 through 22 and `LegacyPendingOnly` clear the deferred queue before
validation and execution. Re-encoding a legacy-origin payload emits a
normalized version-23 payload rather than promoting old sequence IDs to live
authority. A decoded version-23 payload with a nonempty stable deferred queue is
invalid. The encoded policy width remains available and defaults to one.

Live checkpoint capture continues to reject resident live ROB, LSQ, calendar,
or unpublished writeback authority because reservations and live execution
records are intentionally transient. A drained capture has an empty calendar
and planner queue, zero live ROB/LSQ rows, and restorable cumulative counters.

This increment does not add a second top-level writeback payload.

## Execution-Mode Transfer

Detailed-to-timing mode transfer remains non-restorable while live O3 rows are
resident. The resident core keeps admitted ticks, calendar ownership, and any
writeback wake until inherited rows drain.

The transfer/debug artifact exposes:

- configured writeback width;
- reserved future completion count;
- earliest unpublished admitted tick;
- cumulative writeback-port counters decoded from the O3 runtime chunk.

A switched run must match a no-switch baseline for every inherited row's
issue, admitted writeback, and commit tick. The first post-window timing-mode
instruction remains outside the detailed O3 surface.

## Observable Stats

Add a native `o3_runtime.writeback_port` summary and matching text paths:

- `sim.cpu0.o3.writeback_port.cycles`, unit `Cycle`;
- `sim.cpu0.o3.writeback_port.admitted_rows`, unit `Count`;
- `sim.cpu0.o3.writeback_port.deferred_rows`, unit `Count`;
- `sim.cpu0.o3.writeback_port.deferred_row_cycles`, unit `Cycle`;
- `sim.cpu0.o3.writeback_port.max_ready_rows_per_cycle`, unit `Count`;
- `sim.cpu0.o3.writeback_port.max_deferred_rows`, unit `Count`.

`cycles` counts distinct absolute cycles examined by reservation plans. A row
raw-ready at tick `r` and admitted at tick `a` examines every tick in the
inclusive range `r..=a`; a per-runtime tick set deduplicates overlapping plans
and re-entry.

`admitted_rows` counts each unique calendar reservation once, including a
future slot. It is speculative scheduling activity, not an architectural
retirement count. `deferred_rows` counts each row once when `admitted_tick >
raw_ready_tick`. `deferred_row_cycles` adds `admitted_tick - raw_ready_tick`.

`max_ready_rows_per_cycle` is the maximum number of distinct newly raw-ready
rows introduced when the base cycle reaches one absolute tick, including rows
discovered by separate callbacks in that tick. Future-raw-ready rows do not
count early. `max_deferred_rows` is the maximum local buffer depth remaining
after any examined cycle; it likewise excludes rows not yet raw-ready.
Auxiliary per-tick bookkeeping exists only to deduplicate these counters; it
owns no writeback capacity.

The two maxima use max aggregation; the other counters use sum aggregation in
transfer/dump projections. Stats reset does not recount already established
reservations.

The fields are derived from occupancy-aware buffer plans. They must not be
inferred from final instruction counts or from existing `wbRate`, `wbFanout`,
or IEW writeback aliases. Those aliases remain compatibility projections, not
port contention evidence.

The summary resets with O3 runtime stats and is absent in timing mode. JSON,
text, final stats, `m5_dump_stats`, checkpoint/restore decoding, and transfer
debug paths use the same runtime values.

## TDD Matrix

### Focused runtime tests

- width one reserves the oldest of two same-cycle completions and moves the
  younger row to the next cycle;
- width two admits the same exact-fit pair without deferral;
- occupancy-aware buffer planning skips a previously reserved slot and
  preserves deferred-before-new ordering;
- mixed raw-ready ticks never enter the local buffer or counters before their
  base cycle;
- partial re-entry cannot duplicate a reservation, calendar entry, wake, or
  counter and cannot overbook a cycle;
- a multiply producer and zero-latency dependent consumer use admitted, not
  raw-ready, dependency timing;
- FU reservation occurs before the live-retire gate wake is scheduled;
- a completed scalar load waits until its admitted tick before ROB readiness,
  architectural writeback, or dependent wakeup;
- serial and parallel system adapters deduplicate, replace, fire, and prune the
  core-owned scalar-load writeback wake;
- a fixed FU reservation and later scalar-load response share capacity without
  retroactively moving the FU admission;
- rollback removes a future wrong-path calendar entry before publication;
- stats reset preserves live calendar/timing state and clears counters without
  replaying existing reservations;
- version 23 round-trips counters while version 22 decodes them as zero;
- runtime versions 1 through 22 and the pending-only legacy path preserve an
  inspectable deferred snapshot but normalize it before restore or re-encode;
- the public legacy-pending constructor marks `LegacyPendingOnly` rather than
  silently creating a current-origin payload.

### Top-level CLI matrix

- direct-memory width-one FU/dependent collision with exact issue,
  writeback, and commit ticks;
- width-two exact-fit control for the same program;
- direct shared scalar-load/FU collision with load-dependent wakeup and an
  architectural-register assertion before and at the admitted tick;
- cache/fabric/DRAM scalar-load row proving the same port authority and
  hierarchy activity;
- wrong-path or retry cleanup proving a reserved future row never publishes;
- detailed-to-timing switch preserving baseline issue/writeback/commit ticks;
- live checkpoint rejection and drained version-23 restore cleanup;
- timing-mode suppression of writeback-port JSON, text, and debug surfaces;
- final JSON, text, and real `m5_dump_stats` counter evidence;
- CLI and TOML acceptance for widths one and four;
- CLI and TOML rejection for zero and five;
- rejection without `--execute` and with non-RISC-V ISA.

One hierarchy-backed row is sufficient. Repeating every width and collision
shape through both memory routes would add runtime without testing a different
writeback rule.

## Source Boundaries

- `o3_runtime_writeback.rs` owns live completion classification, the transient
  reservation calendar, admitted ticks, cleanup, and plan-derived counters.
- `o3_pipeline.rs` retains the generic policy, buffer, snapshot, and codec and
  gains only occupancy-aware slot planning; it receives no RISC-V-specific
  behavior.
- `o3_runtime_issue.rs` continues to own issue selection and consumes admitted
  producer ticks.
- `o3_runtime_memory.rs` continues to own scalar-memory response and
  retirement lifecycle, delegating reservation and admitted-tick publication.
- `o3_runtime_control_window.rs` continues to own candidate validation,
  forwarding values, and rollback metadata.
- `o3_runtime_checkpoint.rs` owns version-23 counter compatibility and legacy
  decode-origin normalization.
- a focused `riscv_o3_writeback_wake.rs` owns per-core desired, scheduled, and
  detached scalar-load wake state;
- the live-retire gate and serial/parallel system adapters own only scheduler
  wake plumbing; they do not choose writeback slots.
- a focused stats-output module owns the six native text paths;
- top-level tests live in a new focused
  `m5_host_actions/o3/writeback_port.rs` child module;
- source policy requires the runtime owner, generic buffer consumer, focused
  stats emitter, and exact CLI evidence anchors.

No file under `temp/` is committed.

## Slop And Legacy Cleanup

This increment removes the dormant status of
`O3WritebackTransferPolicy`, `O3WritebackTransferBuffer`, and the pending-state
writeback policy by connecting them to live execution. It does not mislabel the
legacy deferred snapshot as a future-slot calendar; the focused runtime owns
that missing transient concept explicitly.

It replaces duplicated readiness shortcuts with one admitted writeback tick,
arms scheduler wakes from that result, and separates execution latency from
port delay. It also gives historical pre-v23 deferred snapshot entries an
explicit decode-only compatibility rule instead of silently turning previously
inert sequence IDs into live runtime authority.

The unused pending-state issue fields are not promoted in this increment.
They remain for the subsequent persistent-IQ design, where ROB-keyed issue
metadata can become one canonical authority rather than another parallel
queue.

## Negative Boundaries

- The shared port covers only the current bounded scalar integer/load live
  domain.
- FP, vector, atomic, MMIO, and arbitrary memory/control writeback remain part
  of the general O3 boundary.
- Writeback admission does not publish architectural state before retirement.
- Issue width, operation-class capacity, and branch rollback remain owned by
  their current focused modules.
- Reservation precedence is deterministic, not a claim of global
  oldest-ready arbitration across completions discovered at different times.
- Timing mode remains free of the detailed O3 writeback-port surface.
- No persistent general IQ, wakeup broadcast network, distributed select,
  restorable live transport, or general O3 engine is claimed.

## Verification

Required gates:

- every behavior test is observed failing for the intended missing runtime
  authority before production implementation;
- focused occupancy-aware writeback-buffer, calendar, and live runtime tests;
- focused config validation and every new CLI row;
- complete scoped-issue, scalar-memory/FU, predicted-control, checkpoint, and
  mode-transfer CLI modules;
- full `rem6-cpu` suite;
- full `rem6` CLI suite;
- workspace all-targets suite;
- `rem6` and `rem6-cpu` source-policy suites;
- rustfmt, `git diff --check`, exact 1,200-line ledger count, and clean status;
- high-intensity read-only review before push.

## Migration Ledger

The CPU heading stays at 74% representative, the raw score stays 8/10, and
both unchecked checklist items remain unchecked. The migrated evidence will
record bounded shared scalar FU/load writeback-port ownership, configuration,
dependency wakeup, rollback, checkpoint, mode-transfer, timing suppression,
stats, and route evidence.

The `Next evidence` boundary may remove only writeback-port contention.
General IQ/wakeup/select, arbitrary mixed memory/control windows, restorable
transport ownership, indirect or unconditional nested controls, fourth/deeper
branch chains, broad FP/vector/atomic/MMIO writeback, and a general O3 engine
remain open.
