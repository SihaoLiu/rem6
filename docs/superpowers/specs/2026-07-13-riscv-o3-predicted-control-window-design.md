# RISC-V O3 Predicted Control Window Design

## Status

Approved for implementation on 2026-07-13 under the active
`temp/improve-rem6-0.md` continuation contract.

This document is an implementation design, not a migration-progress ledger. The
only progress authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

Detailed RISC-V execution currently supports a bounded four-row live O3 window
whose head is scalar memory. The runtime owns the resident ROB and LSQ rows,
live rename mappings, speculative scalar-ALU execution, dependency forwarding,
FU timing, ordered commit, execution-mode transfer, and non-restorable
checkpoint rejection. A direct conditional branch may occupy the final row, but
it is deliberately terminal: it does not issue before normal architectural
retirement, and no predicted-path descendant enters the live O3 authority.

That boundary leaves three explicit CPU migration gaps joined together:

1. Branches are resident but do not issue while older independent memory waits.
2. Predicted-path descendants do not consume ROB, rename, dependency, or FU
   resources under the same authority.
3. A misprediction does not exercise rollback of live descendant rename and
   speculative execution state.

The existing branch-prediction subsystem already owns fetch steering,
speculative predictor history, BTB/RAS metadata, resolution, repair, and fetch
cleanup. This increment must consume that authority rather than create another
predictor or redirect mechanism.

## Alternatives Considered

### Bounded predicted control window

Extend the current scalar-memory window with one independently issuable direct
conditional branch and up to two predicted-path scalar integer descendants.
Reuse existing predictor records and current generic O3 snapshot/handoff state.

This is the selected approach. It directly removes the named branch-issue and
predicted-descendant gaps while staying within one representative 500-2000 line
increment.

### Connect the generic scoped issue scheduler

Wire `O3ScopedIssueScheduler` into live RISC-V execution and make width,
operation-class capacity, and writeback contention authoritative for all live
rows. This is the stronger long-term route to a general O3 engine, but it
changes scheduling ownership for existing scalar-memory and FU windows and is
too broad for the next increment.

### Restorable in-flight transport ownership

Checkpoint and restore a live scalar-memory request together with CPU, route,
cache, fabric, DRAM, and callback state. This improves checkpoint breadth but
does less for the immediate execution-model gap and crosses several components
whose snapshots do not currently share one replay contract.

## Ledger Target

The target is `CPU Execution Models`, currently 8/10 raw and capped at 74%
representative.

This increment advances the unchecked running-O3 item by proving real branch
issue, predicted-path descendant issue, ordered publication, and rollback under
one live ROB/LSQ/rename authority. It does not provide a general issue queue,
arbitrary control flow, arbitrary mixed memory/control windows, or KVM-style
fast forwarding.

The migration update must therefore:

1. Keep both unchecked CPU checklist items unchecked.
2. Keep the CPU score at 74% representative unless a separate whole-ledger
   review demonstrates that the cap classification itself should change.
3. Record only the executable matrix delivered here.
4. Preserve the exact 1,200-line ledger source-policy boundary.

## Goals

1. Issue an independent direct conditional branch before an older scalar load
   responds.
2. Use the existing recorded branch prediction to choose one bounded descendant
   path.
3. Admit up to two scalar integer descendants while keeping the total window at
   four rows.
4. Give descendant rows normal live rename, dependency forwarding, FU latency,
   issue, writeback, and ordered commit evidence.
5. Keep branch and descendant architectural effects unpublished until normal
   in-order retirement.
6. On a correct prediction, validate and reuse early issue timing without
   duplicating architectural writes.
7. On a misprediction, remove all younger ROB, rename, dependency, and
   speculative execution authority before fetching and executing the resolved
   target.
8. Preserve current execution-mode handoff, checkpoint rejection, and timing
   suppression behavior without a handoff schema change.
9. Prove the behavior through real `rem6 run --execute` CLI tests on direct and
   cache/fabric/DRAM routes.

## Non-Goals

1. Do not add indirect branches, `JAL`, `JALR`, returns, traps, interrupts,
   system events, or branch descendants containing memory operations.
2. Do not support more than one branch or more than two descendants in the live
   control window.
3. Do not increase the four-row total live-window limit.
4. Do not create a second branch predictor, prediction record, redirect path,
   or predictor checkpoint format.
5. Do not wire the generic scoped issue scheduler in this increment.
6. Do not make scalar-memory handoff restorable.
7. Do not introduce execution-mode handoff schema v8.
8. Do not claim a general O3 engine or change the CPU checklist score.

## Ownership Model

### Window policy

`riscv_o3_window_policy.rs` owns bounded admission. A scalar-memory window may
transition from straight-line scalar rows to one direct conditional branch and
then to its selected predicted path. The policy rejects a branch whose operands
depend on unresolved live destinations and rejects every unsupported descendant
class.

### Prediction and fetch path

The existing fetch-ahead speculation record remains the sole authority for
predicted direction, predicted target, selected-family speculative history, and
eventual repair. `riscv_fetch_ahead/detailed_o3.rs` may follow the recorded
predicted PC only after the branch is admitted. It must not call a second
predictor or mutate speculative history twice.

### Live O3 control state

A focused `o3_runtime_control_window.rs` module owns control-window candidate
construction, early branch execution metadata, descendant validation, and
rollback helpers. The existing O3 snapshot remains authoritative for ROB, LSQ,
rename, pending data, sequence allocation, and handoff serialization.

### Architectural execution

Normal `riscv_execute.rs` retirement remains the sole authority for
architectural register writes, branch resolution, predictor training/repair,
redirects, traps, and committed instruction accounting. Early execution records
are evidence and forwarding inputs only.

### Transfer and checkpointing

The current v7 live-data handoff transfers the scalar-memory row plus generic
younger O3 snapshot rows. Early branch/descendant execution state is transient
and may be deterministically recomputed after transfer. A live transport-backed
window remains non-restorable and rejects checkpoint capture before bank
metadata commits.

## Admission Rules

The representative live window has these shapes:

1. `load, branch, descendant, descendant`
2. `load, scalar ALU, branch, descendant`
3. Existing branch-free scalar-memory shapes

A branch is eligible for early issue only when all conditions hold:

1. It is `BEQ`, `BNE`, `BLT`, `BGE`, `BLTU`, or `BGEU`.
2. Both nonzero source registers are available from committed state or valid
   earlier speculative scalar rows.
3. Neither source depends on an unresolved live scalar load or an unissued live
   producer.
4. A matching existing branch-speculation record identifies its predicted PC.
5. No earlier branch already exists in the bounded window.

After the branch, a descendant is eligible only when:

1. Its PC is on the branch's selected predicted path.
2. It is an eligible scalar integer ALU instruction with a nonzero destination.
3. Its sources are available from committed state or valid earlier speculative
   rows.
4. It has no trap, system event, memory access, floating-point write, vector
   write, or control-flow effect.
5. Total resident rows do not exceed four.

The first rejection terminates descendant admission. No alternate-path row is
admitted speculatively.

## Early Execution Flow

1. A scalar load enters the existing live memory lifecycle and allocates its ROB
   and LSQ rows.
2. Detailed fetch-ahead decodes the independent branch while the load remains
   outstanding.
3. The existing predictor chooses the predicted PC and records speculation once.
4. The runtime stages the branch as a no-destination ROB row.
5. A cloned hart executes the branch using forwarded valid source values. The
   runtime records the branch execution, predicted PC, producer sequences, and
   issue tick without changing architectural state.
6. Fetch-ahead follows the recorded predicted PC and admits eligible scalar
   descendants until the four-row bound or the first unsupported row.
7. Descendants use the existing speculative scalar execution and forwarding
   path, including FU latency and dependent-source readiness.
8. Normal retirement later re-executes each instruction architecturally and
   validates the recorded execution identity before reusing its issue timing.

The branch issue tick must precede the older load response in the independent
rows. A dependent descendant may issue later according to its producer's ready
tick, but still before ordered commit when the matrix expects it.

## Resolution and Rollback

For a correct prediction:

1. Normal branch execution must match the recorded branch instruction, consumed
   fetch identity, resolved next PC, and prediction record.
2. The runtime validates the branch producer sequence for descendant chains.
3. Descendants retain their recorded issue timing and retire in program order.

For a misprediction:

1. Normal branch resolution and predictor repair run first through the existing
   authority.
2. The runtime discards every younger live ROB row, staged rename mapping,
   speculative execution record, dependency link, and scalar-memory-younger
   sequence at or beyond the branch boundary.
3. The committed rename map remains unchanged by discarded descendants.
4. Existing fetch cleanup removes outstanding wrong-path fetches.
5. The resolved target is fetched and executes exactly once through the normal
   path.

No wrong-path descendant may publish a register write, issue a data request,
mutate memory, or survive in a later checkpoint/handoff artifact.

## Dependency Suppression

The required suppression case is a branch that reads the unresolved load
destination. The branch may be fetched and may remain a terminal resident row,
but it cannot issue early and cannot open a predicted descendant path.

Required evidence:

1. No branch early-issue record.
2. No descendant ROB or rename row.
3. No descendant O3 issue/writeback event.
4. No wrong-path data request or architectural write.
5. Normal branch execution after the load response still preserves architecture.

This boundary prevents the new path from manufacturing branch operands or using
stale committed values.

## Execution-Mode Transfer

The direct host-switch row schedules detailed-to-timing transfer after branch
and descendants are resident and issued, but before the older load response.

The transfer must expose:

1. Four ROB rows and one LSQ row.
2. Three generic younger rows after the scalar-memory entry.
3. Rename pressure for descendant destinations and no destination for the
   branch.
4. The existing v7 scalar-memory handoff and non-restorable marker.
5. No new handoff chunk or schema field.

After transfer, inherited O3 rows drain with baseline-equivalent issue,
writeback, branch resolution, squash, and commit timing. The first instruction
beyond inherited authority executes in timing mode without a new O3 row.

## Checkpoint Behavior

1. A checkpoint requested while the transport-backed control window is live is
   rejected by the existing non-quiescent gate before checkpoint metadata is
   committed.
2. A checkpoint after correct-path drain or misprediction cleanup succeeds.
3. The drained checkpoint contains no stale branch, descendant, speculative
   execution, rename, or live-data authority.
4. Restore from the drained checkpoint does not recreate the discarded window.

## Test-First Matrix

### Focused CPU tests

1. Policy admits an independent direct conditional branch and follows exactly
   one predicted PC.
2. A load-dependent branch suppresses early issue and descendants.
3. Early branch execution accepts valid integer sources and rejects unresolved,
   trapping, memory, system, and unsupported control records.
4. Correct prediction validates the branch and preserves descendant issue
   chains.
5. Misprediction removes younger ROB, rename, dependency, and speculative
   execution state while preserving the committed rename map.
6. Existing branch-free scalar-memory and terminal-branch tests remain unchanged.

### Top-level CLI rows

1. Direct correct-not-taken detailed row: `load, branch, MUL, dependent ALU`;
   four ROB rows, one LSQ row, branch and MUL issue before load response, FU
   latency visible, ordered commits, exact final registers.
2. Direct trained correct-taken detailed row: a repeated branch predicts its
   target on the second iteration; target MUL and dependent ADD retain their
   pre-response issue/writeback timing without refetch identity drift.
3. Cache/fabric/DRAM taken-misprediction row: wrong-path descendants become
   resident and issue, then squash; target witness executes once; wrong-path
   register and memory effects are absent; all hierarchy resources are active.
4. Direct detailed-to-timing switch row: existing v7 transfer captures the four
   rows and preserves baseline timing without schema changes.
5. Direct checkpoint boundary row: live capture rejects; drained capture and
   restore contain no stale authority.
6. Direct timing-mode row: architecture matches while O3 runtime, debug rows,
   and gem5 O3 aliases remain absent.
7. Direct load-dependent-branch suppression row: no branch early issue or
   descendant admission before the load response.

## Source Policy

`o3_runtime_live_window.rs` is at its line ceiling. Control-specific production
logic must be extracted into `o3_runtime_control_window.rs`; focused tests must
live in separate test modules. Existing per-file ceilings may not be raised
merely to accommodate this increment.

The implementation must preserve:

1. Exact 1,200-line migration ledger length.
2. Existing schema-v1 through schema-v7 decode compatibility.
3. Current source-policy anchor coverage for new top-level tests.
4. No committed paths under `temp/`.

## Verification

Required focused verification:

```bash
cargo test -p rem6-cpu predicted_control -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_predicted -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_o3_predicted -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Before push, run the workspace test suite and an xhigh read-only whole-diff
review. Any review finding must be fixed and reverified before commit and push.

## Completion Boundary

This increment is complete only when the real CLI matrix proves independent
branch issue, correct-path descendant issue, misprediction rollback, dependency
suppression, mode-transfer continuity, checkpoint cleanup, and timing-mode
suppression. It remains incomplete if the evidence is only unit-level, if a
second predictor authority is introduced, if wrong-path state survives, or if
the migration score is raised without corresponding representative evidence.
