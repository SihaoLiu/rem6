# RISC-V O3 FP-Load Live Forwarding Design

## Goal

Extend the persistent RISC-V O3 live issue path so a completed scalar
floating-point load can wake and supply a younger scalar FP arithmetic row
before the load commits architecturally.

The increment covers both FP memory widths and arithmetic precisions:

- `FLW` feeding supported single-precision arithmetic; and
- `FLD` feeding supported double-precision arithmetic.

Evidence must prove typed producer identity, live ROB/LSQ and issue-queue
residency, response-owned readiness, bounded writeback arbitration,
producer-to-consumer wakeup, speculative value use, ordered validation and
commit, exact result bytes, failure cleanup, checkpoint and mode-handoff
boundaries, and timing-mode suppression. The top-level matrix spans issue
widths 1, 2, and 4 plus direct and cache/fabric/DRAM routes.

This increment does not add vector-register forwarding, FP conversions or
comparisons to the live lane, status-register dependencies, arbitrary FP
memory shapes, a checkpoint-restorable live IQ, or a general O3 engine. The
CPU checklist remains 8 of 10 and the component remains capped at 74%
representative.

## Current Boundary

The preceding typed-forwarding increment already carries exact integer and
`FloatRegisterWrite` values between persistent live-IQ compute rows. It can
issue supported scalar single-precision FP producers and consumers, wait for
the producer's admitted writeback tick, apply the value only to a speculative
hart clone, and validate the result at ordered retirement.

Scalar FP loads already participate in a different real O3 path:

- `FloatLoad` is a deferred memory-result access with a floating-point ROB and
  rename destination;
- the load owns LSQ residency, response timing, and a shared writeback-port
  reservation;
- the memory response decoder produces an exact float writeback target,
  including FLW NaN-boxing; and
- ordered data completion installs the value in architectural FP state.

Two ownership gaps prevent a younger FP row from using that result live.

First, memory-result window construction retains only optional integer
destinations. An FLW or FLD head therefore occupies the ROB and LSQ but is not
represented as an unresolved typed destination in the live-window policy.

Second, `live_issue_source_value` can materialize an integer value from either
a speculative compute record or a completed live load. Its FP branch searches
only speculative compute records. A completed FLW or FLD cannot supply an
`O3LiveIssueForwardedValue::FloatingPoint`, so the queue must wait until normal
architectural execution drains the load.

Double-precision FP arithmetic has execution records and typed FU latency, but
the focused live-compute operand authority currently admits only the matching
single-precision families. A FLD-only load row would therefore not establish
a precision matrix.

## Considered Approaches

### 1. Add an FP branch to completed-load value lookup

Convert a completed FLW into `FloatRegisterWrite` and leave window policy and
compute admission unchanged.

This is too narrow. It would rely on incidental ROB scanning while the
window's unresolved-destination authority still forgets FP heads, and one FLW
chain would remain single-axis evidence.

### 2. Typed memory-result destinations plus FP precision matrix

Carry typed memory-result destinations through window construction, add exact
completed-load value materialization for FP sources, and extend the existing
scalar FP arithmetic family symmetrically to double precision.

This is the chosen approach. It closes one real memory-to-compute dependency
boundary, reuses existing writeback and queue ownership, and supplies a
representative load-width, arithmetic-precision, route, and issue-width matrix
without adding a new serialized subsystem.

### 3. Full vector-register forwarding or live-IQ restore

Vector forwarding requires durable vector write records, group-aware rename
and result publication, mask/tail/v0 semantics, vector-load ownership, and
VCSR handling. Restorable live IQ state requires a new O3RT version plus queue,
speculative execution, wake, writeback, and transport reconstruction.

Both are valid later subsystems, but neither is a coherent 500-2000-line
follow-on to the current typed FP value path.

## Chosen Architecture

### Typed Memory-Result Destination Authority

Use `O3ArchitecturalRegister` as the dependency identity for every
memory-result destination that enters a live result window. Integer-specific
authorization remains responsible for dependent addresses and other policies
that genuinely require an integer register; it must not remain the source of
truth for compute dependency classes.

Generalize the memory-result window state from a list of integer destinations
to an ordered, deduplicated list of typed architectural destinations. Derive
the identity from the same accepted `MemoryAccessKind` or decoded instruction
that establishes the ROB rename destination:

- scalar load, LR, AMO, and successful nonzero-destination SC results are
  integer identities;
- FLW and FLD results are floating-point identities; and
- currently accepted vector load results are vector identities.

Including vector identities does not enable vector forwarding. It makes the
existing vector live-producer boundary explicit in the same typed policy, so a
younger vector consumer cannot be mistaken for an independent row.

The existing `RiscvScalarIntegerLiveWindow` name is retained to avoid an
unrelated broad rename, but its memory-result constructor accepts typed
destinations. Integer-only callers adapt their registers at the boundary.
Fetch-ahead prediction and runtime staging must derive the same destination
inventory so the predicted window and live ROB/LSQ state cannot disagree.

### Completed FP-Load Value Materialization

Replace the integer-only completed-load source helper with one typed helper.
It accepts a producer sequence and `O3ArchitecturalRegister`, then:

1. Finds exactly one completed live data access with that sequence.
2. Reconstructs the response writeback from the access and retained response
   bytes.
3. Requires the response target class and register index to exactly match the
   requested typed source.
4. Requires the memory-result writeback reservation to have an admitted tick.
5. Returns either the existing integer forwarded value or a
   `FloatRegisterWrite` wrapped as the existing floating-point forwarded value.

FLW preserves the memory decoder's exact 64-bit NaN-boxed value. FLD preserves
all 64 loaded bits. No raw byte reinterpretation is duplicated in the queue.

Readiness is the admitted memory-result writeback tick, not response arrival,
queue service time, or architectural commit. A width-one writeback collision
may therefore delay the dependent FP wake even after the response exists.

Pending-address fallback remains integer-only. A float value cannot become an
address, satisfy an integer source, or enter the pending-address completion
path through class erasure.

### Double-Precision Live Compute Symmetry

Extend the focused scalar FP operand authority with the double-precision forms
that mirror its current single-precision arithmetic inventory:

- `fadd.d` and `fsub.d`;
- `fmul.d`;
- `fmadd.d`, `fmsub.d`, `fnmsub.d`, and `fnmadd.d`;
- `fdiv.d`; and
- `fsqrt.d`.

They use the existing `ScalarFloat` compute class, `Float` issue class,
destination validation, typed forwarding variant, FU latency classes, and
ordered FP publication. No new issue class or public counter is introduced.

The executable matrix uses exact finite add/multiply chains that neither read
pending `fflags` nor depend on a producer's status effects. Comparisons,
conversions, moves, classification, sign injection, dynamic CSR producers, and
status-sensitive chains remain outside the claim.

### Queue, Writeback, and Retirement Flow

The FP load head remains owned by the live data-access path and LSQ. The
younger FP arithmetic row is owned by the persistent issue queue.

1. Fetch-ahead authorizes the bounded memory-result window with the typed FP
   head destination.
2. Runtime staging creates the load ROB/LSQ row and the younger typed compute
   rename and queue rows.
3. Producer discovery finds the nearest older matching floating-point ROB
   destination by class and architectural index.
4. The consumer remains dependency-blocked while the load is resident or its
   response has not won writeback admission.
5. Completion materializes the exact FP value and admitted ready tick.
6. Scheduler wakeup selects the consumer subject to aggregate width and Float
   class capacity.
7. Service applies the value only to a cloned hart and records the speculative
   arithmetic result.
8. Ordered execution validates the speculative identity and publishes the load
   and consumer results in program order.

The canonical hart, FP registers, PC, rounding mode, and `fflags` remain
unchanged during speculative service.

### Failure and Cleanup

The path fails closed for absent bytes, malformed response width, missing or
duplicate response target, wrong class, wrong register, absent writeback
reservation, or unsupported FP instruction shape. Such state never produces a
ready candidate.

Load retry, terminal failure, trap, redirect, queue removal, mode disable, or
validation mismatch removes or invalidates every dependent speculative suffix
through existing sequence-owned cleanup. A retry may reissue the load but must
not leak a duplicate request, stale forwarded value, dependency wake, or
writeback reservation.

Wrong-class WAW cases remain typed. An integer x4 producer cannot satisfy f4,
and an FP f4 producer cannot satisfy x4. Nearest-older matching continues to
apply when multiple FP producers target the same register.

## Checkpoint and Compatibility

Live FP-load, queue, dependency, speculative execution, and writeback state
remain transient. Checkpoint capture and detailed-to-timing handoff continue to
reject while any of that authority is non-quiescent and must create no output
artifact or partial transfer.

Drained restore reconstructs empty transient state and uses existing committed
FP architectural checkpoint data. This increment does not serialize forwarded
values or in-flight memory ownership:

- O3RT remains v23;
- O3PS remains v2; and
- O3DH remains v7.

No compatibility decoder or version bump is justified.

## Representative Matrix

### CPU RED/GREEN Tests

Start with failing tests for:

- typed FLW and FLD destination retention in predicted and runtime windows;
- completed FLW materializing an exact NaN-boxed `FloatRegisterWrite`;
- completed FLD materializing an exact 64-bit `FloatRegisterWrite`;
- dependency blocking before response and before writeback admission;
- wakeup exactly at admitted memory-result writeback;
- width-one collision delay and width-two exact-fit admission;
- speculative single- and double-precision arithmetic with no canonical-state
  mutation;
- nearest-older FP WAW and two-producer FP fan-in;
- wrong-class source rejection and integer-only pending-address fallback;
- retry/failure recursive suffix invalidation and reservation cleanup; and
- the retained vector-load producer boundary.

The RED tests must fail because the current implementation forgets typed
memory-result destinations or cannot materialize an FP completed-load value,
not because a fixture or helper is missing.

### Real CLI Rows

Use real ELF fixtures through `env!("CARGO_BIN_EXE_rem6")` and
`rem6 run --execute`.

| Route | Issue width | Writeback width | Dependency shape | Required evidence |
| --- | ---: | ---: | --- | --- |
| direct | 1 | 1 | `FLW -> FMUL.S -> FADD.S` | response then admitted wake, serial issue, exact `10.0f` bytes `00002041` |
| direct | 2 | 2 | `FLD -> FMUL.D -> FADD.D` plus one colliding independent FP row | exact-fit writeback, dependency still blocks same-cycle bypass, exact `10.0` bytes `0000000000002440` |
| cache/fabric/DRAM | 4 | 1 | table-driven FLW and FLD chains | exact queue/ROB/LSQ identities, delayed width-one wake, cache/transport/fabric/DRAM activity, exact final bytes |

Each completed row asserts:

- one real load request and response for the FP head;
- load issue, response, raw-ready, admitted-writeback, and commit ticks;
- consumer queue residency before the response;
- typed producer sequence linkage and dependency wake timing;
- Float issue-class selection and configured width behavior;
- oldest-first architectural commit;
- no result bytes or FP architectural publication before admission; and
- exact stored final bytes plus zero sticky flags for the selected exact
  arithmetic inputs.

Hierarchy rows complement rather than replace direct rows. Direct rows prove
writeback and issue-width boundaries without hierarchy noise; hierarchy rows
prove the same dependency path is connected to cache, transport, fabric, and
DRAM activity.

### Boundary Rows

Add or retain exact executable negatives for:

- PMP-denied or otherwise terminal FP load failure publishing neither the load
  value nor its dependent result and leaving no stale queue wake;
- integer-load-to-FP and FP-load-to-integer class mismatch;
- FP load values remaining unavailable to pending-address materialization;
- vector load to vector consumer remaining outside live forwarding;
- conversion, comparison, move, classification, and CSR-sensitive FP forms
  following normal execution;
- live checkpoint rejection before and after response/writeback admission;
- detailed-to-timing handoff rejection with no transfer artifact;
- successful drained restore with an empty transient queue; and
- timing mode producing identical architectural bytes without O3 queue,
  dependency, writeback, or debug surfaces.

## Telemetry

Reuse existing memory-result, issue-queue, dependency, writeback-port, ROB/LSQ,
and FP issue-class telemetry. Do not add a counter merely to label FP-load
forwarding.

JSON and debug assertions correlate the same producer and consumer sequences
across:

- memory request and response events;
- queue enqueue, blocked, wake, selected, issued, and removed events;
- writeback admission and slot;
- speculative result timing; and
- ordered commit.

Existing text and `m5_dump_stats` queue/writeback totals must remain reconciled
where the fixture already emits those surfaces.

## Source Policy and File Boundaries

Keep memory response interpretation in `rem6-isa-riscv` and
`riscv_data_completion`; the O3 queue consumes the existing typed writeback
target and must not decode bytes independently.

Keep typed memory-result inventory in focused memory/window owners. Keep
completed source-value materialization beside existing live source lookup.
Keep instruction classification in `o3_live_compute_operands.rs`, queue
validation in `o3_runtime_issue/queue/compute.rs`, and speculative value
application in issue service.

Place CPU tests in focused children and add dedicated CLI fixture, positive,
and boundary children under the existing persistent-IQ family. Do not grow a
capped facade or shared fixture with unrelated FP-load policy.

Source-policy tests must lock:

- one typed memory-result destination inventory;
- no parallel integer-only dependency inventory in the memory-result window;
- exactly the existing integer and floating-point forwarded-value variants;
- integer-only pending-address fallback;
- symmetric single/double scalar FP arithmetic classification;
- response-owned FP value conversion and speculative-clone-only application;
- unchanged O3RT/O3PS/O3DH versions;
- top-level CLI matrix and boundary anchors; and
- honest ledger wording and unchanged score.

Do not edit or commit anything under `temp/`. Do not build or run the gem5
reference tree. Every Cargo command, including formatting, uses
`TMPDIR=$PWD/target/tmp`.

## Ledger Treatment

The CPU evidence may claim bounded scalar FLW/FLD completion feeding supported
single/double FP arithmetic through the persistent live issue queue across
direct and cache/fabric/DRAM routes and issue widths 1, 2, and 4.

It must continue to list:

- broader FP load shapes, conversions, comparisons, moves, classification,
  dynamic-CSR and status-sensitive chains;
- true vector-register producers and destinations plus LMUL, mask, tail, v0,
  vector-load, and VCSR-aware forwarding;
- arbitrary or unbounded mixed dependency graphs;
- positive system issue rows;
- a general load/store queue scheduler and dependent stores/atomics;
- checkpoint-restorable live IQ and transport state; and
- a general O3 engine.

This narrows named FP-load and double-precision gaps but does not complete a
new checklist item. The component remains `8 of 10`, 80% raw, and capped at
74% representative.

## Verification and Closeout

Development follows RED/GREEN TDD. Each production change follows a focused
failing test that is observed to fail for the intended missing behavior.

Closeout runs formatting, focused CPU tests, source policy, exact CLI matrix
and boundary tests, affected crate suites, and the full workspace suite. Any
full-suite failure must be reproduced against commit `3524f384` before it is
classified as pre-existing.

Before push, a high-intensity read-only reviewer audits typed ownership,
response and writeback timing, rollback and replay, canonical-state isolation,
real CLI wiring, checkpoint compatibility, source-policy integrity, dead code,
test strength, and ledger honesty. Findings are fixed and reverified before
the implementation branch is pushed.
