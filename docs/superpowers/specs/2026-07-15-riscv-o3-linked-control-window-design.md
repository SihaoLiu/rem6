# RISC-V O3 Linked Control Window Design

## Status

Approved as the next bounded increment under `temp/improve-rem6-0.md`.

This document is an implementation design. The migration authority remains
`docs/architecture/gem5-to-rem6-migration.md`.

## Context

The detailed RISC-V O3 path can keep one delayed scalar-memory row and up to
three younger rows resident in a four-entry ROB. The current live-control
authority covers direct conditional branches, no-link direct jumps, and
no-link indirect jumps. Real CLI evidence already proves mixed control kinds,
branch-port contention, targeted rollback, direct and cache/fabric/DRAM
execution, mode transfer, checkpoint boundaries, and timing-mode suppression.

The CPU migration ledger remains 8 of 10 items with executable evidence, 80%
raw, capped at 74% representative. Its named O3 gaps now include link-writing
calls and returns, producer-forwarded indirect targets, fourth-and-deeper
control chains, arbitrary broader mixed windows, restorable live transport,
broad writeback classes, and a general O3 engine.

The branch frontend already classifies direct calls, indirect calls, and
returns. It already predicts their targets, performs speculative return-address
stack operations, repairs those operations on rollback, and checkpoints the
RAS. The live O3 control descriptor deliberately excludes every link-writing
or return-shaped form, and speculative control issue currently assumes that a
control has neither a rename destination nor an integer register write.

This increment removes that no-destination assumption without creating a
parallel linked-control implementation.

## Alternatives

### ALU-produced indirect target forwarding

Allow a no-link `JALR` target to come from an older speculative scalar ALU.
The lower issue path can already execute with forwarded operands, but frontend
target selection reads architectural `rs1` before the speculative producer has
necessarily reached admitted writeback. A sound implementation needs a
tick-aware target-value bridge plus a fetch wake/retry path.

This is valuable but is a separate ownership boundary.

### Completed-response checkpoint restore

Restore a completed but unretired scalar-memory operation after its transport
response has returned. This avoids reconstructing transport callbacks, but it
still needs a new checkpoint subpayload, lifecycle reconstruction, compatibility
rules, and exactly-once retirement evidence.

This is the strongest checkpoint alternative but does not naturally compose
with the current control cleanup.

### Bounded linked controls

Extend the existing typed live-control descriptor with an optional integer
link destination. Admit direct calls, independent indirect calls, and returns
whose target source is already committed. Reuse existing rename allocation,
writeback reservation, branch issue, RAS, rollback, and retirement ownership.

This approach is selected because it crosses control kind, rename, writeback,
and RAS axes in one bounded end-to-end increment without adding a new engine.

## Ledger Target

The target is `CPU Execution Models - 74% representative`.

The score and checklist remain unchanged. The increment narrows the linked
control gap to same-window link dependencies, coroutine forms, and broader
linked chains. It does not complete a general O3 engine or KVM-style fast
forwarding.

The migration ledger must remain exactly 1,200 lines. Existing CPU prose and
the `tests/gem5/cpu_tests` row must be updated in place rather than gaining a
second progress narrative.

## Goals

1. Admit direct calls encoded as `JAL rd=x1/x5`.
2. Admit independent indirect calls encoded as
   `JALR rd=x1/x5, rs1=committed non-link register`.
3. Admit returns encoded as `JALR rd=x0, rs1=x1/x5` when the link source is
   committed before the live scalar-memory window opens.
4. Allocate and expose a live integer rename destination for direct and
   indirect calls.
5. Make link-writing controls consume branch issue capacity and one scalar
   writeback slot.
6. Keep returns on the branch issue port without a writeback slot.
7. Preserve correctly predicted linked-control descendants.
8. Roll back wrong-path link destinations, descendant rows, and speculative
   RAS operations together.
9. Prove that the architectural link register remains unchanged until ordered
   retirement behind the older scalar-memory row.
10. Preserve mode-transfer, checkpoint, timing-suppression, and source-policy
    boundaries.

## Non-Goals

1. Do not predict a `JALR` target from a register produced inside the live
   window.
2. Do not allow a same-window call to feed a younger return through speculative
   `x1` or `x5` forwarding.
3. Do not support coroutine `JALR` forms that pop one link register and push the
   other.
4. Do not support `JALR rd=x1/x5, rs1=x1/x5`, including same-register indirect
   calls.
5. Do not support destination-writing jumps whose destination is not `x1` or
   `x5`.
6. Do not increase the four-row live ROB bound or branch-lookahead maximum.
7. Do not add a new branch predictor, BTB, RAS, checkpoint schema, handoff
   schema, or writeback source class.
8. Do not admit memory, MMIO, atomic, CSR, float, vector, system-event, trap, or
   interrupt descendants.
9. Do not raise the CPU migration score or check the general O3 item.

## Supported Forms

One typed helper in `o3_source_operands.rs` remains the only live-control
inventory.

| Instruction form | Branch kind | Sources | Destination | Admission rule |
| --- | --- | --- | --- | --- |
| Direct conditional branch | `DirectConditional` | branch operands | none | existing rule |
| `JAL rd=x0` | `DirectUnconditional` | none | none | immediate target |
| `JALR rd=x0, rs1=non-link` | `IndirectUnconditional` | `rs1` | none | committed source only |
| `JAL rd=x1/x5` | `CallDirect` | none | `rd` | immediate target |
| `JALR rd=x1/x5, rs1=non-link` | `CallIndirect` | `rs1` | `rd` | committed source only |
| `JALR rd=x0, rs1=x1/x5` | `Return` | `rs1` | none | committed link source only |

The descriptor must carry:

1. `BranchTargetKind`;
2. scalar source registers;
3. optional integer destination.

`BranchTargetKind` is the target-source classification. Policy uses
`CallIndirect`, `Return`, and `IndirectUnconditional` plus the descriptor's
source registers to apply the committed-source rule without decoding the
instruction again.

Consumers must not rebuild the opcode or link-register inventory.

## Rejected Forms

The typed helper rejects these encodings, so they fall through the scalar
younger policy and return `Reject` without consuming a row:

1. `JAL rd` where `rd` is not `x0`, `x1`, or `x5`;
2. `JALR rd` where `rd` is nonzero and not `x1` or `x5`;
3. `JALR rd=x1/x5, rs1=x1/x5`, including both coroutine forms with different
   link registers and same-register linked indirect calls.

The helper recognizes otherwise-supported indirect calls and returns even when
their source is not ready. Policy stages that control as
`AdmitTerminalControl`, closes the window, and opens no predicted descendant
when the target source:

1. remains unresolved memory state;
2. was produced by a live scalar ALU in the current window; or
3. was produced by a live call in the current window.

Neither rejected encodings nor terminal controls may use stale architectural
state to fetch a younger target.

## Admission Policy

`RiscvScalarIntegerLiveWindow` keeps the four-row and three-control bounds.

The policy continues to track unresolved scalar-memory destinations and all
destinations produced inside the current window. It additionally treats a
linked control destination like any other live integer destination.

Admission rules are:

1. Direct calls are predicted controls because their target is immediate.
2. Independent indirect calls are predicted controls only when their non-link
   target source is absent from unresolved and live-produced destination sets.
3. Returns are predicted controls only when `x1` or `x5` was committed before
   the current window and is absent from unresolved and live-produced sets.
4. A live-produced or unresolved target source closes the window at that
   control.
5. A link destination is recorded as a live destination so younger reads use
   normal dependency policy and cannot observe stale architectural state.

The policy must not inspect RAS internals. RAS prediction remains a frontend
responsibility; policy only proves that the architectural source is not being
invented from unready live state.

## Rename Ownership

`stage_live_instruction` already derives integer destinations from `JAL` and
`JALR`, allocates a physical register, and records the architectural rename
destination in the ROB. That existing path remains authoritative.

For a supported linked call:

1. the staged ROB row has one integer physical destination;
2. the live rename overlay maps `x1` or `x5` to that destination;
3. the committed rename map remains unchanged while the older load blocks
   commit;
4. rollback removes the staged destination and restores the prior mapping;
5. ordered ROB commit publishes the staged mapping through the existing
   `commit_live_rob_prefix` path.

Returns and no-link controls continue to have no rename destination.

## Runtime Issue And Writeback

The generic control issue kind gains an optional staged rename destination.

A valid linked call execution record must:

1. match the candidate PC and instruction;
2. have the expected `CallDirect` or `CallIndirect` branch kind;
3. produce exactly one integer register write;
4. write the candidate's staged `x1` or `x5` destination;
5. have no trap, system event, memory access, float write, or extra integer
   write.

A valid return or no-link control keeps the current zero-write requirement.

All supported controls consume `O3IssueOpClass::Branch`. Link-writing calls also
reserve one fixed-FU writeback slot. Returns and no-link controls do not reserve
a writeback slot.

The admitted writeback tick, not the raw execution tick, is the dependency-ready
tick for any younger reader of the call's link destination.

## Frontend And RAS Ownership

The existing frontend remains authoritative for:

1. direct call immediate targets;
2. independent indirect call targets read from committed architectural state;
3. return targets supplied by the RAS, with architectural `rs1` fallback;
4. speculative RAS push for direct and indirect calls;
5. speculative RAS pop for returns;
6. branch prediction records, target-provider evidence, and BTB activity.

The positive return fixture seeds `x1` or `x5` and the RAS with a call that
retires before the delayed scalar-memory window opens. The return is therefore
RAS-backed without depending on a same-window speculative link write.

No new O3-owned RAS state is introduced.

## Redirect And Rollback Ownership

`riscv_execute.rs` continues to resolve architectural control flow and branch
prediction. The live O3 runtime only owns descendant cleanup and staged rename
state.

For supported linked controls:

1. a correct prediction with live descendants preserves the path even though
   the architectural next PC is nonsequential;
2. a linked-control repair discards only descendants of that control sequence;
3. an older branch repair discards a younger linked call or return, its rename
   destination, writeback reservation, and live descendants;
4. branch-speculation cleanup squashes the corresponding speculative RAS
   operation;
5. traps and unsupported redirects retain broad cleanup behavior.

The implementation must not add a second RAS rollback path inside O3.

## Executable Program Shapes

### Direct call positive

The direct positive program uses:

1. a delayed cacheable scalar load;
2. `JAL x1` to a direct call target;
3. two scalar descendants at the target.

The load plus three younger rows fills the ROB. The test proves the call and
target descendants issue before the load response, `x1` remains architecturally
unchanged before ordered retirement, and the final link value equals the call
fallthrough PC.

### Indirect call positive

The hierarchy representative uses:

1. a delayed cacheable load through cache/fabric/DRAM;
2. `JALR x5` through a committed non-link target register;
3. two target descendants.

It proves `CallIndirect`, one link write, stable target-source ownership,
ordered commit, exact ROB/LSQ occupancy, and nonzero cache, transport, fabric,
and DRAM activity.

### Return positive

A committed call first seeds a link register and the RAS. Inside the called
function, the program executes:

1. a delayed cacheable scalar load;
2. `JALR x0, 0(x1/x5)`;
3. scalar work at the call fallthrough target.

The test proves `Return`, RAS target provider, one RAS pop, no link write, and
target descendant issue before the delayed load retires.

### Rollback representative

An older direct conditional branch is predicted down a path containing a
linked call, then repairs away from it while the call and a target descendant
are resident.

The test proves:

1. the older branch remains the repair owner;
2. the linked call and descendants leave the ROB;
3. the prior `x1/x5` architectural value and rename mapping survive;
4. the speculative RAS push is squashed;
5. wrong-path Data and Memory effects do not occur.

### Dependency negatives

Negative fixtures cover:

1. an indirect call target produced by the delayed load;
2. an indirect call target produced by a younger live scalar ALU;
3. a return source overwritten by a live scalar row;
4. a same-window call followed by a return using that live link destination;
5. coroutine and same-link indirect-call encodings.

No negative may fetch a descendant from stale architectural state.

## CLI Matrix

Place top-level evidence in a new focused module:

`crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_kind.rs`.

The representative matrix covers:

1. direct-memory detailed direct-call execution with exact four-ROB/one-LSQ
   residency, link rename state, pre-response issue, and ordered commit;
2. cache/fabric/DRAM indirect-call execution with matching architecture and
   nonzero hierarchy activity;
3. RAS-backed return execution with committed link source, no link write, and
   target descendant issue before the load response;
4. older-branch rollback removing a younger linked call, descendants, rename
   state, and RAS push;
5. live-produced target suppression for indirect call and return shapes;
6. detailed-to-timing transfer with exact inherited issue/writeback/commit
   ticks and non-restorable live O3 authority;
7. live checkpoint rejection plus drained checkpoint/restore with zero ROB and
   LSQ rows and no live-data handoff chunk;
8. timing-mode architectural parity with no O3 runtime, O3 trace, or gem5-style
   O3 aliases.

Assertions must include:

1. branch kind and target provider;
2. link-write presence or absence;
3. exact final `x1/x5` values;
4. pre-commit architectural link-register suppression;
5. staged rename destination and physical-register identity where exposed;
6. issue, raw-ready, admitted-writeback, writeback, and commit ordering;
7. branch and writeback resource contention;
8. RAS push/pop/squash/used/correct evidence;
9. exact memory bytes and wrong-path Data/Memory suppression;
10. hierarchy activity, transfer chunks, checkpoint chunks, and final drain.

Committed counts alone are not sufficient.

## Focused Unit Tests

Tests must first fail for:

1. typed descriptor destination metadata for direct and indirect calls;
2. descriptor rejection of non-link destinations, coroutine forms, and
   same-link indirect calls;
3. policy admission of direct call, independent indirect call, and committed
   return;
4. policy terminal behavior for live-produced indirect-call and return sources;
5. call candidate rename destination shape;
6. call execution validation requiring exactly one matching integer write;
7. return validation requiring no integer write;
8. branch issue serialization across calls and returns;
9. link-writing call writeback-port reservation and deferral;
10. rollback removing the call destination and future writeback reservation;
11. correct linked-control prediction preserving descendants;
12. frontend RAS push/pop cleanup through an older repair.

## Source Policy And Slop Removal

The source-policy test must continue to enforce one typed live-control owner.
It must additionally require destination metadata in that owner and reject a
new linked-control opcode inventory in policy, issue, staging, or retirement
consumers.

The change removes the remaining implicit assumption that every control row has
no destination. It must not introduce separate `Call` and `Control` candidate
pipelines, duplicate link-register tests, or branch-kind reconstruction.

New top-level test names must be anchored in
`crates/rem6/tests/source_policy/core_test_anchors.txt`.

## Error Handling

Unsupported linked forms fail closed at admission. A mismatched speculative
execution record is not recorded and cannot claim early issue or writeback.

If a linked control reaches retirement without the expected prediction,
rename, or RAS state, existing broad redirect cleanup applies. No recovery path
may guess a target, synthesize a link write, or retain a partial rename update.

Live checkpoints remain rejected. Drained checkpoints use existing O3 runtime
and branch-predictor/RAS payloads; this increment must not change a checkpoint
payload version.

## Verification

Focused gates:

```text
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue -- --nocapture
cargo test -p rem6-cpu riscv_fetch_ahead -- --nocapture
cargo test -p rem6 --test cli_run link_kind -- --nocapture
cargo test -p rem6 --test cli_run predicted_control -- --nocapture
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
wc -l docs/architecture/gem5-to-rem6-migration.md
git diff --check
```

The increment is complete only after the real CLI matrix passes, the ledger
remains mechanically honest at 1,200 lines and 74%, and a high-intensity
read-only review finds no unresolved correctness, ownership, test-strength, or
score-inflation defect.

## Remaining Boundary

After this increment, the honest linked-control gaps remain:

1. same-window call-to-return link forwarding;
2. coroutine pop-then-push forms;
3. live-produced indirect targets;
4. fourth-and-deeper linked chains;
5. arbitrary broader mixed memory/control windows;
6. restorable live transport and broader writeback classes;
7. a general O3 engine.
