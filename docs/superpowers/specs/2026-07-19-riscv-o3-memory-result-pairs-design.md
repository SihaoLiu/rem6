# RISC-V O3 Memory-Result Pair Window Design

## Goal

Replace the one-result-only detailed-O3 boundary with a bounded memory-result
window containing exactly two result-producing data accesses followed by up to
two scalar integer successors.

The increment must run through `rem6 run --execute`, keep all data requests,
ROB/LSQ rows, rename destinations, writeback reservations, and retirement under
the existing CPU runtime authorities, and prove direct and cache/fabric/DRAM
execution. It does not claim arbitrary result depth, dependent address
generation, translated or mixed-MMIO result pairs, younger side-effecting
atomics, restorable transport ownership, or a general O3 engine.

The CPU checklist remains 8 of 10, 80% raw, capped at 74% representative.

## Current Boundary

The current `MemoryResultScalarSuffix` path admits one FP, vector, atomic,
load-reserved, integer-load, or MMIO result row and then only scalar integer
successors. The runtime already supports multiple live scalar memory rows,
out-of-order response lookup by request, sequence-owned writeback reservations,
and oldest-first data publication. The one-result restriction is enforced by
over-specific policy and authorization names plus explicit `len() == 1` and
scalar-memory-only overlap gates.

Those names become misleading as soon as a second result-producing access is
legal. The increment therefore generalizes the owner instead of adding a
parallel `pair` special case.

## Considered Approaches

### 1. Exact two-result prefix plus scalar suffix

Generalize the current authorization and runtime policy so one independent,
read-only result access may follow the first result. After both data accesses
issue, the existing scalar integer window can stage at most two successors.

This directly covers a named migration gap while preserving the current
four-row bound and existing retirement model. This is the chosen approach.

### 2. Two result rows without scalar successors

This would require less front-end work, but it would create another isolated
terminal shape and defer composition with the scalar wakeup machinery that the
previous increment just established. It is too narrow for the requested
representative matrix.

### 3. General IQ/wakeup/select first

A typed general ready-entry model is the architectural direction beyond this
bounded work, but implementing memory, FP, vector, control, rollback, and
resource arbitration together would exceed a self-contained increment. The
pair window supplies concrete mixed-producer requirements for that later
generalization.

## Chosen Window Shape

The maximum remains four ROB rows:

1. one supported memory-result head;
2. one independent, cacheable, untranslated, read-only memory-result access;
3. one scalar integer successor; and
4. one scalar integer successor.

The second result may be an integer load, floating-point load, or the existing
restricted one-register e64/m1 unit-stride vector load. It may not be an
atomic, store-conditional, store, MMIO access, translated access, fault-only-
first vector, or any access whose integer address/data operands read an
unpublished older integer result.

The first result retains the existing supported head set. In particular, an
atomic may be the oldest row because its side effect is not younger than an
unresolved result. A younger atomic is rejected because an older retry or
failure cannot retract an already submitted atomic side effect.

## Representative Matrix

The positive CLI matrix uses three pair classes on direct and
cache/fabric/DRAM memory:

| Pair | Result classes | Scalar dependency |
| --- | --- | --- |
| disjoint non-`aq`/`rl` `AMOSWAP.D` then `FLD` | integer atomic then FP load | one scalar consumes the atomic result |
| masked e64/m1 `VLE64.V` then `LD` | vector then integer load | one scalar consumes the second load result |
| `FLD` then masked e64/m1 `VLE64.V` | FP then vector | independent DIV plus its scalar continuation |

Each row must prove both requests issue before the first result response, exact
four-row ROB residency, exact class-specific LSQ occupancy, real memory bytes,
final integer/FP/vector witnesses, and oldest-first commit.

A calibrated direct row additionally aligns a scalar DIV completion with a
memory-result completion. Width one must preserve older sequence priority and
width two must admit the exact pair. The pair test does not require the two
memory responses themselves to complete on the same tick.

## Focused Ownership

### Fetch authorization

Replace `O3MemoryResultScalarSuffixAuthorization` with
`O3MemoryResultWindowAuthorization` and rename the core map accordingly. Each
authorization remains keyed by the first consumed fetch request and records:

- optional integer result destination;
- exact physical range; and
- exact memory or MMIO route; and
- explicit `Head` or `YoungerRead` row role.

Move result-specific fetch probing and authorization out of the large
`riscv_fetch_ahead/detailed_o3.rs` owner into a focused
`riscv_fetch_ahead/detailed_o3/data_access_result.rs` module.

The result-window candidate starts from the architectural result head. Before
any scalar successor is accepted, it may admit one completed second result when
all of these hold:

- both instructions are complete, exact fetch identities;
- the second access has a supported read-only result shape;
- the second access is untranslated and resolves to cacheable memory;
- its physical range and result destination are authorized;
- its integer source operands do not read an unresolved older integer result;
- no result or scalar row would exceed the configured four-row bound; and
- no third data access is admitted.

The candidate returns every authorization needed for the completed prefix when
requesting the next fetch. The driver records them atomically before issuing
that fetch. Split 32-bit instructions continue to bind the first consumed
request as execution identity and the last consumed request as sequential
continuity.

An atomic head may open the pair only when it has neither acquire nor release
ordering and the younger read is disjoint. Acquire/release atomics remain
terminal because issuing a later memory result before their completion would
claim ordering semantics this bounded window does not implement.

### Runtime window

Rename `O3DataAccessWindowPolicy::MemoryResultScalarSuffix` to
`MemoryResultWindow`. Add a focused memory-result window state that validates:

- one or two live data accesses;
- every row uses the result-window policy;
- every row still owns a supported result destination shape;
- all rows are resident while the second access is admitted;
- no scalar younger row was staged before the second result; and
- the total ROB-row limit is not exceeded.

`RiscvScalarIntegerLiveWindow::from_memory_results` receives the integer
destinations of both result rows, the occupied result-row count, and the common
row limit. It preserves unresolved destinations so scalar consumers wake only
after the matching admitted result writeback.

### Issue and response ordering

The unissued-data selector may choose a second result only when its exact fetch
authorization matches the execution shape and the runtime can extend the
result window. Ordinary scalar-load heads continue to use
`ScalarMemoryPrefix`; an integer load authorized as the second mixed result
uses `MemoryResultWindow`.

Both result accesses become real `O3LiveDataAccess` rows. Existing response
lookup may complete the younger request first, but publication and retirement
remain head-only through `live_data_accesses.first()`. No minimum-ready or
younger-first publication path is introduced.

When the second result issues, the tail result owns scalar-suffix staging. The
existing dependency and writeback authorities resolve scalar sources against
the nearest preceding physical producer across both result rows.

### Failure cleanup

An older retry or failure truncates the younger result and scalar suffix,
removes their pending CPU ownership, and ignores any later read response. A
younger retry or failure preserves the older result but discards the younger
row and suffix. No younger write-like result is admitted, so cleanup never
pretends to undo an external atomic or device side effect.

Authorization cleanup remains mandatory on issue, failed issue, translation,
fetch reset, redirect, abort, restart, checkpoint restore, and pre-execution
detailed-mode disable. Already executed live result rows retain their runtime
ownership until ordered drain.

## Negative Boundaries

The increment must reject or serialize:

- a third result-producing data access;
- a second atomic, store-conditional, store, or MMIO result;
- an acquire/release atomic head or an atomic/younger-read overlap;
- any translated second result, including cached translated memory;
- a second result whose base or data operand reads the first integer result;
- zero-destination integer results;
- PMA-uncacheable ordinary memory;
- mapped MMIO FP/vector/LR/atomic accesses;
- LMUL2, segment, strided, indexed, fault-only-first, or all-inactive vectors;
- control-flow rows in the result window;
- a fifth ROB row; and
- timing mode.

Live checkpoint and detailed-to-timing capture remain rejected while either
result row is resident. Drained checkpoint compatibility remains unchanged.

## Executable Evidence

Focused CPU tests must first fail and then prove:

- two result rows construct a two-row unresolved window;
- the second result is rejected before authorization and accepted after exact
  authorization;
- an integer load authorized as the second result does not fall back to the
  scalar-memory-prefix policy;
- a dependent second address, side-effecting second result, third result,
  translated result, MMIO result, route mismatch, span mismatch, and PMA change
  fail closed;
- younger-first completion does not publish before the older result;
- scalar consumers wake from the correct first or second result writeback;
- older and younger retry/failure discard the correct suffix; and
- reset, redirect, abort, restore, and detailed-mode lifecycle cleanup leave no
  stale authorization.

Top-level CLI tests must prove:

- direct and cache/fabric/DRAM execution for all three representative pairs;
- both data requests issue before the first response;
- exact four-row ROB and class-specific LSQ residency before publication;
- direct transport-only versus hierarchy cache/fabric/DRAM activity;
- width-one older priority and width-two exact-fit writeback behavior;
- exact final integer, floating-point, vector, and memory witnesses;
- dependent second-address suppression before the first result publishes;
- denied oldest atomic suppression before any younger request; and
- timing-mode architectural equivalence without O3 result-window surfaces.

## Documentation Boundary

After executable evidence passes, update the CPU Migrated, Not migrated, and
Next evidence prose to describe the bounded two-result mixed-data matrix.
Replace only the covered one-result limitation. Keep broader result depth,
translated and mixed-MMIO pairs, dependent address generation, younger
side-effecting results, fifth/deeper rows, general IQ/wakeup/select, restorable
transport ownership, and a general O3 engine open.

Keep the migration ledger exactly 1,200 lines and do not change the score.
