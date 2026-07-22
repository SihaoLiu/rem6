# RISC-V O3 Three Pending Addresses And AGU Width Design

## Goal

Extend the detailed RISC-V O3 memory-result window from two unresolved scalar
load addresses to three and make memory/address-generation issue capacity an
explicit runtime setting. The resulting lane must demonstrate that the live
issue queue can arbitrate several simultaneously ready memory candidates,
preserve exact dependency blocking for a chain, and keep all rows on the real
`rem6 run --execute` data path.

The representative evidence matrix covers:

- three sibling loads whose addresses all depend on the head result;
- a three-deep chain in which every younger address depends on the immediately
  older pending load;
- a mixed fanout in which two siblings become ready together while the third
  remains dependent on the second sibling;
- total issue widths 1, 2, and 4 with memory/AGU widths 1, 2, and 4;
- direct and cache/fabric/DRAM routes;
- exact ROB, rename, addressless-LSQ, issue, writeback, transport, and ordered
  retirement evidence; and
- capacity, graph, fault/replay, checkpoint, mode-transfer, and timing-mode
  boundaries.

This increment closes one bounded but representative form of the migration
ledger's open "more than two unresolved addresses" and "wider AGU and memory
concurrency" rows. It does not claim arbitrary address graphs, translated or
MMIO pending-address execution, dependent stores or atomics, restorable
in-flight transport ownership, or a general O3 engine. The CPU checklist and
74% representative bucket cap remain unchanged.

## Current Boundary

`O3PendingDataAddresses` currently owns at most two sequence-ordered pending
rows. Each row is a four-byte decoded scalar `LD` with a nonzero integer
destination, one address source, one live ROB entry, one integer rename
destination, and one addressless LSQ entry. The source can name either the
memory-result head or the immediately older pending row.

The derived live issue queue already reconstructs all unissued rows from
canonical ROB identities and exact decoded packets. Its dependency table
tracks sequence-keyed producer scopes, while its calendar owns issue-width and
operation-class reservations. The calendar currently hard-codes one memory
slot per tick, so simultaneously ready sibling addresses are serialized even
when total issue width is larger.

The normal data path already supplies the mechanisms needed by this
increment:

- admitted memory-result writeback values for address materialization;
- exact sequence-keyed pending-row lookup and replay;
- transactional staging and suffix cleanup;
- normal PMP, PMA, route, request, response, and retirement ownership;
- direct and hierarchy-backed scalar memory execution;
- live checkpoint and execution-mode transfer boundaries; and
- structured O3 issue, ROB, LSQ, memory-resource, and debug evidence.

The missing authority is a capacity-three collection contract plus a
configurable memory-class issue capacity owned by the live issue calendar.

## Considered Approaches

### 1. Capacity three plus configurable AGU width

Raise the focused pending-address collection capacity to three and add a
separate memory/AGU issue-width setting. The queue remains derived, the
dependency table remains sequence-owned, and the calendar uses the configured
memory capacity after subtracting existing reservations.

This is the chosen approach. It distinguishes total issue bandwidth from
memory-class bandwidth and produces executable sibling, chain, and mixed
dependency evidence without introducing persistent queue state.

### 2. Capacity three with the existing single memory slot

This would prove a deeper unresolved-address collection but would still
serialize all ready addresses. It would not address the ledger's wider-AGU
gap and would mostly extend one capacity axis.

### 3. General address-generation queue with arbitrary dependencies

A general AGU queue would need arbitrary older-producer lookup, stores,
atomics, translation, MMIO, restorable request ownership, and broader
checkpoint state. Those concerns exceed one self-contained increment and
would obscure the exact ownership boundaries established by the derived live
issue queue.

## Configuration Contract

Add a RISC-V execution setting with these external forms:

- CLI: `--riscv-o3-memory-issue-width <1..4>`
- TOML: `riscv_o3_memory_issue_width = 1|2|3|4`

The default is one, preserving current behavior. The configured value must:

- require `--execute` and `--isa riscv`, matching the existing O3 issue-width
  validation surface;
- be between one and `MAX_RISCV_O3_ISSUE_WIDTH`;
- not exceed the configured total `riscv_o3_issue_width`; and
- reach every constructed RISC-V core through the canonical run
  configuration path.

The CPU runtime stores the validated value as `memory_issue_width`. No other
module may derive memory capacity from total issue width or retain a duplicate
configuration field. Timing and functional modes may accept the setting but
must not expose detailed O3 issue surfaces.

The final run artifact records the selected value beside the existing O3
issue configuration so CLI and TOML rows can prove that the requested capacity
reached the runtime. No new counter is required merely to restate the setting;
behavioral issue ticks and existing arbitration counters are the primary
evidence.

## Accepted Dependency Graphs

The full window contains four memory-result rows:

1. one resident scalar `LD` or supported unordered atomic head producing
   register `h`;
2. pending scalar `LD a, offset0(source0)`;
3. pending scalar `LD b, offset1(source1)`; and
4. pending scalar `LD c, offset2(source2)`.

All three pending loads are untranslated, four-byte instructions that perform
doubleword scalar loads into distinct nonzero integer destinations. The head
destination and all pending destinations are distinct.

For pending row `i`, the address source may name:

- the original head destination `h`; or
- the immediately older pending destination.

This rule admits the required representative shapes:

- siblings: `h -> a`, `h -> b`, `h -> c`;
- chain: `h -> a -> b -> c`; and
- mixed fanout: `h -> a`, `h -> b`, `b -> c`.

It intentionally rejects arbitrary non-adjacent pending producers such as
`a -> c` when `b` is the immediately older row, cycles, self-overwrites,
duplicate destinations, unrelated architectural sources, and a fourth
pending address. Those shapes remain future general-IQ work.

One- and two-pending windows remain valid. A full three-pending window consumes
the existing four-row scalar-memory capacity with its head and therefore has
no scalar suffix. Existing two-pending-plus-scalar evidence remains the
cross-class suffix boundary.

## Ownership And State

`O3PendingDataAddresses` remains the sole collection owner. Its capacity
constant changes from two to three, while its existing invariants remain:

- strictly increasing O3 sequence order;
- unique sequence and fetch identities;
- one common root-head identity;
- exact fetch-predecessor lineage;
- unique architectural destinations;
- immediate-producer identity equal to the head or immediately older row;
- exact lookup and removal by sequence or primary fetch; and
- discard-from-sequence rollback semantics.

The collection continues to use a bounded `Vec` behind focused APIs. No second
field, map, spill queue, persistent `O3LiveIssueQueue`, or caller-owned request
slice is introduced.

The live issue calendar remains the sole authority for per-tick reservations.
It adds memory capacity to its captured configuration and computes remaining
memory slots as:

```text
configured memory issue width - memory reservations at this tick
```

using saturating subtraction. Total selected rows remain bounded by the
existing total issue width. Existing issued pending rows and the live head
continue to reserve their exact historical ticks when a fresh derived calendar
is captured.

## Fetch Authorization And Staging

The focused dependent-result-address authorizer extends its current two-row
progression to three. It records the head destination, the immediately older
pending destination, all result destinations, configured row limit, and
dependent-row count.

Authorization is sequential and fail-closed. A candidate is accepted only
when:

- detailed mode, untranslated cacheable memory, and the existing supported
  head shape remain active;
- the instruction is an exact scalar doubleword `LD` with a nonzero new
  destination;
- its source is the head destination or immediately older pending
  destination;
- the configured scalar-memory depth has room for the head and all admitted
  pending rows; and
- no scalar suffix has already started.

Staging accepts one to three authorized requests in program order and remains
transactional. Each accepted row allocates exactly one ROB entry, one integer
physical destination, one rename overlay, one addressless eight-byte LSQ row,
one live fetch identity, and one exact issue packet. Any failure restores the
pre-staging runtime without leaking a partial third row.

No pending row allocates a data `MemoryRequestId`, enters outstanding data,
emits a transport request, or touches a target until its address execution is
selected and bound through the normal data-issue path.

## Scheduling And Data Flow

At each arbitration pass the runtime derives a fresh queue from live ROB rows,
builds the sequence dependency table, captures the calendar, and plans the
current tick.

For three siblings, admitted head writeback resolves all three memory
candidates:

- total width one selects one row per tick regardless of memory width;
- memory width one selects one memory row per tick regardless of total width;
- total width two with memory width two selects the two oldest siblings, then
  the third on the next tick; and
- total width four with memory width four selects all three siblings on the
  same tick.

For the full chain, only the first pending row becomes ready at head
writeback. Every later row remains dependency-blocked through the older row's
address generation, request, response, and admitted writeback. Increasing
either width cannot collapse those dependency edges.

For mixed fanout, the first two siblings become ready together and may
co-issue when both capacities permit. The third row remains blocked on the
second sibling and issues only after that result reaches admitted writeback.

Selected pending rows are prepared in sequence order. Each speculative Hart
clone receives only producer values whose dependency scopes are resolved at
the selected tick. Materialization records the exact selected issue tick and
decoded execution but still allocates no target request. The normal data path
then binds each materialized row by primary fetch identity, performs existing
PMP/PMA/route validation, allocates the real request, and preserves ordered
architectural retirement.

Multiple addresses generated on one tick may submit their real requests on
subsequent CPU turns. The design therefore claims wider address-generation
selection and multiple outstanding memory requests before the first response,
not an unmodeled multi-request transport port in one callback.

## Replay, Fault, And Lifecycle Boundaries

Every failure is sequence-precise:

- stale or mismatched decoded identity replays from the selected row;
- address materialization failure discards that row and every younger row;
- resource blocking records a wake only for the blocked row;
- a middle-row request retry/failure preserves completed older rows and
  cancels the younger pending suffix before any younger target request;
- a head failure cancels all three pending rows;
- redirect, trap, interrupt, reset, and HTM abort cleanup remove the correct
  sequence suffix and restore committed rename mappings; and
- a fourth unresolved address remains outside O3 and executes through the
  architectural fallback path without duplicate requests.

Live checkpoint capture remains rejected while any pending row, materialized
address, live data request, or speculative execution is resident. A drained
checkpoint restores with an empty pending-address collection and preserves
the configured widths through normal run configuration rather than serialized
queue state.

Detailed-to-timing transfer remains non-restorable but timing-preserving: it
must carry the existing live ROB/LSQ and issue/writeback/commit observations,
drain them once, and expose no later detailed O3 activity. Timing mode preserves
architectural results while suppressing the pending-address and O3 issue
surfaces.

Translated memory, readfile MMIO, dependent stores, atomics, LR/SC, floating
point loads, vector loads, cross-line accesses, and arbitrary non-adjacent
address graphs remain rejected by this lane.

## Executable Evidence Matrix

Focused `rem6-cpu` tests must cover:

- collection insertion, ordering, exact lookup, and capacity-three rejection;
- sibling, chain, and mixed authorization;
- total-width and memory-width interaction;
- reservation subtraction when the head or an older pending row already owns
  a tick;
- exact dependency-blocked versus resource-blocked classification;
- sequence-specific wake, replay, and discard behavior;
- transaction rollback when third-row staging fails; and
- cleanup across retry, redirect, reset, interrupt, and HTM boundaries.

Top-level `rem6` CLI tests launch `env!("CARGO_BIN_EXE_rem6")` and cover at
least these rows:

| Topology | Route | Total width | Memory width | Required evidence |
| --- | --- | ---: | ---: | --- |
| Sibling | direct | 1 | 1 | Three ordered address-issue ticks and exact final registers/memory |
| Sibling | direct | 2 | 2 | Two oldest siblings co-issue, third follows |
| Sibling | cache/fabric/DRAM | 4 | 4 | All three addresses issue together and all three younger requests become outstanding before their first response |
| Chain | direct | 4 | 4 | Three dependency-separated issue ticks despite available capacity |
| Chain | cache/fabric/DRAM | 2 | 2 | Exact producer/writeback/issue ordering plus hierarchy activity |
| Mixed fanout | cache/fabric/DRAM | 2 | 2 | Two siblings co-issue; dependent third remains blocked until producer writeback |

Additional boundary rows prove:

- CLI and TOML acceptance for widths 1 and 4;
- rejection of zero, values above four, missing execute/RISC-V prerequisites,
  and memory width greater than total issue width;
- fourth unresolved-address fallback without duplicate data requests;
- invalid graph suppression;
- middle-row replay/failure cancellation;
- live checkpoint rejection and drained restore;
- detailed-to-timing issue/writeback/commit continuity; and
- timing-mode architectural equivalence with no detailed O3 surfaces.

Assertions must include exact register values, memory bytes, Data/Memory
request identities and ordering, `/cores/0/o3_runtime/issue` counters, ROB/LSQ
occupancy, pending address issue ticks, dependency/resource blocked counts,
direct transport activity, hierarchy cache/fabric/DRAM activity, and final
zero live ownership after drain. A committed-instruction count alone is not
sufficient.

## Source Policy And Documentation

Source policy must preserve focused ownership and line caps:

- collection capacity and invariants stay in
  `o3_runtime_pending_address_set.rs`;
- authorization stays in the focused dependent-result-address module;
- transactional allocation stays in
  `o3_runtime_pending_address_staging.rs`;
- dependency discovery stays in `o3_runtime_issue/dependency.rs`;
- reservations and memory capacity stay in
  `o3_runtime_issue/calendar.rs`;
- the issue root remains an orchestration facade; and
- CLI/config parsing follows the existing RISC-V timing configuration owner.

Policy rejects duplicate memory-width fields, memory-capacity computation
outside the calendar, a second pending-address collection, persistent derived
queues/calendars, and caller request-index selection.

After executable evidence passes, update
`docs/architecture/gem5-to-rem6-migration.md` at exactly 1,200 lines. Keep CPU
Execution Models at 8 of 10, 80% raw, capped at 74% representative. Record the
three-pending sibling/chain/mixed matrix and configurable AGU width in
Migrated/Evidence, remove only the exact "more than two unresolved addresses"
and single-AGU wording that the tests close, and retain arbitrary graphs,
translated/MMIO rows, dependent stores/atomics, restorable transport, and the
general O3 engine under Not migrated/Next evidence.

## Verification

The implementation plan must use TDD and organize commits by behavior:

1. configuration and calendar capacity;
2. capacity-three authorization and collection ownership;
3. runtime scheduling, wake, replay, and cleanup;
4. top-level representative CLI matrix and boundaries;
5. source-policy ratchets; and
6. migration-ledger update.

Focused tests run first with a maximum 120-second timeout. Before integration,
run formatting, all affected `rem6-cpu` and `rem6` source-policy tests, the
focused CLI matrix, both crates' all-target tests, and `cargo test --workspace`.
A final read-only review must check that the wider capacity is real runtime
behavior, not a test-only helper or counter claim, and that no documentation
score leads executable evidence.
