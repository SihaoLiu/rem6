# RISC-V O3 Mixed Control Window Design

## Status

Approved for implementation on 2026-07-13.

This document is an implementation design, not a migration-progress ledger. The
only progress authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

Detailed RISC-V execution can currently keep a bounded scalar-memory prefix in
the live O3 runtime while admitting younger scalar integer ALU rows. The runtime
owns the resulting ROB, LSQ, rename, dependency, issue, writeback, and ordered
commit evidence. It can transfer that authority across a detailed-to-timing
mode switch and rejects checkpoints while non-restorable scalar-memory work is
live.

The live window stops before every control-flow instruction. This leaves branch
resolution outside the same authority that owns the older memory and younger
ALU rows. Consequently, the current executable matrix does not prove that a
mixed LSQ/ROB/rename window can terminate at a branch, record branch repair and
squash evidence, suppress wrong-path side effects, survive mode handoff, and
drain without stale checkpoint state.

The existing pieces are deliberately conservative:

1. `RiscvScalarIntegerLiveWindow` admits only scalar ALU instructions.
2. `O3RuntimeState` can stage a live ROB row without a rename destination.
3. Live speculative execution is limited to scalar ALUs and rejects control
   flow, traps, system events, and memory operations.
4. Normal retirement already records branch prediction and O3 branch-event
   evidence before redirect cleanup discards stale live rows.
5. Scalar-memory handoff already counts generic younger ROB rows and transfers
   the O3 runtime snapshot together with live data ownership.

The design extends the first boundary without weakening the other four.

## Alternatives Considered

### Multi-hop trace-replay fabric matrix

Adding a two-hop cache/coherence/fabric/DRAM trace-replay route would improve
NoC integration evidence. It is useful, but CPU execution is the higher
priority in `temp/improve-rem6-0.md`, and the memory checklist would remain
incomplete after one additional route shape.

### Shared LSQ alias descriptor

Consolidating duplicated LSQ JSON, text-stat, and host-action alias mappings is
a valuable slop cleanup. It reduces drift risk but adds no new executable
simulation behavior. It remains the preferred follow-up cleanup after this
runtime slice.

### General branch speculation inside the live O3 window

Speculatively executing branches and admitting predicted-path descendants
would require a broader issue queue, checkpointed predictor history, speculative
architectural overlays, and rollback ownership. That is the eventual general
O3 boundary, not a safe extension of the current bounded window.

## Ledger Target

The target is `CPU Execution Models`, currently 8/10 raw and capped at 74%
representative.

This increment advances the unchecked general O3 ownership item by joining one
LSQ row, younger rename/FU rows, and terminal branch squash behavior under one
live runtime. It does not complete a general O3 engine or KVM-style fast
forwarding. The checklist and score therefore remain unchanged.

The migration ledger update must:

1. Record only the executable matrix delivered by this increment.
2. Keep the general O3 checklist item unchecked.
3. Keep the score at 74% representative.
4. Preserve the exact 1,200-line source-policy boundary.
5. Normalize the CPU section's `Next evidence` marker into an unambiguous field
   without duplicating progress prose.

## Goals

1. Admit a direct conditional branch as the terminal row of a bounded live
   scalar-memory O3 window.
2. Keep the branch in the ROB without allocating a rename destination.
3. Prevent all younger live-window admission after the terminal branch.
4. Preserve scalar ALU speculative issue and forwarding for rows before the
   branch.
5. Resolve and retire the branch through the normal architectural execution
   path only.
6. Record O3 branch, misprediction, repair, and squash evidence before redirect
   cleanup.
7. Suppress wrong-path architectural and memory side effects.
8. Transfer the mixed live window across a detailed-to-timing mode switch.
9. Reject checkpoint capture while the non-restorable live data window exists
   and allow capture after the mixed window drains.
10. Prove the behavior through real `rem6 run --execute` CLI tests.

## Non-Goals

1. Do not speculatively execute a branch on a cloned hart.
2. Do not admit predicted-target or fall-through descendants into the live O3
   window.
3. Do not add indirect branches, `JAL`, `JALR`, returns, traps, interrupts,
   system events, or memory operations as terminal rows.
4. Do not increase the four-row live-window limit.
5. Do not add a second branch predictor, redirect, or squash authority.
6. Do not make live scalar-memory handoff restorable.
7. Do not change timing-mode behavior or expose O3 aliases in timing mode.
8. Do not increase the CPU migration score or check the general O3 item.

## Ownership Model

### Window policy

`riscv_o3_window_policy.rs` owns which decoded instructions may enter the
bounded window. It classifies a supported direct conditional branch as an
admitted terminal row. Unsupported control flow remains rejected.

### Live ROB state

`o3_runtime_live_window.rs` owns allocation of the branch ROB row. The branch
has no physical destination and no staged rename mapping. Its sequence and PC
participate in normal ordered commit and handoff snapshots.

### Speculative ALU issue

`riscv_live_retire_window.rs` continues to execute only eligible scalar ALUs on
a cloned hart. A terminal branch has no speculative issue candidate and remains
unready until normal execution reaches it.

### Architectural branch resolution

`riscv_execute.rs` and the existing branch-retirement helpers remain the sole
authority for resolved target, predictor repair, pipeline redirect, O3 branch
statistics, and fetch discard. The new policy must not duplicate that logic.

### Mode transfer and checkpointing

The existing O3 runtime snapshot and scalar-memory handoff remain authoritative.
The implementation may strengthen validation for a terminal no-rename younger
row, but it must not add another handoff payload unless current typed state is
provably insufficient.

## Admission Rules

Add an explicit terminal-control classification to the scalar integer window.
The exact enum spelling may change, but these outcomes must remain distinct:

1. `AdmitContinue`: stage the scalar ALU and consider another younger row.
2. `AdmitStop`: stage a dependency-blocked scalar ALU and stop admission.
3. `AdmitTerminalControl`: stage the branch and stop admission.
4. `Reject`: do not stage the instruction.

A terminal control row is eligible only when all conditions hold:

1. The window has capacity for one more row.
2. The instruction is `BEQ`, `BNE`, `BLT`, `BGE`, `BLTU`, or `BGEU`.
3. The instruction has no architectural destination.
4. The existing scalar-memory prefix is valid and resident.
5. No younger live-staged row already follows the current scalar-memory tail.

Branch source registers may reference the older load or preceding ALU rows.
The branch is still admitted because it is not issued speculatively. Normal
ordered retirement naturally waits for the older rows and publishes their
architectural state before resolving the branch.

## Fetch And Staging Flow

The detailed fetch-ahead path follows these rules:

1. Existing scalar-memory and scalar-ALU rows fill the window as today.
2. If the next completed instruction is an eligible branch, classify it as the
   terminal row.
3. If the branch has not completed fetching yet, the existing next-PC candidate
   may issue that fetch while capacity remains.
4. Once the branch is decoded, return a blocked fetch-ahead decision so no
   instruction beyond it is admitted by this window.
5. Stage the branch ROB row and record its sequence among scalar-memory younger
   rows.
6. Skip speculative execution because the branch has no scalar-ALU issue
   candidate.

The branch row is initially unready. When normal execution reaches the branch,
`retire_live_staged_instruction` marks it ready, computes ordered commit, and
hands the event to the existing O3 statistics path.

## Redirect And Side-Effect Rules

The representative program uses a taken direct conditional branch whose
fall-through path contains a store sentinel.

Required behavior:

1. The terminal branch appears in the resident ROB before the older load
   response.
2. The fall-through fetch may exist as branch-prediction evidence, but it is not
   a live mixed-window row.
3. Branch retirement records the resolved direction and any misprediction or
   squash counters before live-row cleanup.
4. Redirect cleanup discards fall-through fetch and pipeline state.
5. The wrong-path store emits no data request, no data completion, and no memory
   mutation.
6. The taken target executes normally after redirect and provides an
   architectural success witness.

If the branch prediction happens to match in a selected configuration, the test
must choose or train a deterministic predictor state that produces the required
single squash. Tests must not weaken the assertion to accept either outcome.

## Mode-Switch Behavior

The host-switch row schedules detailed-to-timing transfer after all four rows
are resident and before the scalar-load response.

The transferred evidence must report:

1. Four ROB rows and one LSQ row.
2. Three younger rows after the scalar-memory entry.
3. Rename pressure for the load and two ALU destinations, with no destination
   added by the branch.
4. A terminal branch row with no rename destination.
5. Non-restorable live data ownership.

After transfer, inherited O3 authority drains before normal timing execution
resumes. The branch must retain the same resolved target, squash count,
architectural result, and wrong-path suppression as the no-switch baseline.

## Checkpoint Behavior

Checkpoint behavior is intentionally asymmetric:

1. A checkpoint requested while the load, ALUs, and terminal branch are live is
   rejected by the existing non-quiescent/non-restorable gate before any
   checkpoint bank commits metadata.
2. A later checkpoint after the load responds, the branch redirects, and the
   mixed window drains succeeds.
3. The successful checkpoint contains no stale live ROB, LSQ, rename, branch,
   or scalar-memory-younger authority.

No new checkpoint schema is required unless a failing test proves the current
snapshot cannot encode the terminal no-rename row.

## Test-First Matrix

Implementation begins with failing tests in this order.

### Focused CPU tests

1. Policy admits each direct conditional branch as terminal and rejects
   unconditional, indirect, trap, system-event, and memory instructions.
2. Live staging creates a no-rename branch ROB row and stops before a fifth row.
3. A load-dependent branch remains unissued until normal retirement.
4. Redirect cleanup records the branch event before removing stale younger
   authority.

### Top-level CLI tests

1. Direct detailed run: four resident rows, one LSQ row, ordered commit, one
   branch squash, target-path success, and wrong-path store suppression.
2. Cache/fabric/DRAM detailed run: the same architectural and O3 evidence plus
   nonzero cache, data transport, fabric, and DRAM activity.
3. Tick-limited hierarchy run: resident ROB PCs are exactly load, ALU, ALU,
   branch before the load response.
4. Timing-mode negative: architecture matches, but no O3 runtime, debug rows, or
   gem5 O3 aliases are emitted.
5. Detailed-to-timing switch: transferred counts and final branch evidence
   match the detailed baseline.
6. Checkpoint boundary: live capture rejects before metadata commit; post-drain
   capture succeeds with empty live O3 state.

Tests must assert exact memory bytes, final registers, branch event counts,
ROB/LSQ/rename counts, issue/writeback/commit ordering, data and memory traces,
resource activity, and checkpoint or transfer artifacts. Committed instruction
count alone is insufficient.

## Failure Handling

1. If a branch cannot be represented by the current O3 snapshot, fail the
   focused test before changing a serialized schema.
2. If branch retirement clears the row before statistics observe it, reorder
   the existing retirement operations; do not introduce a second branch event.
3. If a mode switch loses instruction identity, strengthen the existing runtime
   authority rather than encoding branch-specific data in the scalar-memory
   handoff.
4. If checkpoint capture writes any bank before rejecting the live window, fix
   the capture preflight boundary.
5. If the wrong-path store reaches transport, fix admission or redirect cleanup;
   do not hide it by changing the fixture address or assertion.

## Expected Files

Production changes should remain focused in:

1. `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
2. `crates/rem6-cpu/src/o3_runtime_live_window.rs`
3. `crates/rem6-cpu/src/riscv_live_retire_window.rs`
4. `crates/rem6-cpu/src/riscv_execute.rs` only if retirement ordering needs a
   correction
5. `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs` only if validation
   rejects the existing generic row shape

CLI evidence belongs in a focused
`crates/rem6/tests/cli_run/m5_host_actions/o3/lsq_fu_branch.rs` module wired
from the existing O3 test facade.

## Verification

Focused verification:

```text
cargo test -p rem6-cpu scalar_memory_prefix_admits_terminal_branch
cargo test -p rem6-cpu terminal_branch
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_load_alu_branch
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_o3_mixed_load_alu_branch
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_branch_checkpoint
```

Completion verification:

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

The increment is complete only when the full matrix passes, the migration
ledger remains honest and mechanically auditable, and a read-only review finds
no unresolved correctness or abstraction issues.
