# RISC-V O3 Younger Atomic Result Design

## Goal

Extend the bounded detailed-O3 memory-result window with one younger,
side-effecting atomic result while preserving precise predecessor ordering.

The younger atomic must acquire real ROB, LSQ, rename, issue, writeback, and
retirement ownership before the older read result completes, but its memory
request must remain CPU-buffered until the older request succeeds. The
increment must run through `rem6 run --execute`, prove direct and
cache/fabric/DRAM execution, and leave retries, redirects, and mode boundaries
without an externally visible wrong-path atomic mutation.

The CPU checklist remains 8 of 10, 80% raw, capped at 74% representative.

## Current Boundary

The existing memory-result window admits exactly two result-producing data
accesses followed by up to two scalar integer rows. The younger result is
restricted to an untranslated, cacheable, read-only integer, floating-point,
or e64/m1 unit-stride vector load. That restriction is correct for ordinary
submission because an older retry or failure cannot undo a younger atomic that
has already reached the target.

The scalar-memory store-prefix path already solves the relevant transport
problem for writes: a younger store can own O3 and CPU state while its request
remains buffered behind an older request. The mechanism is currently named and
classified as store-only even though its guarded submission, predecessor
readiness, serial/parallel preparation, and rollback behavior are the
authorities needed by a younger atomic result.

## Considered Approaches

### 1. Predecessor-gated younger AMO result

Admit one independent, disjoint, unordered AMO as the second result row. Record
its O3 issue immediately, but hold the transport request in a typed buffered
effect until the older read request completes successfully.

This closes the younger-side-effect result gap without pretending that an
already delivered mutation can be rolled back. This is the chosen approach.

### 2. Cached-translated read pairs

Cached translations have exact physical ranges and can reuse most of the
read-only pair runtime. This is a valuable adjacent increment, but it deepens
translation concurrency rather than side-effect ordering. It remains open.

### 3. Dependent result-address generation

A dependent younger access needs a resident LSQ row with no address, a
writeback-time address-generation wakeup, and precise materialization failure
handling. That is more architecturally general but requires a separate
one-entry unresolved-address owner. It remains open for a later increment.

### 4. Younger store-conditional support

SC is not equivalent to AMO buffering. Its reservation decision currently
occurs before buffering, and canonical LR/SC needs reservation installation at
the older LR's admitted publication plus revalidation when the SC is released.
SC remains outside this increment.

## Chosen Window Shape

The maximum remains four ROB rows:

1. one supported pure-read memory-result head;
2. one independent, disjoint, unordered atomic memory result;
3. one scalar integer successor; and
4. one scalar integer successor.

The head is either:

- a scalar `LD` with a nonzero destination;
- an `FLD`; or
- the existing active, one-register e64/m1 unit-stride vector load.

The younger result is an `AMO*.W` or `AMO*.D` with a nonzero destination. The
representative CLI matrix uses `AMOSWAP.D` and `AMOADD.D`.

The younger atomic must satisfy all of these conditions:

- untranslated cacheable memory;
- neither acquire nor release ordering;
- address and data operands independent of unresolved older integer results;
- a physical range disjoint from the older read;
- a nonzero integer destination;
- exact fetch and execution identity;
- no scalar row already started before it; and
- enough capacity for the two-result prefix within the configured four-row
  limit.

Ordinary scalar-load windows retain scalar-memory-prefix priority. The result
window takes precedence only after the completed adjacent younger instruction
is proven to be an authorized buffered atomic effect; load-plus-ALU and
load-prefix behavior remains unchanged. LR, SC, younger stores, a second
atomic-result chain, translated results, MMIO, and a third result remain open.

## Buffered Effect Ownership

Generalize the internal `BufferedO3Store` authority into
`BufferedO3Effect`. The type continues to own:

- the exact predecessor data request;
- the already recorded `OutstandingDataAccess`;
- the fully formed memory request; and
- guarded serial or parallel transport submission.

The existing scalar-store handoff projection remains available only when the
buffered access is a scalar store. A buffered atomic result is deliberately
non-restorable and causes live handoff/checkpoint capture to reject.

The core-state map and helper names must use the generalized effect name. No
parallel store-only compatibility authority remains in production.

## Fetch Authorization

Add an explicit `YoungerBufferedEffect` role beside `Head` and `YoungerRead`.
The authorization remains keyed by the exact first consumed fetch request and
records the result destination, memory route, and exact physical range.

The pair policy may issue `YoungerBufferedEffect` only when:

- the head instruction is a scalar `LD`, `FLD`, or supported vector load;
- the younger instruction is a supported unordered AMO;
- both physical ranges are exact and disjoint;
- all AMO integer sources are independent of unresolved older integer results;
- the head does not overwrite vector mask state consumed by a younger masked
  operation; and
- neither row uses MMIO or translation.

`YoungerRead` keeps its current policy. An atomic head may still precede a
read-only younger result under the existing disjoint unordered rule, but an
atomic head may not precede another buffered atomic in this increment.

All pair-wide lifecycle cleanup must treat both younger roles as belonging to
the active result window. Role handling should use one focused predicate rather
than repeated equality lists.

## Runtime Admission

The memory-result runtime accepts the younger AMO as the second result only
when the live head is a supported pure read and the ranges are disjoint. The
atomic consumes its existing two-sequence LSQ load/store span while counting as
one result-producing data access and one ROB row.

At younger issue:

1. the authorization is revalidated against route, range, PMA, result shape,
   ordering bits, and role;
2. the result row is staged with `MemoryResultWindow` policy;
3. the AMO request is recorded in `outstanding_data` and the generalized
   buffered-effect map;
4. no transport request is submitted while the predecessor remains
   outstanding; and
5. the tail result stages up to two scalar successors using the existing
   dependency and writeback authorities.

The buffered atomic becomes transport-ready only after the predecessor request
leaves outstanding ownership following successful completion. Submission uses
the existing ownership guard so cancellation before delivery produces no
target call.

## Response, Writeback, And Retirement

The older read result publishes and retires through the existing head-only
memory-result authority. The buffered AMO may be submitted after the older
response succeeds, but its result cannot publish before it becomes the oldest
live data row and wins a writeback slot.

The AMO result must wake an exact scalar consumer only at admitted writeback.
Atomic memory mutation, returned old value, dependent scalar result, and final
memory witnesses must all agree.

Issue trace and transport trace have intentionally different meanings:

- the AMO `issue_tick` records O3/CPU ownership before the predecessor
  response; and
- the AMO `request_sent` record appears only after predecessor success.

CLI assertions must prove both facts instead of conflating issue with transport
visibility.

## Failure And Cleanup

An older retry or failure must:

- discard the buffered atomic result and scalar suffix;
- remove its outstanding request and buffered-effect entry;
- clear its fetch retirement ownership and authorization;
- cancel future writeback reservations; and
- leave the target mutation count at zero.

A younger retry or failure preserves the completed older read but discards the
atomic result's scalar suffix. Redirect, interrupt, reset, restart, restore,
failed submission, and detailed-mode disable must remove stale buffered-effect
and authorization ownership.

Live checkpoint and detailed-to-timing handoff remain rejected while the
buffered atomic exists. Drained checkpoint compatibility is unchanged.

## Representative Matrix

The positive CLI matrix runs each row through direct and cache/fabric/DRAM
memory:

| Head | Younger result | Scalar dependency |
| --- | --- | --- |
| `LD` | unordered `AMOSWAP.D` | AMO address/data remain independent of the load result |
| `FLD` | unordered `AMOSWAP.D` | scalar successor consumes the AMO old value |
| masked e64/m1 `VLE64.V` | unordered `AMOADD.D` | scalar successor consumes the AMO old value |

Each row must prove:

- exact four-row ROB residency before the older response;
- exact LSQ occupancy, including the AMO load/store span;
- only the head request is transport-visible before its response;
- the AMO already owns an O3 issue tick before that response;
- an independent scalar row may issue while both results are unresolved;
- AMO transport submission occurs only after predecessor success;
- oldest-first result publication and commit;
- exact FP/vector, integer, and memory witnesses; and
- direct transport-only versus hierarchy cache/fabric/DRAM activity.

## Negative Boundaries

The increment must reject or serialize:

- acquire, release, or acquire-release younger atomics;
- overlapping younger atomic ranges;
- AMO address or data operands that read an unresolved older integer result;
- atomic, LR, or MMIO heads for the younger-effect lane;
- zero-destination atomics;
- translated or PMA-uncacheable memory;
- SC and ordinary younger stores;
- a second younger side effect or third result;
- scalar-before-effect ordering;
- unsupported vector shapes or all-inactive vector heads;
- a fifth ROB row;
- live checkpoint or mode-transfer capture; and
- timing mode.

Focused retry/failure tests must additionally prove that a buffered AMO never
reaches the target after older cancellation.

## Focused Ownership

The current result and CLI owners are at their source-policy limits. The
increment must use focused modules rather than increase existing caps:

- extract memory-result authorization types from `riscv_fetch_ahead.rs`;
- keep younger side-effect pair policy in a focused child beside the existing
  result pair policy;
- generalize `riscv_data_issue/buffered_store.rs` and its state map under
  effect-oriented names;
- keep runtime predecessor selection in the focused memory-window owner; and
- add `writeback_port/younger_atomic_result.rs` as a top-level CLI leaf rather
  than expanding the near-cap result-class family.

Source policy must ratchet the new owners, reject stale store-only buffer names,
and keep existing roots/facades thin.

## Executable Evidence

Focused CPU tests must first fail and then prove:

- fetch authorization emits `YoungerBufferedEffect` only for exact supported
  AMO pairs;
- runtime admission accepts the AMO as result row two and rejects every
  ordering, overlap, dependency, route, PMA, and shape mismatch;
- issue records the AMO row and buffers its request behind the exact head;
- serial and parallel release submit only after predecessor success;
- older retry/failure removes the buffered AMO without invoking the target;
- younger retry/failure preserves only older rows;
- scalar consumers wake from the admitted AMO result; and
- reset, redirect, abort, restore, and detailed-mode cleanup leave no stale
  authorization or buffered effect.

Top-level CLI tests must prove:

- direct and cache/fabric/DRAM execution for both representative classes;
- pre-response four-ROB and class-specific LSQ residency;
- one pre-response request despite the younger AMO issue event;
- predecessor-gated atomic transport and exact target mutation;
- ordered writeback/commit plus scalar dependency wakeup;
- live action rejection while the buffered effect is resident;
- negative ordering/overlap/dependency boundaries; and
- timing-mode architectural equivalence without O3 result-window surfaces.

## Documentation Boundary

After executable evidence passes, update the CPU Migrated, Not migrated, and
Next evidence prose to describe one exact predecessor-gated younger unordered
atomic result. Keep SC, younger stores, broader side-effect chains, translated
or MMIO pairs, dependent result addresses, fifth/deeper rows, restorable
transport ownership, general IQ/wakeup/select, and a general O3 engine open.

Keep the migration ledger exactly 1,200 lines and do not change the score.
