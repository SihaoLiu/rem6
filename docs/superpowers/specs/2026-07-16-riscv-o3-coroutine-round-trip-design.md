# RISC-V O3 Coroutine Round-Trip Design

**Date:** 2026-07-16

**Status:** Approved by continued direction after the explicit four-row design
proposal.

## Summary

Add one bounded detailed-mode O3 capability for an adjacent coroutine round
trip:

```text
delayed scalar load -> linked call -> distinct-link coroutine -> ordinary return
```

The call contributes a speculative return-address-stack (RAS) `Push`. The
distinct-link coroutine consumes that entry with `PopThenPush`, writes the
other architectural link register, and publishes its exact sequential PC as a
replacement push. One immediately adjacent ordinary return may consume that
replacement with `Pop`.

The two supported directions are:

```text
JAL x1        -> JALR x5, 0(x1) -> JALR x0, 0(x5)
JALR x5, ... -> JALR x1, 0(x5) -> JALR x0, 0(x1)
```

The capability remains inside the existing four-row scalar-memory-prefix
window. It replaces the prior scalar descendant with the ordinary return; it
does not add a fifth ROB row.

## Motivation

The migration ledger now proves both distinct-link coroutine directions, but
still lists another same-window linked control consuming the coroutine
replacement push as open. Current production code intentionally enforces that
boundary:

- `RiscvScalarIntegerLiveWindow` records a call as the only forwardable link
  producer and clears that producer after every `return`-kind instruction;
- `recorded_ras_required_target` accepts only call branch kinds backed by a
  plain RAS `Push` as a producer;
- the existing policy test
  `admitted_coroutine_does_not_publish_its_replacement_push` locks the third
  return as terminal.

The frontend RAS already records the required operations. The missing behavior
is not general indirect forwarding. It is one exact provenance transition from
the coroutine's recorded `PopThenPush` operation to the next adjacent `Pop`.

## Goals

1. Admit an ordinary return immediately after a same-window distinct-link
   coroutine in both link directions.
2. Keep the total resident window at four ROB rows and one LSQ row.
3. Preserve exact call `Push` -> coroutine `PopThenPush` -> return `Pop`
   adjacency and stack-state lineage.
4. Publish the coroutine replacement only when its link destination, sequence,
   RAS operation kind, pushed address, and post-operation stack all match.
5. Consume the published replacement exactly once and clear its authority
   after the ordinary return.
6. Preserve staged integer rename and fixed-FU writeback ownership for the call
   and coroutine; the ordinary return owns branch issue but no integer
   destination.
7. Prove positive execution, suppression, repair, mode switch, checkpoint, and
   timing boundaries through real `rem6 run --execute` CLI tests.
8. Update the migration ledger without changing the CPU score, bucket cap, or
   general O3 checklist state.

## Non-Goals

1. Do not widen `O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS` beyond four.
2. Do not raise `MAX_RISCV_BRANCH_LOOKAHEAD` beyond three.
3. Do not retain the prior scalar descendant in the same round-trip window.
4. Do not admit `JALR x1, imm(x1)` or `JALR x5, imm(x5)`.
5. Do not forward arbitrary live ALU or load values into indirect target
   selection.
6. Do not admit other link-sourced indirect calls or jumps outside the exact
   adjacent distinct-link lineage.
7. Do not let a stale or malformed RAS entry fall back to architectural link
   state.
8. Do not allow a fourth same-window linked/control consumer after the ordinary
   return.
9. Do not add an older unresolved branch around the full round trip; that would
   require four simultaneous control speculations and exceed lookahead three.
10. Do not add a checkpoint schema or widen live transport ownership.
11. Do not raise the migration score from bounded evidence.

## Alternatives Considered

### Five-row call/coroutine/descendant/return chain

Keeping the existing scalar descendant before the return would require a
five-row ROB window. That couples the RAS lineage change to a separate window
capacity, switch payload, checkpoint, issue, and migration-capability widening.
It remains open as part of fourth-and-deeper chains.

### Post-retirement ordinary return

The current wrong-target repair rows already prove that a later ordinary
return can consume the committed replacement after ordered drain. That path is
important but does not close the same-window authority gap.

### General producer-forwarded indirect targets

A generic live-value target path could make the third return fetchable, but it
would bypass the stronger RAS operation and stack-state proof. It would also
widen unrelated `JALR` forms. The chosen design extends exact RAS provenance
only.

## Existing Ownership

The change must remain within existing owners:

- `crates/rem6-cpu/src/riscv_o3_window_policy.rs` owns bounded admission,
  live-destination shadowing, and the forwardable link producer;
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs` owns typed RAS target
  authority and recorded-operation validation;
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs` owns
  focused frontend provenance negatives;
- `crates/rem6-cpu/src/o3_runtime_control_window_tests/coroutine.rs` owns
  exact ROB, rename, issue, writeback, and discard evidence;
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
  owns shared coroutine fixtures and case data;
- a new same-namespace
  `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip.rs`
  owns the new CLI matrix;
- `crates/rem6/tests/source_policy/coroutine_ownership.rs` owns include order,
  test ownership, and line-count ratchets;
- `crates/rem6/tests/source_policy/core_test_anchors.txt` and
  `docs/architecture/gem5-to-rem6-migration.md` own auditable migration
  evidence.

No production runtime file may change unless a focused runtime test proves
that the existing generic control dependency, branch resource, rename,
writeback, or rollback machinery is insufficient.

## Four-Row Sequences

### Forward direct route

Use one delayed direct-memory load followed by:

```text
call:      JAL  x1, coroutine
coroutine: JALR x5, 0(x1)
return:    JALR x0, 0(x5)
```

The call writes its sequential PC to `x1`. The coroutine consumes that exact
call push, writes its sequential PC to `x5`, and transfers to the call
fallthrough. The ordinary return consumes the coroutine replacement and
transfers to the coroutine fallthrough.

### Reverse hierarchy route

Use one delayed cache/fabric/DRAM load and a committed non-link target source,
then:

```text
call:      JALR x5, 0(x11)
coroutine: JALR x1, 0(x5)
return:    JALR x0, 0(x1)
```

The indirect call target remains committed; no live ALU or load value becomes
frontend target authority.

Both routes must expose exactly these resident row roles before the delayed
load response:

1. delayed scalar load;
2. linked call;
3. distinct-link coroutine;
4. ordinary return.

## Window Policy

Replace the call-only forwardable-link concept with an exact pending RAS-push
producer carrying:

- architectural link destination;
- producer instruction sequence;
- producer shape implied by the admitted control.

The state transitions are:

1. A supported linked call records its destination and sequence as a pending
   `Push` producer.
2. A supported distinct-link coroutine may consume that producer. Because the
   coroutine is a `return` with a nonzero link destination and a recorded
   `PopThenPush`, it replaces the pending producer with its own destination and
   sequence.
3. An ordinary return may consume the coroutine producer with `Pop`; it then
   clears the pending producer.
4. A scalar or control write that shadows the pending destination clears the
   producer before a later return can use it.
5. Any missing sequence, unsupported link destination, unresolved source,
   non-adjacent producer, or exhausted control depth remains terminal or
   rejected under existing fail-closed rules.

The policy must not infer replacement-push authority from `BranchTargetKind::Return`
alone. It is valid only for an admitted return-kind control with the supported
distinct-link destination and a sequence that the frontend can bind to the
exact RAS operation.

## Frontend RAS Authority

`PredictedControlTargetAuthority::RasRequired` remains the public typed
authority. No new general target-authority variant is needed.

Generalize the producer side of `recorded_ras_required_target` from:

```text
call branch kind + Push
```

to exactly one of:

```text
call branch kind   + Push
return branch kind + PopThenPush
```

For the coroutine producer, validation must require:

- the producer sequence maps to `BranchTargetKind::Return`;
- the producer operation is exactly `PopThenPush`;
- its pushed address equals the recorded coroutine sequential PC;
- its `stack_after` equals the consumer operation's `stack_before`;
- the consumer operation is immediately adjacent in pending-operation order;
- the consumer operation is exactly `Pop` for the ordinary return;
- the consumer's predicted return equals the replacement address;
- the resulting stack equals the expected stack after one pop.

The existing call-to-coroutine `Push` -> `PopThenPush` path remains unchanged.
Malformed producer kind, wrong replacement address, stale stack shape,
intervening RAS operation, non-adjacent consumer, and repeated consumption must
all fail closed.

## Runtime Ordering

The existing bounded runtime must stage four rows and one LSQ row. Required
ordering is:

```text
call issue       after its inputs are ready
coroutine issue  after call writeback
return issue     after coroutine writeback
commit order     load <= call <= coroutine <= return
```

The call and coroutine each own an integer rename destination and one fixed-FU
writeback reservation. The ordinary return has no integer destination and must
not allocate an integer rename entry or fixed-FU writeback slot.

All three controls remain branch-resource serialized. The third return may
depend on the coroutine control sequence through the existing generic live
control dependency machinery. A focused CPU runtime red test must prove whether
any production runtime change is actually required before such a change is
made.

## Repair And Cleanup

### Lookahead suppression

With branch lookahead two, the call and coroutine may use exact RAS authority,
but the ordinary return must not open the round-trip target. The pre-response
artifact must retain the bounded terminal row while suppressing target fetch
and later success-path activity.

### Middle coroutine target mismatch

A nonzero immediate on the coroutine may make its RAS target differ from the
architectural target. The predicted call fallthrough contains the ordinary
return, so that return may already have consumed the coroutine replacement
with a speculative `Pop` before the coroutine resolves.

Repair must discard the ordinary-return row, remove its control dependency,
reverse its speculative `Pop`, suppress its predicted-target traffic, and
redirect to the coroutine's resolved target. The call and coroutine remain in
program order, both link writes remain owned, and the coroutine's
`PopThenPush` replacement remains valid for a later ordinary return after
ordered repair and drain.

This path uses exactly three control speculations: call, coroutine, and the
discarded ordinary return. Exact rollback counters must be calibrated from the
generated artifact rather than inferred loosely.

### Ordinary-return target mismatch

A nonzero immediate on the ordinary return may make the RAS target differ from
the architectural target. Repair must squash descendants, redirect to the
resolved target, preserve committed call/coroutine link values, and leave no
stale replacement-push authority. The call and coroutine remain correct RAS
consumers/producers; the ordinary return records the incorrect prediction.

The final counters must distinguish two return-kind controls: the coroutine
is correct, while the ordinary return is incorrect. Exact numeric totals must
be calibrated from a real CLI artifact and then locked in the test.

## Mode Switch And Checkpoint

The detailed-to-timing transfer remains non-restorable while the delayed load
owns live transport. The transferred snapshot must contain:

- four ROB rows;
- one LSQ row;
- two staged integer link destinations;
- one outstanding load;
- three younger rows;
- the ordinary return with no integer destination.

Baseline and switched runs must preserve issue, writeback, and commit ticks for
all four rows plus exact branch/RAS counters.

A live checkpoint must reject before writing component state. A drained
checkpoint must contain zero ROB and LSQ rows and no live-data-handoff chunk,
then restore exact final link values and memory bytes. Existing RAS checkpoint
encoding already supports `Push`, `PopThenPush`, and `Pop`. No schema change is
allowed unless a focused red test proves the existing encoding insufficient.

Timing mode must execute the same architectural round trip while exposing no
O3 runtime, structured O3 events, or runtime O3 stats. Intentional zero-valued
debug trace schema aliases remain present when requested.

## CLI Matrix

Add a focused `coroutine/round_trip.rs` same-namespace include with these tests:

1. `rem6_run_o3_same_window_coroutine_round_trip_commits_direct`
2. `rem6_run_o3_same_window_indirect_coroutine_round_trip_commits_cache_fabric_dram`
3. `rem6_run_o3_same_window_coroutine_round_trip_requires_branch_lookahead_three`
4. `rem6_run_o3_same_window_coroutine_round_trip_middle_repair_discards_return`
5. `rem6_run_o3_same_window_coroutine_round_trip_wrong_target_repairs`
6. `rem6_run_host_switch_transfers_o3_same_window_coroutine_round_trip`
7. `rem6_run_o3_same_window_coroutine_round_trip_checkpoint_boundary`
8. `rem6_run_timing_suppresses_o3_same_window_coroutine_round_trip`

The two positive rows cover forward-direct and reverse-hierarchy routes. The
three lifecycle tests use a two-direction case table. Suppression and repair
may use one representative direction each when exact behavior is symmetric,
but together they must exercise both directions.

## Required Assertions

### Positive execution

- exact final `x1` and `x5` link values;
- exact memory bytes and no wrong-path stores;
- exact resident ROB PC order and one LSQ row;
- call issue before coroutine issue, coroutine issue after call writeback, and
  return issue after coroutine writeback;
- ordered commits for load, call, coroutine, and return;
- integer rename mappings for both link destinations and no return destination;
- exactly three admitted writeback rows at width one: load, call, coroutine;
- one call branch kind plus two `return` branch kinds;
- link-write presence on call and coroutine, absence on ordinary return;
- exact RAS pushes `2`, pops `2`, used `2`, correct `2`, incorrect `0`;
- exact RAS target-provider selection `2`;
- direct transport activity with cache/fabric/DRAM zero;
- hierarchy cache, transport, fabric, and DRAM activity all nonzero.

### Suppression and repair

- lookahead two does not fetch the ordinary-return target;
- malformed or stale coroutine replacement producers fail closed in focused
  frontend tests;
- middle coroutine repair removes the ordinary return, its dependency and
  speculative `Pop`, plus wrong-path fetches and data traffic, while preserving
  the coroutine replacement for later ordered consumption;
- ordinary-return wrong-target repair records predicted, resolved, squashed,
  and redirect targets exactly;
- repair leaves no reusable same-window replacement authority;
- timing mode contains no O3 runtime surface.

### Lifecycle

- both directions expose four resident ROB rows and one LSQ row at the live
  boundary;
- switch handoff schema remains version 7 and non-restorable;
- switch preserves issue/writeback/commit ticks and branch/RAS counters;
- live checkpoint rejects with case-labelled diagnostics;
- drained checkpoint and restored runtime contain zero ROB/LSQ rows;
- timing mode preserves final architecture while suppressing O3 runtime,
  traces, and aliases outside the intentional zero debug schema.

## Source Ownership

Extend `coroutine_ownership.rs` so the root includes, in order:

```text
coroutine/suppression.rs
coroutine/repair.rs
coroutine/lifecycle.rs
coroutine/round_trip.rs
```

The new file owns all eight round-trip test definitions. Existing concern files
must not duplicate those names. Preserve the root line ceiling and keep every
child below the existing focused-module ratchet. If the new matrix exceeds a
single child ratchet, split round-trip lifecycle evidence into another
same-namespace file rather than raising the ceiling.

## Migration Ledger Update

Keep the CPU heading at `74% representative`, raw score at `8/10`, and general
O3 unchecked. Record the bounded three-control round trip in the coroutine
narrative and `tests/gem5/cpu_tests` row.

Remove only this open item:

- another same-window linked control consuming the coroutine replacement push.

Preserve as open:

- same-link forms;
- other link-sourced indirect controls outside the exact adjacent lineage;
- producer-forwarded targets for further control descendants;
- fourth-and-deeper linked/control chains;
- arbitrary broader mixed windows and a general O3 engine.

The ledger remains exactly 1,200 lines.

## Verification

Focused development commands:

```text
cargo test -p rem6-cpu coroutine_round_trip -- --nocapture
cargo test -p rem6-cpu replacement_push -- --nocapture
cargo test -p rem6 --test cli_run coroutine_round_trip -- --nocapture
cargo test -p rem6 --test source_policy coroutine_cli_evidence_uses_focused_same_namespace_includes -- --nocapture
```

Final verification:

```text
cargo test -p rem6-cpu coroutine -- --nocapture
cargo test -p rem6 --test cli_run coroutine -- --nocapture
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

Run real filtered CLI rows before the full suite. Any production runtime edit
requires a focused red test that fails without it.

## Completion Criteria

This increment is complete only when:

1. both link directions execute the four-row call/coroutine/return round trip;
2. exact `Push` -> `PopThenPush` -> `Pop` provenance fails closed for malformed
   operations and stale stack state;
3. the coroutine replacement is consumed once and then cleared;
4. call/coroutine rename and writeback plus ordinary-return branch ownership
   are exact;
5. direct and hierarchy positives pass with exact architecture, memory,
   timing, predictor, RAS, and resource assertions;
6. lookahead, rollback, wrong-target repair, switch, checkpoint, and timing
   boundaries pass;
7. same-link, general live indirect forwarding, five-row chains, and fourth
   controls remain rejected or open;
8. the ledger remains honest and exactly 1,200 lines without score inflation;
9. independent read-only review finds no dead API, weak assertion, accidental
   general forwarding, or ownership regression;
10. all required verification passes and commits are pushed to `origin/main`.

## Remaining Boundary

This increment proves one exact three-control round trip in a four-row
scalar-memory-prefix window. It does not prove a scalar descendant plus a later
return in the same window, a fourth linked/control consumer, same-link forms,
other link-sourced indirect controls, arbitrary producer-forwarded targets, or
a general O3 engine.
