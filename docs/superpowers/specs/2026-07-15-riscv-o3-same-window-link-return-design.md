# RISC-V O3 Same-Window Link Return Design

**Date:** 2026-07-15

**Status:** Approved for implementation planning

## Summary

Extend the existing bounded detailed RISC-V O3 scalar-memory-prefix window so a
linked call and its return can both remain predicted controls in the same live
window.

The supported shape is deliberately narrow:

1. one delayed cacheable scalar-memory head;
2. one already-supported linked call that writes `x1` or `x5`;
3. one `JALR x0, imm(x1/x5)` return whose source is the link written by that
   same live call;
4. one younger instruction fetched from the return target.

The call supplies two independent forms of authority:

1. its staged integer rename destination supplies the architectural return
   source at runtime;
2. its speculative return-address-stack push supplies the frontend target.

The return is admitted only when both authorities are used. The frontend must
not fall back to stale committed `x1` or `x5` state for this case.

The increment stays inside the current four-row scalar-memory window and the
current branch-lookahead limit. It does not widen the general O3 model.

## Motivation

The migration ledger currently keeps CPU execution at `74% representative`,
with 8 of 10 raw checklist items supported. The linked-control increment added
direct calls, independent indirect calls, committed-RAS returns, rollback,
mode-switch, checkpoint, hierarchy, and timing evidence, but intentionally left
same-window call-to-return forwarding open.

Today the behavior is partially real:

1. the call is predicted and pushes its fallthrough address onto the
   speculative RAS;
2. the call stages a physical link destination and can execute before the
   delayed load responds;
3. the return can be staged as a terminal control and can consume the call's
   forwarded link write at issue time;
4. the return target descendant is not fetched because policy treats every
   live-produced indirect target as terminal.

That last rule is correct for arbitrary loads and ALUs but too coarse for a
same-window call. The call link is not an arbitrary unknown target. Its value is
the statically known sequential PC, and the frontend already records the same
value through the speculative RAS push.

This increment replaces that coarse live-target rule with explicit producer
provenance and target authority while retaining fail-closed behavior for all
other live indirect targets.

## Goals

1. Admit a same-window return only when its latest live `x1` or `x5` producer is
   a supported linked call in the current window.
2. Require the speculative RAS to provide the return target for that admission.
3. Keep runtime source forwarding, rename, issue, writeback, commit, and rollback
   under their existing owners.
4. Prove the behavior through real `rem6 run --execute` direct and
   cache/fabric/DRAM rows.
5. Cover lookahead suppression, overwritten-link suppression, older-control
   rollback, mode switch, checkpoint, and timing mode.
6. Preserve the migration score at `74% representative`, raw 8 of 10.
7. Keep coroutine forms, same-link indirect calls, general live-produced
   indirect targets, and deeper windows explicitly open.

## Non-Goals

1. Supporting coroutine `JALR` forms that pop one link register and push the
   other.
2. Supporting `JALR rd=x1/x5, rs1=x1/x5` linked indirect calls.
3. Fetching an indirect target produced by a live scalar ALU or load.
4. Adding a general tick-aware indirect-target wakeup bridge.
5. Increasing the four-row O3 scalar-memory window.
6. Increasing the maximum branch-lookahead depth.
7. Adding a new checkpoint or execution-mode handoff schema.
8. Adding new public configuration, statistics, or compatibility APIs.
9. Raising the CPU migration score or checking the general O3 checklist item.

## Alternatives Considered

### Coroutine pop-then-push first

Coroutine forms already have generic RAS, checkpoint, and branch-kind support.
Adding them to the live-control descriptor would cross several surfaces, but it
would primarily complete an instruction encoding over existing ownership.

This remains a useful follow-up, but it does not add the same intra-window
producer-to-consumer control-flow boundary.

### Committed same-link indirect calls first

Allowing `JALR x1, imm(x1)` and `JALR x5, imm(x5)` with committed sources would
be smaller. It would test source-equals-destination rename behavior but would
not enable a live producer to drive a younger control target.

This remains separate from both coroutine and same-window return semantics.

### General producer-forwarded indirect targets

A live ALU or load target requires a new frontend wakeup and retry contract.
Runtime forwarding occurs after the row is already admitted, while frontend
target selection currently reads the RAS or committed architectural state.

Bundling that target bridge into this increment would blur the bounded
call/return invariant and substantially increase correctness risk.

## Existing Ownership

The implementation must preserve these existing owners:

| Concern | Existing owner |
| --- | --- |
| Live control opcode, source, kind, and destination classification | `o3_source_operands.rs` |
| Scalar-memory-prefix admission and live destination tracking | `riscv_o3_window_policy.rs` |
| Detailed O3 additional-fetch candidate selection | `riscv_fetch_ahead/detailed_o3.rs` |
| Live-row staging and completed-fetch replay | `o3_runtime_live_window.rs` and `riscv_live_retire_window.rs` |
| Branch target and RAS selection | `riscv_fetch_ahead.rs` |
| Speculative RAS push/pop recording | `riscv_fetch_ahead.rs` and `return_address_stack.rs` |
| Staged rename destination allocation | existing O3 live-window staging |
| Source forwarding and issue readiness | existing O3 control-window and issue code |
| Fixed-FU writeback reservation | existing O3 writeback authority |
| Ordered commit and rename publication | existing O3 retire authority |
| Branch repair and RAS squash | existing branch execution/fetch cleanup |
| Live mode-transfer and checkpoint policy | existing execution-mode handoff and checkpoint owners |

No second opcode inventory, RAS, rename map, writeback queue, or rollback path is
introduced.

## Policy Provenance

`RiscvScalarIntegerLiveWindow` currently tracks:

1. unresolved scalar-memory destinations;
2. every destination produced inside the current window;
3. row and predicted-control depth.

Add one focused `forwardable_link_destination: Option<Register>` provenance
owner. It holds at most the latest supported linked-call destination across
`x1` and `x5`; `None` means no live call-produced link may admit a return.

The owner follows latest-writer and single-consumer semantics:

1. a supported linked call records its destination as live, then assigns
   `Some(destination)`; every later supported linked call to `x1` or `x5`
   replaces the prior owner, including a call that switches link registers;
2. a generic scalar destination clears provenance only when it overwrites the
   current owner, while an unrelated generic destination leaves the owner
   unchanged;
3. a return receives RAS-required admission only when its one source equals the
   owner, and an admitted return consumes the provenance entirely by restoring
   `None`;
4. unresolved memory state never creates forwardable-link provenance.

The existing shadowing helper should remain the single operation that removes an
overwritten unresolved destination and records the live destination. Link
provenance is layered on that operation rather than duplicating it. Supported
call recognition continues to use the existing live-control descriptor and
`BranchTargetKind`; no second opcode inventory is introduced.

## Admission Decisions

The existing predicted-control decision is insufficient because committed
returns may use normal RAS-or-architectural target selection, while a
same-window return must require RAS authority.

Introduce one distinct internal decision for a RAS-required predicted control:

```rust
RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
```

This variant remains distinct through detailed fetch candidate selection.

Admission rules become:

1. direct calls remain ordinary predicted controls;
2. independent indirect calls with committed non-link targets remain ordinary
   predicted controls;
3. committed-source returns remain ordinary predicted controls;
4. a return whose live source matches the one forwardable link owner becomes a
   RAS-required predicted control and consumes that owner;
5. a return whose source is unresolved, produced by a load, produced by a scalar
   ALU, or overwritten after the call remains terminal;
6. all currently rejected instruction forms remain rejected;
7. row and control-depth limits remain unchanged.

The policy does not inspect RAS internals. It proves producer provenance and
communicates the required target authority to the frontend.

## Frontend Target Authority

Detailed O3 fetch candidate selection must carry whether target selection is
normal or RAS-required through one internal typed authority:

```rust
enum PredictedControlTargetAuthority {
    Normal,
    RasRequired,
}
```

`DetailedFetchAheadCandidate::ReadyPredictedControl` carries this authority to
the fetch-ahead driver.

For `Normal`:

1. current direct-call behavior is unchanged;
2. current independent indirect-call behavior is unchanged;
3. a committed return may use RAS first and architectural `rs1` fallback as it
   does today.

For `RasRequired`:

1. the instruction must classify as `Return`;
2. the RAS top must exist before the return's speculative pop is recorded;
3. the target provider must be `RAS`;
4. target selection must not read committed `hart.read(rs1)` as fallback;
5. a missing RAS target returns no fetch-ahead decision and opens no descendant
   fetch.

Once the RAS-required prediction is recorded, repeated candidate evaluation may
reuse that recorded predicted PC. The original prediction remains the typed
proof that stale architectural state was not used.

Live-row staging and completed-fetch replay treat
`AdmitPredictedRasControl` as a predicted control for row ownership, control
dependencies, and recorded-PC traversal. They do not collapse it to normal
target authority before the initial frontend prediction is created.

## RAS Ordering

The call and return use the existing RAS operations:

1. call prediction pushes the call's sequential PC;
2. return target lookup reads that speculative top;
3. return prediction records one speculative pop;
4. ordered retirement commits the push before the pop;
5. repair squashes pending operations through the existing RAS rollback owner.

The positive window therefore has two pending RAS operations while the delayed
load blocks commit.

No O3-owned copy of RAS state is added.

## Runtime Data Flow

No new runtime execution kind is required.

The existing flow is authoritative:

1. the call ROB row owns one staged integer destination for `x1` or `x5`;
2. the call execution record produces exactly one matching link write;
3. the call reserves one fixed-FU writeback slot;
4. the return candidate resolves its source through the staged rename overlay;
5. source forwarding supplies the call's link value to the return's cloned hart;
6. issue readiness keeps the return behind the call's admitted writeback tick;
7. the return consumes branch issue authority and no integer writeback slot;
8. the fallthrough descendant remains control-dependent on the return;
9. all rows commit in program order behind the delayed load.

Focused runtime tests must prove these relationships. Production runtime changes
are permitted only if the red tests expose a missing generic behavior.

## Repair And Cleanup

### Older control repairs away from the chain

Repair must remove:

1. the linked call;
2. its staged rename destination;
3. its writeback reservation and speculative execution;
4. the return;
5. both pending RAS operations;
6. wrong-path data and memory effects.

The prior committed link register and rename mapping remain intact.

### Call or return prediction repair

Existing immediate-control repair ownership remains unchanged:

1. a call repair discards the return and all younger descendants;
2. a return repair discards only the return-target descendants;
3. matching RAS operations are repaired by the existing branch cleanup path;
4. no duplicate O3 RAS repair path is introduced.

## Mode Switch And Checkpoint

No schema change is expected.

### Detailed-to-timing switch

A representative live transfer occurs after the return-target descendant issues
but before the delayed load responds.

The transferred state must preserve:

1. four ROB rows and one LSQ row;
2. the call rename destination;
3. issue, admitted-writeback, writeback, and commit timing;
4. control dependency ordering;
5. frontend branch and RAS speculation needed for correct later resolution;
6. non-restorable live-data handoff behavior.

The first post-window timing instruction remains outside O3 authority.

### Checkpoint

1. a checkpoint while the four-row window is live remains rejected;
2. a drained checkpoint succeeds;
3. drained restore has zero ROB and LSQ rows;
4. drained restore has no live-data handoff chunk;
5. historical payload compatibility is unchanged.

## Executable Program Shapes

### Direct positive

Use:

1. delayed cacheable scalar load;
2. `JAL x1` to a short function;
3. `JALR x0, 0(x1)` in that function;
4. one scalar instruction at the call fallthrough target.

The live snapshot must contain exactly those four rows. The return target
descendant must issue before the load response.

### Hierarchy positive

Use:

1. delayed cacheable scalar load through cache/fabric/DRAM;
2. independent `JALR x5` through a committed non-link target;
3. `JALR x0, 0(x5)` in the target function;
4. one scalar fallthrough descendant.

This row proves the same dataflow with an indirect call and nonzero cache,
transport, fabric, and DRAM activity.

### Lookahead suppression

Run the direct program with branch lookahead one. The call may be predicted, but
the same-window return must not open its fallthrough descendant.

### Overwritten-link suppression

Insert a live scalar write to the same link register after the call and before
the return. The return remains terminal and no stale call-fallthrough target is
fetched.

### Rollback representative

An older mispredicted direct conditional branch owns a wrong path containing the
call and return. Together with the delayed load, those rows fill the four-row
window and require branch lookahead three. Repair removes both controls and
records RAS push/pop squash evidence; the positive rows separately prove the
return-target descendant.

## CLI Matrix

Add a focused module:

`crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_return.rs`

Register it from `predicted_control.rs`. Reuse shared execution and assertion
helpers from `window_support.rs`; do not grow `link_kind.rs` further.

Required top-level tests:

1. `rem6_run_o3_same_window_link_return_commits_direct`
2. `rem6_run_o3_same_window_indirect_link_return_commits_cache_fabric_dram`
3. `rem6_run_o3_same_window_link_return_requires_branch_lookahead_two`
4. `rem6_run_o3_same_window_overwritten_link_return_stays_terminal`
5. `rem6_run_o3_older_branch_discards_same_window_link_return_chain`
6. `rem6_run_host_switch_transfers_o3_same_window_link_return`
7. `rem6_run_o3_same_window_link_return_checkpoint_boundary`
8. `rem6_run_timing_suppresses_o3_same_window_link_return`

The matrix uses real `env!("CARGO_BIN_EXE_rem6")` execution through
`rem6 run --execute`.

## Required Assertions

Positive rows must prove more than retirement counts.

### Architectural witnesses

1. exact final `x1` or `x5` call link value;
2. exact fallthrough descendant register value;
3. exact memory bytes showing only intended path effects;
4. absence of wrong-path stores.

### Live O3 witnesses

1. exact four-row ROB PC order;
2. exact one-row LSQ occupancy;
3. call issue before load response;
4. return issue after the call becomes ready and before load response;
5. fallthrough descendant issue after return validation and before load response;
6. staged link rename destination while the architectural link remains old;
7. one call writeback slot and no return writeback slot;
8. ordered commit ticks.

### Branch and RAS witnesses

1. call kind `call_direct` or `call_indirect`;
2. return kind `return`;
3. call link-write true and return link-write false;
4. return target provider `ras`;
5. one speculative push and one speculative pop;
6. RAS used/correct evidence for the return;
7. repair rows expose both operation squashes.

### Route witnesses

The hierarchy row proves nonzero:

1. cache data activity;
2. data transport activity;
3. fabric activity;
4. DRAM activity.

### Suppression witnesses

1. lookahead one has no return-target descendant fetch;
2. overwritten-link return has no stale target fetch;
3. timing mode has no O3 runtime, O3 trace, or O3 aliases;
4. live checkpoint rejects and drained restore is empty.

## TDD Sequence

### Policy red tests

Add failing tests proving:

1. direct and indirect linked calls assign their destination as the one
   forwardable owner;
2. a later supported call replaces the prior owner even when it changes from
   `x1` to `x5` or vice versa;
3. same-window returns must match the current owner, receive RAS-required
   predicted admission, and consume the owner so it cannot admit another return;
4. a generic overwrite of the current owner clears provenance, while an
   unrelated generic destination leaves it unchanged;
5. unresolved memory and load/ALU-produced return sources never create usable
   provenance and remain terminal;
6. existing unsupported forms remain rejected;
7. lookahead depth remains enforced.

### Frontend red tests

Add failing detailed fetch tests proving:

1. call push feeds same-window return prediction;
2. return prediction uses RAS and opens the fallthrough descendant;
3. missing RAS authority does not fall back to committed `x1` or `x5`;
4. call and return create two pending RAS operations;
5. existing committed-return behavior is unchanged.

### Runtime red tests

Add focused tests proving:

1. return candidate depends on the call's staged rename producer;
2. return issue waits for admitted call writeback;
3. call and return serialize on branch issue authority;
4. fallthrough descendant depends on return validation;
5. rollback removes call, return, rename, execution, reservation, and dependency
   state.

### CLI red tests

Add the direct positive first. It must fail because the current policy keeps the
return terminal and the fourth row is absent.

Add the remaining matrix only after the direct red case is understood.

## File Boundaries

Expected production files:

1. `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
2. `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
3. `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`
4. `crates/rem6-cpu/src/riscv_fetch_ahead.rs`
5. `crates/rem6-cpu/src/o3_runtime_live_window.rs`
6. `crates/rem6-cpu/src/riscv_live_retire_window.rs`

Expected focused tests:

1. policy tests in `riscv_o3_window_policy.rs`;
2. detailed frontend tests in
   `riscv_fetch_ahead/tests/detailed_o3_control.rs`;
3. runtime tests in existing focused O3 control/issue test modules;
4. new CLI matrix `predicted_control/link_return.rs`;
5. module registration in `predicted_control.rs`;
6. shared helper changes only in `window_support.rs` when they remove real
   duplication.

Documentation and policy files:

1. `crates/rem6/tests/source_policy/core_test_anchors.txt`
2. `docs/architecture/gem5-to-rem6-migration.md`

Do not add production behavior to root facades or duplicate live-control opcode
classification outside `o3_source_operands.rs`.

## Migration Ledger Update

After executable evidence passes:

1. keep heading `CPU Execution Models - 74% representative`;
2. keep raw score `8 of 10`, or 80% raw capped at 74%;
3. keep the general O3 checklist item unchecked;
4. add the eight CLI anchors;
5. describe bounded same-window direct/indirect call-to-return evidence;
6. remove `same-window call-to-return link forwarding` from open gaps;
7. retain coroutine pop-then-push forms;
8. retain linked indirect calls whose target source is `x1` or `x5`;
9. retain general producer-forwarded indirect targets;
10. retain fourth-and-deeper linked/control chains, arbitrary broader mixed
    windows, and the general O3 engine;
11. preserve the exact 1,200-line ledger invariant.

## Commit Boundaries

Use behavior-oriented commits:

1. `cpu: classify same-window link returns`
   - policy provenance and frontend target authority;
   - focused policy/frontend tests.
2. `cpu: execute same-window link returns`
   - runtime dependency/issue/rollback changes only if red tests require them;
   - focused runtime tests.
3. `test: cover same-window link returns`
   - real CLI matrix and focused helper ownership.
4. `docs: record same-window link return evidence`
   - anchors and migration ledger.

If runtime production code requires no change, keep commit 2 test-only or fold
its tests into commit 1 without creating an artificial implementation commit.

## Verification

Focused gates:

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
cargo test -p rem6-cpu detailed_o3_control -- --nocapture
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue -- --nocapture
cargo test -p rem6 --test cli_run same_window_link_return -- --nocapture
cargo test -p rem6 --test cli_run predicted_control -- --nocapture
```

Completion gates:

```text
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
cargo test --workspace --all-targets --quiet
wc -l docs/architecture/gem5-to-rem6-migration.md
git diff --check
```

Before push, high-intensity read-only reviews must separately inspect:

1. producer provenance and stale-target fail-closed behavior;
2. frontend RAS-required authority and RAS ordering;
3. runtime rename, issue, writeback, rollback, and cleanup;
4. CLI matrix strength and real executable evidence;
5. migration-ledger honesty, source-policy ownership, and Slop/dead-code risk.

## Completion Criteria

The increment is complete only when:

1. the direct red test fails on the current terminal boundary and passes after
   implementation;
2. direct and hierarchy real CLI rows contain load, call, return, and
   fallthrough descendant simultaneously;
3. the return target is RAS-provided without architectural fallback;
4. runtime forwarding and issue order are proven;
5. rollback, switch, checkpoint, and timing rows pass;
6. full workspace tests pass;
7. the ledger remains exactly 1,200 lines and at 74%, raw 8 of 10;
8. mandatory read-only review finds no unresolved correctness, ownership,
   test-strength, dead-code, or score-inflation issue;
9. commits are pushed and local/remote `main` hashes match.

## Remaining Boundary

After this increment, the honest linked-control gaps remain:

1. coroutine pop-then-push forms;
2. same-link and other linked indirect calls whose target source is `x1` or
   `x5`;
3. indirect targets produced by general live ALU or memory results;
4. fourth-and-deeper linked/control chains;
5. arbitrary broader mixed memory/control windows;
6. restorable live transport ownership;
7. broad FP/vector/atomic/MMIO writeback;
8. a general O3 engine.
