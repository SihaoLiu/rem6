# RISC-V O3 Live Checkpoint Restore Design

## Goal

Make a bounded, representative subset of CPU-owned live RISC-V O3 state
checkpoint-restorable through the real `rem6 run` host checkpoint and restore
path.

The increment covers two complementary states:

- a compute-only persistent issue queue containing ready and
  dependency-blocked rows; and
- a completed scalar `FLW` or `FLD` response that has left transport, owns a
  memory-result writeback reservation, and still blocks younger scalar FP
  arithmetic before publication.

Restore must reproduce exact queue selection, writeback admission,
producer-to-consumer wakeup, ordered retirement, final registers and bytes,
and statistics without duplicate issue, memory request, response, writeback,
or retirement.

This increment does not restore a request before its response reaches the CPU.
Cache, fabric, DRAM, MMIO, translation, and transport continuations remain
non-restorable because those requests currently own boxed callbacks and
resource reservations that have no durable transaction identity. The design
must keep those checkpoint attempts rejected and must not describe a
response-admitted CPU result as transport restoration.

The CPU and Stats checklist scores remain honest and capped at their current
representative values. The increment closes explicit live-IQ and scheduled
restore evidence gaps but does not constitute a general O3 engine or general
live transport checkpointing.

## Current Boundary

`O3RT` v23 already serializes the stable O3 projection:

- ROB rows, including live-staged metadata and ready ticks;
- LSQ rows;
- the checkpoint-normalized rename map;
- stable pending pipeline state;
- dependency-producer markers;
- aggregate runtime statistics; and
- an optional live retire gate.

It intentionally does not serialize the persistent issue queue, live rename
overlay, staged issue packets, speculative execution records, writeback
reservation calendar, completed live data results, pending-address state,
control lineage, or O3 scheduler wake ownership. `O3RuntimeState::restore`
clears those fields.

The RISC-V checkpoint port therefore finalizes only drained writeback state
and rejects unless the complete data-access and live-issue lifecycle is
quiescent. Host scheduler checkpointing already supports component-owned event
claims: in-order pipeline and live-retire-gate wakes can be excluded from the
scheduler projection and discarded or rebound by their owning CPU component.
O3 writeback wakes already retain an exact scheduler instance and pending
event snapshot, but are not yet included in those claims.

The preceding FP-load forwarding increment provides useful executable
boundaries. It discovers both:

- a queued-before-response tick, where transport still owns the load; and
- a response-admitted tick, where the response is resident in CPU-owned live
  result state and the dependent FP row remains in the issue queue.

Both checkpoints currently fail as non-quiescent. The first failure remains
correct. The second becomes the representative live restore path.

## Considered Approaches

### 1. Remove the quiescence gate

Allow capture whenever `O3RT` can encode the current ROB, LSQ, and rename map.

This is incorrect. `O3RT` normalizes the live rename map back to committed
state and omits the queue, issue packet, writeback calendar, result bytes, and
scheduler wake. Restore would silently discard the only owners capable of
finishing the in-flight rows.

### 2. Expand `O3RT` v24 into a full runtime image

Append every transient `O3RuntimeState` and `RiscvCoreState` field to the
current payload.

This conflates the stable runtime checkpoint with a large and still partial
core image. It would version unrelated drained checkpoints, duplicate
non-restorable transport authority, and encourage raw serialization of
transactional and debug-only fields. It also makes absence of live state less
explicit.

### 3. Add an optional validated `O3LC` live overlay

Keep `O3RT` v23 as the stable base and add one optional live checkpoint chunk
owned by the same RISC-V checkpoint record. The overlay stores only the
canonical state needed to resume the supported CPU-owned rows. Its decoder
validates every cross-reference against `O3RT` before any component mutates.

This is the chosen approach. A missing overlay retains exact legacy drained
semantics. A present overlay has one current version, one owner, explicit
supported-state gates, and an independently testable corruption boundary.

The existing `O3DH` handoff payload is not reused. It is a same-run execution
mode transfer format, explicitly non-restorable, and covers resident scalar
transport ownership rather than persistent live issue state.

## Chosen Architecture

### Stable Base And Live Overlay

Add `RISCV_O3_LIVE_CHECKPOINT_CHUNK` with magic `O3LC` and version 1.
`RiscvCoreCheckpointRecord` owns an optional
`RiscvO3LiveCheckpointPayload` beside its required `O3RuntimeCheckpointPayload`.

Capture follows this rule:

1. If all transient O3 state is empty, write only the existing `O3RT` chunk.
2. If the transient state matches one supported live profile, write `O3RT`
   plus `O3LC` atomically.
3. If any unsupported live authority exists, reject before writing any bank.

Decode first validates each chunk independently, then validates the live
overlay against the stable runtime payload. Restore applies the stable base
first and the already-validated overlay second. The overlay never acts as an
alternative authority for ROB, LSQ, aggregate stats, committed architectural
registers, PMP, vector state, or branch predictors.

### Source-Local Checkpoint Preparation

A scheduled source-local `Checkpoint` or `RestoreCheckpoint` may establish a
bounded prepare interval from its source event through its delivery tick. Each
attached RISC-V core owns a reference-counted prepare deadline. While a
deadline is active, the core continues data responses, instruction responses,
O3 service, writeback, and retirement, but does not issue a new instruction
transport request. Preparation never cancels an issued request, drops a
completed fetch, changes the queue wake, or makes otherwise unsupported
authority capturable.

Checkpoint delivery still rejects capture unless instruction transport is
fully drained and every normal live-profile guard passes. Restore preparation
never relaxes prepared-image validation, scheduler-snapshot requirements, or
unsupported-authority rejection. The exact delivery deadline is the only
prepare authority allowed during either operation.

Every delivery releases its own reference after success or failure. A failed
delivery must not clear overlapping preparation owned by another event.
Expired references cannot block later fetches. Successful restore replaces
the destination timeline and clears all destination-timeline preparation;
the delivery's final release is therefore idempotent after that scrub.
Immediate and non-source-local checkpoint/restore callers keep their existing
behavior.

The prepare reference is orchestration state, not simulated architectural or
O3 state, so it is never serialized in `O3RT` or `O3LC`. Serial and parallel
schedulers use the same source-event-before-delivery ordering and must produce
the same manifest payload length/checksum evidence and restored queue timing.
Tests cover successful cleanup, expiry, overlapping failed restore, successful restore scrub,
captured-row retirement during the restore interval, suppression of younger
fetch issue, and outstanding-at-delivery rejection without cancellation.

### Supported Live Profiles

Version 1 supports exactly two profiles.

`ComputeQueue` permits:

- one or more live-staged ROB rows with replayable pending execution events;
- an ordered persistent issue membership projection;
- a live rename map that differs from the committed rename map; and
- one requested issue-service wake deadline.

Every supported row must be backed by one completed, single-request
instruction fetch. Split-fetch packets and producer-forwarded fetch identities
remain rejected in version 1; they require a separate fragment projection and
cross-fragment decoder contract.

It requires no live data access, pending address, speculative execution,
control lineage, translation, outstanding instruction or data transport
request, buffered effect, or writeback reservation.

`CompletedFpLoad` adds:

- exactly one completed, not-yet-published `FLW` or `FLD` live data access;
- its exact fetch and data request identities;
- exact issue, response, raw-ready, admitted-writeback, and latency ticks;
- the completion physical address, width, byte offset, response bytes, and
  typed FP writeback target;
- its live LSQ and ROB sequence span;
- exactly one memory-result writeback reservation; and
- bounded younger replayable scalar FP queue rows.

It requires `outstanding_data`, `buffered_o3_effects`, pending and ready
translations, translation frontend work, forwarding overlays, and every
pre-response continuation to be empty. The load must already have a validated
CPU completion. Stores, atomics, vector memory, MMIO, translated pending
requests, retries, failures, and partial forwarding overlays remain rejected
in version 1.

Capture also rejects an active issue transaction or decision mutation. Host
actions run between scheduler callbacks, so a supported checkpoint observes a
stable service boundary rather than serializing rollback scratch state.

An immutable active or retained issue-decision projection at that boundary is
normalized, not rejected. One core lock produces both chunks: `O3RT` receives
the aggregate returned by `O3RuntimeState::stats()`, including the projected
decision delta, while `O3LC` starts with an empty decision window and preserves
the service generations needed to avoid replaying the captured decision. The
source runtime is not mutated. A transaction-active service turn still fails
capture.

### Replayable Pending Event Projection

The issue queue derives packets from live-staged fetch identities. Do not
serialize a second queue packet object or assign persistent numeric tags to
the large `RiscvInstruction` enum.

Instead, add one bounded `RiscvO3LiveCheckpointEvent` projection for every
pending event referenced by the supported ROB suffix. It stores:

- fetch tick, partition, route, endpoint, request identity, PC, access size,
  event kind, and exact fetched bytes;
- execution PC, next PC, instruction byte count, integer writes, FP writes,
  and the optional supported FP-load memory access;
- data-access completion kind where applicable; and
- whether the event counts as a retired instruction.

Decode uses the existing RISC-V decoder on the raw fetched bytes and requires
the decoded instruction and width to match the execution projection. It then
reconstructs `RiscvExecutionRecord` and `RiscvCpuExecutionEvent` through their
typed constructors. Version 1 rejects traps, system events, branch predictor
updates, in-order cycle records, vector writes, split-fetch packets,
producer-forwarded fetch identities, and memory shapes other than the single
completed FP load.

Each live issue row stores only its ROB sequence and one consumed fetch request
identity. Restore recreates the live-staged identity from the decoded event and
binds the issue packet through the existing production binding path. Queue
materialization must succeed after every row is rebound. This leaves decoded
instruction ownership and dependency classification in their current
canonical modules.

### Runtime Projection

The live overlay stores a durable projection rather than cloning all private
runtime fields:

- the live rename map;
- ordered resident issue sequences;
- requested service tick, mutation generation, last-service generation, and
  live issue telemetry;
- replayable pending event projections and their executed/issued fetch sets;
- the completed pending fetch stream, its post-window fetch frontier, and the
  exact `CpuCore` next request sequence;
- the completed FP live data result when present;
- younger live-data sequence ownership and bounded memory-result
  authorizations required by that result window;
- the memory-result writeback reservation, including source and counted bit;
- live writeback counted/published membership needed for exactly-once stats;
- the complete finalized writeback-port ownership baseline, including partial
  tick maps and the closed-before boundary; and
- one scheduled O3 wake authority: scheduler instance, partition, tick, order,
  and serial or parallel event kind.

Applying the overlay replaces destination-timeline operational state rather
than appending to it. It trims `RiscvCoreState` execution events and
executed/issued fetch membership at or beyond the checkpoint's exact next
request sequence, replaces same-identity live rows, and installs the projected
memberships. Older retired rows may remain as history. Because normal PC restore
clears `CpuCore`'s pending fetch stream, overlay apply seeds only the ordered
completed `CpuFetchEvent` rows, sets the next fetch PC after the supported
window, and replaces `CpuCore.next_sequence` with the checkpoint value after
proving it is strictly beyond every restored identity. `CpuCore`'s separate
history log remains observational and is not checkpoint authority. This makes
the same payload valid both for an in-place host restore after source progress
and for focused restore into a fresh core.

`O3RT` continues to carry its version-23 aggregate statistics. When `O3LC` is
present, overlay validation reconstructs the live writeback schedule from the
calendar and counted set, combines it with the serialized
`O3FinalizedWritebackPortStats`, and requires all six resulting writeback-port
aggregates to equal `O3RT`. Overlay apply replaces the baseline seeded by
normal `O3RT` restore before installing the calendar and recomputing the same
aggregate. This prevents the live reservation from being counted once in
`O3RT` and again after restore.

Decision windows, active scheduler turns, rollback snapshots, trace history,
dirty trace indices, and finalized debug history are not serialized. They must
be inactive, externally accumulated, derivable, or empty at capture. Restore
rebuilds default transactional scratch state and restores the telemetry
baseline so later deltas are not double-counted.

Cross-validation requires:

- every issue sequence names exactly one live-staged ROB row;
- every live rename entry references a physical register owned by the ROB or
  stable checkpoint state;
- consumed request identities are ordered, unique where required, and backed
  by a replayable event;
- the supported fetch PCs form the exact non-control frontier encoded by the
  replayable events, and the restored fetch stream has no outstanding owner;
- the exact next fetch request sequence is beyond every projected request;
- the completed FP result names one matching LSQ span, ROB destination,
  decoded instruction, completion target, and writeback reservation;
- reservation ticks and wake ticks are not before the captured cycle;
- writeback slots fit the restored width and do not collide;
- no sequence, request, architectural destination, or physical destination is
  duplicated; and
- materializing the restored queue yields the same ordered sequences before
  the overlay is accepted.

The overlay accepts exactly one scheduled wake whose desired tick matches its
event tick. Detached wakes, multiple wakes, a mismatched scheduler instance or
partition, and a competing restored event at the same partition and tick are
rejected. The no-competing-event constraint makes a fresh scheduler order safe
without adding a checkpoint-only callback ordering API; the original order is
retained for diagnostics and corruption checks.

### Scheduler Wake Ownership

Add every pending O3 writeback wake to the host's scheduler-owned event list
as `discard_on_restore`. At capture, the scheduler checkpoint excludes the
source wake because the CPU overlay owns the deadline. At restore, the same
claim list is computed before CPU mutation so current-timeline O3 wakes are
discarded with the scheduler projection.

Restore order is:

1. Decode and cross-validate all CPU and scheduler chunks.
2. Validate that every live overlay has an attached or borrowed scheduler
   snapshot chunk, rather than discard-only scheduler access, and that its
   deadline can be scheduled in the restored partition/tick domain.
3. Restore `O3RT`, architectural state, and `O3LC` for every core.
4. Restore the scheduler projection while discarding current owned wakes.
5. Recompute the desired O3 wake from restored runtime authority, require it
   to equal the payload deadline, and schedule one new callback with the
   payload's serial or parallel event kind.
6. Record its exact pending event identity in the core wake tracker.

Extract the existing normal-run O3 wake scheduling body into one canonical
helper used by both turn scheduling and restore rebinding. No separate
checkpoint-only callback behavior is permitted.

A high-level host restore of `O3LC` without an actual scheduler snapshot is
rejected during preflight. Borrowed `DiscardOnly` mode is insufficient because
it cannot restore the checkpoint tick domain. Low-level CPU payload decode
remains independently testable, but the real system path never resumes a live
queue without its wake owner.

### Atomicity And Failure

All structural, version, range, cross-reference, scheduler-availability, and
deadline checks run before any live component mutates. Decode produces a
`PreparedRiscvCoreRestore` containing the fully materialized stable state,
live runtime, replacement fetch stream, and scheduler rebind request. Multi-core
banks prepare every record before restoring the first core. Installation of a
prepared CPU image is infallible; scheduler rebind inputs and order isolation
are validated before the scheduler projection is applied, leaving only the
existing canonical schedule operation in the commit phase.

Malformed or unsupported live state must not partially restore architectural
registers, O3 runtime, scheduler time, memory, or checkpoint metadata. Failed
capture must not create an output artifact or modify an existing checkpoint
label.

After restore, each queued row can issue once, each memory completion can
publish once, and each row can retire once. Retry, trap, redirect, mode disable,
or validation mismatch after restore uses existing sequence-owned suffix
cleanup and removes the restored queue, reservation, and wake authority.

## Compatibility

The format matrix becomes:

- `O3RT` remains version 23;
- `O3PS` remains version 2;
- `O3DH` remains version 7 and non-restorable; and
- optional `O3LC` starts at version 1.

Checkpoints without `O3LC` decode and restore exactly as before. `O3LC`
requires `O3RT`; a live chunk without its stable base is invalid. A handoff
chunk and a live checkpoint chunk cannot coexist. Unknown versions, trailing
bytes, truncated counts, invalid booleans/tags, duplicate chunks, and
cross-chunk mismatches fail closed.

## Representative Matrix

### CPU And System RED/GREEN Tests

Start with failing tests for:

- a compute queue payload preserving live rename and ordered resident rows;
- replayable event encode/decode using exact raw instruction bytes;
- queue packet reconstruction through the existing binder;
- a completed FLW and FLD result preserving exact response and typed
  writeback data;
- exact writeback reservation and telemetry restoration;
- source and destination O3 scheduler wakes being excluded, discarded, and
  rebound exactly once;
- projected issue decisions being normalized once into O3RT statistics;
- finalized plus live writeback ownership recomposing the O3RT aggregate;
- multi-core and multi-bank decode failure causing no partial restore; and
- every named unsupported live authority retaining non-quiescent rejection.

RED must fail because live capture is rejected or the new chunk is absent,
not because a fixture helper or module is missing.

### Real CLI Rows

Use real ELF fixtures through `env!("CARGO_BIN_EXE_rem6")` and
`rem6 run --execute`.

| Profile | Route | Shape | Required evidence |
| --- | --- | --- | --- |
| compute | direct | ready integer row plus dependency-blocked younger row | exact captured queue order, restored wake, select/writeback/commit ticks, final registers, exactly-once stats |
| FP result | direct | `FLW -> FMUL.S -> FADD.S` | response precedes checkpoint, no second data request, restored load publication and dependent wake, exact `00002041` bytes |
| FP result | direct | `FLD -> FMUL.D -> FADD.D` | exact 64-bit value, typed FP destination, writeback slot and ordered commits |
| FP result | cache/fabric/DRAM | table-driven FLW and FLD width-four rows | response already at CPU, hierarchy activity retained from source execution, no hierarchy request after restore, exact final bytes |
| control | timing mode | same binaries and host schedule | same architecture, no `O3LC`, no O3 IQ/writeback/debug evidence |

For each detailed row, discover the checkpoint boundary from a baseline trace,
then restore after enough source-timeline progress that a missing restore would
produce a different result or event sequence. Compare final architectural
state and exact relevant ticks against a replay baseline anchored at the
checkpoint, not merely against a successful exit code.

### Required Negative Rows

- queued-before-response direct and hierarchy checkpoints remain rejected;
- split-fetch and producer-forwarded live queue rows remain rejected;
- detached, multiple, wrong-kind, wrong-partition, and same-tick competing O3
  wake authority remains rejected;
- outstanding instruction fetch, resident data transport, MMIO, pending
  translation, pending address, store,
  atomic, vector, retry, failure, forwarding overlay, and unsupported issue
  packet profiles remain rejected;
- unknown `O3LC` version, bad magic, truncation, trailing bytes, invalid tag or
  bool, excessive count, and address/tick overflow are rejected;
- duplicate sequence/request IDs, missing ROB/LSQ/rename owner, wrong FP
  target, wrong width, missing bytes, and colliding writeback slots are
  rejected without partial restore;
- a live checkpoint restored without scheduler authority is rejected; and
- detailed-to-timing handoff remains rejected and emits no transfer artifact.

## Telemetry

Reuse existing IQ, issue, writeback-port, ROB/LSQ, memory-result, FP class,
host checkpoint, cache, fabric, and DRAM telemetry. Add checkpoint-specific
summary fields only for durable state that cannot be inferred from existing
surfaces:

- live overlay version and payload bytes;
- live profile;
- replayable event, resident issue row, and writeback reservation counts; and
- rebound O3 wake count and tick.

The restored runtime stats synchronizer must consume restored live issue
telemetry rather than substituting `O3LiveIssueTelemetry::default()`. Text,
JSON, debug traces, and `m5_dump_stats` remain reconciled where the fixtures
already emit them.

## Source Policy And File Boundaries

Keep the live payload and runtime projection in focused `rem6-cpu` modules.
Keep host ordering, scheduler claims, and component-chunk selection in
`rem6-system`. Keep CLI fixtures and assertions in focused `rem6` test
children. Do not place codec logic in `lib.rs`, the host action root, or the
existing large FP forwarding fixture.

Add line caps and one-owner assertions for:

- the `O3LC` codec and event projection;
- live runtime capture/restore and cross-validation;
- scheduler wake rebinding;
- CPU/system corruption tests; and
- real CLI live restore fixtures and matrix assertions.

Source policy must lock the four format versions, require pre-response
transport rejection, prevent `O3DH` reuse, and require the real CLI path. The
migration ledger remains exactly 1200 lines and names both the completed
CPU-owned profiles and the remaining pre-response transport/general-O3 gap.

## Delivery Sequence

1. Add the optional chunk contract, replayable event codec, runtime projection,
   validation, and compatibility tests.
2. Add scheduler-owned O3 wake claims/rebinding and compute-only live restore.
3. Add response-admitted FLW/FLD restore with direct and hierarchy CLI rows.
4. Add corruption and retained-rejection matrices, source policy, telemetry,
   and bounded ledger wording.

Each implementation commit stays between roughly 500 and 2000 changed lines,
runs focused RED/GREEN tests, formats with repository-local `TMPDIR`, passes
`git diff --check`, and is pushed before the next phase.
