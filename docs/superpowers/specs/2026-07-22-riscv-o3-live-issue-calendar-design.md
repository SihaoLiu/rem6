# RISC-V O3 Live Issue Calendar Design

## Goal

Extract live detailed-O3 issue reservation and arbitration ownership from the
current scheduling loop into one focused cycle calendar. The calendar must
derive occupied issue width and operation-class slots from canonical runtime
state, invoke the existing scoped scheduler, and return one typed cycle plan
without becoming a second ROB, issue queue, checkpoint owner, or execution
authority.

The increment is behavior preserving. It must retain same-tick partial
re-entry, dependency wakeup, functional-unit contention, pending-address
replay boundaries, issue statistics, checkpoint rejection, host switching,
and timing-mode suppression through the top-level `rem6 run --execute` path.

This is an architectural step toward a general running O3 engine. It does not
claim a persistent general issue queue, arbitrary wakeup/select, additional
functional units, a wider memory/AGU lane, restorable live transport state, or
the still-open full-O3 migration item. The CPU checklist and representative
score remain unchanged.

## Current Boundary

`O3RuntimeState::schedule_live_speculative_issues` currently owns several
separate responsibilities in one loop:

- candidate discovery and exact fetch-identity validation;
- pending-address replay and wake boundaries;
- dependency-table construction;
- reconstruction of head, prior-issue, and pending-address reservations;
- effective issue-width and operation-class capacity calculation;
- `O3ScopedIssueScheduler` construction and invocation;
- execution preparation and transactional recording;
- same-tick issue-decision aggregation;
- issue-stat application; and
- resource- and dependency-driven time advancement.

The generic `O3ScopedIssueScheduler` already provides the correct one-cycle
oldest-ready plan. The live runtime additionally needs a cycle-visible view of
capacity already consumed by the head and by rows selected during earlier
entries. That reservation logic currently lives at the bottom of
`o3_runtime_issue.rs` as private counters and capacity reduction helpers.

The behavior is covered, but the ownership is not suitable for continued O3
growth. Adding more operation classes, issue ports, or persistent queue state
to the current loop would further combine scheduling policy with runtime side
effects and replay control.

## Considered Approaches

### 1. Derived live issue calendar

Create a focused calendar from canonical runtime state for each arbitration
pass. It owns reservation reconstruction, capacity reduction, scoped scheduler
invocation, and typed cycle-plan results. The runtime loop continues to own
candidate discovery, execution, replay, mutation, and time advancement.

This is the chosen approach. It establishes one reservation and arbitration
authority without duplicating persistent state.

### 2. Persistent checkpointed issue calendar

Store an issue calendar or issue queue directly in `O3RuntimeState` and add it
to checkpoint state. This would require synchronization with the existing ROB,
rename, speculative execution, pending-address, and writeback owners before a
general issue queue contract exists. It would also widen checkpoint semantics
for a behavior-preserving increment.

### 3. Static helper extraction only

Move the existing capacity function into a child file while leaving scheduler
construction and cycle-plan interpretation in the root loop. This reduces line
count but does not establish a meaningful arbiter boundary and would invite a
second scheduler owner when new issue classes arrive.

### 4. Incrementally maintained transient calendar

Capture reservations once at scheduling entry and append newly issued rows to
that local calendar. This appears efficient, but writeback replanning can move
producers and invalidate speculative descendants while a batch records. A
local incremental view could therefore retain reservations for rows no longer
present in canonical runtime state.

The calendar is instead rebuilt before every arbitration pass. Correctness is
more important than avoiding a small scan of the bounded live window.

## Chosen Architecture

Add `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs` with these focused
types:

- `O3LiveIssueCalendar`: a read-only derived reservation inventory plus the
  configured issue width;
- `O3LiveIssueReservations`: occupied global width and per-class slots for one
  tick;
- `O3LiveIssueCyclePlan`: the live-runtime wrapper around one
  `O3ScopedIssuePlan`; and
- `O3LiveIssueTickDecision`: same-tick aggregation returned to the runtime for
  stats application.

`o3_runtime_issue.rs` declares `mod calendar` and imports only the narrow
calendar interface. `O3ScopedIssueScheduler` construction, the live issue
queue identifier, reservation counters, and capacity reduction move into the
calendar owner.

The existing `O3LiveIssueHeadReservation` remains the entry contract used by
data-access and fixed-FU heads. Candidate and dependency types remain in their
current owners. The generic scheduler remains unchanged and continues to know
nothing about `O3RuntimeState`, RISC-V execution, pending addresses, or
checkpointing.

## Reservation Capture

Before every scheduler invocation,
`O3LiveIssueCalendar::capture(runtime, head)` derives reservations from three
canonical sources:

1. reserve the supplied head when its issue tick matches the queried cycle;
2. reserve one memory-class slot for each tick with a selected
   pending-address materialization; and
3. reserve every still-live speculative execution at its recorded issue tick,
   excluding the head sequence so it is not counted twice.

Only speculative rows whose sequence still owns a live-staged ROB entry are
eligible. Rows removed by replay, redirect, rollback, writeback replanning,
retirement, reset, or mode transfer therefore disappear from the next derived
calendar automatically.

The reservation inventory is keyed by tick in a `BTreeMap`. Each tick records:

- total occupied issue width;
- integer-ALU slots;
- integer-multiply slots;
- branch slots; and
- memory/AGU slots.

Float and system reservations continue to consume global width without
creating a supported live scheduling capacity for those classes. This matches
the current behavior and does not authorize new FP or system issue shapes.

Selected pending-address rows are represented once per tick, preserving the
current single memory/AGU lane even when more than one pending row exists in
the bounded collection. The calendar does not inspect transport requests or
materialize addresses.

## Cycle Arbitration

For one tick, the runtime builds the existing
`O3LiveIssueDependencyTable`. The calendar receives the tick, dependency
table, and complete candidate slice. It then:

1. reads the tick reservation or an empty reservation;
2. subtracts occupied slots from the configured live capacities;
3. constructs `O3ScopedIssueScheduler` with the configured issue width and
   remaining operation-class capacities;
4. supplies reserved global width, resolved dependency scopes, and all scoped
   candidates; and
5. maps the generic scheduler result into `O3LiveIssueCyclePlan`.

Capacity semantics remain exact:

- integer ALU capacity equals issue width minus existing integer-ALU rows;
- integer multiply, branch, and memory each retain one modeled slot minus an
  existing reservation;
- global issue width is reduced by every reserved row regardless of class;
- zero capacities are omitted from the scheduler input; and
- saturating subtraction preserves current fail-closed behavior for malformed
  or over-reserved derived state.

`O3LiveIssueCyclePlan` exposes issued, resource-blocked, and
dependency-blocked rows plus reserved width. It does not execute instructions,
forward values, mutate dependencies, or decide pending-address replay.

## Runtime Lifecycle

`schedule_live_speculative_issues` remains the orchestration owner. Its loop
uses this order:

1. validate that the head is still represented by live runtime ownership;
2. stop after flushing stats when every request is already recorded;
3. discover and deduplicate scheduling candidates;
4. preserve the existing missing pending-candidate replay boundary;
5. build the dependency table;
6. capture a fresh calendar and request the cycle plan;
7. prepare the complete selected batch;
8. transactionally record the prepared batch or execute the existing
   pending-address replay path;
9. aggregate the plan outcome for the current tick; and
10. retry, advance, jump to dependency resolution, or stop according to the
    existing resource and dependency rules.

A successful batch is not appended to the local calendar. If another
arbitration pass occurs at the same tick, a newly captured calendar observes
the updated runtime state. This also observes any descendant invalidation or
reservation movement caused by writeback replanning during batch recording.

Pending-address behavior remains outside the calendar:

- a blocked pending memory candidate records its existing next wake and stops;
- a dependency-blocked pending candidate does not jump beyond the current
  wake entry boundary;
- failed materialization replays from the exact pending sequence; and
- stale pending candidates discard from the existing sequence boundary.

The calendar neither creates nor clears wake state.

## Decision And Stats Semantics

Move `O3LiveIssueTickDecision` and its aggregation logic into the calendar
module, but keep stats mutation in `O3RuntimeState`.

For each plan attempt at one tick, the runtime reports the number of rows that
were actually recorded:

- a successful batch reports `plan.issued().len()`;
- replay or failed materialization reports zero; and
- a plan with no selected rows reports zero.

The decision aggregates newly recorded rows across repeated passes at the
same tick, keeps the latest resource- and dependency-blocked row counts, and
records the maximum of `reserved_width + recorded_rows`. Before any stop or
tick change, the runtime takes the decision and calls the existing
`record_issue_cycle` path exactly once.

`live_issue_cycle_ticks` remains the authority preventing duplicate cycle
counts across separate scheduling entries. `reset_stats` continues to clear
both scheduler counters and that tick set. JSON, text, stats-dump, checkpoint,
and host-switch surfaces retain their current names and values.

No new statistic is added for this extraction.

## Failure And Checkpoint Semantics

Calendar capture and planning are read-only. Generic scheduler errors map to
the existing `O3RuntimeError::InvalidLiveIssuePlan`. No calendar operation can
leave partial runtime state.

Execution preparation and recording retain the existing cloned-runtime
transaction. If one selected row cannot be materialized or recorded, no
calendar state needs rollback because the derived calendar is discarded.
Pending replay keeps its current staged cleanup behavior.

Tick advancement retains saturating arithmetic. When a next tick cannot be
greater than the current tick, scheduling flushes the current decision and
stops. Unknown dependency resolution remains unresolved rather than guessing
a wake time.

The calendar is not stored in `O3RuntimeState`, snapshots, or checkpoints.
Checkpoint version and payload fields remain unchanged. Live scoped-issue
checkpoints continue to fail closed through the existing CPU quiescence rule,
while drained checkpoint/restore preserves only canonical runtime and stats
state.

## Focused Ownership

Production ownership is:

- `o3_runtime_issue/calendar.rs`: reservation capture, capacity reduction,
  scheduler construction, cycle-plan wrapper, and tick-decision aggregation;
- `o3_runtime_issue/dependency.rs`: dependency scopes and resolution timing;
- `o3_runtime_issue/pending_address.rs`: pending-address scheduler and wake
  adapters;
- `o3_runtime_issue.rs`: requests, candidate discovery, execution preparation,
  transactional recording, replay orchestration, time advancement, and stats
  application; and
- `o3_pipeline_scoped_issue.rs`: generic one-cycle oldest-ready planning.

The proposed source-policy caps are:

- `o3_runtime_issue.rs` at the existing 800 lines;
- `o3_runtime_issue/calendar.rs` at 450 lines;
- `o3_runtime_issue/calendar_tests.rs` at 450 lines;
- `o3_runtime_issue/dependency.rs` at the existing 500 lines; and
- `o3_pipeline_scoped_issue.rs` at its existing owner boundary.

Source policy must require:

- exactly one calendar module declaration;
- `O3LiveIssueCalendar::capture` delegation from the issue root;
- `O3ScopedIssueScheduler::new`, the live queue identifier, reservation
  counters, and capacity reduction only in `calendar.rs`;
- no calendar field in `O3RuntimeState` or checkpoint structures;
- no second reservation scan in the issue root or live-retire facade; and
- compiled calendar-test anchors plus the production and test line caps.

The old source-policy assertion that requires scheduler construction in
`o3_runtime_issue.rs` is migrated to require it in the calendar owner. Other
issue authority remains in the root and must not be duplicated elsewhere.

## TDD And Executable Evidence

TDD starts with focused calendar tests that fail while reservation and
scheduler ownership remains in the root. The calendar test module proves:

- head reservation consumes width and the matching operation-class slot;
- a recorded head is excluded from the prior-speculative scan;
- prior live rows at one tick consume cross-class capacities independently;
- selected pending-address ticks consume one memory slot;
- retired, replayed, redirected, or otherwise non-live speculative rows are
  not captured;
- reconstructing after canonical removal releases the stale reservation;
- full width, multiply, branch, and memory reservations classify ready rows as
  resource blocked;
- unresolved scopes remain dependency blocked even when capacity exists; and
- repeated same-tick decisions aggregate recorded rows and maximum occupancy
  without double-counting issue cycles.

Existing runtime tests remain mandatory evidence for:

- same-tick partial re-entry without overbooking;
- unknown and exact return-owner handling;
- long-FU dependency wakeup;
- sequence-ordered selected rows;
- rollback and replay cleanup; and
- issue-stat reset.

No new synthetic CLI fixture is added. The existing top-level scoped-issue
matrix already invokes `env!("CARGO_BIN_EXE_rem6")` and proves the complete
external behavior needed by this extraction:

- width-one serialization;
- width-two cross-resource co-issue;
- same-multiply resource contention through cache/fabric/DRAM;
- dependency wait behind a long-FU head;
- JSON, text, and stats-dump arbitration counters;
- timing-mode suppression;
- live checkpoint rejection and drained restore; and
- host-switch preservation of issue timing.

Expected verification includes:

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu live_issue_calendar
cargo test -p rem6-cpu scoped_issue
cargo test -p rem6-cpu --test source_policy o3_runtime_issue
cargo test -p rem6 --test cli_run o3_scoped_issue
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --all-targets
cargo test --workspace
```

Commands use the repository-local `target/tmp` as `TMPDIR` when the host
temporary filesystem lacks space.

## Documentation Boundary

The implementation may update existing O3 issue evidence text to name the
derived live issue calendar and its real CLI matrix. It must not mark the full
running-O3 item complete, add a new checklist row, or change the CPU score.

The migration ledger remains exactly 1,200 lines. Persistent general issue
queue ownership, arbitrary FP/vector issue, broader memory concurrency,
additional execution resources, full squash/recovery, KVM/fast-forward, and a
general checkpointable O3 engine remain open.
