# RISC-V O3 Memory-Result Scalar-Suffix Design

## Goal

Replace the current terminal-only FP/vector/atomic/MMIO result boundary with a
bounded detailed-O3 window that can keep representative scalar integer work
resident behind one result-producing data access.

The increment must execute through `rem6 run --execute`, preserve one shared
ROB/rename/writeback authority, and prove direct memory, cache/fabric/DRAM, and
readfile-MMIO routes. It does not claim multiple outstanding non-scalar data
accesses, general IQ scheduling, restorable transport ownership, or a general
O3 engine.

The CPU checklist remains 8 of 10, 80% raw, capped at 74% representative.

## Current Boundary

Detailed O3 already gives selected integer, floating-point, LR/AMO/SC, vector,
and MMIO results a live ROB/LSQ row and a shared writeback-port reservation.
Only cacheable scalar integer loads may open a younger scalar window. Every
other supported result is recorded with `O3DataAccessWindowPolicy::None`, even
when the front end has a cacheable, fault-free result head and completed scalar
successors.

That boundary leaves two kinds of slop:

1. The fetch path can classify and route representative data-result heads but
   deliberately throws away their bounded younger scalar opportunity.
2. The runtime's generic data-access younger-window machinery is artificially
   restricted to the scalar-memory-prefix policy even though register-class
   aware rename and shared writeback already support these result rows.

## Chosen Matrix

Each positive row contains one result head and up to three scalar integer
successors, for a maximum of four ROB rows:

| Result head | Route | Result dependency | Younger work |
| --- | --- | --- | --- |
| `FLD` | direct and cache/fabric/DRAM | no integer destination | independent `DIV`, dependent scalar continuation |
| masked e64/m1 `VLE64.V` | direct and cache/fabric/DRAM | no integer destination | independent `DIV`, dependent scalar continuation |
| `AMOSWAP.D` | direct and cache/fabric/DRAM | integer result | independent `DIV`, DIV-dependent continuation, AMO-result-dependent terminal row |
| readfile `LD` | MMIO | integer result | independent `DIV`, DIV-dependent continuation, MMIO-result-dependent terminal row |

The long-latency integer row is calibrated to become raw-ready in the same
cycle as the older memory result. Width one must admit the older result and
defer the younger DIV; width two must admit the exact pair. Result-dependent
rows remain resident and blocked until the result's admitted writeback, while
all architectural state remains ordered by ROB commit.

The representative axes are result register class, memory/device route,
independent versus result-dependent wakeup, shared writeback width, and
pre-admission versus final architectural visibility.

## Admission Model

Add one explicit `MemoryResultScalarSuffix` data-access policy. It is available
only when the front end has authorized a real younger fetch window for the
exact fetch request. Data issue consumes that authorization when the head
request is submitted.

The policy accepts the same bounded result shapes already owned by memory
result writeback, except store-conditional:

- nonzero-destination integer loads, including MMIO loads;
- floating-point loads;
- nonzero-destination load-reserved and atomic-memory operations; and
- one-register, e64, non-fault-only-first unit-stride vector loads with at
  least one active byte.

Ordinary cacheable scalar integer loads retain `ScalarMemoryPrefix`, including
multiple scalar memory rows and store forwarding. The new policy owns exactly
one data-access row plus scalar integer successors. It rejects a second data
access and does not enable control-flow descendants.

The existing `--riscv-o3-scalar-memory-depth` bound remains the common bounded
row limit for data-access scalar suffixes. No new configuration surface or
checkpoint schema is added.

## Fetch Authorization

The detailed fetch-ahead owner gains a typed result-window candidate. It may
authorize a result head only when:

- detailed mode is enabled;
- the exact result shape is supported;
- untranslated or cached-translated memory has a valid physical span;
- non-MMIO memory is cacheable under PMA;
- translation and permission probes do not fault or remain unresolved; and
- the MMIO-aware driver has selected an actual untranslated MMIO route for an
  integer load.

Cached-translated MMIO retains its existing one-row transferable handoff and
does not open this suffix.

The candidate uses the existing scalar integer window classifier. FP/vector
heads begin with no unresolved integer destination. Integer result heads seed
their destination as unresolved, so an exact consumer is admitted as a
terminal blocked row. The candidate returns the first consumed fetch request,
exact physical span, and memory-versus-MMIO route along with the next PC; the
driver records that typed identity as the sole authorization for data issue.
This keeps split 32-bit fetch continuity separate from execution identity.

Authorization is consumed at issue and cleared on abort, translation deferral
or fault, discarded fetches, and control-boundary cleanup.
It is not inferred from a coincidental completed fetch, so direct test fixtures
or stale fetches cannot accidentally widen the runtime.

Depth one disables result-suffix authorization and pending-successor waiting,
so tests and deployments that need terminal result timing can opt out without
adding a separate configuration surface.

## Runtime Ownership

`O3RuntimeState::data_access_integer_window` constructs a scalar window for
the new policy only when exactly one resident data-access row owns the tail and
no younger sequence has already been staged. The existing live-window owner
then allocates ROB/rename rows, plans issue and writeback, records dependencies,
and retires in order.

No second issue queue, result calendar, or response callback is introduced.
The existing response path remains authoritative:

1. transport or MMIO completion records raw-ready result state;
2. shared writeback arbitration admits the result;
3. admitted publication wakes the result-dependent scalar row;
4. independent and transitively dependent scalar rows retain their own issue
   and writeback timing; and
5. all rows commit in sequence order.

## Negative Boundaries

The increment must keep these cases outside the new window:

- stores and any second data-access successor;
- zero-destination integer results;
- store-conditional, which retains its focused result lifecycle;
- uncacheable ordinary memory;
- mapped FP, vector, load-reserved, or atomic MMIO;
- cached-translated MMIO;
- translation misses, permission faults, and device-boundary failures;
- LMUL2, segment, strided, indexed, fault-only-first, or all-inactive vector
  loads;
- control-flow successors; and
- timing mode.

Live checkpoint and detailed-to-timing transfer remain rejected while a
non-scalar or MMIO result window is resident because the existing handoff does
not encode these data/result semantics. Drained compatibility remains covered
by existing tests.

## Executable Evidence

Focused CPU tests must prove:

- result-window construction with and without an integer destination;
- independent scalar admission and exact result-dependent terminal behavior;
- second-data-access and unsupported-result rejection;
- fetch authorization for cacheable direct, cached-translated, and MMIO heads;
- suppression for PMA-uncacheable, translation-faulted, unsupported-vector,
  mapped noninteger-MMIO, and device-boundary rows;
- split-fetch execution identity plus stale route/span/PMA rejection;
- authorization consumption at data issue; and
- response-time wakeup through the existing speculative issue owner.

Top-level CLI tests must prove:

- exact four-row ROB residency before the result response;
- one LSQ row for FP/vector/MMIO and the existing AMO span for atomic;
- result/DIV raw-ready collision with older-result priority at width one and
  exact fit at width two;
- independent issue before result response;
- result-dependent issue at or after admitted result writeback, never before;
- pre-admission absence of the result and all ordered architectural witnesses;
- exact final register, vector, and memory witnesses;
- direct transport-only, hierarchy cache/fabric/DRAM, and readfile-MMIO route
  activity; and
- timing-mode architectural equivalence without O3 result-window surfaces.

## Documentation Boundary

After all executable evidence passes, update the CPU Migrated, Not migrated,
and Next evidence text to record the bounded result-suffix matrix and remove
only the covered terminal-only gap. Keep broader multi-data FP/vector/atomic/
MMIO windows, arbitrary fifth/deeper chains, restorable transport ownership,
and general O3 scheduling open. Preserve the migration ledger at exactly 1,200
lines and do not change the score.
