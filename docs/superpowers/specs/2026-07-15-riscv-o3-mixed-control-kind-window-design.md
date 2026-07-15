# RISC-V O3 Mixed Control-Kind Window Design

## Status

Approved as the next bounded increment under `temp/improve-rem6-0.md`.

This document is an implementation design. The migration authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

The detailed RISC-V O3 path can keep one delayed scalar-memory row and up to
three younger rows resident in a four-entry ROB. Existing executable evidence
covers scalar ALU descendants and one, two, or three direct conditional control
rows. It also covers scoped issue and writeback contention, mode transfer,
checkpoint rejection while the window is live, and timing-mode suppression.

The remaining CPU ledger gaps explicitly include indirect or unconditional
deeper control chains and arbitrary mixed memory/control windows. The current
implementation cannot use the existing direct-jump frontend support inside the
live O3 window because policy, runtime issue, issue classification, control
dependency tracking, and redirect cleanup all hard-code direct conditional
branches.

The code already has a complete branch taxonomy in `riscv_branch_kind.rs` and
generic frontend speculation for `JAL` and `JALR`. This increment removes the
conditional-specific O3 abstraction leak and proves a mixed control-kind chain
through the real CLI.

## Alternatives

### Fourth direct-conditional row

Increasing branch lookahead and ROB depth would add another point on the
existing depth axis. It would require widening the bounded window without
closing the control-kind gap.

### Restorable live transport

Restoring an in-flight memory transport would advance a larger CPU/checkpoint
boundary, but it spans CPU, system, transport, checkpoint schemas, ownership,
and duplicate-response prevention. It is not a coherent companion to the
control-specific cleanup.

### Mixed no-link control kinds

Generalize the live control descriptor and execute one delayed load followed by
no-link direct unconditional, direct conditional, and no-link indirect
unconditional controls. This is selected because it crosses a new matrix axis,
reuses existing prediction authority, and removes duplicated control matching.

## Ledger Target

The target is `CPU Execution Models`, currently 8 of 10 items with executable
evidence, 80% raw, capped at 74% representative.

This increment narrows the open indirect/unconditional nested-control gap. It
does not deliver a general O3 engine or KVM-style fast forwarding, so the score
and checklist remain unchanged.

The ledger update must preserve exactly 1,200 lines and replace existing CPU
prose rather than append a duplicate progress narrative.

## Goals

1. Admit `JAL rd=x0` as a no-link direct unconditional live control.
2. Admit `JALR rd=x0, rs1!=x1/x5` as a no-link indirect unconditional live
   control when its target source is architecturally stable.
3. Mix those controls with existing direct conditional controls in the same
   bounded scalar-memory window.
4. Keep every supported control in the ROB without a rename destination or
   writeback-port reservation.
5. Classify all supported live controls as branch issue operations.
6. Preserve correctly predicted descendants across direct and indirect jumps.
7. Discard only younger control descendants when a live control repairs.
8. Keep branch prediction, BTB, statistics, and architectural execution in
   their existing owners.
9. Prove no link-register mutation, wrong-path memory access, or timing-mode O3
   leakage.
10. Preserve mode-transfer and checkpoint boundaries.

## Non-Goals

1. Do not support `JAL` or `JALR` with a nonzero destination.
2. Do not support direct or indirect calls, returns, coroutine RAS forms, or
   link-register writes.
3. Do not predict a `JALR` target from a source produced inside the live window.
4. Do not add forwarded frontend target computation.
5. Do not increase the four-row ROB window or branch-lookahead maximum of three.
6. Do not admit traps, interrupts, system events, memory descendants, atomics,
   CSR operations, float operations, or vector operations.
7. Do not add a new branch predictor, BTB, RAS, checkpoint, or handoff schema.
8. Do not raise the CPU migration score.

## Typed Control Authority

Replace `o3_direct_conditional_sources` with one live-control descriptor owned
next to the O3 operand helpers. The exact type name may vary, but it must return
both the `BranchTargetKind` and scalar source registers for exactly these forms:

1. `BEQ`, `BNE`, `BLT`, `BGE`, `BLTU`, and `BGEU` as
   `DirectConditional`.
2. `JAL rd=x0` as `DirectUnconditional` with no sources.
3. `JALR rd=x0, rs1!=x1/x5` as `IndirectUnconditional` with `rs1` as its
   source.

It must reject every destination-writing jump even when the destination is not
a conventional link register. It must also reject return-shaped `JALR` forms.
Consumers must use this descriptor instead of maintaining opcode allowlists.

## Admission Policy

`RiscvScalarIntegerLiveWindow` continues to own the four-row bound and the
three-control lookahead bound.

The window must distinguish two source sets:

1. unresolved scalar-memory destinations, which make any dependent control
   terminal;
2. all destinations produced inside the live window, which make a dependent
   `JALR` terminal because frontend target selection would otherwise read stale
   architectural state.

A terminal control is staged but opens no descendants. It may execute later
through normal forwarding once its source becomes ready.

An independent no-link `JAL` or `JALR` is an admitted predicted control. The
next fetch PC must come from the existing recorded branch speculation. Missing
prediction state blocks admission rather than guessing a target.

## Runtime Issue

The speculative issue kind becomes a generic control carrying its expected
`BranchTargetKind`. A supported control candidate:

1. has no rename destination;
2. receives scalar source forwarding through the existing dependency path;
3. consumes branch issue capacity but no scalar writeback slot;
4. rejects execution records with traps, system events, memory accesses, float
   writes, integer writes, or a different control kind.

Immediate control dependencies remain unchanged: each younger control or
descendant depends on the nearest older predicted control. Validating an older
control releases that dependency; repairing it discards only younger rows.

## Redirect Ownership

`riscv_execute.rs` must keep conditional predictor-training decisions separate
from live O3 control ownership.

For any supported live control:

1. a correct prediction with live descendants preserves the predicted path;
2. a repair discards descendants from that control sequence and preserves older
   rows;
3. a trap or unsupported redirect retains the existing broad cleanup behavior;
4. architectural branch resolution and statistics remain in the current
   retirement path.

This split is required because every `JAL` and `JALR` changes the architectural
next PC even when prediction was correct.

## Executable Program Shape

The positive representative uses:

1. a delayed cacheable scalar load;
2. `JAL x0` to a direct target;
3. a direct conditional branch with a deterministic prediction;
4. `JALR x0` through a stable non-link register to an indirect target.

The four rows fill the ROB. Target and fallthrough blocks contain distinct
register markers and stores so the tests can prove the exact path and suppress
skipped memory side effects.

The rollback representative makes the middle conditional branch repair while
the younger `JALR` is resident. The older direct `JAL` remains committed, while
the indirect control and its wrong-path effects disappear.

The dependency negative places the `JALR` target in a register produced by the
older load or a younger live scalar row. The `JALR` may be staged as terminal,
but no predicted descendant may enter the ROB before normal resolution.

## CLI Matrix

Place the new evidence in
`crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/mixed_kind.rs`.

The focused matrix covers:

1. direct detailed positive execution with exact four-row ROB and one-row LSQ
   residency before the load response;
2. cache/fabric/DRAM positive execution with matching architecture and nonzero
   cache, transport, fabric, and DRAM activity;
3. middle conditional repair that preserves the older `JAL` and suppresses the
   younger `JALR` plus wrong-path Data and Memory records;
4. producer-dependent `JALR` terminal behavior;
5. branch-lookahead-two suppression of the third control;
6. detailed-to-timing transfer with baseline-equivalent issue, writeback, and
   commit ticks;
7. live checkpoint rejection and drained checkpoint/restore with empty ROB and
   LSQ;
8. timing-mode architectural parity with no O3 runtime, debug, or gem5-style O3
   aliases.

Assertions must include branch kinds, no link writes, `x1`/`x5` stability,
exact memory bytes, branch issue timing, ordered commit, resource activity,
transfer/checkpoint artifacts, and runtime drain. Committed counts alone are
not sufficient.

## Focused Unit Tests

Tests must first fail for:

1. mixed conditional/`JAL x0`/`JALR x0` policy admission;
2. rejection of calls, returns, and destination-writing jump forms;
3. producer-dependent `JALR` terminal admission;
4. generic control candidate kind and no-destination shape;
5. branch issue-class reservation for direct and indirect unconditional rows;
6. correct predicted jump preserving descendants;
7. jump repair discarding only descendants;
8. frontend progression through recorded mixed-kind predictions.

## Source Policy

Source policy should enforce one live-control operand authority and prevent
conditional-only helper ownership from returning. New executable test anchors
must be added without expanding the migration ledger beyond 1,200 lines.

## Verification

Focused gates:

```text
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu riscv_fetch_ahead -- --nocapture
cargo test -p rem6 --test cli_run mixed_control_kind -- --nocapture
```

Completion gates:

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
cargo test --workspace --all-targets --quiet
git diff --check
```

The increment is complete only after the real CLI matrix passes, the ledger
remains mechanically honest, and a high-intensity read-only review finds no
unresolved ownership or correctness defect.
