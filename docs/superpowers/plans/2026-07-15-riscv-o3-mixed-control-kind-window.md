# RISC-V O3 Mixed Control-Kind Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the bounded scalar-memory O3 live window across no-link direct unconditional, direct conditional, and no-link indirect unconditional controls while consolidating conditional-specific control authority.

**Architecture:** One typed O3 live-control operand helper returns branch kind and scalar sources for the supported no-link forms. Policy, live staging, speculative issue, issue arbitration, and redirect cleanup consume that helper; the existing frontend speculation, BTB, architectural execution, branch statistics, mode transfer, and checkpoint gates remain authoritative.

**Tech Stack:** Rust workspace, `rem6-cpu`, top-level `rem6 run --execute` CLI integration tests, JSON artifacts, source-policy tests, Cargo.

---

## File Map

- Modify `crates/rem6-cpu/src/o3_source_operands.rs`: own the typed live-control operand descriptor.
- Modify `crates/rem6-cpu/src/o3_runtime.rs`: re-export the focused helper inside the O3 runtime module tree.
- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: admit supported mixed controls and keep producer-dependent JALR terminal.
- Modify `crates/rem6-cpu/src/o3_runtime_live_window.rs`: track every supported live control sequence and immediate dependency.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window.rs`: issue and validate generic no-link controls.
- Modify `crates/rem6-cpu/src/o3_runtime_issue.rs`: reserve branch issue capacity for all supported controls.
- Modify `crates/rem6-cpu/src/riscv_execute.rs`: preserve correct predicted jump descendants and target rollback to the repairing control.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`: cover generic control candidates, dependencies, and rollback.
- Modify `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`: cover direct/indirect unconditional branch issue arbitration.
- Modify `crates/rem6-cpu/tests/source_policy.rs`: prevent conditional-only live-control authority from returning.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/mixed_kind.rs`: own the top-level mixed-kind matrix.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`: wire the focused test module.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: anchor the new executable evidence.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: narrow the CPU open gap without changing score or line count.

### Task 1: Lock The Typed Control Boundary In Failing Policy Tests

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Modify: `crates/rem6-cpu/src/o3_source_operands.rs`

- [ ] **Step 1: Add instruction builders for no-link and linked jumps**

Add test helpers in `riscv_o3_window_policy.rs`:

```rust
fn jal_with_destination(rd: u8) -> RiscvInstruction {
    RiscvInstruction::Jal {
        rd: Register::new(rd).unwrap(),
        offset: Immediate::new(8),
    }
}

fn jalr_with_registers(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Jalr {
        rd: Register::new(rd).unwrap(),
        rs1: Register::new(rs1).unwrap(),
        offset: Immediate::new(0),
    }
}
```

- [ ] **Step 2: Add the mixed-control admission test**

```rust
#[test]
fn scalar_memory_prefix_opens_mixed_no_link_control_paths() {
    let mut window = scalar_load_window(4);

    assert_eq!(
        window.classify_younger(jal_with_destination(0)),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(beq()),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(jalr_with_registers(0, 9)),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert!(window.is_full());
}
```

- [ ] **Step 3: Add destination and RAS rejection tests**

```rust
#[test]
fn scalar_memory_prefix_rejects_link_writing_and_return_controls() {
    for instruction in [
        jal_with_destination(1),
        jal_with_destination(2),
        jalr_with_registers(1, 9),
        jalr_with_registers(2, 9),
        jalr_with_registers(0, 1),
        jalr_with_registers(0, 5),
    ] {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window.classify_younger(instruction),
            RiscvScalarIntegerYoungerDecision::Reject,
            "{instruction:?}"
        );
    }
}
```

- [ ] **Step 4: Add producer-dependent JALR terminal tests**

Use one load destination and one younger ALU destination:

```rust
#[test]
fn live_producer_keeps_no_link_jalr_terminal() {
    let mut load_dependent = scalar_load_window(4);
    assert_eq!(
        load_dependent.classify_younger(jalr_with_registers(0, 4)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );

    let mut alu_dependent = scalar_load_window(4);
    assert_eq!(
        alu_dependent.classify_younger(addi(9, 0)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );
    assert_eq!(
        alu_dependent.classify_younger(jalr_with_registers(0, 9)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );
}
```

- [ ] **Step 5: Run the focused policy tests and verify RED**

Run:

```text
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
```

Expected: the new `JAL`/`JALR` admission assertions fail because current policy returns `Reject`.

### Task 2: Lock Runtime Issue And Rollback Behavior In Failing Tests

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`

- [ ] **Step 1: Add no-link jump builders to the focused runtime tests**

Use `JAL x0` and `JALR x0, x9` builders matching Task 1.

- [ ] **Step 2: Add a generic control candidate shape test**

Stage load, `JAL x0`, direct conditional, and `JALR x0, x9`; assert each control candidate has no destination, its instruction identity is retained, and the runtime records immediate control dependencies:

```rust
assert_eq!(jal_candidate.destination(), None);
assert_eq!(jalr_candidate.destination(), None);
assert_eq!(runtime.live_control_dependencies.get(&branch_sequence), Some(&jal_sequence));
assert_eq!(runtime.live_control_dependencies.get(&jalr_sequence), Some(&branch_sequence));
```

- [ ] **Step 3: Add targeted descendant cleanup coverage**

After staging the same chain, call:

```rust
runtime.discard_live_control_descendants_from_at(branch_sequence, 12);
```

Assert the load, `JAL`, and repairing conditional remain while `JALR` and its speculative timing are removed.

- [ ] **Step 4: Add branch issue-class coverage**

Configure issue width two, stage a scalar ALU plus `JAL x0` and `JALR x0, x9`, then assert the branch reservation serializes the two controls while allowing cross-resource ALU/branch co-issue. Use exact issue ticks rather than aggregate counts only.

- [ ] **Step 5: Run the focused runtime tests and verify RED**

Run:

```text
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue -- --nocapture
```

Expected: candidate creation or branch issue-class assertions fail for `JAL`/`JALR`.

### Task 3: Add The Top-Level Positive CLI Test And Verify RED

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/mixed_kind.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`

- [ ] **Step 1: Wire the focused child module**

Add:

```rust
#[path = "predicted_control/mixed_kind.rs"]
mod mixed_kind;
```

- [ ] **Step 2: Build the deterministic RV64 fixture**

Create a program with these live rows:

```text
load x12, 0(x10)
jal x0, direct_target
beq x7, x8, conditional_target
jalr x0, 0(x11)
```

Initialize `x11` before the load to the indirect target. Put stores after the skipped `JAL` fallthrough and after the skipped `JALR` fallthrough. Put a success marker and store at the indirect target.

- [ ] **Step 3: Add the direct detailed positive test**

Run with branch lookahead three and assert:

```rust
assert_eq!(
    resident_rob_pcs(&resident),
    [LOAD_PC, JAL_PC, BRANCH_PC, JALR_PC]
);
assert_eq!(resident.pointer("/cores/0/o3_runtime/snapshot/lsq/count").and_then(Value::as_u64), Some(1));
```

Assert all three controls issue before the load response, commit in order, emit branch kinds `direct_unconditional`, `direct_conditional`, and `indirect_unconditional`, leave `x1` and `x5` unchanged, suppress skipped stores, and expose `max_rob_occupancy=4` and `max_lsq_occupancy=1`.

- [ ] **Step 4: Run the exact CLI test and verify RED**

Run:

```text
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_control_kind_commits_direct -- --nocapture
```

Expected: the live ROB lacks the jump controls because current O3 policy rejects them.

### Task 4: Implement The Generic Live-Control Authority

**Files:**
- Modify: `crates/rem6-cpu/src/o3_source_operands.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`

- [ ] **Step 1: Define the typed operand descriptor**

Add a focused type and helper:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveControlOperands {
    kind: BranchTargetKind,
    sources: Vec<Register>,
}

impl O3LiveControlOperands {
    pub(crate) const fn kind(&self) -> BranchTargetKind { self.kind }
    pub(crate) fn sources(&self) -> &[Register] { &self.sources }
}

pub(crate) fn o3_live_control_operands(
    instruction: RiscvInstruction,
) -> Option<O3LiveControlOperands> {
    let kind = riscv_branch_target_kind(instruction);
    let supported = match instruction {
        RiscvInstruction::Beq { .. }
        | RiscvInstruction::Bne { .. }
        | RiscvInstruction::Blt { .. }
        | RiscvInstruction::Bge { .. }
        | RiscvInstruction::Bltu { .. }
        | RiscvInstruction::Bgeu { .. } => true,
        RiscvInstruction::Jal { rd, .. } => rd.is_zero(),
        RiscvInstruction::Jalr { rd, rs1, .. } => {
            rd.is_zero() && !is_riscv_link_register(rs1)
        }
        _ => false,
    };
    supported.then(|| O3LiveControlOperands {
        kind,
        sources: o3_scalar_integer_source_registers(&instruction),
    })
}
```

Use the actual `Register` zero predicate available in the crate. Re-export the helper/type through `o3_runtime.rs` for sibling runtime modules.

- [ ] **Step 2: Remove the conditional-only helper**

Delete `o3_direct_conditional_sources` and replace every consumer with `o3_live_control_operands`.

- [ ] **Step 3: Track live-produced destinations in policy**

Add `live_destinations: Vec<Register>` to `RiscvScalarIntegerLiveWindow`. Initialize it from head/prefix destinations and update it whenever a scalar destination is admitted.

For a supported control:

```rust
let operands = o3_live_control_operands(instruction).unwrap();
let depends_on_unresolved = operands
    .sources()
    .iter()
    .any(|source| self.unresolved_destinations.contains(source));
let jalr_target_is_live = operands.kind() == BranchTargetKind::IndirectUnconditional
    && operands
        .sources()
        .iter()
        .any(|source| self.live_destinations.contains(source));
```

Return `AdmitTerminalControl` and close the window for either unsafe dependency. Otherwise increment control depth and return `AdmitPredictedControl`.

- [ ] **Step 4: Run the policy tests and verify GREEN**

Run:

```text
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
```

Expected: all policy tests pass, including the new mixed and rejection cases.

### Task 5: Implement Runtime Issue, Staging, And Redirect Ownership

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_execute.rs`

- [ ] **Step 1: Generalize live control sequence tracking**

In `stage_live_retire_window` and `stage_live_scalar_memory_younger_window`, use `o3_live_control_operands(instruction).is_some()` instead of direct conditional matching. Preserve immediate control dependency assignment and existing terminal/predicted decisions.

- [ ] **Step 2: Generalize speculative control candidates**

Replace:

```rust
DirectConditional
```

with:

```rust
Control { kind: BranchTargetKind }
```

Create candidates from `o3_live_control_operands`, require no destination/rename mapping, and validate that execution has no writes and the same control kind.

- [ ] **Step 3: Classify every supported control as a branch issue row**

Change `live_issue_op_class` to return `O3IssueOpClass::Branch` whenever `o3_live_control_operands(instruction).is_some()`.

- [ ] **Step 4: Generalize redirect preservation and targeted rollback**

In `riscv_execute.rs`, derive `live_control_sequence` from the generic helper while retaining `instruction_is_conditional_branch` for conditional predictor training.

For any supported live control with no trap:

```rust
if branch_prediction_redirects {
    state
        .o3_runtime
        .discard_live_control_descendants_from_at(sequence, retire_tick);
    state.refresh_o3_writeback_wake(retire_tick);
}
```

Correctly predicted jump descendants remain resident even though the architectural next PC is non-sequential. Unsupported redirects retain broad cleanup.

- [ ] **Step 5: Run runtime and CPU suites and verify GREEN**

Run:

```text
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue -- --nocapture
cargo test -p rem6-cpu --lib --quiet
```

Expected: all pass with no warnings.

### Task 6: Complete The CLI Matrix

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/mixed_kind.rs`

- [ ] **Step 1: Make the direct positive row GREEN**

Run the exact positive test until the four live PCs, branch kinds, issue timing, memory bytes, and no-link assertions pass.

- [ ] **Step 2: Add the lookahead-two negative**

At a tick before the load response, assert the ROB contains exactly load, `JAL`, and conditional branch, with no `JALR` row.

- [ ] **Step 3: Add the hierarchy-backed positive row**

Run the same correct mixed-control path through `cache-fabric-dram`. Assert matching registers, memory bytes, no-link behavior, ordered commit, and nonzero cache, transport, fabric, and DRAM activity.

- [ ] **Step 4: Add the hierarchy-backed rollback row**

Make the middle conditional branch taken under the deterministic initial predictor. Assert:

```rust
assert!(event_at_pc_if_present(&completed, JALR_PC).is_none());
assert_no_data_address(&completed, WRONG_JALR_STORE_ADDRESS);
```

Assert the older `JAL` event remains, the conditional records one repair/squash, and cache, transport, fabric, and DRAM activity are nonzero.

- [ ] **Step 5: Add the producer-dependent JALR terminal row**

Build a variant where `JALR rs1` is the load destination or a live ALU destination. At the resident tick, assert the JALR row may be present but no predicted target descendant is resident or fetched before normal resolution.

- [ ] **Step 6: Add mode-transfer evidence**

Schedule detailed-to-timing transfer after the four rows are resident and before the load response. Compare baseline and switched issue/writeback/commit ticks for every live row and assert the transfer remains non-restorable with four ROB rows, one LSQ row, and three younger rows.

- [ ] **Step 7: Add checkpoint boundary evidence**

Reject a checkpoint while all four mixed-control rows are live. After the window drains, capture and restore a checkpoint, assert no live-data handoff chunk, and assert the decoded O3 runtime has zero ROB and LSQ rows.

- [ ] **Step 8: Add timing-mode suppression**

Assert architectural and memory results match, while `/debug/o3_trace` is empty and no `sim.cpu0.o3.*` or `system.cpu.*` O3 aliases are present.

- [ ] **Step 9: Run the focused and complete predicted-control matrix**

Run:

```text
cargo test -p rem6 --test cli_run mixed_control_kind -- --nocapture
cargo test -p rem6 --test cli_run predicted_control -- --nocapture
```

Expected: all mixed-kind and existing predicted-control tests pass.

### Task 7: Add Mechanical Ownership And Ledger Evidence

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add source-policy ownership checks**

Assert `o3_source_operands.rs` owns `O3LiveControlOperands` and `o3_live_control_operands`, old `o3_direct_conditional_sources` is absent, and policy/runtime/issue/execute consumers reference the generic helper rather than parallel branch opcode lists.

- [ ] **Step 2: Add exact CLI anchors**

Add every new top-level test function name to `core_test_anchors.txt`.

- [ ] **Step 3: Update the CPU ledger without changing score**

Replace CPU evidence prose to record the direct/hierarchy mixed-kind matrix, no-link behavior, dependent-JALR terminal boundary, rollback, transfer, and timing suppression. Keep:

```text
### CPU Execution Models - 74% representative
```

and leave the general O3 and KVM checklist items unchecked. Narrow the remaining gap to link-writing calls/returns, producer-forwarded indirect targets, fourth/deeper chains, arbitrary mixed windows, restorable transport, and a general O3 engine.

- [ ] **Step 4: Verify source policy and the exact ledger boundary**

Run:

```text
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: both source-policy suites pass and the ledger reports exactly `1200` lines.

### Task 8: Verify, Review, Commit, And Push

**Files:**
- Review every file listed in the File Map.

- [ ] **Step 1: Run focused verification**

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run predicted_control --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
git diff --check
```

- [ ] **Step 2: Run workspace verification**

```text
cargo test --workspace --all-targets --quiet
```

Expected: exit code zero and no failing targets.

- [ ] **Step 3: Request high-intensity read-only review**

Reviewers must check behavior, target-source safety, redirect cleanup, issue/writeback ownership, dead code, source-policy quality, ledger honesty, and test strength. Fix every confirmed finding and rerun the affected gates.

- [ ] **Step 4: Inspect the final diff**

```text
git status --short --branch
git diff --stat
git diff --check
```

Confirm no `temp/` path is staged or committed.

- [ ] **Step 5: Commit behavior-scoped increments**

Use English commit subjects without attribution. Suitable boundaries are:

```text
docs: design mixed O3 control kinds
cpu: support mixed no-link O3 controls
docs: record mixed O3 control evidence
```

- [ ] **Step 6: Push and verify remote main**

```text
git push origin main
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
git ls-remote origin refs/heads/main
```

Expected: clean synchronized worktree and identical local, tracking, and remote SHAs.
