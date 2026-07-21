# RISC-V O3 Two Pending Result-Addresses Design

## Goal

Extend the detailed RISC-V O3 memory-result window from one unresolved scalar
load address to a bounded set of two. Both younger loads must allocate ROB,
rename, and addressless LSQ state before their address producers complete,
remain transport-invisible until their own dependency is admitted at
writeback, and enter the existing data path without duplicate O3 allocation.

The increment must prove two distinct dependency topologies:

- sibling loads whose addresses both depend on the head memory result; and
- a chained load whose address depends on the first younger load result.

The window continues to execute through `rem6 run --execute`, uses the scoped
issue scheduler for all wakeup and selection, preserves ordered retirement,
and fails closed on capacity, replay, route, fault, and lifecycle boundaries.

This closes one exact bounded form of multiple unresolved addresses and deeper
mixed-data result depth. It does not claim a general issue queue, arbitrary
address-generation queue, or general O3 engine. The CPU checklist remains 8
of 10, 80% raw, capped at 74% representative.

## Current Boundary

The current dependent-result-address lane owns exactly one
`O3PendingDataAddress` in `Option<O3PendingDataAddress>`. Fetch authorization
admits only the first scalar `LD` that consumes the memory-result head. Runtime
helpers, scheduler adapters, wake seeding, unissued-data selection, binding,
replay, and source-policy tests all encode the same singleton assumption.

That lane already supplies the mechanisms needed for a bounded extension:

- live-staged ROB and rename allocation;
- LSQ rows whose address is `None`;
- sequence-keyed typed data dependencies;
- admitted memory-result writeback values;
- memory-class issue candidates and resource arbitration;
- canonical execution materialization from forwarded register values;
- normal-path PMP, PMA, line, route, forwarding, request, and response
  ownership; and
- sequence-boundary cleanup for younger live-staged rows.

The missing authority is an ordered, capacity-two pending-address owner. A
second ad hoc optional field would duplicate all singleton branches and would
not establish the collection semantics needed by later IQ work.

## Considered Approaches

### 1. Ordered capacity-two pending-address owner

Replace the singleton runtime field with one focused collection that stores at
most two pending rows in increasing O3 sequence order. All lookup, iteration,
wake, replay, and removal operations are identity-based. Existing per-row
validation and normal data-path binding remain authoritative.

This is the chosen approach. It exercises real multi-entry scheduling while
keeping capacity, accepted instruction shapes, and replay boundaries exact.

### 2. General address-generation queue and IQ

A general queue could admit arbitrary loads, stores, atomics, translation, and
MMIO. That would combine several still-open migration boundaries and exceed a
self-contained increment. The two-entry owner should instead produce concrete
requirements and executable evidence for that later generalization.

### 3. Add a second optional pending-address field

`pending_data_address_2` would minimize the initial diff, but every caller
would need first/second branching, wake ordering would be implicit, and replay
would be prone to clearing the wrong row. Source policy must reject this
parallel singleton representation.

## Chosen Windows

The memory-result window remains capped at four ROB rows even when the wider
untranslated scalar live-window setting is eight.

The sibling topology is:

1. a scalar `LD` or unordered atomic head that produces pointer `p`;
2. scalar `LD a, offset0(p)` with an unresolved address;
3. scalar `LD b, offset1(p)` with an unresolved address; and
4. one scalar integer row that consumes the head, `a`, `b`, or a supported
   combination of those results.

The chained topology is:

1. a scalar `LD` or unordered atomic head that produces pointer `p`;
2. scalar `LD q, offset0(p)` with an unresolved address;
3. scalar `LD value, offset1(q)` with an unresolved address; and
4. one scalar integer row that consumes the head or either younger result.

Both pending instructions are untranslated four-byte scalar doubleword loads
with nonzero destinations. The head destination and both younger destinations
must be distinct. Each pending source must name either the head destination or
the immediately older pending destination. The exact accepted producer graph
is therefore sibling or one-deep chain; cross-links, cycles, self-overwriting
loads, unrelated sources, and a third unresolved address are rejected.

The single-pending window remains valid. A second pending load is optional and
must appear before any scalar suffix starts. Once a scalar row is admitted, no
later memory-result row may join the window. Depths below four retain existing
truncation behavior and cannot admit the full two-pending matrix.

Dependent stores, atomics, LR/SC, floating-point loads, vector loads,
translation, and MMIO remain outside this lane.

## Fetch Authorization

Keep `YoungerDependentRead` and its typed `DependentSource` address authority.
The authority continues to record the exact decoded load, destination, source
register, width, immediate, fetch identity, and memory-result lineage without
claiming a physical range.

Generalize dependent authorization so the producer can be either:

- the exact integer-result head; or
- the immediately older authorized dependent load.

The data-result candidate tracks up to three result destinations: the head and
two dependent loads. Before scalar execution begins, each next candidate is
tested against the currently authorized producer graph. The second dependent
authorization is accepted only when:

- the first dependent authorization was consumed immediately before it;
- the configured row limit is at least four;
- its source equals the head destination for a sibling or the first pending
  destination for a chain;
- its nonzero destination is distinct from all older result destinations;
- it is a four-byte doubleword scalar `LD`; and
- the candidate still has one row for the selected scalar suffix.

The focused authorization helper owns this bounded progression so the current
`data_access_result.rs` root, already at its source-policy cap, does not grow.

Fetch authorization performs no range, PMP, PMA, line, route, or overlap
claim for either pending row. Existing reset, redirect, abort, restart,
restore, and detailed-mode cleanup clears every unconsumed authorization.

## Pending-Address Collection

Add one `O3PendingDataAddresses` owner to `O3RuntimeState`. It contains a
sequence-ordered `Vec<O3PendingDataAddress>` with an invariant capacity of two.
The collection is the sole authority for:

- capacity checks and ordered insertion;
- lookup by O3 sequence, primary fetch request, or consumed fetch request;
- oldest materialized execution selection for normal data issue;
- minimum requested wake tick;
- replay and discard from a sequence boundary;
- exact-row removal after normal-path binding; and
- consistency checks across all resident rows.

A vector is used only as a fixed-capacity ordered owner. No map, second option,
spill collection, or unbounded insertion API is added. Production source must
contain one collection field and one capacity constant.

Each `O3PendingDataAddress` retains its current per-row metadata and adds the
fetch-lineage predecessor request needed to reconstruct the remaining younger
window after an older pending row binds. For every pending row this is the
previous instruction's last consumed fetch request, including the head's last
consumed request for the first pending row. Producer identity remains a
register plus O3 sequence; fetch lineage is never used as a substitute for
dependency identity.

Immediate-producer metadata and root-head metadata are separate. Every row
records the register and sequence that resolve its address. It also records
the original memory-result head sequence, range, and atomic kind. A chained
second row therefore waits on the first pending load while still validating
its materialized range against the original atomic head.

## Staging And Allocation

Staging accepts one or two authorized pending requests in program order. It
resolves each request's producer against already resident O3 ownership:

- the head sequence for the first row;
- the same head sequence for a sibling second row; or
- the first pending sequence for a chained second row.

Each pending row allocates exactly once:

- one live-staged ROB entry;
- one physical integer register and rename overlay;
- one LSQ load row with `address=None` and eight bytes;
- one live fetch identity; and
- one typed data dependency on its producer sequence.

After both pending rows are staged, the scalar suffix policy is initialized
with all three unresolved result destinations and three occupied rows. At most
one scalar suffix row is admitted in the full two-pending window.

Staging is transactional. Any invalid producer, duplicate identity, capacity
failure, rename failure, LSQ failure, suffix mismatch, or scheduling failure
discards from the first newly allocated sequence and restores the previous
rename mapping. No partially staged second row may survive.

On successful staging, every consumed dependent authorization is removed from
`memory_result_window_authorizations` only after scheduling succeeds. On
failure, the caller discards all newly staged rows and the existing candidate
cleanup owns the complete authorization set. No path may remove only the first
authorization or leave the second stale after success.

Neither pending row allocates a `MemoryRequestId`, enters `outstanding_data`,
creates an `O3LiveDataAccess`, emits a Data/Memory request, or touches a target.

## Scoped Issue And Wakeup

Every pending row becomes its own memory-class
`O3LiveIssueSchedulingCandidate`, keyed by its sequence. Its `waits_on` scope
names its exact producer sequence and its produced data scope names its own
sequence. Existing scalar suffix candidates consume the same sequence scopes.

For siblings, admitted head writeback resolves both pending candidates:

- issue width one selects the older pending load first and records the younger
  as resource-blocked for a later wake; and
- issue width two still selects only one pending memory candidate because the
  modeled memory/AGU class has one slot, but may co-issue a ready scalar row in
  another operation class.

This increment preserves the existing single memory-class capacity. It does
not widen the modeled hardware merely to make both sibling addresses issue in
one tick. The second sibling receives the next scheduler-owned wake and issues
in a later tick.

For a chain, admitted head writeback resolves only the first pending load. The
second remains dependency-blocked through address generation, request,
response, and writeback of the first. It becomes selectable only when the
first younger load result reaches admitted writeback.

Scheduler preparation sorts selected rows by `(pending first, sequence)`.
Materializing one row forwards only values whose producer scopes are resolved
at the selected tick. Siblings may both be ready, but the single memory slot
materializes them in sequence order across ticks. A chain can never
materialize both rows from one producer wake because the first load's data
result is not yet available.

Collection scheduler adapters must be identity-based:

- candidate metadata is found by sequence and exact fetch identity;
- producer ready ticks are queried for all rows that wait on that sequence;
- resource-blocked wake ticks update only the blocked row;
- the global pending wake is the minimum row wake tick; and
- a failed candidate replays from that candidate's sequence, not from the
  collection head unconditionally.

Wake seeding starts from the oldest remaining unresolved pending row. A typed
`O3PendingDataAddressWakeSeed` keeps three authorities separate:

- `fetch_predecessor_request` reconstructs that row and later fetch identities;
- `head_reservation` carries the immediate producer sequence and memory-class
  reservation; and
- `younger_pcs` names the exact remaining live-staged window.

This seed remains valid when an older pending row has already bound and left
the collection. No single `MemoryRequestId` is reused for both fetch walking
and producer lookup.

## Materialization And Normal Data-Path Binding

Per-row materialization keeps the existing canonical checks. It must:

1. match the selected sequence, fetch identity, decoded instruction, rename
   destination, and addressless LSQ row;
2. resolve exactly one expected producer sequence and register;
3. clone architectural hart state and apply the admitted forwarded value;
4. execute the exact scalar load at the staged PC;
5. require sequential, trap-free execution with no unrelated side effect; and
6. record the scheduler-selected address-generation issue tick on that row.

The unissued-data selector returns the oldest materialized pending execution.
If an older materialized row remains while a younger sibling is selected on a
later wake, normal data issue still submits them in sequence order. Each row
retains its own scheduler-selected issue tick, and the younger remains
available after the older binds.

Prepared-issue validation looks up the exact row by fetch identity. The normal
path remains the sole owner of effective address, PMP, PMA, cacheability, line
shape, route, forwarding, request allocation, and transport submission.

Successful binding removes only the matched collection row and reuses its
existing sequence, ROB entry, LSQ entry, and rename destination to create one
`O3LiveDataAccess`. The LSQ address changes from `None` to the canonical
physical address. No second O3 allocation occurs. Other pending rows and their
wake state remain intact.

The O3 event retains the selected address-generation tick. Each Data/Memory
request records its actual submission tick, which must be equal or later.

## Replay And Failure Semantics

Replay is sequence-precise:

- head retry or failure discards both pending rows and the scalar suffix;
- first pending validation, materialization, preparation, or submission
  failure discards the first pending row, the second pending row, and suffix;
- second pending failure discards only the second row and suffix, preserving
  any older bound or completed first load; and
- suffix failure follows its existing sequence-boundary behavior.

After replay, the normal architectural path refetches the discarded rows and
owns any eventual trap, MMIO serialization, or unsupported access. O3 does not
add a second trap authority.

If either materialized address becomes MMIO, PMA-uncacheable, unsupported
cross-line, or otherwise invalid for the lane, no request is submitted for
that row. For an atomic head, every materialized younger range must be
disjoint from the atomic range. The sibling rows are not required to be
disjoint from each other because both are reads; existing cache and LSQ paths
own their normal alias behavior.

The atomic overlap check always uses root-head metadata, never the chained
row's immediate producer metadata. Negative evidence includes a chained second
row whose final range overlaps the original atomic head while the first row is
disjoint.

Redirect, interrupt, reset, restart, restore, failed submission, and detailed-
mode disable remove every affected pending row, scalar descendant, issue
selection, and future wake. Cleanup leaves no fetch identity, rename, LSQ,
request, outstanding-data, or writeback residue.

Any nonempty pending-address collection remains transient and non-restorable.
Live checkpoint and detailed-to-timing handoff continue to reject while one or
two rows are present. Drained checkpoint compatibility is unchanged.

Lifecycle accounting that previously added a boolean pending count must use
the collection's exact row count. Public and test-visible retirement counts
must therefore distinguish zero, one, and two resident pending rows.

## Representative Matrix

Top-level positive evidence uses both topologies across the existing result
producers, routes, and issue widths:

| Topology | Head result | Route | Issue width | Required scheduling witness |
| --- | --- | --- | --- | --- |
| sibling | scalar `LD` pointer | direct | 1 | older pending issues first |
| sibling | scalar `LD` pointer | cache/fabric/DRAM | 2 | one pending co-issues with a ready scalar |
| chain | scalar `LD` pointer | direct | 1 | second waits for first result |
| chain | scalar `LD` pointer | cache/fabric/DRAM | 2 | width cannot bypass dependency |
| sibling | unordered `AMOSWAP.D` old pointer | direct | 1 | both ranges disjoint from head |
| chain | unordered `AMOSWAP.D` old pointer | cache/fabric/DRAM | 2 | two admitted writeback wakes |

Before the head response, every full-window row must prove:

- exact four-row ROB residency;
- two younger LSQ entries with JSON `address: null`;
- three distinct live integer rename destinations;
- unchanged architectural destination registers;
- focused CPU evidence of two sequence-ordered collection rows;
- CLI inference of both pending rows from exact ROB, rename, and addressless
  LSQ residency; and
- only the head request visible in Data/Memory transport evidence.

After admitted writeback, sibling rows must prove oldest-first memory issue at
both widths. Width two may co-issue the older pending load with a head-ready
scalar row, but the second memory row remains resource-blocked until a later
tick. Chain rows must prove that the second address issue occurs no earlier
than the first younger load's admitted writeback.

Every row must also prove:

- exact LSQ address resolution for both pending rows;
- exactly two younger requests reaching the intended route;
- scalar suffix wakeup from the correct producer scopes;
- sequence-ordered commit despite different readiness;
- exact final registers and dumped memory;
- direct-route transport-only activity or complete cache/fabric/DRAM activity;
  and
- dependency- and resource-blocked issue counters matching the topology.

## Negative And Lifecycle Matrix

Focused CPU and CLI evidence must keep these cases outside the new lane:

- a third unresolved address;
- a second pending load after scalar suffix execution starts;
- unrelated, cyclic, self-overwriting, or duplicate-destination producer
  graphs;
- dependent stores, atomics, LR/SC, floating-point loads, and vector loads;
- zero-destination or non-doubleword pending loads;
- acquire, release, or acquire-release atomic heads;
- translated heads or younger accesses;
- MMIO heads or materialized MMIO routes;
- stale fetch identities or changed producer sequences for either row;
- PMA-uncacheable and unsupported cross-line addresses for either row;
- atomic-head overlap by either materialized range;
- live checkpoint and detailed-to-timing handoff with one and two rows; and
- timing mode.

Capacity pressure must backpressure or truncate at the fetch-window boundary;
it must not create an untracked third row or silently execute it against stale
architectural register state.

Lifecycle tests must cover first-row replay removing the complete younger
chain, second-row replay preserving older ownership, width-two sequential
sibling binding, resource-wake replacement, interrupt cleanup, reset, restart,
and failed submission. Every case asserts exact collection row count and empty
stale wake state.

## Focused Ownership

The singleton implementation is already at several source-policy caps. The
increment must extract focused owners instead of widening roots:

- `o3_runtime_pending_address.rs` keeps the row type and per-row exact
  validation and materialization primitives;
- `o3_runtime_pending_address_set.rs` owns capacity, ordering, lookup,
  iteration, wake aggregation, exact removal, row counts, binding extraction,
  and replay boundaries;
- `o3_runtime_pending_address_staging.rs` owns transactional allocation of one
  or two rows and scalar suffix setup;
- `o3_runtime_issue/pending_address.rs` owns collection-facing scheduler and
  wake adapters;
- `riscv_fetch_ahead/detailed_o3/dependent_result_address.rs` owns static
  producer-graph authorization;
- `riscv_live_retire_window/dependent_result_address.rs` owns fetch-lineage
  collection and staging calls; and
- `riscv_data_issue/dependent_result_address.rs` owns exact-row normal-path
  validation and bind handoff.

The production caps are:

- `o3_runtime_pending_address.rs` at 650 lines, with collection logic removed;
- `o3_runtime_pending_address_set.rs` at 350 lines;
- `o3_runtime_pending_address_staging.rs` at 350 lines;
- `o3_runtime_issue/pending_address.rs` at 300 lines;
- fetch authorization at 200 lines;
- live-retire staging at 250 lines; and
- dependent data-issue binding at 350 lines.

`o3_runtime.rs` remains below 1,200 lines and
`detailed_o3/data_access_result.rs` remains at or below its existing 450-line
cap. The focused authorization helper absorbs the bounded progression needed
to keep that root from growing.

The current Task 3, Task 5, and Task 8 source-policy assertions are
deliberately migrated:

- replace the exact `Option<O3PendingDataAddress>` field assertion with one
  exact `pending_data_addresses: O3PendingDataAddresses` field;
- move `stage_pending_data_address_window` to the staging owner;
- move `has`, fetch lookup, ordered execution selection, wake aggregation,
  discard, replay-from-sequence, count, and exact-row removal helpers to the
  set owner;
- move `bind_pending_data_address_issue` to the set owner and keep the
  dependent data-issue child as its only normal-path caller;
- keep pure row validation and materialization in the row owner;
- require lifecycle retirement accounting to use collection length rather
  than `usize::from(has_pending_data_address())`; and
- replace singleton helper-count tests with collection and capacity
  invariants while retaining all existing one-row behavior tests.

`o3_runtime.rs` attaches `o3_runtime_pending_address_set.rs` and
`o3_runtime_pending_address_staging.rs` exactly once alongside the row owner.
`o3_runtime_issue.rs` attaches `o3_runtime_issue/pending_address.rs` exactly
once. Task 5 binding assertions and Task 8 final production-owner scans must
point to these focused files and reject stale definitions in the old row,
runtime, issue-root, and data-issue-root files.

The CPU pending-address test family adds
`o3_runtime_pending_address_tests/multiple.rs`, capped at 550 lines. Existing
test files retain their current caps and the complete family remains below
2,100 lines.

Source policy must require each module and compiled test anchor, cap each new
owner, and reject:

- multiple collection fields or collection type definitions;
- `pending_data_address_2` and other parallel option fields;
- pending-address `HashMap` or `BTreeMap` owners;
- insertion without the capacity-two guard;
- collection iteration or removal logic in large roots and facades; and
- stale singleton assumptions such as exact `Option<O3PendingDataAddress>`
  ownership counts.

CLI evidence uses new focused children instead of consuming the remaining 113
lines in the existing family:

- `dependent_result_address/two_pending.rs`, capped at 700 lines, owns the
  parameterized positive sibling/chain matrix;
- `dependent_result_address/two_pending/boundaries.rs`, capped at 500 lines,
  owns negative and lifecycle cases; and
- the new child family remains below 1,050 aggregate lines.

The parent dependent-result-address module only declares the new child and
shares existing helpers. `writeback_ownership.rs` must require both module
paths and caps without weakening the current parent/boundary aggregate limit.
It also owns ordered positive and boundary anchor arrays and verifies every
anchor is defined exactly once in its assigned child. `core_test_anchors.txt`
must list the same anchors exactly once.

The positive anchors are:

- `rem6_run_o3_two_pending_result_address_sibling_width_one_direct`;
- `rem6_run_o3_two_pending_result_address_sibling_width_two_hierarchy`;
- `rem6_run_o3_two_pending_result_address_chain_width_one_direct`;
- `rem6_run_o3_two_pending_result_address_chain_width_two_hierarchy`;
- `rem6_run_o3_two_pending_result_address_atomic_sibling_direct`; and
- `rem6_run_o3_two_pending_result_address_atomic_chain_hierarchy`.

The boundary anchors are:

- `rem6_run_o3_two_pending_result_address_rejects_third_unresolved`;
- `rem6_run_o3_two_pending_result_address_replays_first_failure`;
- `rem6_run_o3_two_pending_result_address_replays_second_failure`;
- `rem6_run_o3_two_pending_result_address_rejects_atomic_chain_overlap`;
- `rem6_run_o3_two_pending_result_address_rejects_live_checkpoint_and_handoff`;
  and
- `rem6_run_o3_two_pending_result_address_timing_mode_suppresses_o3_evidence`.

These exact names make the focused `two_pending_result_address` test filter
non-vacuous.

No compatibility alias or deprecated parallel authority is added.

## Executable Evidence

TDD starts with focused failures proving that the current lane rejects a second
dependent authorization and cannot retain, schedule, materialize, or bind two
pending rows.

Focused CPU tests then prove:

- exact sibling and chain authorization plus all graph rejections;
- ordered capacity-two insertion and third-row refusal;
- two ROB, rename, and addressless LSQ allocations without duplication;
- sibling oldest-first memory scheduling at widths one and two, including
  width-two co-issue with a ready scalar rather than a second memory row;
- chain wakeup across two admitted memory-result writebacks;
- typed wake seeds after the older pending row binds;
- oldest materialized data selection and exact-row binding;
- sequence-boundary replay and lifecycle cleanup;
- exact lifecycle counts plus checkpoint and mode-transfer rejection for one
  and two rows; and
- source-policy ownership and line caps.

Top-level CLI tests in the new two-pending child family invoke
`env!("CARGO_BIN_EXE_rem6")`. They prove the representative matrix,
pre-response two-row residency inferred from exact ROB/rename/addressless-LSQ
state, sibling and chain timing, exact transport counts, exact architectural
and memory witnesses, negative suppression, live-action rejection, and
timing-mode omission of O3 evidence.

Expected verification includes:

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --test cli_run two_pending_result_address
cargo test -p rem6 --test source_policy
cargo test --workspace
```

Commands use the repository-local `target/tmp` as `TMPDIR` when the host
temporary filesystem lacks space.

## Documentation Boundary

After executable evidence passes, update the CPU Migrated, Not migrated, Next
evidence, and test-ledger text to record one exact capacity-two untranslated
integer-result address-generation window with sibling and chained dependency
graphs, two addressless LSQ rows, scheduler-owned wakeup, normal-path binding,
direct and hierarchy routes, and sequence-precise replay.

Keep arbitrary broader mixed windows, translated and MMIO result pairs,
dependent stores and atomics, FP/vector dependent addresses, more than two
unresolved addresses, general IQ/wakeup/select, restorable transport ownership,
and a general O3 engine open. Preserve the CPU score at 74% representative and
the migration ledger at exactly 1,200 lines.
