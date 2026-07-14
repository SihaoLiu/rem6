# RISC-V O3 Nested Control Window Design

## Status

Approved for implementation on 2026-07-13 under the active
`temp/improve-rem6-0.md` continuation contract.

This document is an implementation design, not a migration-progress ledger. The
only progress authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

Detailed RISC-V execution now supports one bounded predicted control edge while
an older independent scalar load is outstanding. The current four-row window
can contain one load, one direct conditional branch, and two predicted-path
scalar descendants. The branch and descendants issue transiently, retain FU
latency, occupy ROB and rename state, transfer through the existing execution
mode handoff, and either retire in order or disappear on misprediction.

That increment deliberately rejects a second branch. The remaining gap is not
just branch-count breadth: nested control requires the runtime to distinguish
two rollback boundaries. An older misprediction must remove the younger branch
and everything below it, while a younger misprediction must preserve the older
branch and remove only its own descendants. This distinction is necessary
before adding speculative memory descendants or broader control classes.

## Alternatives Considered

### Bounded nested direct-conditional window

Keep the four-row limit and admit exactly this representative shape:

`scalar load, branch 1, branch 2, scalar descendant`

Both branches reuse existing prediction records and direct-conditional
execution. The runtime records immediate control ownership as branch 2 owned by
branch 1 and the scalar descendant owned by branch 2.

This is the selected approach. It directly addresses the first named CPU O3
gap, exercises nested repair and rollback, and reuses the current predictor,
ROB, rename, transient execution, transfer, and checkpoint boundaries.

### Speculative memory descendant

Allow a predicted path to contain a load or buffered store. This would add more
mixed memory/control breadth, but it requires a new LSQ admission contract for
control-dependent rows and a precise rule for wrong-path transport visibility.
It should follow nested control ownership rather than be combined with it.

### Scoped issue-slot contention

Serialize dependency-ready live rows through a bounded issue-width policy. This
attacks an important scheduler gap, but it changes timing for every current
scalar live window. A useful version should define a reusable scheduling
authority rather than insert a local tick increment into one loop.

### Restorable transport ownership

Restore one in-flight scalar load by reconstructing the execution event,
outstanding data record, request allocator, O3 live-memory row, and transport
response path. Even the narrow direct-memory case crosses checkpoint, CPU,
system, and transport ownership and has duplicate-response hazards. It remains
a separate increment.

## Ledger Target

The target is `CPU Execution Models`, currently 8/10 raw and capped at 74%
representative.

This increment narrows the explicit `multi-branch` gap by proving two nested
direct conditional branches under one live O3 authority. It does not provide a
general issue queue, arbitrary branch depth, indirect control, speculative
memory descendants, restorable transport ownership, or a general O3 engine.

The migration update must therefore:

1. Keep both unchecked CPU checklist items unchecked.
2. Keep the CPU score at 74% representative.
3. Record only the executable two-branch matrix delivered here.
4. Preserve the exact 1,200-line ledger source-policy boundary.

## Goals

1. Admit two independent direct conditional branches behind one outstanding
   scalar load without increasing the four-row live-window limit.
2. Follow each branch's already-recorded predicted PC exactly once.
3. Record branch 2 as control-dependent on branch 1 and the scalar descendant
   as control-dependent on branch 2.
4. Issue both branches and the scalar descendant transiently before the older
   load responds in the independent representative row.
5. Preserve normal FU timing, fetch identity, ROB residency, rename ownership,
   architectural invisibility, and ordered commit.
6. On an older-branch misprediction, discard branch 2 and every younger live
   row and transient execution.
7. On a younger-branch misprediction, preserve branch 1 and discard only rows
   younger than branch 2.
8. Suppress nested-path admission when branch 2 depends on the unresolved load.
9. Preserve current execution-mode handoff, checkpoint rejection, drained
   restore, timing-mode suppression, and split-fetch identity behavior.
10. Prove the behavior through real `rem6 run --execute` CLI tests on direct
    and cache/fabric/DRAM routes.

## Non-Goals

1. Do not support a third live branch.
2. Do not support indirect branches, `JAL`, `JALR`, calls, returns, traps,
   interrupts, or system events in the nested window.
3. Do not admit memory, floating-point, vector, atomic, or system descendants.
4. Do not increase the four-row live-window limit.
5. Do not add an issue-width configuration or new scheduler statistics.
6. Do not create a second branch predictor, prediction record, history stack,
   redirect path, or repair mechanism.
7. Do not change the v7 live-data handoff schema.
8. Do not make live scalar-memory handoff restorable.
9. Do not claim a general O3 engine or change the CPU checklist score.

## Ownership Model

### Admission policy

`riscv_o3_window_policy.rs` owns the bounded shape. It replaces the current
single open-control boolean with a small control-depth model. The first two
eligible direct conditionals may open predicted paths; a third control row or
any unsupported descendant closes admission.

Unresolved register destinations remain authoritative. A branch whose source
depends on the outstanding load or an unavailable speculative producer remains
terminal and cannot open another predicted path.

### Prediction and fetch

The existing branch speculation map remains the sole authority for predicted
direction, target, selected predictor history, and repair. Detailed O3 fetch
looks up the matching record for each admitted branch and follows its recorded
predicted PC. It must not call prediction twice or synthesize a target from the
decoded immediate.

Nested rows require `--riscv-branch-lookahead 2` in the CLI matrix. The option
widens only the existing fetch speculation budget; it does not change the O3
four-row limit.

### Live control dependencies

`O3RuntimeState::live_control_dependencies` continues to map a live row to its
immediate controlling branch sequence:

1. `branch_2_sequence -> branch_1_sequence`
2. `descendant_sequence -> branch_2_sequence`

The map is not a second predictor history. It is transient execution ownership
used to gate issue, validate producer chains, and select the rollback suffix.

### Architectural execution

Normal `riscv_execute.rs` retirement remains the sole authority for register
writes, branch resolution, predictor training and repair, redirects, committed
instruction counts, and final O3 trace events. Early branch execution remains
an unpublished validation record.

### Transfer and checkpointing

The existing v7 live-data handoff carries the outstanding scalar load and three
generic younger ROB rows. Both branch rows have no rename destination; the
scalar descendant retains its rename entry. Transient branch execution records
are recomputed or validated through existing transfer behavior and do not add a
new serialized chunk.

A live nested window remains non-restorable and must reject checkpoint capture.
A drained checkpoint contains no live control dependency or speculative
execution state and restores normally.

## Admission Rules

The representative live window is exactly:

1. one cacheable scalar load head,
2. one independent direct conditional branch,
3. one independent direct conditional branch on branch 1's predicted path,
4. one eligible scalar integer descendant on branch 2's predicted path.

A direct conditional is eligible when:

1. it is `BEQ`, `BNE`, `BLT`, `BGE`, `BLTU`, or `BGEU`,
2. its nonzero sources are available from committed state or valid earlier
   speculative scalar producers,
3. no source depends on the unresolved scalar load,
4. a matching existing branch speculation record identifies its predicted PC,
5. fewer than two control rows have already been admitted, and
6. total resident rows remain at or below four.

After branch 2, the final descendant is eligible only when it satisfies the
current predicted scalar descendant allowlist and lies on branch 2's selected
predicted path. A third branch, memory operation, unsupported instruction, or
path mismatch terminates admission.

## Early Execution Flow

1. The scalar load enters the existing live memory lifecycle and allocates one
   ROB and one LSQ row.
2. Detailed fetch-ahead decodes branch 1 while the load remains outstanding.
3. The existing predictor record selects branch 1's predicted PC.
4. The runtime stages branch 1, executes it on a cloned hart, and records its
   fetch identity, issue tick, resolved next PC, and sequence.
5. Fetch-ahead follows branch 1's recorded predicted PC and admits branch 2.
6. Branch 2 records branch 1 as its control producer, executes only after a
   valid branch 1 record exists, and follows its own recorded predicted PC.
7. The scalar descendant records branch 2 as its control producer and uses the
   existing speculative scalar execution and FU timing path.
8. Normal retirement later executes the load, branch 1, branch 2, and the
   descendant architecturally in program order.

No architectural register or memory effect is published by early execution.
The independent matrix requires both branches and the descendant to issue
before the older load response, but does not introduce a new issue-width claim.

## Resolution and Rollback

### Correct branch 1 and correct branch 2

1. Branch 1 validates its exact instruction and complete consumed-fetch vector.
2. Branch 1 validation removes only its producer edge from branch 2; branch 2's
   ownership of the scalar descendant remains intact.
3. Branch 2 validates its exact instruction and complete consumed-fetch vector.
4. Branch 2 validation releases the scalar descendant for normal retirement.
5. All four rows retain ordered commit and the descendant publishes once.

### Branch 1 misprediction

1. Existing predictor repair and redirect logic resolves branch 1.
2. The runtime discards every live row with sequence greater than branch 1.
3. Branch 2, the scalar descendant, their control edges, speculative execution
   records, rename entries, and wrong-path fetches disappear.
4. The resolved branch 1 target executes exactly once.

### Branch 2 misprediction

1. Branch 1 has already validated as correct and remains committed evidence.
2. Existing predictor repair and redirect logic resolves branch 2.
3. The runtime discards only rows with sequence greater than branch 2.
4. The scalar descendant and its rename/speculative state disappear.
5. The resolved branch 2 target executes exactly once.

The two cases must be asserted separately. A cleanup implementation that always
discards from the oldest live branch is incorrect.

## Dependency Suppression

The required negative row makes branch 2 read the unresolved load destination.
Branch 1 may issue and open its predicted path, but branch 2 remains a terminal
resident row:

1. branch 2 has no early execution record,
2. no descendant row is admitted,
3. no descendant rename entry exists,
4. normal branch 2 execution waits for the load response, and
5. final architectural behavior remains correct.

A focused policy test also proves a third direct conditional is rejected after
two admitted control rows.

## Fetch Identity

Each branch and scalar descendant must retain its complete consumed-request
vector. For a split 32-bit fetch, readiness and retirement validation compare
both request IDs in order. A replacement suffix with identical instruction
bytes must not reuse an older branch execution record.

The nested slice adds one branch-2 split-fetch regression because a stale inner
branch identity could otherwise preserve an invalid descendant chain after the
outer branch validates.

## Execution-Mode Transfer

The direct host-switch row schedules detailed-to-timing transfer after both
branches and the descendant have issued but before the older load responds.

The transfer must expose:

1. four ROB rows and one LSQ row,
2. three generic younger rows after the scalar load,
3. no rename destinations for either branch,
4. one live scalar rename destination for the descendant,
5. the existing v7 live-data handoff,
6. `restorable = false`, and
7. baseline-equivalent issue, writeback, branch outcome, squash, and commit
   timing after inherited rows drain.

The first instruction beyond inherited authority executes in timing mode
without a new O3 row.

## Checkpoint Behavior

1. A checkpoint requested while the transport-backed nested window is live is
   rejected before checkpoint metadata commits.
2. A checkpoint after correct drain or misprediction cleanup succeeds.
3. The drained checkpoint contains no live-data handoff, control dependency,
   speculative execution, or live rename state.
4. Restore from the drained checkpoint does not recreate either branch window.

## Test-First Matrix

### Focused CPU tests

1. Policy admits two direct conditionals and one scalar descendant within four
   rows.
2. Policy rejects a third direct conditional and every memory descendant.
3. A load-dependent branch 2 remains terminal.
4. Staging records `branch 2 -> branch 1` and `descendant -> branch 2`.
5. Branch 2 cannot issue without a valid branch 1 execution record.
6. Correct branch 1 validation preserves branch 2's descendant ownership.
7. Older rollback removes branch 2 plus its descendant.
8. Younger rollback preserves branch 1 and removes only the scalar descendant.
9. Branch 2 split-fetch suffix replacement invalidates its speculative record.

### Top-level CLI rows

1. Direct correct/correct detailed row: both branches and the scalar descendant
   issue before the older load response; all four rows commit in order; exact
   final registers and branch outcomes match.
2. Cache/fabric/DRAM branch-1-misprediction row: the resident snapshot contains
   all four rows before repair; branch 2 and descendant disappear; the resolved
   outer target executes once; hierarchy resources are active.
3. Direct branch-2-misprediction row: branch 1 remains a correct committed event;
   branch 2 records one squash; only the descendant disappears; the inner
   resolved target executes once.
4. Direct branch-2-load-dependency row: branch 1 issues early, branch 2 waits for
   the load response, and no scalar descendant becomes resident.
5. Direct detailed-to-timing switch row: v7 transfers four ROB rows and one LSQ
   row and preserves baseline timing without a schema change.
6. Direct checkpoint boundary row: live capture rejects; drained capture and
   restore contain no stale nested control authority.
7. Direct timing row: architecture matches while O3 runtime, debug events, and
   gem5-style O3 aliases remain absent.

## Source Policy

Production changes must remain in focused owners:

1. bounded admission in `riscv_o3_window_policy.rs`,
2. transient control execution and dependency ownership in
   `o3_runtime_control_window.rs`,
3. fetch-path selection in `riscv_fetch_ahead/detailed_o3.rs`, and
4. architectural resolution in `riscv_execute.rs` only where current generic
   rollback hooks need correction.

`o3_runtime_live_window.rs` is already near its source ceiling. New nested
control logic must not be accumulated there. If CLI evidence would push
`predicted_control.rs` near its child-module limit, split nested rows into a
focused `predicted_control/nested.rs` child.

The implementation must preserve:

1. exact 1,200-line migration ledger length,
2. existing schema-v1 through schema-v7 decode compatibility,
3. current source-policy anchor coverage for new top-level tests,
4. all existing one-branch predicted-control behavior, and
5. no committed paths under `temp/`.

## Verification

Required focused verification:

```bash
cargo test -p rem6-cpu nested_control -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested:: -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Run every new module-qualified CLI test with `--exact` so each command proves it
selected one test. Before push, run the complete workspace test suite and an
xhigh read-only whole-diff review. Fix and reverify every finding.

## Completion Boundary

This increment is complete only when real CLI evidence distinguishes correct
nested retirement, outer rollback, inner rollback, dependency suppression,
mode-transfer continuity, checkpoint cleanup, split-fetch identity, and timing
suppression. It remains incomplete if evidence is unit-only, if branch 1 and
branch 2 share an ambiguous rollback boundary, if prediction is invoked twice,
if stale inner-branch state survives repair, or if the migration score rises.
