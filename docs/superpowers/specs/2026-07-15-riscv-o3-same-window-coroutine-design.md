# RISC-V O3 Same-Window Coroutine Design

**Date:** 2026-07-15

**Status:** Approved for implementation by the repeated `continue` direction.

## Summary

Add one bounded detailed-mode O3 capability for RISC-V coroutine control
transfers:

- `JALR x5, imm(x1)`;
- `JALR x1, imm(x5)`.

These distinct-link forms are architecturally classified as returns, write the
other link register, and perform a return-address-stack (RAS) pop followed by a
push of their sequential PC. The bounded positive window contains one delayed
scalar load, one same-window linked call, one coroutine transfer consuming that
call's exact RAS push, and one scalar descendant depending on the coroutine's
new link value.

The implementation must preserve the current fail-closed boundary. It does not
provide general live indirect-target forwarding, same-link indirect calls,
another same-window return consuming the coroutine's new push, or a wider O3
engine.

## Motivation

The migration ledger still lists coroutine pop-then-push forms as open CPU O3
evidence. The frontend RAS already supports `PopThenPush`, checkpoint codecs
already encode it, and branch classification already labels distinct-link
`JALR` as `return`. The live O3 control descriptor deliberately rejects those
instructions, so no real `rem6 run --execute` path currently demonstrates the
behavior.

Admitting the descriptor alone would be incorrect. Same-window return target
authority currently validates that the exact call push is followed by a plain
`Pop`. A coroutine must instead prove that it is followed by `PopThenPush` and
that the pushed replacement address is the coroutine's exact sequential PC.
The trace path also assumes only `call_direct` and `call_indirect` branch kinds
can write a link register, which is false for a coroutine classified as
`return`.

## Goals

1. Admit both distinct-link coroutine directions as bounded live O3 controls.
2. Preserve exact same-window call-to-coroutine RAS lineage.
3. Bind the expected RAS consumer operation and replacement address.
4. Stage the coroutine destination in integer rename state and one fixed-FU
   writeback slot.
5. Allow one scalar descendant to consume the new link value after coroutine
   writeback.
6. Derive branch link-write evidence from the execution record rather than the
   branch-kind label.
7. Prove direct and cache/fabric/DRAM execution plus suppression, repair, mode
   switch, checkpoint, and timing boundaries through real CLI runs.
8. Update the migration ledger without changing the CPU score or its cap.

## Non-Goals

1. Do not admit `JALR x1, imm(x1)` or `JALR x5, imm(x5)`.
2. Do not forward arbitrary live ALU or load values into frontend indirect
   target selection.
3. Do not let an imprecise or stale RAS entry fall back to architectural link
   state for a same-window coroutine.
4. Do not consume the coroutine's replacement push with another same-window
   linked control. That remains part of deeper linked/control chains.
5. Do not widen the four-row ROB window, scalar-memory prefix, issue width
   model, or general branch-lookahead limit.
6. Do not add a checkpoint schema solely for this capability.
7. Do not raise the migration score from bounded evidence.

## Alternatives Considered

### General producer-forwarded indirect targets

This would let a live ALU or load result steer descendant `JALR` fetch. It is a
larger capability, but it needs a new frontend target-value authority and a
broader invalidation lifecycle. It remains open after this increment.

### Link-sourced indirect calls

Same-link forms and calls whose target is `x1` or `x5` are narrower than general
forwarding but have different RAS semantics. Combining them with coroutine
support would make the boundary harder to audit. They remain rejected.

### Committed-source coroutine only

A coroutine using a committed link source would avoid extending exact
same-window RAS authority. It would exercise less O3 behavior and would not
prove call-to-coroutine dependency, rename, or target lineage. The chosen
design uses the exact latest same-window call push instead.

## Existing Ownership

- `riscv_branch_kind.rs` classifies distinct-link `JALR` as `return`.
- `riscv_fetch_ahead.rs` derives `PopThenPush` from distinct link registers.
- `return_address_stack.rs` owns speculative `Push`, `Pop`, and
  `PopThenPush` operations.
- `o3_source_operands.rs` owns the live-control descriptor boundary.
- `riscv_o3_window_policy.rs` owns bounded row admission and exact latest-link
  provenance.
- `riscv_fetch_ahead/detailed_o3.rs` owns same-window target authority.
- `riscv_live_retire_window.rs` reconstructs the recorded predicted path for
  live retirement.
- `o3_runtime_control_window.rs` owns speculative issue, source forwarding,
  rename matching, and fixed-FU writeback reservation.
- `o3_runtime_retire.rs` and `o3_runtime.rs` derive branch trace evidence.

No new public API or new ownership layer is required.

## Live-Control Descriptor

`o3_live_control_operands` must accept a `JALR` when:

- `rd` is `x1` or `x5`;
- `rs1` is `x1` or `x5`;
- `rd != rs1`.

The descriptor is:

- branch kind: `return`;
- sources: `[rs1]`;
- destination: `Some(rd)`.

Same-link forms remain `None`. Existing no-link returns and indirect calls keep
their current descriptors.

## Window Policy

The current exact latest-link policy remains authoritative.

For a coroutine whose source is committed, the row is an ordinary predicted
control using normal RAS state. For a coroutine whose source is the exact
latest same-window linked call destination, the row is
`AdmitPredictedRasControl` and carries that call's push sequence. For any other
live or unresolved source, the coroutine is terminal and cannot open descendant
fetch.

The coroutine destination shadows older mappings and becomes a live integer
destination. After admitting the coroutine, the prior forwardable call
provenance is consumed. The coroutine's new push is not advertised as another
forwardable call owner in this bounded increment.

## Exact RAS Consumer Authority

Replace the implicit "required target means a plain pop" assumption with an
explicit expected consumer operation:

- `Pop` for an ordinary same-window return;
- `PopThenPush { pushed_address }` for a coroutine.

`PredictedControlTargetAuthority::RasRequired` continues to carry the exact
call push sequence and pushed return address, and additionally carries the
expected consumer operation. For `PopThenPush`, `pushed_address` is the
coroutine's exact sequential PC.

Create one shared authority-construction helper used by detailed fetch-ahead
and live-retire path reconstruction. It must derive the expected consumer from
the instruction, look up the exact call return address by sequence, and return
`None` on any mismatch. This removes the existing duplicated authority-building
logic and prevents the two paths from drifting.

Recorded target validation must require:

1. the producer sequence belongs to a linked call;
2. the consumer sequence belongs to a `return`;
3. the producer operation is the exact `Push` identified by the producer
   sequence;
4. the consumer operation immediately follows that push;
5. the consumer kind matches `Pop` or `PopThenPush` exactly;
6. the consumed predicted return equals the call's exact pushed address;
7. a `PopThenPush` replacement equals the coroutine's exact sequential PC;
8. the recorded prediction is taken and targets the validated consumed address.

Missing, stale, reordered, wrong-kind, wrong-address, or discarded operations
are invalid. An invalid recorded target cannot retry as a fresh prediction and
cannot fall back to the hart's architectural link register.

## Runtime Data Flow

The direct positive window is:

1. delayed scalar load;
2. linked call writing `x1`;
3. coroutine `JALR x5, 0(x1)`;
4. scalar descendant reading `x5`.

The hierarchy positive reverses the link registers and uses an indirect linked
call:

1. delayed scalar load through cache/fabric/DRAM;
2. indirect linked call writing `x5` from a committed non-link target;
3. coroutine `JALR x1, 0(x5)`;
4. scalar descendant reading `x1`.

The call must write back before the coroutine issues. The coroutine must reserve
an integer rename destination and one fixed-FU writeback slot. The descendant
must issue only after coroutine writeback. All four rows remain resident behind
the delayed load and retire in sequence order.

## Link-Write Evidence

`o3_branch_link_register_write` must stop filtering by branch kind. It is called
only for a branch event, so its authority is whether the execution record
contains a write to `x1` or `x5`.

This keeps ordinary returns at `link_write = false`, keeps direct and indirect
calls at `true`, and reports a coroutine `return` at `true`. The CLI evidence
must assert both the `return` kind and link-write presence so this distinction
cannot regress.

## Repair And Cleanup

### Source overwrite

A scalar overwrite of the call's link destination before the coroutine removes
exact forwardable ownership. The coroutine may occupy the terminal row and
issue after the overwrite becomes ready, but it must not fetch descendants
before ordered retirement resolves its target.

### Older branch repair

An older conditional-branch misprediction must discard the call, coroutine,
and descendant rows; restore both link-register mappings; remove both pending
RAS operations; suppress wrong-path data traffic; and leave no retired call or
coroutine event.

For one `Push` plus one `PopThenPush`, exact rollback stats are expected to show
`pushes = 3`, `pops = 3`, and `squashes = 2`: the speculative operations record
two pushes and one pop, then squashing the `Push` records one inverse pop and
squashing the `PopThenPush` records one inverse push plus one inverse pop.

### Coroutine target mismatch

A nonzero `JALR` immediate may make the RAS target differ from the architectural
target. Normal branch repair must discard descendants and redirect to the
resolved target without corrupting the replacement push. After repair, a later
ordinary return must consume that replacement and reach the coroutine's
sequential PC. A focused unit negative is insufficient here: the real CLI
matrix must prove this link-writing `return` mismatch path end to end.

## Mode Switch And Checkpoint

The detailed-to-timing transfer remains non-restorable while live transport is
owned by the detailed CPU. The transferred O3 snapshot must contain four ROB
rows, one LSQ row, both staged link destinations, one outstanding load, and
three younger rows. Baseline and switched runs must preserve issue, writeback,
and commit ticks for all four rows.

A live checkpoint must reject before writing component state. A drained
checkpoint must contain zero ROB and LSQ rows and no live-data handoff chunk,
then restore the final architectural link values and memory witnesses exactly.
Existing RAS checkpoint encoding already supports `PopThenPush`; no schema
change is expected.

## CLI Matrix

Add a focused `predicted_control/coroutine.rs` module with these tests:

1. `rem6_run_o3_same_window_coroutine_commits_direct`
2. `rem6_run_o3_same_window_indirect_coroutine_commits_cache_fabric_dram`
3. `rem6_run_o3_same_window_coroutine_requires_branch_lookahead_two`
4. `rem6_run_o3_same_window_overwritten_coroutine_source_stays_terminal`
5. `rem6_run_o3_older_branch_discards_same_window_coroutine_chain`
6. `rem6_run_host_switch_transfers_o3_same_window_coroutine`
7. `rem6_run_o3_same_window_coroutine_checkpoint_boundary`
8. `rem6_run_timing_suppresses_o3_same_window_coroutine`
9. `rem6_run_o3_same_window_coroutine_wrong_target_repairs_descendants`

## Required Assertions

### Positive execution

- exact final `x1` and `x5` values;
- exact memory bytes and no wrong-path stores;
- exact resident ROB PC order and one LSQ row;
- call issue before coroutine issue, with coroutine issue after call writeback;
- descendant issue after coroutine writeback;
- ordered commits for load, call, coroutine, and descendant;
- integer rename mappings for both link destinations;
- four admitted writeback rows at width one: load, call, coroutine, descendant;
- branch kinds `call_direct` or `call_indirect` plus `return`;
- link-write presence on both call and coroutine;
- exact RAS pushes `2`, pops `1`, used `1`, correct `1`, incorrect `0`;
- exact RAS target-provider selection `1`;
- direct transport activity with cache/fabric/DRAM zero;
- hierarchy cache, transport, fabric, and DRAM activity all nonzero.

### Suppression and repair

- lookahead one opens the call target but not the coroutine target descendant;
- overwrite maps the source link to the overwrite row and leaves the coroutine
  terminal;
- a nonzero-immediate coroutine records a wrong RAS target, squashes its
  descendant, redirects to the resolved target, commits the coroutine link
  write, and lets a later ordinary return consume the preserved replacement
  RAS push;
- older repair restores committed link values and removes wrong-path rows,
  fetches, data traffic, rename ownership, and RAS operations;
- timing mode has no O3 runtime, trace, or aliases;
- live checkpoint rejects and drained restore is empty;
- switch payload counts and first load target/address/width are exact.

## Unit TDD Surface

Add red tests before implementation for:

1. live-control descriptor acceptance of both distinct-link directions;
2. continued rejection of both same-link forms;
3. committed-source coroutine admission;
4. exact same-window call-to-coroutine RAS admission and push sequence;
5. scalar-overwrite terminal behavior;
6. destination shadowing and dependent descendant admission;
7. exact `PopThenPush` recorded-target acceptance;
8. wrong consumer kind rejection;
9. wrong replacement address rejection;
10. discarded lineage rejection with no fresh retry;
11. runtime issue/writeback ownership for a `return` with a link destination;
12. trace link-write evidence derived from the actual register write.

## File Boundaries

Expected implementation files:

- `crates/rem6-cpu/src/o3_source_operands.rs`
- `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead.rs`
- `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- `crates/rem6-cpu/src/o3_runtime.rs`
- focused existing CPU test modules adjacent to those owners
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
- `crates/rem6/tests/source_policy/core_test_anchors.txt`
- `docs/architecture/gem5-to-rem6-migration.md`

Do not modify `temp/reference_designs/gem5`. Do not commit any file under
`temp/`.

## Migration Ledger Update

Keep the CPU heading at `74% representative`, raw score at `8/10`, and the
general O3 checklist item unchecked. Add the bounded coroutine matrix to the
migrated narrative and test-map row. Replace the broad open coroutine claim
with the precise remaining boundary:

- consuming the coroutine's replacement push in another same-window linked
  control;
- broader linked/control chains;
- same-link and other link-sourced indirect calls;
- general producer-forwarded indirect targets.

The ledger must remain exactly 1200 lines unless source-policy constraints are
changed and separately justified.

## Verification

Focused development commands:

```text
cargo test -p rem6-cpu live_control_descriptor -- --nocapture
cargo test -p rem6-cpu coroutine -- --nocapture
cargo test -p rem6-cpu same_window_coroutine -- --nocapture
cargo test -p rem6 --test cli_run same_window_coroutine -- --nocapture
```

Final verification:

```text
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
cargo fmt --all -- --check
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
cargo test --workspace --all-targets --quiet
```

Run the filtered CLI tests with real `rem6 run --execute` binaries before the
full suite. Treat any unrelated full-workspace failure as unproven until an
isolated rerun and a fresh complete workspace rerun establish whether it is
flaky.

## Completion Criteria

This increment is complete only when:

1. both coroutine directions execute through bounded detailed O3 windows;
2. exact RAS consumer kind and replacement-address authority fail closed;
3. the coroutine owns rename, issue, writeback, commit, and link-write evidence;
4. direct and hierarchy CLI positives pass with exact assertions;
5. lookahead, overwrite, rollback, switch, checkpoint, and timing boundaries
   pass;
6. the migration ledger remains honest and exactly 1200 lines;
7. independent read-only review finds no dead API, weak assertion, accidental
   general forwarding, or score inflation;
8. all required verification passes;
9. the implementation commits are pushed to `origin/main`.

## Remaining Boundary

This increment proves one call-to-coroutine-to-scalar-descendant chain in a
four-row scalar-memory-prefix window. It does not prove a coroutine round trip,
another same-window return consuming the replacement push, arbitrary live
indirect targets, fourth-and-deeper linked/control chains, or a general O3
engine.
