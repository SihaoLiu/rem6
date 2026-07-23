# RISC-V O3 Persistent Cross-Class Issue Queue Design

## Goal

Replace the current capture-and-future-simulation live issue loop with one
bounded, runtime-owned issue queue that survives scheduler turns. The queue
must retain unissued supported rows across resource and dependency stalls,
service exactly one simulated tick at a time, and use the existing O3
writeback wake as its scheduler authority.

The representative queue arbitrates the live instruction classes that rem6
can currently execute speculatively with exact identity:

- scalar integer ALU;
- scalar integer multiply and divide;
- pending scalar data-address materialization through the memory/AGU class;
  and
- supported branch and linked-control rows.

This is the precise meaning of cross-class for this increment. FP and vector
memory-result destinations remain supported through the existing data-result
path, but FP and vector arithmetic are not added to the live speculative issue
queue. System instructions also remain outside the queue.

The increment advances rem6 from a derived queue that can simulate several
future ticks inside one call toward a cycle-visible running-O3 owner. It does
not add a checkpoint-restorable live IQ, general LSQ scheduling, arbitrary
instruction classes, arbitrary dependency graphs, or a general O3 engine. The
CPU and Stats checklist counts and 74% representative caps remain unchanged.

## Current Boundary

`O3RuntimeState::schedule_live_speculative_issues` currently performs a local
future-tick simulation. For every pass it:

- scans live ROB rows and staged identities to capture a fresh queue;
- rebuilds the dependency table;
- rebuilds issue reservations from the supplied head and recorded runtime
  rows;
- plans one tick;
- transactionally records selected rows; and
- advances a local tick until the queue drains or reaches a replay boundary.

This architecture no longer depends on caller-owned request slices, but it is
still not a running issue queue. A resource-blocked or dependency-blocked row
is represented only by canonical ROB and identity metadata after the call
returns. The next scheduling entry reconstructs queue membership instead of
servicing state owned by the issue stage.

The current architecture also records speculative executions at future issue
ticks before the scheduler reaches those ticks. This gives useful bounded
timing evidence but prevents the issue stage from becoming a normal
cycle-visible component with explicit wake ownership, resident occupancy, and
turn-by-turn cleanup.

The existing canonical owners are already sufficient for a persistent queue:

- the ROB owns sequence order, readiness, and staged rename destinations;
- staged fetch identities own exact decoded packets and consumed fetch
  requests;
- the rename map owns architectural-to-physical mappings;
- live speculative executions own issued fixed-FU and control results;
- live data accesses and pending data addresses own memory lifecycle state;
- control lineages own control dependencies;
- the writeback calendar owns result-port reservations; and
- the O3 writeback wake owns scheduler event identity and deduplication.

The missing owner is a focused issue-stage state that remembers which bound
sequences are still queued and when they must next be serviced.

## Considered Approaches

### 1. Persistent membership with derived readiness

Store one sequence-ordered unissued inventory plus issue wake and same-tick
decision state on `O3RuntimeState`. At each service turn, materialize queue
entries from those sequences and canonical runtime metadata, derive dependency
and calendar views, and plan only the current tick.

This is the chosen approach. Queue residency survives scheduler turns without
duplicating rename, dependency, packet, or reservation authorities.

### 2. Persist fully materialized candidates and calendars

Store packets, scheduling candidates, dependency scopes, forwarded values,
and reservation calendars as mutable issue-stage state. This is closer to a
large standalone simulator IQ, but every ROB mutation, writeback replan,
pending-address transition, redirect, and control-lineage change would need to
update several duplicated structures transactionally.

That synchronization burden is not justified for the bounded live window and
would widen checkpoint semantics before rem6 supports the missing instruction
classes.

### 3. Keep capture/rebuild and only broaden evidence

Add more CLI tests or instruction-class mappings while retaining the local
future-tick loop. This is smaller, but it does not establish persistent issue
ownership, scheduler-turn wakeup/select, queue occupancy, or a natural path to
a running O3 engine.

## Chosen Architecture

Add `O3LiveIssueState` in
`crates/rem6-cpu/src/o3_runtime_issue/state.rs` and store exactly one instance
on `O3RuntimeState`.

The state owns:

- a sequence-ordered `Vec<u64>` of bound, unissued rows;
- the next requested issue-service tick, if any;
- current and peak resident occupancy observations;
- the active same-tick issue decision and its stats-reset baseline; and
- narrow generation data needed to make enqueue and service idempotent.

The state does not own:

- decoded packets or consumed fetch requests;
- ROB, LSQ, or rename entries;
- dependency scopes or forwarded values;
- functional-unit or writeback reservations;
- speculative execution payloads;
- pending-address payloads; or
- checkpoint scheduler-event identity.

Those facts remain in their existing canonical owners. Persistent queue state
therefore means persistent issue membership and progress authority, not a
second copy of the instruction pipeline.

`o3_runtime_issue/service.rs` owns current-tick planning and service outcome
construction. `o3_runtime_issue/transaction.rs` owns bounded selected-batch
commit. The existing `queue.rs`, `dependency.rs`, `calendar.rs`, and
`pending_address.rs` retain their focused responsibilities.

## Queue Membership

Binding an exact `O3LiveIssuePacket` to a live staged identity remains a
generic lifecycle operation. A successful bind conditionally enqueues its
sequence only when the row belongs to the supported live issue lane or owns a
pending-address replay boundary. FP/vector memory-result heads and other
live rows that execute through a different canonical path may retain a bound
packet without becoming queue residents.

Conditional enqueue is valid only when:

- the sequence names one live-staged ROB row;
- the staged identity accepts the exact decoded instruction and ordered fetch
  request list;
- the instruction can be represented by an existing supported issue kind, or
  it is a valid pending-address row that must retain replay authority; and
- the sequence has not already issued or completed its pending-address
  materialization.

The immutable packet remains owned by `O3LiveStagedFetchIdentity`. The queue
stores only the sequence. This avoids a second decoded-packet owner while
allowing later service turns to recover exact execution bytes and request
identity.

Enqueue is idempotent:

- rebinding the same packet leaves one queue row;
- rebinding a different packet fails without mutation;
- enqueueing an already issued sequence is a no-op only when canonical issued
  state matches; and
- duplicate sequence membership is a runtime consistency error in test and
  source-policy validation.

Head rows may briefly enter the queue through the common binding path. A
successful fixed-FU head record, data-access issue, or pending-address
materialization removes the exact sequence. This keeps packet binding generic
without creating a second head-specific admission path.

## Materialized Service View

For one service turn, `O3LiveIssueState` materializes an
`O3LiveIssueQueue` from its resident sequence list. Materialization joins each
sequence to the current ROB row, bound packet, rename destination,
pending-address metadata, speculative execution state, and control lineage.

Unlike the current queue capture, this operation does not scan the complete
ROB to discover membership. The persistent issue inventory decides which rows
must be considered. Canonical runtime state still decides whether each
resident row is currently valid and how it must be scheduled.

An ordinary resident row that no longer has matching canonical ownership is a
consistency boundary, not a silent exclusion. Expected pending-address replay
continues to return the exact `ReplayPending(sequence)` outcome so existing
suffix cleanup remains authoritative.

Readiness stays derived:

- `O3LiveIssueDependencyTable` resolves data and control dependencies from
  current runtime state;
- `O3LiveIssueCalendar` derives reservations from all live issued fixed-FU,
  control, pending-address, and data-access rows;
- forwarded values are materialized only for selected rows; and
- writeback admission remains owned by the writeback calendar.

The calendar no longer requires a caller-supplied head reservation during
queue service. Fixed-FU heads are recorded before younger service, and memory
heads already exist as live data rows, so all occupied issue slots can be
derived from runtime state. The head reservation type may remain as the narrow
head-recording contract if that keeps admission code focused.

## Current-Tick Service

Replace local future-tick simulation with
`O3RuntimeState::service_live_issue_queue_at(hart, now)`.

One call performs exactly one simulated issue tick:

1. Materialize the resident queue view.
2. Resolve pending replay before arbitration.
3. Build the dependency table.
4. Capture current reservations.
5. Plan oldest-ready issue at `now`.
6. Prepare exact selected executions from bound packets and forwarded values.
7. Commit the selected batch transactionally.
8. Remove only successfully committed sequences.
9. Record the latest same-tick decision.
10. Compute the next required service tick.

The service does not loop to a later tick. A later issue tick requires a real
scheduler turn through the existing O3 wake path.

Capacity semantics remain:

- total issue capacity equals configured issue width;
- integer ALU capacity equals issue width;
- integer multiply/divide capacity is one modeled slot;
- branch capacity is one modeled slot;
- memory/AGU capacity is bounded by configured memory issue width and the
  modeled pending-address constraints; and
- every issued row also consumes global width.

Oldest-ready selection remains sequence ordered. A younger independent row may
issue while an older row waits on a typed dependency or on a class-specific
resource that does not constrain the younger row. Among rows competing for the
same available capacity, ROB sequence order remains authoritative; enqueue
order can never override architectural age.

## Wake Ownership

The existing `RiscvO3WritebackWakeState` remains the sole scheduler-event
owner. The requested issue-service tick joins the current minimum over memory
result publication, pending data addresses, restored retire-gate progress,
forwarded control, translated result retry, and writeback progress.

Both wake-minimum entry points must include the issue-service tick:

- `RiscvCore::requested_o3_writeback_wake_tick`, which publishes external
  scheduler demand; and
- `RiscvCoreState::refresh_o3_writeback_wake`, which recomputes demand after
  internal lifecycle mutation.

They must call one shared desired-tick helper so their source lists cannot
drift. The helper includes issue service in the same minimum and determines
whether a due-current issue wake may use `requested_tick_with_current`.

Conditional enqueue requests service at the row's admission tick, including
the current tick when the scheduler permits it. Live-window callers bind
packets and refresh wake demand; they no longer simulate younger issue through
future ticks inline. The fired O3 wake is the normal entry to queue service for
both newly resident and previously blocked rows.

After one service turn:

- ready rows left behind by global or class width request `now + 1`;
- dependency-blocked rows request their earliest known admitted producer-ready
  tick;
- a pending-address resource boundary retains its existing exact wake rule;
- an empty queue requests no issue wake; and
- an unresolved row without a legal future wake fails closed through the
  existing replay or consistency path.

Readiness changes may move the correct wake earlier or later. Any operation
that replans writeback or removes a producer must refresh the issue wake.
Stale-early wakes are allowed because service will observe the row still
blocked and request a later tick. Stale-late wakes are forbidden because they
would hide available issue bandwidth.

`mark_o3_writeback_wake_fired` services all due O3 progress in deterministic
order:

1. finalize the fired wake authority;
2. process pending-address wake work that may enqueue or remove rows;
3. service the issue queue at the current tick;
4. refresh writeback, control, retire-gate, translated-result, and issue wake
   minima; and
5. expose one new requested scheduler event, if needed.

This keeps one event authority while allowing pending-address materialization
to make additional rows ready in the same callback.

## Same-Tick Re-Entry And Statistics

The current implementation can enter scheduling more than once at one tick.
Persistent queue service must preserve that behavior without double-counting
blocked rows or issue cycles.

`O3LiveIssueState` therefore retains one active tick decision containing:

- sequences successfully issued at the tick;
- each resident sequence's latest resource- or dependency-blocked
  classification;
- maximum occupied plus issued width observed at the tick; and
- the observation baseline established by the most recent stats reset.

Every same-tick service replaces the latest blocked classification and adds
only newly committed issued sequences. It does not call the additive
`record_issue_cycle` API immediately. Advancing to a later tick or explicitly
finalizing a quiescent current tick seals the decision and publishes it to
`O3RuntimeStats` exactly once.

Live stats reads project the unsealed decision onto a copy of aggregate stats
without mutating either owner. Repeated JSON, text, or stats-dump reads at the
same tick therefore return the same counters. When that decision later seals,
the aggregate receives the same values once and the projection no longer adds
them.

Stats reset preserves queue membership, current occupancy, and the requested
service tick. It clears aggregate counters, rebases peak occupancy to current
occupancy, and records the active decision as the reset baseline so pre-reset
issued or blocked rows cannot reappear in later projections or publication.

## Transactional Recording

The current selected-batch path clones the complete `O3RuntimeState`. A
persistent issue state would make that clone larger and would hide which
authorities a batch can mutate.

Replace it with an issue-specific transaction that stages only:

- selected speculative executions or pending-address materializations;
- ROB readiness updates;
- writeback reservations and any bounded replan;
- pending-address changes;
- queue removals and next-wake changes; and
- issue-decision updates and transient telemetry deltas.

Preparation remains read-only. Commit occurs only after every selected row has
validated exact packet identity, candidate materialization, rename ownership,
writeback admission, and replay status. A failed ordinary row leaves the
runtime and queue unchanged. A typed pending replay commits only the existing
sequence-boundary replay cleanup.

No queue row is removed before its issued or materialized state is durable.

## Cleanup And Failure Semantics

Queue cleanup follows the same sequence boundaries as canonical live state:

- successful issue or address materialization removes the exact row;
- normal retirement removes a remaining exact row before retirement metadata
  is finalized;
- redirect or control squash removes the wrong-path suffix;
- pending replay removes from the replay sequence;
- memory retry or terminal failure removes the affected suffix according to
  existing data lifecycle policy;
- mismatched retirement identity removes or invalidates the same descendants
  as the ROB path;
- HTM abort, reset, restart, trap fallback, and detailed-policy disable clear
  the applicable queue state; and
- `restore` always starts with an empty live issue state.

Suffix cleanup must update queue membership, next-service wake, active tick
decision, and occupancy together. A removed row may not remain as scheduler
authority or as a blocked-stat contribution.

Cleanup remains idempotent. A late callback or repeated discard after the
canonical row is gone must not resurrect queue membership, consume capacity,
or publish a second issue result.

## Checkpoint Semantics

The persistent queue is per-run live state, not durable checkpoint state.
O3RT remains version 23.

A nonempty queue, requested issue wake, or active live issue transaction makes
the CPU non-quiescent for checkpoint capture. A stats-only active tick decision
is sealed during quiescence finalization when the queue and transaction are
already empty; it does not by itself reject an otherwise drained checkpoint.
Live checkpoint attempts reject before mutation through the existing CPU and
system quiescence contract.

`RiscvCore::data_access_lifecycle_is_quiescent` must additionally require
`O3RuntimeState::live_issue_is_quiescent()`. That focused query covers empty
membership, no requested issue-service tick, and no active issue transaction.
`RiscvCore::finalize_quiescent_o3_writeback_for_checkpoint` may seal the
stats-only decision only after the live issue query and existing writeback
authority checks pass. Because the shared desired-tick helper feeds
`RiscvO3WritebackWakeState`, `has_pending_checkpoint_authority` also observes
scheduled or detached issue wakes through the existing wake owner.

A drained checkpoint contains no queue payload. Restore reconstructs stable
ROB, LSQ, rename, pending pipeline, and stats state through O3RT v23 and clears
all transient issue membership and wake state. Existing rejection of nonempty
stable deferred writeback remains unchanged.

Any future restorable live queue requires an explicit new schema and
transactional restore contract. This design must not reinterpret spare O3RT
fields or serialize queue state into the live-retire-gate payload.

## Detailed-To-Timing Handoff

The existing O3DH v7 handoff remains same-run and `restorable = false`.
Persistent queue state does not create another handoff chunk. O3DH carries
live data rows and a younger-row count, but it does not carry decoded issue
packets or unissued IQ identity.

Detailed-to-timing transfer therefore requires the persistent issue queue to
be empty. A resident fixed-FU, control, or pending-address row rejects the
switch before mutation rather than being inferred from the younger-row count.

The existing supported scalar live-data handoff remains executable after all
younger issue rows have left the queue while live data lifecycle rows are still
resident. Successful transfer then preserves the issue, response, writeback,
and commit timing already carried by O3DH and clears only already-empty issue
membership during source cleanup. Timing mode exposes no detailed queue
surface afterward.

Durable checkpoint restore continues to reject registries containing the
non-restorable O3 live-data handoff chunk.

## Stats And Debug Evidence

Retain the existing issue counters:

- issue cycles;
- issued rows;
- resource-blocked row cycles;
- dependency-blocked row cycles; and
- maximum rows issued or occupied per cycle.

Add focused queue evidence:

- enqueue count;
- service-turn count;
- wake-request count;
- current resident occupancy;
- peak resident occupancy; and
- issued rows by supported live issue class.

These new values live in a focused transient `O3LiveIssueTelemetry` snapshot,
not in checkpointed `O3RuntimeStats`. O3RT v23 continues to encode and restore
the existing issue-cycle, issued-row, blocked-row, and maximum-width counters.
Queue telemetry resets on stats reset, O3 restore, and detailed-mode teardown;
after checkpoint restore it reports activity since restore rather than
historical pre-checkpoint queue activity.

JSON publishes queue telemetry under the existing O3 runtime issue object.
Text stats use stable `sim.cpuN.o3.*` names, and `m5_dump_stats` publishes the
same values under host-action samples. Timing mode suppresses the detailed
queue surface. Source policy and checkpoint tests must prove that the transient
telemetry is absent from O3RT v23 encoding.

O3 debug events must identify:

- sequence and PC;
- queued, selected, retained, replayed, squashed, or retired action;
- issue class;
- service and next-wake tick;
- raw and admitted writeback tick when selected; and
- exact cleanup boundary for replay or squash.

Debug output is evidence, not scheduling authority.

## Executable Evidence Matrix

All representative tests run the real binary through `rem6 run --execute`.

### Width-one oldest-ready direct

Use an older dependency-blocked scalar ALU row followed by independent integer
multiply and branch-capable work. Issue width one must select one oldest-ready
row per real scheduler turn, retain the blocked row, and wake it at the exact
producer-ready tick.

Anchor:

- `rem6_run_o3_persistent_iq_width_one_oldest_ready_cross_class_direct`.

### Width-two co-issue direct

Use ready integer ALU and multiply rows plus one dependency-blocked row. Issue
width two must co-issue the two oldest ready rows, preserve class capacity,
and later issue the dependent row without queue recapture from the full ROB.

Anchor:

- `rem6_run_o3_persistent_iq_width_two_coissues_ready_cross_class_direct`.

### Width-four hierarchy

Use cache/fabric/DRAM with one pending data address, independent integer ALU
and multiply rows, and supported control work. The queue must respect total,
memory, multiply, and branch capacities while transport activity proves the
memory row used the hierarchy.

Anchor:

- `rem6_run_o3_persistent_iq_width_four_respects_class_caps_hierarchy`.

### Wakeup matrix

Cover producer-ready, writeback-replanned, resource-next-cycle, and
pending-address wakes. Assert queue residency before wake, exact issue tick,
no duplicate scheduler authority, and no inflated same-tick stats.

Anchor:

- `rem6_run_o3_persistent_iq_cross_class_wakeup_matrix_direct`.

### Squash and replay

Use a predicted wrong-path suffix containing at least two classes and a
pending-address replay case. Assert exact queue suffix removal, no later issue
or transport from removed rows, preserved older results, and correct
architectural state.

Anchor:

- `rem6_run_o3_persistent_iq_squash_discards_wrong_path_queue_suffix`.

### Stats and debug

Run real queue activity through JSON, text stats, `m5_dump_stats`, and O3 debug
output. Assert occupancy, service, wake, class, blocked, issue, writeback, and
commit witnesses from the same run.

Anchors:

- `rem6_run_o3_persistent_iq_text_stats_expose_queue_counters`;
- `rem6_run_o3_persistent_iq_stats_dump_exposes_queue_counters`; and
- `rem6_run_o3_persistent_iq_debug_exposes_residency_and_cleanup`.

### Handoff, checkpoint, and timing boundaries

Preserve a supported detailed-to-timing scalar live-data transfer after the IQ
drains, reject every nonempty queue transfer without mutation, reject live
checkpoint, restore a drained checkpoint, and prove timing-from-start has no
detailed queue surface.

Anchors:

- `rem6_run_host_switch_preserves_o3_persistent_iq_ticks`;
- `rem6_run_o3_persistent_iq_checkpoint_boundary`; and
- `rem6_run_timing_suppresses_o3_persistent_iq_surface`.

Every positive matrix asserts final registers, memory bytes where applicable,
ordered retirement, exact issue/writeback/commit ticks, and direct or hierarchy
resource activity. Recovery tests assert request counts so a retry cannot hide
duplicate execution or transport.

## Focused Ownership

Production ownership is:

- `o3_runtime_issue/state.rs`: resident sequence inventory, wake request,
  occupancy, and same-tick decision publication state;
- `o3_runtime_issue/service.rs`: one-tick queue materialization, planning,
  next-wake calculation, and service outcome;
- `o3_runtime_issue/transaction.rs`: bounded selected-batch commit and replay
  transaction;
- `o3_runtime_issue/queue.rs`: immutable packets, materialized queue entries,
  scheduling candidate types, and validation;
- `o3_runtime_issue/dependency.rs`: typed data/control readiness;
- `o3_runtime_issue/calendar.rs`: current reservations, capacities, and
  generic scheduler invocation;
- `o3_runtime_issue/pending_address.rs`: pending-address adapters;
- `riscv_o3_writeback_wake.rs`: the one external scheduler wake owner; and
- existing live-window, memory, control, handoff, and checkpoint modules:
  canonical lifecycle mutation and cleanup calls only.

Do not add queue service logic to `riscv_live_retire_window.rs`, which is
already at its focused size boundary. Callers bind packets and request issue
service through narrow runtime methods.

## Source Policy

Replace the current prohibition on any persistent live issue queue with a
narrow ownership rule:

- `O3RuntimeState` contains exactly one `O3LiveIssueState` field;
- no other production type stores a second persistent IQ inventory for the
  same unissued rows;
- persistent decoded packets remain owned only by staged fetch identities;
- dependency and calendar types are never stored persistently;
- queue service does not clone the complete `O3RuntimeState`;
- the O3 writeback wake remains the only scheduler-event identity owner;
- checkpoint payloads contain no live issue state or transient queue
  telemetry; and
- all queue cleanup delegates from canonical sequence-boundary lifecycle
  paths.

Add focused test modules instead of extending files already near their caps.
Initial source limits are:

- `o3_runtime_issue/state.rs`: 450 lines;
- `o3_runtime_issue/service.rs`: 600 lines;
- `o3_runtime_issue/transaction.rs`: 450 lines;
- each focused issue-state test child: 500 lines;
- persistent-IQ CLI owner: 900 lines; and
- persistent-IQ source-policy owner: 600 lines.

Existing caps for `o3_runtime.rs`, `o3_runtime_issue.rs`, `queue.rs`,
`calendar.rs`, `dependency.rs`, `riscv_live_retire_window.rs`, and their tests
remain ratchets, not targets to fill.

## Migration Ledger

Update the CPU and Stats evidence text to describe a bounded per-run persistent
cross-class live issue queue with scheduler-turn wakeup/select. Keep the
following limitations explicit:

- no FP or vector arithmetic issue queue rows;
- no system issue rows;
- no general load/store queue scheduler;
- no dependent stores or arbitrary atomics in the live issue queue;
- no arbitrary nonadjacent or unbounded dependency graph;
- no checkpoint-restorable live issue or transport state;
- no general O3 engine; and
- no KVM backend.

The CPU checklist remains 8 of 10 and capped at 74% representative. The Stats
checklist and 74% representative cap also remain unchanged. New queue evidence
strengthens the existing checked items but does not satisfy the open general
running-O3 item.

## Acceptance Criteria

The milestone is complete only when all of these are true:

- unissued supported rows persist across scheduler turns;
- production issue service scans resident membership, not the full ROB, to
  discover its queue;
- one service call models one tick and never records future issue early;
- the existing O3 wake owns all later queue progress;
- width and class capacities remain exact across direct and hierarchy routes;
- dependency and resource stalls retain rows and wake at correct ticks;
- same-tick re-entry does not double-count stats;
- selected-batch failure is transactional without a full runtime clone;
- every lifecycle boundary removes exact queue ownership;
- live checkpoint and unsupported handoff fail before mutation;
- drained restore and supported same-run handoff with an empty IQ remain
  correct;
- JSON, text, stats-dump, debug, architecture, and transport evidence agree;
- source-policy ownership and line caps pass; and
- the migration ledger remains explicit and score-honest.
