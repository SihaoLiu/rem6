# RISC-V O3 FP and Vector-Result Live Issue Design

## Goal

Extend the persistent RISC-V O3 live issue queue with two bounded compute
classes that already have durable architectural result records:

- scalar single-precision floating-point arithmetic with a floating-point
  destination; and
- vector-to-scalar operations with an integer destination and committed vector
  sources.

The increment must prove that these rows are resident in the runtime-owned
queue, participate in oldest-ready and class-cap arbitration, survive
scheduler turns, reserve writeback where required, and publish architectural
results through the existing ordered-retirement path.

The executable matrix spans issue widths 1, 2, and 4, direct and
cache/fabric/DRAM routes, JSON/text/stats-dump/debug evidence, live checkpoint
rejection, drained restore, and timing-mode suppression. System instructions,
dependent floating-point or vector sources, and vector-destination arithmetic
remain explicit boundaries.

This increment does not add a checkpoint-restorable live IQ, floating-point or
vector result forwarding, vector register write records, arbitrary mixed-class
dependency graphs, or a general O3 engine. The CPU checklist count and 74%
representative cap remain unchanged unless the final executable evidence
supports a separate ledger decision.

## Current Boundary

The persistent queue currently represents four trace classes:

- scalar integer;
- integer multiply/divide;
- memory AGU; and
- control.

The lower runtime already has most of the machinery required for scalar FP:

- `O3RegisterClass::FloatingPoint` is supported by ROB and rename entries;
- scalar FP execution emits `FloatRegisterWrite` values;
- ordered retirement installs floating-point rename entries;
- scalar FP add, multiply, FMA, divide, and square-root latency classes are
  recorded by the real O3 runtime; and
- the generic issue vocabulary already contains `O3IssueOpClass::Float`.

The live queue still excludes FP because normal live staging derives only an
integer destination, queue admission only recognizes scalar-integer or control
operands, recorded-execution validation rejects all float writes, and the live
calendar gives `Float` no class capacity.

Vector-destination arithmetic has a deeper blocker. The ISA execution record
does not contain vector register writes; vector arithmetic mutates hart vector
state directly. A speculative vector-destination row therefore cannot be
retained and validated with the same durable execution metadata used by the
live queue.

A smaller vector result family is already representable. `vmv.x.s`,
`vcpop.m`, and `vfirst.m` read vector state but write one integer register
through `RegisterWrite`. They can be executed speculatively when all vector
inputs are committed before the live window opens. This is the honest vector
boundary for this increment.

System instructions remain architectural serialization points. Traps, CSR
effects, privilege changes, TLB invalidations, wait-for-interrupt events, and
host actions must not enter the ordinary speculative live issue lane.

## Considered Approaches

### 1. Scalar FP only

Add a `Float` queue class and floating-point destination validation, but leave
all vector instructions outside the queue.

This is the smallest change, but it leaves the vector axis as another
single-class omission and does not establish a typed operand boundary that can
distinguish committed non-integer sources from live producers.

### 2. Scalar FP plus vector-to-scalar results

Add one focused typed compute-operand authority, admit independent scalar FP
and vector-to-scalar rows, give them separate issue classes and telemetry, and
reject live non-integer producer dependencies.

This is the chosen approach. It exercises real FP and vector instruction
families without pretending that vector register writeback or non-integer
forwarding exists.

### 3. Full vector-destination arithmetic

Add vector write effects to `RiscvExecutionRecord`, vector rename/writeback
publication, vector source forwarding, vector dependency wakeup, and persistent
IQ evidence for `vmul.vv` or another vector-destination operation.

This is the correct later architecture, but it is a separate subsystem-sized
increment. Folding it into this work would widen ISA record semantics,
writeback ownership, rollback, checkpoint compatibility, and architectural
publication at once.

## Chosen Architecture

### Typed Compute Operands

Add a focused CPU-internal module for live compute operands. It defines one
small architectural-register identity containing:

- `O3RegisterClass`; and
- the architectural register index.

The module returns a destination plus ordered source identities for only the
instruction families supported by the live issue lane. Existing scalar integer
helpers remain authoritative for their established breadth; the new helper
adapts those operands into typed identities and adds the bounded FP and vector
result families.

Supported scalar FP v1 instructions are the single-precision arithmetic forms
with one FP destination:

- `fadd.s` and `fsub.s`;
- `fmul.s`;
- `fmadd.s`, `fmsub.s`, `fnmsub.s`, and `fnmadd.s`;
- `fdiv.s`; and
- `fsqrt.s`.

Double precision, comparisons, conversions, moves, classification, and sign
injection remain outside this live lane. They already execute through the
normal detailed path and can be added later as representative families rather
than by broad pattern matching.

Supported vector-result v1 instructions are:

- `vmv.x.s`;
- unmasked `vcpop.m`; and
- unmasked `vfirst.m`.

Masked reductions remain outside v1 because they add the implicit `v0` source
and a second vector dependency shape without improving the issue-class claim.
Vector configuration must already be valid in committed hart state.

### Staging and Rename Ownership

`stage_live_instruction` derives its rename destination from the typed live
compute operand helper before falling back to existing control behavior.

- scalar FP rows stage `O3RegisterClass::FloatingPoint` destinations;
- vector-to-scalar rows stage `O3RegisterClass::Integer` destinations; and
- zero integer destinations remain non-renaming rows and are not admitted.

The ROB and rename map remain the only owners of staged destination identity.
The queue stores sequence membership and re-materializes the rename entry from
canonical runtime state.

The existing scalar-rooted live window gains typed live-destination tracking.
Scalar integer forwarding behavior is unchanged. FP and vector sources are
admitted only when no older live-staged row owns the same typed destination.
If such a producer exists, classification stops before the dependent row is
staged. The instruction later executes normally after the live window drains.

Replay records that normal-execution ownership as a transient fetch identity
only after staging reaches the matching producer row. The identity suppresses
later standalone speculative admission until the rejected instruction retires
or mode-disable/fetch/reset/restore cleanup removes it. It is not queue
membership and does not extend the O3RT checkpoint payload.

This makes dependent FP/vector exclusion an admission rule rather than an
unserviceable resident queue state.

### Queue Candidate Shape

Rename `O3LiveSpeculativeIssueKind::Scalar` to `Compute`. The variant continues
to hold exactly one `O3RenameMapEntry`, which may now identify an integer or
floating-point destination.

Queue materialization uses the typed compute helper:

1. Match the decoded instruction to its supported destination and sources.
2. Match the destination to the staged rename entry.
3. Preserve existing integer producer discovery and forwarding.
4. Reject the candidate if any floating-point or vector source has an older
   live-staged producer.
5. Build a candidate with no forwarded non-integer values.

The service path continues to clone the hart and apply only integer forwarded
writes. Scalar FP and vector-result rows read committed FP/vector state from
that clone.

Recorded execution validation is destination-class aware:

- integer compute destinations require exactly one matching integer write and
  no FP writes;
- floating-point compute destinations require exactly one matching FP write
  and no integer writes; and
- vector, condition-code, and miscellaneous destinations are rejected.

All compute rows reject traps, system events, memory accesses, unexpected
control flow, extra result writes, or mismatched instruction bytes.

### Issue Classes and Calendar

Use these scheduler classes:

- scalar FP rows: `O3IssueOpClass::Float`;
- vector-to-scalar rows: new `O3IssueOpClass::Vector`;
- existing integer, memory, branch, and system vocabulary: unchanged.

The persistent live calendar gains one slot per tick for `Float` and one slot
per tick for `Vector`. Total issue width still applies across all classes.

The separate class capacities are deliberate. A width-two tick may coissue one
FP and one vector-result row, while two FP rows contend for the same FP slot.
This provides observable class arbitration rather than merely tagging rows
after selection.

`O3PendingStateCheckpointPayload` advances from O3PS v1 to v2 because the
serialized op-class vocabulary gains `Vector`. The decoder accepts both v1 and
v2. Existing codes 0 through 5 keep their meanings; v2 assigns code 6 to
`Vector`. This codec change does not make live IQ membership checkpointable.

### Writeback and Retirement

Scalar FP compute rows use the existing scalar FP latency and fixed-FU
writeback reservation path. Their float register writes remain speculative
metadata until ordered architectural execution and retirement.

Vector-to-scalar rows have the latency already defined by the normal execution
path. They still reserve the shared fixed-FU writeback port because they carry
an integer destination. Same-tick readiness is admitted through the existing
writeback calendar and ordered commit machinery.

No new architectural publication owner is introduced. The real CPU execution
event remains responsible for applying FP status effects and final register
state; the live record supplies timing, identity, and ordered-retirement
evidence.

### Telemetry and Debug Surfaces

Extend `O3LiveIssueTraceClass` and `O3LiveIssueTelemetry` with:

- `scalar_float`; and
- `vector_to_scalar`.

The names are intentionally narrower than `vector` or `vector_arithmetic`.
They describe exactly what the runtime can prove.

Expose both counters through the existing canonical surfaces:

- JSON `/cores/0/o3_runtime/issue/queue/issued_by_class/*`;
- text `sim.cpu0.o3.issue_queue.issued_by_class.*`;
- stats dump `sim.host_actions.stats_dump.cpu0.o3.issue_queue.issued_by_class.*`;
  and
- debug lifecycle records under
  `/debug/o3_trace/0/issue_queue/events[*]/issue_class`.

Existing aggregate `issued_rows`, resource-blocked rows, dependency-blocked
rows, occupancy, service-turn, and wake counters remain the arithmetic
authorities. The two class counters must reconcile as subsets of issued rows.

No `system` issued counter is added because system rows are not admitted.

## Representative Matrix

### Direct, Width 1

Use one independent direct-memory scalar load as a bounded batching prelude.
Its five-row live window contains, in order, a long scalar DIV, one independent
`fadd.s`, one independent `vmv.x.s`, and a second independent scalar FP row.
The load reservation consumes the initial width-one turn, after which DIV and
the mixed rows serialize in queue order. This prelude is necessary because the
direct frontend otherwise binds independent four-byte instructions one fetch
round trip at a time and never presents a multi-row arbitration turn.

Required evidence:

- the FP and vector-result PCs appear as queued and selected lifecycle rows;
- their issue classes are `scalar_float` and `vector_to_scalar`;
- width one serializes selection ticks;
- FP issue/writeback/commit timing follows the existing FP latency;
- the vector-to-scalar integer value is exact; and
- a later store or dump proves the FP result bits.

### Direct, Width 2

Use the same direct-memory batching prelude. The load reservation and oldest
DIV consume the first service turn, leaving two independent scalar FP rows and
one independent vector-to-scalar row resident for the next width-two turn.

Required evidence:

- one FP row and the vector-result row can coissue when both are ready;
- the second FP row is retained as resource blocked because the FP class has
  one slot;
- all four younger rows share one queued tick, and DIV is selected exactly one
  tick before the mixed-class coissue;
- its selected tick is later than the first FP row; and
- resource-blocked row-cycle telemetry increases.

### Cache/Fabric/DRAM, Width 4

Use one real cacheable memory head to open the window, followed by independent
scalar integer, scalar FP, and vector-to-scalar rows.

Required evidence:

- exact four-row ROB residency;
- one memory, one scalar-integer, one scalar-FP, and one vector-result issue
  witness;
- cache, transport, fabric, and DRAM activity from the real hierarchy route;
- exact architectural integer and FP result bytes; and
- no fifth row admitted into the bounded window.

### Boundaries

The matrix must include these negative or suppression cases:

- an FP consumer of an older live FP destination remains outside the queue and
  later produces the correct architectural result;
- a vector-destination instruction such as `vmul.vv` remains outside the
  queue, has no `vector_to_scalar` lifecycle row, and later produces the exact
  vector bytes through normal execution;
- a system or host-action instruction remains outside the queue and executes
  only at its architectural boundary;
- a checkpoint attempt while mixed-class queue rows are live fails with the
  existing non-quiescent error and creates no checkpoint artifact;
- a drained checkpoint restores with an empty transient queue while O3PS v1
  and v2 payload compatibility is covered at the codec level; and
- timing mode produces the same architectural outputs without O3 queue,
  class-counter, or issue lifecycle surfaces.

## Error Handling and Invariants

The implementation fails closed at each ownership boundary:

- unsupported instruction families do not enqueue;
- a typed destination must exactly match the staged rename entry;
- live FP/vector source producers prevent admission;
- a materialized non-integer producer dependency is a consistency error, not a
  silently ready row;
- recorded result class and count must exactly match the destination;
- all class reservations remain bounded by total width and per-class capacity;
- system events, traps, and memory effects are never accepted as compute rows;
- Vector O3PS codes are legal only in v2 payloads; and
- restore clears transient live queue telemetry and membership as before.

Rollback, replay, redirect, retirement, mode handoff, and stats reset continue
to use the existing sequence-owned queue lifecycle. Rejected dependent rows
remain outside that queue; their transient normal-execution identity follows
fetch-sequence cleanup and is cleared by retirement, mode disable, reset, and
restore.

## Source Policy and File Boundaries

Keep operand classification in a new focused module rather than expanding the
already broad scalar source helper. Keep compute-specific queue adaptation and
destination-class validation in `o3_runtime_issue/queue/compute.rs` so the
capped `queue.rs` remains a thin sequence/materialization owner. Keep
mixed-class CLI evidence in focused children below `persistent_iq.rs`; the
existing parent remains the shared fixture and helper owner.

Add source-policy assertions for:

- focused file line caps;
- one typed compute operand authority;
- exact FP and vector-result family lists;
- Float and Vector calendar capacity;
- destination-class-aware validation;
- stable class names on every output surface;
- O3PS v1/v2 compatibility and Vector code ownership;
- matrix CLI anchors; and
- honest ledger non-claims.

Do not edit or commit anything under `temp/`. Do not build or run the gem5
reference tree.

## Ledger Treatment

The CPU section may add executable evidence for persistent scalar FP and
vector-to-scalar queue rows. It must continue to list these gaps:

- vector-destination arithmetic;
- FP/vector live-producer forwarding and arbitrary mixed dependency graphs;
- positive system issue rows;
- a general load/store queue scheduler;
- checkpoint-restorable live IQ/transport state; and
- a general O3 engine.

The component remains `8 of 10` and `74% representative` for this increment.
No checklist item or percentage changes solely because two new queue classes
exist.

## Verification

Development follows TDD. Unit and source-policy tests first demonstrate absent
typed operands, class reservations, destination validation, codec support, and
output fields. Production changes then make those tests pass. Real CLI tests
must launch `CARGO_BIN_EXE_rem6` and assert queue lifecycle records, stats,
architectural memory bytes, route activity, checkpoint behavior, and timing
suppression.

Every Cargo command uses `TMPDIR=$PWD/target/tmp`. Focused tests run during each
task; the completed branch runs formatting, the relevant crate suites, CLI
matrix tests, source policy, and the full workspace test suite. A final
read-only high-intensity review checks architecture, real wiring, dead code,
test strength, checkpoint compatibility, and ledger honesty before commit and
push.
