# RISC-V Vector Architectural Checkpoint Design

## Context

RISC-V core checkpoints currently persist the program counter, integer and
floating-point registers, PMP state, hart run state, pipeline state, branch
predictors, and the drained O3 runtime. They do not persist architectural
vector state. A checkpoint taken after a committed vector instruction can
therefore restore successfully while silently replacing vector registers,
`vl`, `vtype`, `vxrm`, and `vxsat` with whatever values happen to be present in
the destination core.

This is an architectural continuity defect, not an O3 transient-state feature.
The next FP/vector live-forwarding increment will make more vector state visible
before retirement, but checkpoint capture remains restricted to quiescent,
drained cores. This increment closes the committed vector-state gap first.

## Ledger Boundary

The work strengthens the existing RISC-V checkpoint evidence under
`Stats, Debug, Trace, and Checkpoint` and removes committed vector state from
the checkpoint omissions. It does not make live issue queues or transport
checkpoint-restorable, add vector-destination O3 execution, or broaden the CPU
execution-model score. The CPU score remains 8/10 and 74% representative.

The migration ledger remains exactly 1,200 lines. Its existing evidence and
gap text will be consolidated in place rather than expanded with new rows.

## Considered Approaches

### Implement FP/vector live forwarding first

This directly attacks an O3 execution-model gap, but leaves a checkpoint that
can lose already committed vector results. It would improve transient execution
while retaining a more fundamental architectural correctness hole.

### Combine checkpoint continuity and live forwarding

Both changes touch vector ownership, but their invariants are independent.
Checkpoint continuity concerns committed architectural state and manifest
compatibility; forwarding concerns rename bindings, producer readiness, and
speculative clones. Combining them creates a large cross-crate review surface
and obscures failures at the retirement boundary.

### Stage architectural vector checkpointing first

This is the selected approach. The current increment establishes one complete,
versioned authority for committed vector state. Typed FP/vector live forwarding
follows as a separate increment and can rely on checkpoint continuity already
being correct.

## Architectural State Owner

`rem6-isa-riscv` will expose `RiscvVectorArchitecturalState`, containing:

- `RiscvVectorConfig`, which owns `vl` and `vtype`;
- `RiscvVectorFixedPointState`, which owns `vxrm` and `vxsat`; and
- all 32 architectural vector registers, each exactly 16 bytes for the current
  128-bit VLEN implementation.

The value is a complete projection of the vector fields already owned by
`RiscvHartState`; it is not a second mutable authority. Hart snapshot and
restore methods copy between the value and the existing fields. The default
value exactly matches a newly constructed hart: invalid vector configuration,
round-nearest-up, clear saturation, and zeroed vector registers.

`RiscvCore` exposes snapshot and restore methods that hold the core lock once.
Restore updates the real hart as one operation and synchronizes the checker
hart once. Checkpoint code does not loop through public per-register mutations
that would expose intermediate checker states.

## Checkpoint Wire Format

Current captures emit a one-byte `riscv-state-version` marker with value `1`
and one `vector-state` chunk per RISC-V CPU component. The marker identifies
which architectural chunks are required; the vector chunk remains the sole
authority for vector values. Its fixed version-1 little-endian layout is:

| Offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | payload version, currently `1` |
| 1 | 4 | `vl` |
| 5 | 8 | `vtype` |
| 13 | 1 | `vcsr` bits (`vxrm[2:1]`, `vxsat[0]`) |
| 14 | 512 | vector registers `v0` through `v31` |

Version 1 is therefore exactly 526 bytes. Register bytes retain architectural
little-endian lane order and are concatenated in register-index order.

The codec lives in `riscv_checkpoint/vector_state.rs`, separate from the root
checkpoint orchestration. Decode rejects an unknown version, any non-exact
payload length, and reserved high `vcsr` bits. It reconstructs fixed-point
state through the public CSR-bit behavior so the encoded representation remains
the architectural one.

No parallel `vl`, `vtype`, fixed-point, or per-register chunks are introduced.
The one chunk is the sole manifest authority for vector state.

## Compatibility

Legacy manifests have neither `riscv-state-version` nor `vector-state`.
That pair of absences decodes to `RiscvVectorArchitecturalState::default()`.
This is an intentional compatibility behavior change: the old restore path
left the destination core's pre-restore vector values untouched, so identical
legacy manifests could produce different state. The deterministic default
matches a reset hart and the existing missing-`fregs` compatibility policy.

Version 1 requires `vector-state`. A marker without the required chunk fails
with `MissingChunk`; a vector chunk without a marker and unknown marker values
also fail closed. Current capture always writes both chunks, so ordinary chunk
loss cannot silently masquerade as legacy compatibility. As with other
optional legacy chunks, simultaneous removal of both the generation marker and
the state chunk is indistinguishable from a genuine legacy manifest without an
authenticated global manifest generation; that broader format problem is out
of scope.

This increment does not change the O3 runtime payload version. Live issue,
writeback, and transport state remain subject to the existing drained capture
gate and live-data handoff rejection.

Unknown future vector-state payload versions fail closed. Future formats can
add a new decoder branch without changing the meaning of version 1.

## Capture And Restore Ordering

Capture first runs the existing quiescence validation, snapshots a complete
record, and writes the vector chunk beside the current architectural and
microarchitectural chunks.

Single-core restore decodes and validates the complete vector payload before
mutating the core. Vector state is applied only from the decoded record. Bank
restore decodes every CPU record before it begins applying records. A malformed
or missing version-1 vector chunk on CPU 1 therefore cannot partially restore
CPU 0.

The existing fallible PMP, predictor, pipeline, and O3 restore path stays
unchanged. Some of those components can still fail after earlier state has been
applied, so this increment does not claim general single-core or bank-wide
restore atomicity. Vector application occurs after those calls, ensuring an
unrelated late restore failure cannot leave newly applied vector state behind.

## Public Record Surface

`RiscvCoreCheckpointRecord` stores the decoded vector architectural state and
exposes a borrowed accessor. The legacy convenience constructor fills this
field with the architectural default, preserving existing fixtures and equality
semantics.

No CLI schema or stats path exposes raw register contents. Manifest component
counts stay unchanged, while chunk counts increase by two for newly captured
RISC-V cores; existing aggregate output derives those totals from actual
chunks.

## Evidence Matrix

Focused ISA and system tests cover:

- exact state projection and single-lock hart restore;
- exact version-1 bytes for config, fixed-point CSR state, low and high vector
  register indices;
- direct capture, mutation, and restore of all vector state classes;
- a restored vector register consumed by a real decoded vector instruction;
- legacy manifests without `vector-state` restoring architectural defaults;
- rejection of truncated payloads, unknown versions, and reserved `vcsr` bits
  without single-core mutation;
- multicore bank rejection when CPU 1 is malformed, with neither CPU restored;
- component-order capture and independent per-CPU vector values; and
- continued rejection of live O3 checkpoint state.

Representative hierarchy tests cover checkpoint capture/restore through the
system bank rather than only calling the codec. Existing host-action checkpoint
tests supply the CLI route and are updated only where exact chunk totals change.

## Source Policy

Source-policy coverage requires:

- one root state-version write, one root vector-state write, and paired legacy
  versus current decode policy;
- codec ownership in `riscv_checkpoint/vector_state.rs`;
- no split vector-state chunk literals in the root module;
- the record and record-parts structures to carry the complete architectural
  value;
- current capture to emit vector state; and
- the migration ledger to remain exactly 1,200 lines.

The root `riscv_checkpoint.rs` remains below the 1,800-line system source cap.
New behavior tests live in a focused child module instead of growing the
already large integration-test root.

## Exclusions

This increment does not add:

- checkpoint-restorable live issue queues, live writeback transfers, or live
  data transport;
- general transactional rollback for non-vector checkpoint restore failures;
- vector control/status state beyond fields implemented by the current hart;
- a variable VLEN checkpoint format;
- O3 FP/vector producer forwarding or vector-destination execution;
- vector floating-point execution support; or
- new user-facing checkpoint commands, JSON fields, stats, or debug schemas.

## Verification

Verification requires an observed RED/GREEN TDD boundary, focused ISA and
system tests, source-policy checks, all targets for affected crates, existing
checkpoint CLI suites, the full workspace, formatting, protected-path and
ledger checks, and an independent high-intensity read-only review before the
implementation branch is pushed.
