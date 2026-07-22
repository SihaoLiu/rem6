# RISC-V O3 Derived Live Issue Queue Design

## Goal

Replace caller-owned live issue request slices with one transient queue derived
from canonical detailed-O3 runtime state. The queue must inventory every
currently supported bound live row, preserve exact fetch identity, feed the
existing dependency and calendar owners, and allow oldest-ready selection
across partial scheduling re-entry.

This increment advances the running-O3 boundary from a generic scheduler fed
by narrow caller slices toward queue-owned IQ/wakeup/select. It keeps the
currently supported scalar, control, and capacity-two pending-address shapes.
It does not add a persistent checkpointed IQ, new instruction classes, wider
memory concurrency, or a general O3 engine. The CPU checklist and 74%
representative score remain unchanged.

## Current Boundary

The live issue calendar now owns per-tick reservations, effective capacities,
scheduler construction, cycle planning, and same-tick decision aggregation.
The dependency table owns data and control readiness. The scheduling loop still
receives `&[O3LiveIssueRequest]` from the retire-window caller and reconstructs
its candidate inventory from only that slice.

That request-slice boundary has four structural problems:

- a live row remains eligible only while a caller supplies it again;
- scheduling candidates retain a `request_index` coupling to an ephemeral
  vector;
- exact decoded instruction and fetch-request identity are not owned by the
  staged runtime identity after binding; and
- candidate inventory, deduplication, stale pending detection, and execution
  packet lookup remain split between the caller and the root scheduling loop.

The ROB, rename map, pending-address set, staged fetch identities, recorded
speculative executions, writeback reservations, and control lineages already
provide the canonical state needed to derive a queue. The missing authority is
a focused read-only capture that joins those owners without duplicating them.

## Considered Approaches

### 1. Derived live issue queue

Bind an exact decoded issue packet to each staged fetch identity, then rebuild a
sequence-ordered queue from canonical runtime state before every arbitration
pass. The queue owns candidate inventory and identity validation. Dependency,
calendar, execution, replay, and mutation retain their existing owners.

This is the chosen approach. It removes caller-slice ownership and
`request_index` coupling while keeping one source of persistent O3 state.

### 2. Persistent checkpointed issue queue

Store queue entries directly in `O3RuntimeState` and serialize them. This would
create synchronization duties with the ROB, rename map, pending-address set,
writeback state, and invalidation paths before cross-class queue semantics are
complete. It also widens checkpoint compatibility without being required for
partial re-entry.

### 3. Typed capacity-two dependent-address reads

Add FP and vector pending dependent-address result rows while retaining the
current caller-slice issue path. This would provide a useful bounded matrix but
would leave the general IQ/wakeup/select ownership gap intact.

### 4. Static request-slice extraction

Move `live_issue_candidates` into a child module while continuing to pass a
request slice. This reduces root line count but does not change ownership or
behavior and would preserve the sequence-to-index coupling.

## Chosen Architecture

Add `crates/rem6-cpu/src/o3_runtime_issue/queue.rs` with four focused concepts:

- `O3LiveIssuePacket`: the exact decoded instruction and consumed fetch
  requests bound to one staged sequence;
- `O3LiveIssueQueueEntry`: one sequence-keyed supported scheduling candidate
  plus its immutable packet;
- `O3LiveIssueQueue`: a sequence-ordered transient inventory; and
- `O3LiveIssueQueueCapture`: either a valid queue or a typed pending-address
  replay boundary.

The queue is derived, not stored. `O3RuntimeState` must not gain an
`O3LiveIssueQueue` field, and checkpoint payloads must not encode queue state.
Queue capture runs again after every execution record, replay cleanup, or tick
advance that can change eligibility or readiness.

`o3_runtime_issue.rs` retains the scheduling loop, execution preparation,
transactional recording, replay mutation, time advancement, and stats
application. `dependency.rs` remains the sole dependency-scope and resolution
authority. `calendar.rs` remains the sole reservation, capacity, scheduler,
cycle-plan, and tick-decision authority.

## Bound Fetch Packets

`O3LiveStagedFetchIdentity` continues to be created when a live ROB row is
staged. Its initial instruction identity remains sufficient for admission and
rename validation. Fetch completion then binds one immutable
`O3LiveIssuePacket` containing:

- the exact `RiscvDecodedInstruction`, including instruction length;
- the exact ordered `Vec<MemoryRequestId>` consumed by fetch; and
- the decoded instruction's architectural instruction, which must equal the
  initially staged instruction.

Binding is idempotent. Rebinding the same packet succeeds. A different decoded
instruction, instruction length, request sequence, request agent, or request
ordering fails without replacing the original binding.

Callers bind newly completed younger instructions and then invoke live issue
scheduling. They no longer construct or pass `O3LiveIssueRequest` vectors.

## Queue Capture

Capture scans live-staged ROB entries in increasing sequence order. For each
entry it joins the ROB row with staged fetch identity, pending-address metadata,
rename ownership, recorded speculative execution state, and current control
lineage.

A normal queue entry exists only when all of these are true:

- the ROB row is still live staged;
- it is not the current reserved head;
- it has one exact bound issue packet;
- it has not already been recorded as a live speculative execution;
- its current instruction kind is supported by the existing live issue lane;
- its staged rename destination matches the decoded destination; and
- its data and control producers can be represented by the existing typed
  dependency authority.

Unbound non-head rows remain resident but are not eligible and are not counted
as resource- or dependency-blocked. Invalidated identities live in their
existing invalidated owner and never re-enter the queue.

Pending-address rows are stricter. An authorized pending row must have its
bound packet and exact producer metadata. If that join fails, capture returns
`ReplayPending(sequence)` rather than silently dropping the row. The root then
performs the existing sequence-boundary suffix cleanup.

Capture rejects duplicate live sequences or a bound supported row that cannot
be classified consistently. These are runtime consistency errors, not normal
queue exclusions.

## Candidate Ownership

Move live scheduling candidate identity out of
`o3_runtime_control_window.rs` and into the queue owner. Queue entries own:

- sequence and PC;
- decoded issue packet;
- supported candidate kind;
- issue operation class;
- data producer sequences and registers; and
- optional control dependency.

The existing supported kinds remain:

- scalar integer destination;
- supported live control with an optional integer link destination; and
- scalar pending data address with an integer rename destination.

Float, vector, and system issue candidates remain outside v1. The existing
capacity-two pending-address boundary also remains unchanged.

`request_index` is removed. Selected scheduler rows are resolved back to queue
entries by unique O3 sequence. Execution preparation uses the packet carried by
that exact entry.

## Arbitration Data Flow

Each scheduling pass follows this order:

1. Capture a fresh `O3LiveIssueQueue` from runtime state and the current head.
2. Handle a typed pending replay boundary before planning.
3. Build `O3LiveIssueDependencyTable` from the queue entries.
4. Capture a fresh `O3LiveIssueCalendar` from runtime state and the current
   head reservation.
5. Ask the calendar to plan the queue entries at the current tick.
6. Resolve selected sequences to exact queue entries.
7. Materialize executions from their bound decoded packets and forwarded
   values.
8. Record the selected batch transactionally.
9. Recapture before another arbitration pass.

The queue does not decide readiness, consume capacity, execute instructions,
record state, advance time, or apply stats.

## Partial Re-entry Semantics

Once a staged row has a bound issue packet, caller lifetime no longer controls
its eligibility. It remains visible to later queue captures until it issues,
is invalidated, is replayed, or leaves the ROB.

This permits a later scheduling invocation to consider an older blocked row
and newly bound younger rows together. The existing oldest-ready scheduler may
select a younger independent row while the older row waits on a typed
dependency. When the dependency resolves, the older row remains available
without the caller reconstructing its original request slice.

Same-tick partial re-entry retains current semantics. The queue and calendar
are both rebuilt after mutation, while `O3LiveIssueTickDecision` replaces the
blocked classification for the latest plan and accumulates only successfully
recorded issued rows. Queue recapture alone never increments issue statistics.

## Replay And Failure Semantics

Expected pending replay uses the typed `ReplayPending(sequence)` capture or
batch outcome. The root clones runtime state, removes the affected pending row
and younger suffix, commits that cleanup, flushes the current tick decision,
and returns to the existing replay path.

Other failure rules are:

- a mismatched packet binding returns false and preserves the first binding;
- an unbound ordinary row is ineligible, not an error;
- a selected sequence missing from the captured queue is an invalid plan;
- a selected entry that no longer materializes is an execution consistency
  error, except for the existing pending replay case;
- duplicate sequence identity is rejected before dependency planning; and
- redirect, retry, failure, and reset cleanup continue to remove queue input by
  mutating canonical owners, never by mutating a stored queue.

## Checkpoint, Mode, And Stats Semantics

The queue is transient and has no checkpoint representation. A live checkpoint
continues to reject because canonical unsnapshotted ROB, LSQ, pending-address,
or speculative execution state is active. A drained checkpoint captures no
queue state and restores with an empty derived queue.

Detailed-to-timing transfer continues to serialize the existing live execution
authority and inherited issue/writeback/commit ticks. It does not serialize a
second IQ. Timing mode reaches the same architectural and memory witnesses
without detailed O3 queue or issue surfaces.

V1 adds no public stats fields and does not change checkpoint versions. The
existing issue artifact remains authoritative for arbitration cycles, issued
rows, resource-blocked row cycles, dependency-blocked row cycles, and maximum
rows issued per cycle. Exact ROB/LSQ residency and per-instruction
issue/writeback/commit ticks provide the queue behavior evidence.

## Executable Evidence Matrix

The main evidence must run the real `rem6` binary through
`rem6 run --execute`.

### Oldest-ready ALU/MUL select

The focused runtime test owns the exact partial-reentry proof because no
current CLI path uniquely exposes an omitted caller request slice. Use a real
older dependency-blocked scalar row followed by younger independent ALU and
multiply rows for direct width-one and width-two CLI runs that prove:

- ready younger rows may issue before the blocked older row;
- width and multiply-port pressure remain deterministic;
- the older row issues at its exact producer wake tick; and
- architectural registers, memory bytes, and ordered commit are correct.

The representative anchors are:

- `rem6_run_o3_general_iq_oldest_ready_width_one_direct`; and
- `rem6_run_o3_general_iq_oldest_ready_width_two_direct`.

### Pending-address and scalar mixed select

Use a cache/fabric/DRAM result window with two pending scalar addresses and one
independent scalar row bound across separate captures. At issue width two, one
memory row and the scalar row may co-issue, while the sibling memory row is
resource-blocked by the one-slot memory class. Evidence includes exact
four-row ROB residency, three-row LSQ residency, request ordering, memory
bytes, issue counters, and cache/transport/fabric/DRAM activity.

The representative anchor is:

- `rem6_run_o3_general_iq_pending_address_and_scalar_hierarchy`.

### Control release

Use a supported control row and descendant that are bound before the control
scope resolves, plus an independent younger scalar row. The independent row
may issue first. The descendant remains dependency-blocked until control
writeback plus one and then issues exactly once.

The representative anchor is:

- `rem6_run_o3_general_iq_control_release_orders_descendant`.

### Lifecycle and suppression

Focused CLI rows prove:

- redirect or pending replay removes only the affected suffix;
- live checkpoint rejects and a drained checkpoint restores empty;
- detailed-to-timing transfer preserves inherited issue/writeback/commit
  ticks; and
- timing mode preserves architecture while suppressing detailed O3 surfaces.

The representative anchors are:

- `rem6_run_o3_general_iq_checkpoint_boundary`;
- `rem6_run_host_switch_preserves_o3_general_iq_ticks`; and
- `rem6_run_timing_suppresses_o3_general_iq_surface`.

## Focused Unit Evidence

Focused `rem6-cpu` tests must cover:

- sequence-ordered capture across previously bound rows;
- an unbound row becoming eligible only after exact packet binding;
- idempotent binding and mismatched rebinding rejection;
- invalidated identity exclusion;
- stale pending metadata returning the exact replay sequence;
- duplicate sequence rejection;
- sequence-based selected-entry lookup with no request index;
- unsupported FP/vector/System instruction exclusion;
- the unchanged third pending-address boundary; and
- same-tick recapture without issue-stat double counting.

## Focused Ownership

Production ownership is:

- `o3_runtime_issue/queue.rs`: bound packets, queue entries, candidate
  inventory, capture validation, sequence lookup, and typed replay outcome;
- `o3_runtime_issue/dependency.rs`: dependency scopes and resolution timing;
- `o3_runtime_issue/calendar.rs`: reservations, capacities, scheduler, cycle
  plans, and tick-decision aggregation;
- `o3_runtime_issue.rs`: scheduling loop, execution preparation,
  transactional recording, replay mutation, time advancement, and stats;
- `o3_runtime_live_window.rs`: staged identity lifecycle and packet binding;
  and
- `riscv_live_retire_window.rs`: fetch completion and invocation of live issue
  scheduling.

Proposed source-policy caps are:

- `o3_runtime_issue.rs` at the existing 800 lines;
- `o3_runtime_issue/queue.rs` at 600 lines;
- `o3_runtime_issue/queue_tests.rs` at 450 lines;
- `o3_runtime_issue/calendar.rs` at the existing 450 lines; and
- `o3_runtime_issue/dependency.rs` at the existing 500 lines.

Source policy must require:

- exactly one production queue module and one compiled queue test module;
- queue candidate and packet authority only in `queue.rs`;
- no `O3LiveIssueQueue` field in any production struct;
- no production `O3LiveIssueRequest` or `request_index` selection path;
- queue capture inside the scheduling loop before dependency and calendar
  planning;
- sequence-based selected-entry resolution;
- caller delegation without constructing issue request vectors; and
- focused production/test line caps and exact required test anchors.

## TDD Sequence

Implementation starts with failing queue tests and a failing partial re-entry
runtime test. The initial RED evidence must show that the existing API can see
only the current request slice and that an older bound row disappears from a
later plan unless the caller supplies it again.

After queue capture and packet binding are GREEN, migrate the scheduling root,
then add source policy, then add the CLI matrix. The final commands are:

```bash
cargo fmt --all -- --check
cargo test -p rem6-cpu --lib live_issue_queue -- --nocapture
cargo test -p rem6-cpu scoped_issue -- --nocapture
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo test -p rem6 --test cli_run o3_general_iq -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --all-targets
cargo test --workspace
```

Commands use repository-local `target/tmp` as `TMPDIR` when the host temporary
filesystem lacks space.

## Documentation Boundary

The implementation updates the existing CPU execution evidence text and test
anchors to name the derived queue and its real CLI matrix. The migration
ledger remains exactly 1,200 lines.

The running-O3 checklist item remains unchecked, and the CPU score remains 74%
representative. A persistent or fully general cross-class IQ, arbitrary
FP/vector issue, broader memory/AGU concurrency, more than two unresolved
addresses, dependent stores and atomics, restorable live transport,
comprehensive squash/recovery, and KVM or equivalent fast-forward remain open.

## Non-goals

This increment does not:

- persist or checkpoint an issue queue;
- add queue occupancy or readiness stats;
- change checkpoint versions;
- add Float, vector, or System issue capacity;
- widen the pending-address capacity beyond two;
- admit dependent stores, atomics, translated memory, or MMIO;
- change issue width or operation-class capacity rules;
- replace the ROB, LSQ, rename map, dependency table, or issue calendar; or
- mark the running-O3 migration item complete.
