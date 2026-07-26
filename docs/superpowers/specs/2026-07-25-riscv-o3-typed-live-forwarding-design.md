# RISC-V O3 Typed Live Forwarding Design

## Goal

Extend the persistent RISC-V O3 live issue queue with one bounded typed
forwarding path:

- a scalar single-precision floating-point result produced by an older live
  compute row may feed a younger supported scalar FP compute row; and
- an existing vector-to-scalar row with an integer destination may feed a
  younger supported scalar integer row through the existing integer forwarding
  path.

The increment must prove real producer-to-consumer residency, dependency
wakeup, nearest-producer selection, speculative execution, ordered validation,
rollback, and exact architectural results. Evidence must span issue widths 1,
2, and 4, direct and cache/fabric/DRAM routes, live checkpoint rejection,
drained restore, and timing-mode suppression.

This increment does not add vector register write records, live vector source
forwarding, double-precision or status-sensitive FP dependency coverage, FP
load forwarding, arbitrary dependency graphs, or a checkpoint-restorable live
IQ. A true vector-register chain such as `vmul.vv -> vmv.x.s` remains an
explicit negative boundary. The CPU checklist count and 74% representative cap
remain unchanged.

## Current Boundary

The preceding mixed-compute increment already supports independent scalar
single-precision FP rows and bounded vector-to-scalar rows in the persistent
queue. Its operand authority represents integer, floating-point, and vector
architectural registers, and staged destinations retain their register class.

Live dependency forwarding is still integer-only:

- queue compute metadata discards typed source identity after classification;
- producer discovery matches only integer rename destinations;
- `O3LiveIssueSourceProducer` stores an integer `Register`;
- candidate materialization stores only `RegisterWrite` values;
- the control window resolves only integer writes or completed load values;
  and
- issue service applies only integer writes to the cloned hart.

The window policy and compute queue therefore reject a scalar FP consumer when
an older live row owns one of its FP sources. The rejected row later executes
normally and produces a correct result, but it does not participate in live
dependency scheduling.

The architecture already has the durable scalar FP value required for the
bounded change. `RiscvExecutionRecord` owns `FloatRegisterWrite` values for
supported scalar FP compute rows, and `RiscvHartState::write_float` can install
the exact NaN-boxed value into a speculative clone.

Vector destinations have a different ownership model. Vector arithmetic
mutates hart vector state directly and `RiscvExecutionRecord` has no vector
register writes. Correct forwarding would also need LMUL register groups,
masked and tail-undisturbed semantics, implicit `v0`, old destination values,
VCSR side effects, and vector-load publication. The new architectural vector
checkpoint state is committed state, not a speculative vector-result store.

## Considered Approaches

### 1. Scalar FP forwarding only

Generalize the live value path for `FloatRegisterWrite` and prove a dependent
FP chain, while leaving vector evidence unchanged.

This is the smallest implementation, but it does not demonstrate that the
existing vector-result issue family interoperates with the dependency engine.

### 2. Scalar FP plus vector-result-to-integer forwarding

Add typed scalar FP forwarding and prove that a vector-to-scalar row can feed a
younger scalar integer row through the existing integer value path. Keep true
vector-register producers unsupported.

This is the chosen approach. It demonstrates two mixed-class producer bridges
without inventing vector speculative-state semantics that the ISA execution
record cannot represent.

### 3. Full vector-register forwarding

Add `VectorRegisterWrite` effects, vector rename and writeback publication,
group-aware producer identity, mask and tail merge semantics, vector-load
publication, and vector status handling before admitting a chain such as
`vmul.vv -> vmv.x.s`.

That is the correct later subsystem, but it is too broad for the same bounded
increment. It changes ISA records, runtime ownership, rollback, and checkpoint
compatibility together.

## Chosen Architecture

### Typed Producer Identity

Use `O3ArchitecturalRegister` as the source identity throughout live compute
dependency discovery. It already contains the architectural register class and
index required to distinguish integer, FP, and vector sources.

Generalize `O3LiveIssueSourceProducer` from:

- producer sequence plus integer `Register`

to:

- producer sequence plus `O3ArchitecturalRegister`.

Producer discovery scans older live-staged ROB rows in reverse order and
selects the first rename destination whose class and architectural index match
the source. This preserves nearest-older WAW semantics. Repeated use of the
same source deduplicates the exact typed `(producer sequence, source)` pair.

Integer zero remains non-dependent. FP register zero is an ordinary
architectural register and must not receive integer-zero treatment.

Control and pending-address callers adapt their integer sources into typed
identities. Their behavior and accepted instruction families do not change.

### Compute Admission

Keep `o3_live_compute_operands` as the sole instruction-to-typed-operands
authority. Compute candidate metadata retains the complete ordered typed source
list rather than reducing it to integer sources.

Supported live FP dependencies are limited to the scalar single-precision
compute families already admitted independently:

- `fadd.s` and `fsub.s`;
- `fmul.s`;
- `fmadd.s`, `fmsub.s`, `fnmsub.s`, and `fnmadd.s`;
- `fdiv.s`; and
- `fsqrt.s`.

An older live integer or FP destination may become a producer. A vector source
with an older live vector destination remains unforwardable and prevents queue
admission. A vector source that is already committed remains readable from the
canonical hart clone, preserving the existing vector-to-scalar behavior.

FP loads, double precision, conversions, comparisons, moves, classification,
sign injection, CSR-derived dependencies, and broader status-sensitive chains
remain outside this forwarding claim even if normal architectural execution
supports them.

### Transient Forwarded Values

Introduce one CPU-internal transient value enum with exactly two variants:

- integer `RegisterWrite`; and
- floating-point `FloatRegisterWrite`.

There is deliberately no vector variant. The type represents materialized
speculative operands, not architectural checkpoint state.

Move typed producer discovery and value materialization into a focused queue
submodule so `o3_runtime_issue/queue.rs` remains below its source-policy line
cap. The queue candidate stores the deduplicated typed values, producer
sequences, and maximum forwarded-ready tick.

For each producer:

1. Locate the speculative execution with the exact producer sequence.
2. Match the requested source class and architectural index to exactly one
   corresponding result write.
3. Use the producer's admitted writeback tick as the dependency-ready tick.
4. Add the producer sequence once even when multiple source operands resolve
   to that producer.
5. Deduplicate forwarded values by typed architectural register identity.

Integer pending-address materialization retains its existing completed-load
fallback. No equivalent FP-load fallback is introduced.

### Speculative Issue Service

Issue service continues to clone the canonical `RiscvHartState` for each
selected row. It applies every materialized value according to its variant:

- integer writes use `RiscvHartState::write`; and
- FP writes use `RiscvHartState::write_float` with the exact recorded 64-bit
  NaN-boxed value.

The service then sets the consumer PC and executes the already-decoded
instruction on that clone. The resulting execution record must still satisfy
the existing destination-class-aware validation before transaction recording.

Speculative preparation must not mutate the canonical integer registers, FP
registers, PC, CSRs, or `fflags`. The ordered architectural execution later
recomputes and publishes the real destination and sticky FP flags. This is
sound for the bounded arithmetic dependency because the consumer reads source
registers and the already-committed rounding mode, not the producer's pending
sticky exception flags.

### Dependency and Lifecycle Ownership

The dependency table remains keyed by producer sequence. It does not need a
register-class redesign because typed identity is resolved before the table is
built. A consumer is dependency-blocked until every unique producer sequence
has a speculative result and admitted writeback tick.

Existing sequence-owned behavior remains authoritative for:

- oldest-ready arbitration and class capacities;
- producer wakeup;
- transaction rollback;
- recursive speculative suffix invalidation;
- writeback reservation cleanup;
- redirect and replay cleanup;
- retirement validation; and
- timing/O3 mode handoff.

The new tests must prove those paths with FP values rather than assuming the
integer tests transfer automatically.

## Fail-Closed Behavior

No unsupported or inconsistent typed value may be treated as ready.

- A typed source must match the nearest older staged rename destination by
  both class and index.
- An integer source resolves only an exact `RegisterWrite` or the existing
  pending-address completed-load value.
- An FP source resolves only an exact `FloatRegisterWrite` from the matching
  speculative execution.
- A vector source with a live producer prevents speculative admission.
- An unsupported source shape prevents admission, so the instruction follows
  the established normal execution path.
- Missing, duplicate, wrong-class, or wrong-register result writes for a
  resident dependency prevent materialization. If such an impossible row is
  selected, it remains a runtime consistency error under the existing service
  contract and must not partially mutate runtime state.
- Traps, memory effects, system events, unexpected control flow, or result
  shape mismatches remain invalid recorded compute executions.

Rollback must restore queue membership, dependency rows, telemetry decisions,
writeback reservations, and speculative executions atomically. Recursive
invalidation begins at the failed producer sequence and removes every
speculative descendant, independent of register class.

## Checkpoint and Version Compatibility

Forwarded values, producer identities, dependency rows, and speculative
executions remain transient runtime state. Checkpoint attempts while the live
issue queue is queued, issued, or in mode handoff continue to fail with the
existing non-quiescent error and create no artifact.

Drained restore reconstructs an empty transient queue and preserves the
committed integer, FP, vector, and O3 architectural state already covered by
the checkpoint system.

This increment does not change serialized vocabulary or payload layout:

- O3RT remains v23;
- O3PS remains v2; and
- O3DH remains v7.

No compatibility decoder or version bump is justified for CPU-internal
transient enum variants.

## Representative Matrix

### CPU RED/GREEN Tests

Start with failing tests that require:

- live-window admission for `fadd.s f4, f1, f2` followed by
  `fmul.s f5, f4, f3`;
- retention of the negative `vmul.vv -> vmv.x.s` boundary;
- typed queue metadata for FP sources;
- nearest-older FP producer selection after WAW;
- multiple FP producer sequences for a fan-in consumer;
- exact `FloatRegisterWrite` materialization and forwarded-ready tick;
- dependency blocking before producer writeback and wakeup at the admitted
  writeback tick;
- speculative `9.0f` output from a `3.0f -> multiply by 3.0f` chain;
- no canonical FP register, PC, rounding-mode, or `fflags` mutation during
  speculative preparation;
- transaction rollback with FP forwarded values;
- recursive invalidation of FP-dependent descendants; and
- cleanup and force-normal replay after update, removal, redirect, or mode
  handoff.

Tests for absent or wrong-class FP writes must prove fail-closed behavior. The
existing unsupported vector-producer tests remain and continue to pass.

### Real CLI: Direct Widths 1 and 2

Promote the existing dependent FP boundary binary into positive queue evidence:

- `fadd.s f4, f1, f2` produces `3.0f`;
- `fmul.s f5, f4, f3` consumes the live `f4` result and produces `9.0f`; and
- a later architectural store exposes exact little-endian bytes `00001041`.

At width 1, the consumer must remain dependency-blocked until the producer's
admitted writeback tick and issue on a later eligible turn. At width 2, the
consumer still cannot issue before the data dependency even when aggregate
width is available.

Assert exact queued, selected, issued, wake, writeback, and commit identities,
producer sequence linkage, and architectural result bytes. Focused CPU tests
pin the typed source class and index that are not part of the public CLI schema.

Add a vector-result bridge in which an admitted `vmv.x.s`, unmasked
`vcpop.m`, or unmasked `vfirst.m` writes an integer register and a younger
supported scalar integer instruction consumes that live result. The consumer
must use the ordinary integer forwarded-value variant and produce exact stored
bytes. This does not claim vector-register forwarding.

### Real CLI: Cache/Fabric/DRAM Width 4

Run the same bounded dependency families behind a real cacheable memory head
through cache, transport, fabric, and DRAM. Required evidence includes:

- exact resident and issued sequence identities;
- FP producer-to-consumer dependency and wake timing;
- vector-result-to-integer dependency and wake timing;
- scalar FP, vector-to-scalar, and scalar-integer issue classes;
- cache, transport, fabric, and DRAM activity;
- exact final integer and FP memory bytes; and
- no unsupported vector-destination lifecycle row.

This hierarchy witness complements rather than replaces the direct width
cases. It proves the live queue is wired into the real top-level execution path
while memory timing is active.

### Boundaries and Suppression

Keep or add exact negative evidence for:

- `vmul.vv -> vmv.x.s`, with correct final vector/scalar architectural output
  through normal execution and no speculative vector-source dependency;
- FP loads or unsupported FP forms falling back to normal execution;
- checkpoint rejection while typed rows are queued;
- checkpoint rejection after issue while speculative dependency state exists;
- detailed-to-timing handoff rejection without mutation while typed queue or
  speculative dependency state remains live;
- successful drained O3RT v23 restore with an empty transient queue; and
- timing mode producing the same architectural bytes without O3 queue,
  dependency, class-counter, or issue-lifecycle surfaces.

The representative matrix must cover direct widths 1 and 2 plus hierarchy
width 4. It must not replace those axes with repeated variants of one width or
one memory route.

## Telemetry

Reuse the existing queue, dependency, writeback, and issue-class telemetry.
The implementation adds no new public counter solely for forwarded value
class.

JSON, text, stats-dump, and debug evidence must continue to reconcile:

- queue occupancy and resident sequences;
- dependency-blocked rows and wake events;
- producer and consumer lifecycle identities;
- `scalar_float`, `vector_to_scalar`, and `scalar_integer` issued classes;
- resource and dependency blocking; and
- writeback and commit timing.

Where debug records expose source-producer identity, typed source class and
architectural index must be unambiguous. Existing serialized telemetry fields
remain stable unless an already-transient debug-only field is extended in a
backward-compatible shape.

## Source Policy and File Boundaries

Keep instruction classification in `o3_live_compute_operands.rs`. Keep
compute-specific destination validation in
`o3_runtime_issue/queue/compute.rs`. Extract typed producer discovery and
forwarded-value materialization into a focused queue submodule rather than
growing the capped `o3_runtime_issue/queue.rs` beyond 600 lines.

Keep service application of typed writes close to speculative hart cloning.
Do not move architectural publication out of ordered execution. Keep CLI
fixtures under the existing mixed-compute children rather than growing the
shared persistent-IQ parent.

Source-policy assertions must cover:

- focused file line caps;
- one typed architectural-register identity;
- exactly two transient forwarded-value variants;
- typed nearest-producer matching;
- explicit lack of a vector forwarded-value variant;
- FP write application only to the speculative clone;
- unchanged O3RT/O3PS/O3DH versions;
- real CLI matrix anchors and timing suppression; and
- honest ledger non-claims.

Do not edit or commit anything under `temp/`. Do not build or run the gem5
reference tree. Every Cargo command, including formatting, uses
`TMPDIR=$PWD/target/tmp`.

## Ledger Treatment

The CPU evidence may claim:

- bounded scalar single-precision FP live-producer forwarding for the existing
  live compute family; and
- an existing vector-to-scalar integer result feeding a supported scalar
  integer live consumer.

It must continue to list these gaps:

- true vector-register producers and destinations;
- vector LMUL groups, masks, tail policy, `v0`, vector loads, and VCSR-aware
  forwarding;
- FP loads, double precision, conversions, broader FP families, and
  status-sensitive producer-to-consumer semantics;
- arbitrary or unbounded mixed dependency graphs;
- positive system issue rows;
- a general load/store queue scheduler;
- checkpoint-restorable live IQ and transport state; and
- a general O3 engine.

The component remains `8 of 10` and `74% representative`. This increment
narrows one named dependency gap; it does not complete the O3 checklist item.

## Verification and Closeout

Development follows RED/GREEN TDD. Focused tests run after each implementation
step. The completed branch runs formatting, the affected CPU and CLI suites,
source policy, exact matrix tests, and the full workspace test suite. Any
failure must be reproduced against the pre-change base before being classified
as pre-existing.

Before push, a high-intensity read-only review must inspect architecture,
typed ownership, replay and rollback, canonical-state isolation, real CLI
wiring, checkpoint compatibility, dead code, test strength, and ledger
honesty. Findings are fixed and reverified before the implementation commits
are pushed.
