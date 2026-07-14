# RISC-V O3 Predicted Control Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a bounded detailed-mode O3 window in which an independent direct conditional branch and its predicted scalar descendants issue before an older load responds, while correct-path retirement remains ordered and mispredicted descendants roll back cleanly.

**Architecture:** Reuse the existing branch predictor/speculation maps and the existing v7 scalar-memory handoff. Extract transient live issue logic from the full `o3_runtime_live_window.rs` into a focused control-window module, teach the detailed fetch walker to follow one recorded predicted path, and generalize transient execution records to accept no-destination direct conditional branches plus scalar M-extension descendants. Normal RISC-V retirement remains authoritative for architectural writes, branch repair, redirect, and commit.

**Tech Stack:** Rust, existing `rem6-cpu` O3 runtime and RISC-V fetch/retire drivers, `rem6 run --execute` CLI integration tests, Cargo test/source-policy harnesses.

---

## File Structure

Production ownership after this increment:

- Create `crates/rem6-cpu/src/o3_runtime_control_window.rs`: transient scalar/control issue candidates, source forwarding, execution validation, producer-chain validation, and rollback helpers.
- Modify `crates/rem6-cpu/src/o3_runtime.rs`: register the focused module and import its private runtime types.
- Modify `crates/rem6-cpu/src/o3_runtime_live_window.rs`: keep ROB staging, ordered retirement, rename publication, and scalar-memory younger-row ownership; delegate transient issue behavior.
- Modify `crates/rem6-cpu/src/o3_source_operands.rs`: define the exact direct-conditional and post-branch scalar descendant operand allowlists.
- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: distinguish dependent terminal branches from independently issuable predicted controls and admit bounded post-control descendants.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`: expose an already-decoded branch as a normal prediction decision and follow its recorded predicted PC for later fetches.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`: convert the detailed-control candidate into the existing `fetch_ahead_decision` path so speculation is recorded exactly once.
- Modify `crates/rem6-cpu/src/riscv_live_retire_window.rs`: build the staged younger window across one predicted control edge and record transient branch/descendant execution.
- Modify `crates/rem6-cpu/src/riscv_execute.rs`: preserve correct-path descendants and invoke branch-boundary rollback before generic redirect cleanup on a mismatch.

Focused tests and evidence:

- Create `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`.
- Create `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3.rs` to register the focused CLI module.
- Modify `crates/rem6-cpu/tests/source_policy.rs` to enforce the control-window module boundary.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt` and `docs/architecture/gem5-to-rem6-migration.md` for executable evidence only.

### Task 1: Extract Transient Control-Window Ownership

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Write the failing source-policy test**

Add a focused ownership test:

```rust
#[test]
fn o3_runtime_control_window_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join("src/o3_runtime.rs")).unwrap();
    let live = fs::read_to_string(crate_dir.join("src/o3_runtime_live_window.rs")).unwrap();
    let module_path = crate_dir.join("src/o3_runtime_control_window.rs");

    assert!(root.contains("mod o3_runtime_control_window;"));
    assert!(module_path.exists());
    let module = fs::read_to_string(module_path).unwrap();
    for anchor in [
        "struct O3LiveSpeculativeExecution",
        "struct O3LiveSpeculativeIssueCandidate",
        "fn live_speculative_issue_candidate",
        "fn record_live_speculative_execution",
        "fn live_speculative_source_forwarding",
        "fn invalidate_live_speculative_execution_chain",
    ] {
        assert!(module.contains(anchor), "missing control owner {anchor}");
        assert!(!live.contains(anchor), "live-window module still owns {anchor}");
    }
}
```

- [ ] **Step 2: Run the source-policy test and verify the missing module failure**

Run:

```bash
cargo test -p rem6-cpu --test source_policy o3_runtime_control_window_lives_in_focused_module -- --exact --nocapture
```

Expected: FAIL because `src/o3_runtime_control_window.rs` and its module declaration do not exist.

- [ ] **Step 3: Extract behavior without changing semantics**

Move these existing private types and methods verbatim from
`o3_runtime_live_window.rs` into the new module:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveSpeculativeExecution {
    pub(super) consumed_requests: Vec<MemoryRequestId>,
    pub(super) sequence: u64,
    pub(super) producer_sequences: Vec<u64>,
    pub(super) issue_tick: u64,
    pub(super) execution: RiscvExecutionRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveSpeculativeIssueCandidate {
    pub(super) sequence: u64,
    pub(super) pc: Address,
    pub(super) instruction: RiscvInstruction,
    pub(super) destination: O3RenameMapEntry,
    pub(super) producer_sequences: Vec<u64>,
    pub(super) forwarded_register_writes: Vec<RegisterWrite>,
    pub(super) dependency_ready_tick: u64,
}
```

Register it in `o3_runtime.rs`:

```rust
#[path = "o3_runtime_control_window.rs"]
mod o3_runtime_control_window;
```

Import the moved types from `o3_runtime_control_window` and retain
`O3LiveRetiredInstruction` plus staging/retirement helpers in
`o3_runtime_live_window`.

- [ ] **Step 4: Run focused runtime and policy tests**

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime_live_window::tests -- --nocapture
cargo test -p rem6-cpu --test source_policy o3_runtime_control_window_lives_in_focused_module -- --exact --nocapture
```

Expected: all existing live-window tests PASS and the new ownership test PASS.

- [ ] **Step 5: Commit the structural extraction**

```bash
git add crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_live_window.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/tests/source_policy.rs
git commit -m "refactor: isolate O3 control window state"
```

### Task 2: Define Predicted-Control Admission and Operand Boundaries

**Files:**
- Modify: `crates/rem6-cpu/src/o3_source_operands.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`

- [ ] **Step 1: Add failing policy tests**

Add tests that lock these outcomes:

```rust
#[test]
fn independent_branch_opens_one_predicted_control_path() {
    let mut window = scalar_load_window(4);
    assert_eq!(
        window.classify_younger(beq()),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(mul(7, 5, 6)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );
    assert_eq!(
        window.classify_younger(addi(8, 7)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );
    assert!(window.is_full());
}

#[test]
fn load_dependent_branch_remains_terminal() {
    let mut window = scalar_load_window(4);
    assert_eq!(
        window.classify_younger(beq_with_sources(4, 0)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );
    assert_eq!(
        window.classify_younger(addi(8, 0)),
        RiscvScalarIntegerYoungerDecision::Reject
    );
}

#[test]
fn predicted_control_rejects_memory_and_second_control_rows() {
    for instruction in [scalar_load(), beq(), jal(), RiscvInstruction::Ecall] {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window.classify_younger(bne()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(instruction),
            RiscvScalarIntegerYoungerDecision::Reject
        );
    }
}
```

- [ ] **Step 2: Run the policy tests and verify the missing variant failure**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_o3_window_policy::tests -- --nocapture
```

Expected: compilation FAIL because `AdmitPredictedControl` and post-control M-extension admission do not exist.

- [ ] **Step 3: Add exact operand helpers**

Add these APIs in `o3_source_operands.rs`:

```rust
pub(crate) fn o3_direct_conditional_sources(
    instruction: RiscvInstruction,
) -> Option<Vec<Register>> {
    matches!(
        instruction,
        RiscvInstruction::Beq { .. }
            | RiscvInstruction::Bne { .. }
            | RiscvInstruction::Blt { .. }
            | RiscvInstruction::Bge { .. }
            | RiscvInstruction::Bltu { .. }
            | RiscvInstruction::Bgeu { .. }
    )
    .then(|| o3_scalar_integer_source_registers(&instruction))
}

pub(crate) fn o3_predicted_scalar_descendant_operands(
    instruction: RiscvInstruction,
) -> Option<(Register, Vec<Register>)> {
    let supported = o3_speculative_scalar_alu_operands(instruction).is_some()
        || matches!(
            instruction,
            RiscvInstruction::Mul { .. }
                | RiscvInstruction::Mulh { .. }
                | RiscvInstruction::Mulhsu { .. }
                | RiscvInstruction::Mulhu { .. }
                | RiscvInstruction::Mulw { .. }
        );
    supported.then(|| {
        (
            o3_scalar_integer_destination(instruction)
                .expect("predicted scalar descendant has an integer destination"),
            o3_scalar_integer_source_registers(&instruction),
        )
    })
}
```

Keep divide/remainder descendants out of this increment so the matrix has one
bounded, deterministic FU-latency family.

- [ ] **Step 4: Implement the policy state machine**

Add `control_open: bool` to `RiscvScalarIntegerLiveWindow`, initialize it to
`false`, and add the enum variant:

```rust
pub(crate) enum RiscvScalarIntegerYoungerDecision {
    AdmitContinue,
    AdmitStop,
    AdmitTerminalControl,
    AdmitPredictedControl,
    Reject,
}
```

In `classify_younger`:

```rust
if self.control_open {
    let Some((destination, sources)) = o3_predicted_scalar_descendant_operands(instruction) else {
        return RiscvScalarIntegerYoungerDecision::Reject;
    };
    if destination.is_zero()
        || sources
            .iter()
            .any(|source| self.unresolved_destinations.contains(source))
    {
        return RiscvScalarIntegerYoungerDecision::Reject;
    }
    self.rows += 1;
    return RiscvScalarIntegerYoungerDecision::AdmitContinue;
}

if self.admits_terminal_control {
    if let Some(sources) = o3_direct_conditional_sources(instruction) {
        self.rows += 1;
        if sources
            .iter()
            .any(|source| self.unresolved_destinations.contains(source))
        {
            return RiscvScalarIntegerYoungerDecision::AdmitTerminalControl;
        }
        self.control_open = true;
        return RiscvScalarIntegerYoungerDecision::AdmitPredictedControl;
    }
}
```

- [ ] **Step 5: Run policy and operand tests**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_o3_window_policy::tests -- --nocapture
cargo test -p rem6-cpu --lib o3_source_operands -- --nocapture
```

Expected: PASS, including all pre-existing terminal-branch and scalar-ALU tests.

- [ ] **Step 6: Commit admission policy**

```bash
git add crates/rem6-cpu/src/o3_source_operands.rs crates/rem6-cpu/src/riscv_o3_window_policy.rs
git commit -m "cpu: define predicted control admission"
```

### Task 3: Follow the Existing Prediction in Detailed Fetch-Ahead

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`

- [ ] **Step 1: Register the focused test module and add failing fetch tests**

Register:

```rust
mod detailed_o3_control;
```

Add tests that construct a completed load plus completed branch and assert:

```rust
#[test]
fn detailed_scalar_window_returns_existing_branch_prediction_decision() {
    let core = detailed_scalar_load_core_with_completed_branch(false);
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8008));
    let speculation = decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x8004));
    assert!(!speculation.predicted_taken());
}

#[test]
fn detailed_scalar_window_follows_recorded_taken_target() {
    let core = detailed_scalar_load_core_with_recorded_branch(true, 0x8010);
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8010));
    assert!(decision.branch_speculation().is_none());
}

#[test]
fn dependent_terminal_branch_does_not_open_descendant_fetch() {
    let core = detailed_scalar_load_core_with_dependent_branch();
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
```

- [ ] **Step 2: Run the focused tests and verify the terminal-block failure**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control -- --nocapture
```

Expected: FAIL because detailed O3 maps `AdmitPredictedControl` to `Blocked` and cannot follow a recorded branch PC.

- [ ] **Step 3: Add a detailed control candidate**

Extend `DetailedFetchAheadCandidate`:

```rust
ReadyPredictedControl {
    request: MemoryRequestId,
    pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
},
```

When `scalar_integer_window_candidate_from` sees
`AdmitPredictedControl`, return that candidate if the branch request has no
entry in `state.branch_speculations`. If the entry exists, read the pending
speculation from `state.branch_predictor`, choose target for taken or sequential
PC for not-taken, and continue scanning from that PC.

Use one helper:

```rust
fn recorded_predicted_pc(
    state: &RiscvCoreState,
    request: MemoryRequestId,
    sequential_pc: Address,
) -> Option<Address> {
    let id = state.branch_speculations.get(&request.sequence())?;
    let pending = state.branch_predictor.pending_speculation(*id)?;
    if pending.predicted_taken() {
        pending.target()
    } else {
        Some(sequential_pc)
    }
}
```

- [ ] **Step 4: Route the candidate through the existing predictor function**

In `driver.rs`, handle the new candidate by calling the existing
`fetch_ahead_decision`:

```rust
DetailedFetchAheadCandidate::ReadyPredictedControl {
    request,
    pc,
    sequential_pc,
    instruction,
} => {
    return fetch_ahead_decision(
        &mut state,
        &completed,
        request,
        pc,
        sequential_pc,
        instruction,
        translated,
    );
}
```

This preserves the existing prepare/record speculation flow at fetch issue and
prevents duplicate predictor updates.

- [ ] **Step 5: Run detailed fetch and predictor regression tests**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::speculative_history -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::selected -- --nocapture
```

Expected: all PASS with one speculation record per branch.

- [ ] **Step 6: Commit detailed fetch steering**

```bash
git add crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs
git commit -m "cpu: follow predicted O3 control paths"
```

### Task 4: Record Early Branch and Descendant Execution

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`

- [ ] **Step 1: Register focused runtime tests and write failures**

Register the test module from `o3_runtime.rs` under `#[cfg(test)]`:

```rust
#[path = "o3_runtime_control_window_tests.rs"]
mod o3_runtime_control_window_tests;
```

Add tests for these exact boundaries:

```rust
#[test]
fn independent_branch_candidate_has_no_destination_and_issues_early() {
    let mut runtime = staged_load_branch_descendant_runtime();
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), beq())
        .unwrap();
    assert!(candidate.destination().is_none());
    assert_eq!(candidate.issue_tick(11), 11);
}

#[test]
fn unresolved_load_source_rejects_branch_candidate() {
    let runtime = staged_load_dependent_branch_runtime();
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8004), beq_x4_x0())
        .is_none());
}

#[test]
fn predicted_mul_and_dependent_add_form_a_valid_issue_chain() {
    let mut runtime = staged_load_branch_mul_add_runtime();
    record_branch(&mut runtime, 11, 0x8008);
    record_mul(&mut runtime, 12, 42);
    let add = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), add_x8_x7())
        .unwrap();
    assert_eq!(add.issue_tick(12), 13);
}

#[test]
fn discarding_branch_boundary_removes_younger_rename_and_issue_state() {
    let mut runtime = staged_load_branch_mul_add_runtime();
    let committed = runtime.snapshot().rename_map().to_vec();
    let branch_sequence = runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(0x8004))
        .unwrap()
        .sequence();
    runtime.discard_live_control_descendants_from(branch_sequence);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(runtime.snapshot().rename_map(), committed);
    assert_eq!(runtime.live_speculative_executions.len(), 1);
}
```

Define `request`, `scalar_load_event`, `addi`, and `add` in the new test file
with the exact implementations currently used by
`o3_runtime_live_window::tests`. Add local `beq`, `beq_x4_x0`, `mul`, and
`add_x8_x7` instruction constructors, then build each
`staged_load_*_runtime` helper by calling `stage_live_scalar_memory_issue`
followed by `stage_live_scalar_memory_younger_window` with the PCs shown in the
assertions.

- [ ] **Step 2: Run the focused tests and verify branch candidate failure**

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime_control_window_tests -- --nocapture
```

Expected: FAIL because the candidate requires a destination and rejects control execution.

- [ ] **Step 3: Generalize the transient candidate shape**

Change the candidate destination to optional and add a kind:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveSpeculativeIssueKind {
    Scalar { destination: O3RenameMapEntry },
    DirectConditional,
}

pub(crate) struct O3LiveSpeculativeIssueCandidate {
    sequence: u64,
    pc: Address,
    instruction: RiscvInstruction,
    kind: O3LiveSpeculativeIssueKind,
    producer_sequences: Vec<u64>,
    forwarded_register_writes: Vec<RegisterWrite>,
    dependency_ready_tick: u64,
}
```

Keep these candidate accessors:

```rust
impl O3LiveSpeculativeIssueCandidate {
    pub(crate) const fn destination(&self) -> Option<O3RenameMapEntry> {
        match self.kind {
            O3LiveSpeculativeIssueKind::Scalar { destination } => Some(destination),
            O3LiveSpeculativeIssueKind::DirectConditional => None,
        }
    }

    pub(crate) fn forwarded_register_writes(&self) -> &[RegisterWrite] {
        &self.forwarded_register_writes
    }

    pub(crate) fn issue_tick(&self, earliest_tick: u64) -> u64 {
        earliest_tick.max(self.dependency_ready_tick)
    }
}
```

Candidate construction must:

1. Use `o3_direct_conditional_sources` for direct conditional branches.
2. Use `o3_predicted_scalar_descendant_operands` for scalar descendants.
3. Require no rename destination for a branch and the exact staged rename
   destination for a scalar row.
4. Reuse `live_speculative_source_forwarding`, which already rejects an
   unresolved load producer.

- [ ] **Step 4: Validate branch versus scalar execution records**

In `record_live_speculative_execution`, keep common checks for fetch identity,
PC, instruction, traps, system events, memory, and FP writes. Then match kind:

```rust
let valid_kind = match candidate.kind {
    O3LiveSpeculativeIssueKind::Scalar { destination } => {
        execution.next_pc()
            == execution.pc().wrapping_add(u64::from(execution.instruction_bytes()))
            && execution.register_writes().len() == 1
            && execution_writes_rename_destination(&execution, destination)
    }
    O3LiveSpeculativeIssueKind::DirectConditional => {
        execution.register_writes().is_empty()
            && o3_direct_conditional_sources(execution.instruction()).is_some()
    }
};
```

Record both kinds in the existing `O3LiveSpeculativeExecution` vector.

- [ ] **Step 5: Add branch-boundary rollback**

Implement:

```rust
pub(crate) fn discard_live_control_descendants_from(&mut self, branch_sequence: u64) {
    self.snapshot.reorder_buffer.retain(|entry| {
        !entry.is_live_staged() || entry.sequence() <= branch_sequence
    });
    self.live_scalar_memory_younger_sequences
        .retain(|sequence| *sequence <= branch_sequence);
    self.live_speculative_executions
        .retain(|execution| execution.sequence <= branch_sequence);
    self.live_retired_instructions
        .retain(|instruction| instruction.sequence <= branch_sequence);
    self.stats
        .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
}
```

The generic full-window discard remains unchanged for traps and redirects that
invalidate the branch itself.

- [ ] **Step 6: Run focused and existing live-window tests**

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime_control_window_tests -- --nocapture
cargo test -p rem6-cpu --lib o3_runtime_live_window::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit transient execution support**

```bash
git add crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_control_window_tests.rs crates/rem6-cpu/src/o3_runtime_live_window.rs
git commit -m "cpu: issue predicted control windows"
```

### Task 5: Stage the Predicted Path and Integrate Branch Retirement

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_execute.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`

- [ ] **Step 1: Add failing integration tests for correct and wrong paths**

Add focused tests that build fetch events across one branch edge and assert the
staged PC sequence:

```rust
assert_eq!(
    runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .map(|entry| entry.pc().get())
        .collect::<Vec<_>>(),
    [0x8000, 0x8004, 0x8008, 0x800c]
);
```

Add a branch-retirement test where the recorded path is fall-through and the
actual branch is taken. After retirement, assert only rows through the branch
remain and the committed rename map has no descendant destinations.

- [ ] **Step 2: Run tests and verify the sequential-window failure**

Run:

```bash
cargo test -p rem6-cpu --lib predicted_control -- --nocapture
```

Expected: FAIL because `completed_fetch_instruction_window` always advances to
the sequential PC and `AdmitPredictedControl` does not continue staging.

- [ ] **Step 3: Traverse one recorded control edge**

Change the completed-window helper to derive the next PC after each accepted
instruction:

```rust
fn next_live_window_pc(
    state: &RiscvCoreState,
    instruction: &RiscvCompletedFetchInstruction,
    decision: RiscvScalarIntegerYoungerDecision,
) -> Option<Address> {
    let sequential = Address::new(
        instruction.pc().get() + u64::from(instruction.decoded().bytes()),
    );
    if decision == RiscvScalarIntegerYoungerDecision::AdmitPredictedControl {
        recorded_predicted_pc(state, instruction.last_consumed_request(), sequential)
    } else {
        Some(sequential)
    }
}
```

Keep accepting after `AdmitPredictedControl`; stop after
`AdmitTerminalControl`, `AdmitStop`, `Reject`, or the row limit.

- [ ] **Step 4: Execute accepted rows transiently**

In `record_o3_live_speculative_younger_executions`, continue using a cloned
hart, apply forwarded writes, execute the decoded instruction, and record it.
The generalized candidate now accepts the branch and the post-branch MUL/ALU.

Use `candidate.issue_tick(issue_tick)` for the recorded tick so dependency
readiness controls the dependent descendant.

- [ ] **Step 5: Preserve or roll back descendants at branch retirement**

Before generic `discard_live_staged_instructions`, derive the runtime branch
sequence from the retiring staged row. When the branch prediction redirects,
call `discard_live_control_descendants_from(branch_sequence)` so the branch row
can finish retirement while younger state is removed. A trap or non-control
redirect continues to use the full-window discard.

Do not bypass `retire_branch_predictions`; it remains responsible for predictor
repair and fetch cleanup.

- [ ] **Step 6: Run CPU integration regressions**

Run:

```bash
cargo test -p rem6-cpu --lib predicted_control -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests -- --nocapture
cargo test -p rem6-cpu --lib o3_runtime_live_window::tests -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit live-driver integration**

```bash
git add crates/rem6-cpu/src/riscv_live_retire_window.rs crates/rem6-cpu/src/riscv_execute.rs crates/rem6-cpu/src/o3_runtime_control_window_tests.rs
git commit -m "cpu: retire predicted O3 control paths"
```

### Task 6: Add Direct, Hierarchy, and Suppression CLI Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`

- [ ] **Step 1: Register the CLI module and add the failing direct row**

Register:

```rust
#[path = "o3/predicted_control.rs"]
mod predicted_control;
```

Build a real RV64 ELF containing:

```text
load x12, [data]
beq  x5, x6, target      # independent, predicted not taken
mul  x13, x7, x8         # predicted descendant
add  x14, x13, x9        # dependent descendant
target witness and m5 exit
```

The direct test must assert:

```rust
assert!(event_u64(branch, "issue_tick") < event_u64(load, "lsq_data_response_tick"));
assert!(event_u64(mul, "issue_tick") < event_u64(load, "lsq_data_response_tick"));
assert_eq!(event_u64(add, "issue_tick"), event_u64(mul, "writeback_tick"));
assert_eq!(snapshot_rob_pcs(&resident), [LOAD_PC, BRANCH_PC, MUL_PC, ADD_PC]);
assert_eq!(resident.pointer("/cores/0/o3_runtime/snapshot/lsq/count").and_then(Value::as_u64), Some(1));
```

Also assert exact final `x12`, `x13`, `x14`, branch direction, zero squash, and
nondecreasing commit ticks.

- [ ] **Step 2: Run the direct row and verify it fails before implementation is complete**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_o3_predicted_descendants_commit_direct -- --exact --nocapture
```

Expected: FAIL because the branch/descendant live path is not yet observable
through the top-level driver or issue ticks are not early.

- [ ] **Step 3: Add the taken-misprediction hierarchy row**

Use the same load delay through `cache-fabric-dram`, choose register values that
make the branch taken while the cold basic predictor selects fall-through, and
put a sentinel register write plus store on the wrong path.

Assert:

1. Branch and wrong-path scalar descendants have issue ticks before load
   response.
2. Branch event reports predicted-not-taken, resolved-taken, mispredicted, and
   squashed.
3. Wrong-path destination registers retain their pre-window values.
4. Wrong-path store address appears in neither Data nor Memory traces.
5. The target witness executes once.
6. Cache, transport, fabric, and DRAM activity are all nonzero.
7. Final live snapshot and drained checkpoint contain no wrong-path rename row.

Pair this row with a focused runtime test that records branch, MUL, and ADD
speculative issue state before invoking branch-boundary descendant discard.

Add a trained correct-taken direct row that executes the same branch twice,
selects the taken target on the second prediction, and proves the target MUL
and dependent ADD retain pre-response issue timing without a refetch.

- [ ] **Step 4: Add the load-dependent branch suppression row**

Use `beq x12, x0, target`, where `x12` is the outstanding load destination.
Assert branch `issue_tick >= lsq_data_response_tick`, descendant PCs are absent
from the resident ROB before response, no descendant O3 event exists before
normal branch execution, and architecture still reaches the correct target.

- [ ] **Step 5: Run the three-row matrix**

Run:

```bash
cargo test -p rem6 --test cli_run rem6_run_o3_predicted -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_o3_load_dependent_branch_suppresses_predicted_descendants -- --exact --nocapture
```

Expected: PASS.

- [ ] **Step 6: Add stable source-policy anchors and commit**

Add these exact test names to `core_test_anchors.txt`:

```text
rem6_run_o3_predicted_descendants_commit_direct
rem6_run_o3_correctly_predicted_taken_descendants_commit_direct
rem6_run_o3_predicted_descendants_squash_cache_fabric_dram
rem6_run_o3_load_dependent_branch_suppresses_predicted_descendants
```

Commit:

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs crates/rem6/tests/source_policy/core_test_anchors.txt
git commit -m "test: cover predicted O3 control paths"
```

### Task 7: Prove Mode Transfer, Checkpoint Cleanup, and Timing Suppression

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`

- [ ] **Step 1: Add the detailed-to-timing transfer regression row**

Derive a switch tick strictly after branch and descendant issue and before the
load response. Assert the state transfer:

```rust
assert_eq!(handoff.pointer("/schema_version").and_then(Value::as_u64), Some(7));
assert_eq!(handoff.pointer("/resident_rows").and_then(Value::as_u64), Some(1));
assert_eq!(handoff.pointer("/younger_rows").and_then(Value::as_u64), Some(3));
assert_eq!(runtime.pointer("/snapshot_rob_entries").and_then(Value::as_u64), Some(4));
assert_eq!(runtime.pointer("/snapshot_lsq_entries").and_then(Value::as_u64), Some(1));
assert_eq!(transfer.pointer("/restorable").and_then(Value::as_bool), Some(false));
```

Compare load, branch, MUL, and ADD issue/writeback/commit ticks to the no-switch
baseline.

- [ ] **Step 2: Run the transfer row**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_host_switch_transfers_o3_predicted_descendants -- --exact --nocapture
```

Expected: PASS because execution-mode transfer keeps the live runtime in place
and the existing v7 handoff already serializes the scalar-memory plus generic
ROB/LSQ authority. A schema change is a test failure, not an accepted fix.

- [ ] **Step 3: Add live-reject and drained-restore checkpoint rows**

For the live checkpoint, assert nonzero exit, empty stdout, and stderr containing
`checkpoint component is not quiescent: cpu0`.

For the drained checkpoint/restore row, assert one successful checkpoint, zero
ROB/LSQ entries in its decoded O3 runtime chunk, no `o3-live-data-handoff`
chunk, no wrong-path rename destination, and baseline-equivalent final
registers after restore.

- [ ] **Step 4: Add the timing-mode suppression row**

Run the correct-path binary with `--m5-switch-cpu-mode timing`. Assert exact
architecture, no `/cores/0/o3_runtime`, an empty O3 trace, and no stats whose
path begins with `sim.cpu0.o3.` or the gem5 O3 alias prefixes.

- [ ] **Step 5: Run lifecycle controls**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_host_switch_transfers_o3_predicted_descendants -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_o3_predicted_descendant_checkpoint_boundary -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_timing_suppresses_o3_predicted_descendants -- --exact --nocapture
```

Expected: PASS.

- [ ] **Step 6: Anchor and commit lifecycle evidence**

Add the three exact test names to `core_test_anchors.txt`, then commit:

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs crates/rem6/tests/source_policy/core_test_anchors.txt
git commit -m "test: prove predicted control lifecycle"
```

### Task 8: Update the Migration Ledger Without Score Inflation

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update the CPU evidence in place**

In the CPU section:

1. Keep `### CPU Execution Models - 74% representative`.
2. Keep `8 of 10` and both unchecked items unchanged.
3. Add one concise migrated paragraph covering independent branch issue,
   predicted scalar descendants, correct-path ordered commit, taken
   misprediction rollback, dependency suppression, direct/hierarchy rows,
   schema-v7 transfer reuse, live checkpoint rejection, drained restore, and
   timing suppression.
4. Remove `predicted-path descendants` and `actual branch issue` from the open
   gap sentence.
5. Retain general issue scheduling, arbitrary mixed memory/control windows,
   restorable transport ownership, wider control classes, and KVM as open.

- [ ] **Step 2: Preserve the exact ledger line count**

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: exactly `1200` lines. Fold prose into existing paragraphs rather than
adding blank lines.

- [ ] **Step 3: Run ledger and anchor policy tests**

Run:

```bash
cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --exact --nocapture
cargo test -p rem6 --test source_policy architecture_docs_have_clear_boundaries -- --exact --nocapture
```

- [ ] **Step 4: Commit the evidence update**

```bash
git add docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record predicted control evidence"
```

### Task 9: Verify, Review, Commit Fixes, and Push

**Files:**
- Review every path changed since `1efbe1929dfcca3e170b9db15cac1b2fa6c34b2b`

- [ ] **Step 1: Run focused verification**

```bash
cargo test -p rem6-cpu predicted_control -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_predicted -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_o3_correctly_predicted_taken_descendants_commit_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_host_switch_transfers_o3_predicted_descendants -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::rem6_run_o3_predicted_descendant_checkpoint_boundary -- --exact --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
git diff --check origin/main..HEAD
```

Expected: every command exits 0.

- [ ] **Step 2: Run the full workspace suite**

```bash
cargo test --workspace --all-targets --quiet
```

Expected: exit 0. Do not widen this increment to pre-existing clippy warnings;
run clippy only for touched crates without `-D warnings` if a compile diagnostic
requires it.

- [ ] **Step 3: Run the mandatory xhigh read-only whole-diff review**

Review `1efbe192..HEAD` for:

1. Duplicate predictor or redirect authority.
2. Branches issuing with unresolved operands.
3. Wrong-path ROB, rename, dependency, fetch, or memory state surviving squash.
4. Incorrect issue/writeback/commit timing reuse.
5. Schema-v7 or historical decoder regressions.
6. Timing-mode O3 leakage.
7. Ledger score inflation or line-count drift.
8. Missing direct/hierarchy/suppression/lifecycle matrix rows.
9. Split-fetch prediction identity and correctly predicted taken-path retention.

- [ ] **Step 4: Fix every review finding test-first**

For each finding, add or strengthen the smallest reproducing test, verify it
fails, apply the focused fix, rerun that test, then rerun Step 1. Commit review
fixes separately:

```bash
git add crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_control_window_tests.rs crates/rem6-cpu/src/o3_runtime_live_window.rs crates/rem6-cpu/src/o3_source_operands.rs crates/rem6-cpu/src/riscv_o3_window_policy.rs crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs crates/rem6-cpu/src/riscv_live_retire_window.rs crates/rem6-cpu/src/riscv_execute.rs crates/rem6-cpu/tests/source_policy.rs crates/rem6/tests/cli_run/m5_host_actions/o3.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md
git commit -m "fix: harden predicted control window"
```

- [ ] **Step 5: Confirm repository hygiene**

Run:

```bash
git status --short --branch
git diff --name-only origin/main..HEAD
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: no unstaged files, no `temp/` path in the commit range, and a 1,200
line ledger.

- [ ] **Step 6: Push main and verify the remote head**

```bash
git push origin main
git rev-parse HEAD
git rev-parse origin/main
```

Expected: local and remote hashes match.
