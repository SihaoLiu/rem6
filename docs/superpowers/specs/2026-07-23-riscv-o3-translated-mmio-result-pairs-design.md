# RISC-V O3 Translated And MMIO Result-Pair Design

## Goal

Extend the bounded detailed RISC-V O3 memory-result window so exactly two
result-producing accesses may overlap when address translation is enabled and
when one or both translated targets resolve to readfile MMIO. The increment
must keep translation, target selection, request ownership, response matching,
writeback admission, and ordered retirement on the real `rem6 run --execute`
path.

The representative matrix covers:

- two translated cacheable scalar result loads through direct memory;
- two translated cacheable scalar result loads through
  cache/fabric/DRAM;
- one translated cacheable scalar result load paired with one translated
  readfile-MMIO scalar result load;
- writeback widths one and two;
- two requests outstanding before the first response;
- exact translation, ROB, LSQ, transport, MMIO, writeback, and retirement
  evidence; and
- dependency, ordering, translation-fault, lifecycle, transfer, and timing
  boundaries.

This increment closes the migration ledger's explicit translated or MMIO
result-pair gap. It does not claim arbitrary translated result depth, two
independent device buses, translated FP/vector/atomic pairs, page-table-walk
transport, arbitrary mixed memory/device graphs, restorable in-flight
transport checkpoints, or a general O3 engine.

The CPU Execution Models component currently has 8 of 10 checklist items, 80%
raw coverage, and a 74% representative cap. After the executable matrix is
complete, the ledger score and cap may be raised only to the level justified
by the evidence; the design does not predeclare a numeric result.

## Current Boundary

The existing memory-result window supports two untranslated result-producing
accesses and multiple outstanding cacheable requests. It intentionally rejects
a translated or MMIO second result. The translated and MMIO drivers also retain
single-outstanding gates:

- translated serial and parallel drive paths stop while any data request is
  outstanding;
- untranslated and translated MMIO preparation stops while any data request is
  outstanding; and
- the translated cluster driver skips all data progress for a core with an
  outstanding request.

Those gates preserve in-order behavior but are now narrower than the detailed
O3 runtime. The O3 runtime already owns the relevant bounded authorities:

- a sequence-ordered memory-result window;
- exact ROB, rename, and LSQ rows;
- total and memory-class issue width;
- request-keyed outstanding-data state;
- result writeback reservations;
- oldest-first publication and retirement;
- translation queues keyed by fetch identity; and
- live data handoff records with per-request memory or MMIO targets.

The missing behavior is an admission rule that lets those authorities govern a
second translated or MMIO request without weakening the ordinary single-
outstanding paths.

## Considered Approaches

### 1. Add CLI evidence around the existing single-request behavior

This would not close the migration gap. The production drivers currently block
the overlap that the evidence needs to prove, and the existing result-pair test
owner has only minimal line-budget headroom.

### 2. Remove all outstanding-request gates

This would change timing and in-order execution, allow unsupported stores and
device effects to overlap, and bypass the configured O3 issue and window
limits. The blast radius is larger than the required increment.

### 3. Add bounded O3 translated/MMIO admission

Permit progress past an outstanding request only when the next access is an
exact authorized row in the same detailed-O3 memory-result window and the
existing issue, depth, target, and lifecycle authorities admit it. Ordinary
timing, functional, and in-order paths remain single-outstanding.

This is the chosen approach.

## Accepted Window Shape

The positive window remains four ROB rows:

1. one scalar integer result load;
2. one independent scalar integer result load;
3. one independent scalar DIV used for writeback collision evidence; and
4. one scalar integer consumer of either result.

Both data accesses are eight-byte `LD` instructions with distinct nonzero
destinations. They use exact four-byte decoded fetch identities, disjoint
virtual ranges, and distinct physical ranges. The second address must not read
the first result or another unresolved integer destination.

Accepted target combinations are:

- translated cacheable memory followed by translated cacheable memory; and
- translated cacheable memory followed by translated readfile MMIO.

The reverse MMIO-then-memory order may be used as a focused CPU boundary but is
not required in the top-level representative matrix. Two MMIO results may be
covered if they fall out naturally from the same authority, but they are not a
completion requirement. Stores, SC, LR, atomics, FP loads, vector loads,
fault-only-first accesses, and zero-destination integer loads remain outside
the positive translated/MMIO pair.

## Two-Stage Authorization

The existing untranslated result-window authorization binds an exact physical
range and route before issue. Translation requires a split contract.

### Fetch-stage authorization

When the detailed O3 fetch window accepts a translated result row, it records:

- exact fetch identity and O3 sequence;
- `Head` or `YoungerRead` role;
- result destination;
- virtual address and access size;
- translated access requirement;
- allowed target classes: cacheable memory, or cacheable memory plus readfile
  MMIO for the mixed row; and
- the shared result-window identity.

This authorization does not guess a physical address or target route.

### Translation-stage binding

Translation completion consumes the exact fetch authorization and binds once:

- translated physical address and span;
- memory route and endpoint, or exact MMIO route;
- PMA/PMP outcome;
- request byte offset; and
- target class.

Binding fails closed on fetch, sequence, virtual span, result destination,
target class, route, or physical-span mismatch. A bound row cannot be rebound
to a different target. Translation faults consume or discard the matching
authorization through the existing precise trap path and never allocate a
data request.

The existing pending and ready translated-data collections remain the sole
translation queues. No pair-specific queue or duplicated translation state is
introduced.

## Bounded Request Admission

Replace blanket translated/MMIO outstanding checks with one focused query that
answers whether data progress is currently legal.

Admission is true only when all of these hold:

- the core is in detailed O3 mode;
- the next data access belongs to a live `MemoryResultWindow` row;
- its fetch identity, sequence, destination, and bound target match the
  two-stage authorization;
- every older outstanding request belongs to the same result window;
- the next row has an O3-selected issue tick no later than the current tick;
- the configured total issue width and memory issue width have capacity at that
  tick;
- scalar-memory depth and ROB/LSQ bounds remain satisfied;
- no ordering, overlap, side-effect, trap, redirect, or retry boundary blocks
  the row; and
- the admitted pair count remains exactly bounded at two.

When these conditions are false, existing behavior is preserved: translated
and MMIO paths wait for the current request to drain.

The cluster still submits at most one data action per core per drive pass.
Repeated scheduler turns at the same tick may submit both selected requests,
matching the existing memory-issue calendar without inventing a multi-request
transport callback.

## Target Routing

Translated cacheable rows continue through the normal data route and therefore
exercise direct transport or cache/fabric/DRAM according to the run
configuration.

Translated readfile-MMIO rows continue through `MmioBus` and must not emit an
ordinary data-transport request. A mixed pair therefore has two independent
request owners:

- the cacheable row owns one `MemoryRequestId` and memory route; and
- the device row owns one `MemoryRequestId` plus its exact `MmioRoute`.

Both remain entries in the canonical outstanding-data map and are matched by
request identity on completion. Route selection may not depend on pair order,
response order, or a shared mutable "current target" field.

## Completion, Writeback, And Retirement

Either request may respond first. Completion updates only the matching live
data row and preserves the other request. Architectural publication remains
head-only and ordered through the existing live data retirement authority.

For the calibrated direct translated-memory row, both results reach raw-ready
on the same tick:

- writeback width one admits the older result first and defers the younger by
  one cycle; and
- writeback width two admits the exact pair on the same cycle.

The scalar consumer issues only after its selected result reaches admitted
writeback. The DIV row remains independent and supplies cross-class contention
evidence without becoming a third memory result.

No translated/MMIO special case may publish registers directly from a
translation or device callback.

## Fault, Retry, And Ordering Boundaries

The increment must preserve sequence-precise cleanup:

- a missing or denied translation for the younger row allocates no request,
  preserves the older request, and stages the precise younger fault behind
  ordered retirement;
- a younger PMP/PMA or target-class failure discards the younger row and scalar
  suffix without resubmitting the older request;
- an older failure or retry cancels the younger row and ignores a later stale
  younger completion;
- a younger failure preserves an already completed older row;
- a second address dependent on the first result remains unissued until the
  first admitted writeback;
- acquire/release or overlapping atomic heads remain terminal;
- a third result remains outside the pair; and
- redirect, trap, interrupt, reset, restart, and HTM abort remove the exact
  affected sequence suffix.

Every boundary must assert request counts so recovery cannot hide duplicate or
escaped transport activity.

## Checkpoint And Mode Transfer

Live checkpoint capture remains rejected while either translation,
authorization, request, response, writeback reservation, or result row is
resident. A drained checkpoint restores with empty pair-specific live state and
the normal configured issue widths.

Detailed-to-timing mode transfer is supported after both rows are target-bound
and transport-owned. The existing live data handoff format must carry both
requests in sequence order with their independent memory/MMIO targets. Transfer
must preserve inherited issue, response, writeback, and commit ticks, drain
each request once, and expose no later detailed O3 activity.

Addressless, translating, partially bound, or otherwise non-transferable pair
state rejects the switch without mutation. Timing mode from program start
executes architecturally with one request at a time and exposes no detailed O3
pair surface.

## Representative CLI Matrix

Positive evidence lives in a new focused result-class child rather than the
nearly full existing pair module.

| Row | Route | Width | Required evidence |
| --- | --- | --- | --- |
| translated memory pair | direct | 1 | both requests before first response, serialized writeback |
| translated memory pair | direct | 2 | exact-fit pair writeback |
| translated memory pair | cache/fabric/DRAM | 1 | cache, fabric, DRAM, and transport activity |
| translated memory plus translated MMIO | direct plus readfile | 1 | one ordinary request, one MMIO request, no cache/fabric/DRAM activity |
| translated memory plus translated MMIO | cache/fabric/DRAM plus readfile | 1 | hierarchy activity only for the cacheable row |

Every positive row must prove:

- two translations and exact virtual-to-physical bindings;
- both result rows issued before the earliest response;
- exact four-row ROB and two-row LSQ residency;
- two distinct outstanding request identities;
- route-specific memory and MMIO activity;
- pre-writeback registers remain unpublished;
- the correct consumer wakes from the selected result;
- oldest-first publication and commit; and
- exact final register and memory witnesses.

Focused boundary evidence covers dependent second address, translation fault,
PMP/PMA or target mismatch, ordered atomic head, third result exclusion, live
checkpoint rejection, pre-bind switch rejection, two-request handoff, drained
restore, and timing suppression.

## CPU-Level Evidence

Focused `rem6-cpu` tests must first fail and then prove:

- two translated result authorizations can coexist without a physical target;
- translation binds each authorization exactly once by fetch and sequence;
- memory-memory and memory-MMIO target pairs are accepted;
- an unrelated outstanding request cannot open the bounded admission path;
- issue width one and two select the expected rows;
- younger-first response updates only the younger row and does not publish;
- request-keyed MMIO completion cannot complete the memory row or vice versa;
- translation, PMP/PMA, retry, redirect, and stale-completion cleanup is
  sequence precise; and
- handoff accepts exactly two fully bound requests while checkpoint capture
  still rejects live transport.

Create focused child test modules rather than extending the near-cap
`o3_runtime_memory_result_tests.rs` root.

## Source Ownership

Production roots are already close to their generic limits:

- `riscv_translation.rs` is 1,739 lines;
- `riscv_cluster.rs` is 1,723 lines;
- `riscv_data_issue.rs` is 1,619 lines; and
- `o3_runtime_memory_result_tests.rs` is 1,708 lines.

The implementation must extract focused ownership instead of filling those
roots. Expected ownership is:

- translated O3 request preparation and admission in a focused
  `riscv_translation` child;
- translated/MMIO cluster progress helpers in the existing focused cluster
  translation owner or a new child;
- shared O3 pair-admission predicates in a focused data-issue or runtime child;
- CPU tests in new focused child modules; and
- CLI tests in
  `result_classes/translated_mmio_pairs.rs`, with `fixture.rs` and
  `boundaries.rs` children.

Add `source_policy/o3_translated_mmio_pair_ownership.rs` to own exact module
paths, line caps, anchor inventory, no-`include!` rules, duplicate ownership
checks, and rustfmt checks. Update the broad writeback owner only where its
closed child inventory requires awareness of the new module.

The source-policy driver is currently 1,399 of 1,400 lines. Free headroom by
moving the existing checkpoint projection test attribute into its owning
module and removing the root forwarding test. Do not raise the driver cap.

## Documentation And Score Boundary

After executable evidence passes, update the CPU Migrated, Not migrated, and
Next evidence prose. Replace only the translated or MMIO pair limitation.
Keep arbitrary translated depth, broad translated result classes, dependent
translated stores/atomics, page-table-walk transport, arbitrary mixed target
graphs, restorable in-flight checkpoints, fifth/deeper requests, and a general
O3 engine open.

Keep the migration ledger exactly 1,200 lines. Any score or cap change must be
derived from the completed matrix and source-policy scoring rules rather than
from this design document alone.

## Verification

Implementation follows red-green-refactor:

1. add focused failing CPU authorization, binding, admission, and cleanup tests;
2. add failing CLI translated-memory and mixed-MMIO pair rows;
3. implement the smallest production changes that satisfy the bounded
   contract;
4. add lifecycle, handoff, timing, ownership, anchor, and ledger evidence; and
5. refactor only after the focused suites pass.

Required final verification includes:

- focused `rem6-cpu` translated/MMIO pair tests;
- focused `rem6` CLI pair and boundary tests;
- rem6 source-policy tests;
- `cargo test -p rem6-cpu --all-targets`;
- `cargo test -p rem6 --all-targets`; and
- `cargo test --workspace --all-targets`.

All build and test commands must use a home-backed `TMPDIR` while the root
filesystem has no temporary-file headroom. Independent high-intensity review
must inspect production behavior, lifecycle cleanup, CLI evidence, source
ownership, and ledger claims before the branch is merged and pushed.
