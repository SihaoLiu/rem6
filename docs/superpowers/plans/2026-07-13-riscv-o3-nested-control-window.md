# RISC-V O3 Nested Control Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the bounded detailed-mode O3 control window from one direct conditional branch to two nested direct conditional branches plus one scalar descendant, with distinct outer- and inner-misprediction rollback evidence.

**Architecture:** Keep the four-row `load + branch 1 + branch 2 + scalar descendant` shape. Generalize the existing scalar-window admission policy from one open control edge to a maximum depth of two, retain immediate control ancestry in `live_control_dependencies`, and reuse the current prediction, transient execution, ROB/rename, architectural retirement, v7 handoff, and checkpoint boundaries. No new predictor, issue scheduler, handoff schema, or restorable transport state is introduced.

**Tech Stack:** Rust, existing `rem6-cpu` detailed O3 runtime, RISC-V branch speculation and fetch-ahead drivers, `rem6 run --execute` CLI integration tests, Cargo source-policy tests, and the canonical 1,200-line migration ledger.

---

## File Structure

Production ownership:

- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: replace the one-control boolean with a bounded two-control depth and keep dependency suppression authoritative.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window.rs`: keep immediate control ancestry and sequence-bounded rollback here, and prune dependency edges for every execution removed by stale-identity chain invalidation.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`: prove nested ancestry, validation, and outer/inner rollback.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`: prove two recorded predictions are followed in order and split inner branches use their prefix request identity.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`: add the split inner-branch replacement regression while preserving the existing live-retire complete-vector regression.

CLI evidence:

- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`: register a focused nested child module and expose existing helper functions only as far as the child requires.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs`: own the nested binary builder, command wrapper, assertions, and seven top-level rows.

Evidence policy:

- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: require every new CLI row by exact name.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record the executable nested-control matrix while keeping CPU at 74% representative and the file at exactly 1,200 lines.

### Task 1: Admit Two Predicted Control Edges

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`

- [ ] **Step 1: Replace the old second-control rejection test with failing nested-control tests**

Add these tests beside the existing predicted-control policy rows:

```rust
#[test]
fn scalar_memory_prefix_opens_two_predicted_control_paths() {
    let mut window = scalar_load_window(4);

    assert_eq!(
        window.classify_younger(beq()),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(bne()),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(mul(7, 5, 6)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );
    assert!(window.is_full());
}

#[test]
fn load_dependent_inner_branch_remains_terminal() {
    let mut window = scalar_load_window(4);

    assert_eq!(
        window.classify_younger(beq()),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
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
fn nested_control_rejects_third_branch_and_memory_descendants() {
    for instruction in [beq(), scalar_load(), jal(), RiscvInstruction::Ecall] {
        let mut window = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(4).unwrap()],
            1,
            O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS,
        )
        .unwrap();

        assert_eq!(
            window.classify_younger(beq()),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
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

Use a row limit of four. Do not increase `O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS` to make the third-control case fit.

- [ ] **Step 2: Run the policy tests and verify the second branch fails admission**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_o3_window_policy::tests::scalar_memory_prefix_opens_two_predicted_control_paths -- --exact --nocapture
cargo test -p rem6-cpu --lib riscv_o3_window_policy::tests::load_dependent_inner_branch_remains_terminal -- --exact --nocapture
```

Expected: the first test FAILS because the current `control_open` path rejects a second branch. The dependency test must also fail until branch classification is checked at both control depths.

- [ ] **Step 3: Replace `control_open` with a bounded depth**

Change the policy state to:

```rust
pub(crate) struct RiscvScalarIntegerLiveWindow {
    unresolved_destinations: Vec<Register>,
    rows: usize,
    row_limit: usize,
    admits_terminal_control: bool,
    control_depth: usize,
    control_closed: bool,
}

const MAX_PREDICTED_CONTROL_DEPTH: usize = 2;
```

Initialize `control_depth` to zero. In `classify_younger`, classify an eligible direct conditional before the post-control scalar allowlist:

```rust
if self.admits_terminal_control && scalar_integer_terminal_control(instruction) {
    if self.control_depth >= MAX_PREDICTED_CONTROL_DEPTH {
        return RiscvScalarIntegerYoungerDecision::Reject;
    }
    let sources = o3_direct_conditional_sources(instruction)
        .expect("terminal scalar control has direct conditional sources");
    self.rows += 1;
    if sources
        .iter()
        .any(|source| self.unresolved_destinations.contains(source))
    {
        self.control_closed = true;
        return RiscvScalarIntegerYoungerDecision::AdmitTerminalControl;
    }
    self.control_depth += 1;
    return RiscvScalarIntegerYoungerDecision::AdmitPredictedControl;
}

if self.control_depth > 0 {
    let Some((destination, sources)) = o3_predicted_scalar_descendant_operands(instruction)
    else {
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
    self.unresolved_destinations
        .retain(|unresolved| *unresolved != destination);
    return RiscvScalarIntegerYoungerDecision::AdmitContinue;
}
```

Delete the old `control_open` branch. Preserve `control_closed` so an unresolved branch source terminates all younger admission.

- [ ] **Step 4: Run the complete policy suite**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_o3_window_policy::tests -- --nocapture
```

Expected: all policy tests PASS, including existing one-branch, scalar-FU, memory rejection, and zero-destination rows.

- [ ] **Step 5: Commit the admission boundary**

```bash
git add crates/rem6-cpu/src/riscv_o3_window_policy.rs
git commit -m "cpu: admit nested predicted controls"
```

### Task 2: Prove Immediate Control Ancestry and Rollback Boundaries

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`

- [ ] **Step 1: Add a nested runtime fixture**

Add a helper that stages one load, two branches, and one scalar descendant:

```rust
fn nested_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let outer = beq(5, 6);
    let inner = beq(7, 8);
    let descendant = mul(9, 1, 2);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), outer),
            (Address::new(0x8008), inner),
            (Address::new(0x800c), descendant),
        ],
    );
    (runtime, outer, inner, descendant)
}
```

- [ ] **Step 2: Add failing ancestry and issue-gating tests**

```rust
#[test]
fn nested_control_dependencies_follow_immediate_branch() {
    let (runtime, _, _, _) = nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer();
    let outer = rob[1].sequence();
    let inner = rob[2].sequence();
    let descendant = rob[3].sequence();

    assert_eq!(runtime.live_control_dependencies.get(&inner), Some(&outer));
    assert_eq!(
        runtime.live_control_dependencies.get(&descendant),
        Some(&inner)
    );
}

#[test]
fn inner_control_waits_for_outer_execution_record() {
    let (mut runtime, outer, inner, _) = nested_control_runtime();
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .is_none());

    let outer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), outer)
        .unwrap();
    runtime.record_live_speculative_execution(
        outer_candidate,
        &[request(11)],
        11,
        RiscvExecutionRecord::new(outer, 0x8004, 0x8008, Vec::new(), None),
    );

    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .is_some());
}
```

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime::o3_runtime_control_window_tests::nested_control_dependencies_follow_immediate_branch -- --exact --nocapture
cargo test -p rem6-cpu --lib o3_runtime::o3_runtime_control_window_tests::inner_control_waits_for_outer_execution_record -- --exact --nocapture
```

Expected: PASS after Task 1 if staging already composes correctly. If ancestry is wrong, fix only `stage_live_scalar_memory_younger_window` so each row records the most recently admitted branch sequence.

- [ ] **Step 3: Add outer- and inner-boundary rollback tests**

Record valid speculative executions for both branches and the descendant, then assert the two boundaries separately:

```rust
#[test]
fn outer_control_discard_removes_inner_branch_and_descendant() {
    let (mut runtime, outer, inner, descendant) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();

    runtime.discard_live_control_descendants_from(rob[1].sequence());

    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [Address::new(0x8000), Address::new(0x8004)]
    );
    assert_eq!(runtime.live_speculative_executions.len(), 1);
    assert_eq!(runtime.live_speculative_executions[0].execution.instruction(), outer);
    assert!(runtime
        .live_speculative_executions
        .iter()
        .all(|issued| ![inner, descendant].contains(&issued.execution.instruction())));
}

#[test]
fn inner_control_discard_preserves_outer_branch() {
    let (mut runtime, outer, inner, _) = issued_nested_control_runtime();
    let inner_sequence = runtime.snapshot().reorder_buffer()[2].sequence();

    runtime.discard_live_control_descendants_from(inner_sequence);

    let instructions = runtime
        .live_speculative_executions
        .iter()
        .map(|issued| issued.execution.instruction())
        .collect::<Vec<_>>();
    assert_eq!(instructions, [outer, inner]);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
}
```

Implement `issued_nested_control_runtime` by recording outer, then inner, then descendant candidates in that order. Use `RegisterWrite::new(reg(9), 42)` for the descendant result.

```rust
fn issued_nested_control_runtime() -> (
    O3RuntimeState,
    RiscvInstruction,
    RiscvInstruction,
    RiscvInstruction,
) {
    let (mut runtime, outer, inner, descendant) = nested_control_runtime();
    let outer_execution =
        RiscvExecutionRecord::new(outer, 0x8004, 0x8008, Vec::new(), None);
    let outer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), outer)
        .unwrap();
    runtime.record_live_speculative_execution(
        outer_candidate,
        &[request(11)],
        11,
        outer_execution,
    );

    let inner_execution =
        RiscvExecutionRecord::new(inner, 0x8008, 0x800c, Vec::new(), None);
    let inner_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), inner)
        .unwrap();
    runtime.record_live_speculative_execution(
        inner_candidate,
        &[request(12)],
        12,
        inner_execution,
    );

    let descendant_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), descendant)
        .unwrap();
    runtime.record_live_speculative_execution(
        descendant_candidate,
        &[request(13)],
        13,
        RiscvExecutionRecord::new(
            descendant,
            0x800c,
            0x8010,
            vec![RegisterWrite::new(reg(9), 42)],
            None,
        ),
    );
    (runtime, outer, inner, descendant)
}
```

- [ ] **Step 4: Prove outer validation preserves the inner chain**

Add:

```rust
#[test]
fn outer_control_validation_preserves_inner_control_chain() {
    let (mut runtime, _, inner, descendant) = issued_nested_control_runtime();
    let rob = runtime.snapshot().reorder_buffer().to_vec();
    let outer_sequence = rob[1].sequence();
    let inner_sequence = rob[2].sequence();
    let descendant_sequence = rob[3].sequence();

    runtime.validate_live_speculative_producer(outer_sequence);

    assert!(!runtime.live_control_dependencies.contains_key(&inner_sequence));
    assert_eq!(
        runtime.live_control_dependencies.get(&descendant_sequence),
        Some(&inner_sequence)
    );
    let inner_record = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.execution.instruction() == inner)
        .unwrap();
    assert!(inner_record.producer_sequences.is_empty());
    assert!(runtime
        .live_speculative_executions
        .iter()
        .any(|issued| issued.execution.instruction() == descendant));
}
```

If this fails, keep `validate_live_speculative_producer` sequence-local:

```rust
for issued in &mut self.live_speculative_executions {
    issued
        .producer_sequences
        .retain(|producer| *producer != sequence);
}
self.live_control_dependencies
    .retain(|_, control| *control != sequence);
```

Do not clear all control dependencies when validating the outer branch.

- [ ] **Step 5: Run focused runtime tests and commit**

```bash
cargo test -p rem6-cpu --lib o3_runtime::o3_runtime_control_window_tests -- --nocapture
git add crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_control_window_tests.rs
git commit -m "cpu: preserve nested control ownership"
```

Expected: all existing one-branch control-window tests and all new nested tests PASS.

### Task 3: Follow Two Recorded Prediction Decisions

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`
- Inspect: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`

- [ ] **Step 1: Add a nested completed-fetch fixture**

```rust
fn detailed_nested_control_core(split_inner: bool) -> RiscvCore {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let outer = b_type(12, 1, 2, 0x0);
    let inner = b_type(8, 3, 4, 0x0).to_le_bytes();
    let mul = r_type(0x01, 7, 8, 0x0, 9, 0x33);
    let mut fetches = vec![
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, outer.to_le_bytes().to_vec()),
    ];
    if split_inner {
        fetches.push((2, 0x8008, inner[..2].to_vec()));
        fetches.push((3, 0x800a, inner[2..].to_vec()));
        fetches.push((4, 0x800c, mul.to_le_bytes().to_vec()));
    } else {
        fetches.push((2, 0x8008, inner.to_vec()));
        fetches.push((3, 0x800c, mul.to_le_bytes().to_vec()));
    }
    let core = core_with_completed_fetches(fetches);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(4);
    core
}
```

- [ ] **Step 2: Add the failing two-prediction walk test**

```rust
#[test]
fn detailed_scalar_window_follows_two_recorded_control_paths() {
    let core = detailed_nested_control_core(false);

    let outer = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(outer.branch_speculation().unwrap().pc(), Address::new(0x8004));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&outer).unwrap(),
    );

    let inner = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(inner.branch_speculation().unwrap().pc(), Address::new(0x8008));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&inner).unwrap(),
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
```

Run:

```bash
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control::detailed_scalar_window_follows_two_recorded_control_paths -- --exact --nocapture
```

Expected: FAIL before Task 1 and PASS once repeated `AdmitPredictedControl` decisions are accepted. The final `None` proves the completed scalar descendant filled the fourth row and the walker did not request a fifth row. Preserve the existing loop in `scalar_integer_window_candidate_from`: update `previous_request`, derive the sequential PC, call `recorded_predicted_pc`, and continue from the recorded PC for every admitted control row.

- [ ] **Step 3: Add the split inner-branch identity test**

```rust
#[test]
fn detailed_split_inner_control_keys_prediction_to_prefix_request() {
    let core = detailed_nested_control_core(true);
    let outer = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&outer).unwrap(),
    );

    let inner = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(inner.branch_speculation().unwrap().sequence(), 2);
    assert_eq!(inner.pc(), Address::new(0x800c));
}
```

The inner prediction identity must use `first_consumed_request()`, while successor chaining continues from `last_consumed_request()`.

- [ ] **Step 4: Run fetch-ahead suites and commit**

```bash
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::speculative_history -- --nocapture
git add crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs
git commit -m "cpu: follow nested predicted control paths"
```

Expected: all detailed O3 and speculative-history tests PASS without a second predictor invocation.

### Task 4: Add the Correct Nested CLI Row

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs`

- [ ] **Step 1: Register the focused child module**

At the top of `predicted_control.rs`, add:

```rust
#[path = "predicted_control/nested.rs"]
mod nested;
```

The child uses `use super::*;` and defines its own constants, binary builder, command wrapper, and assertions. Do not move or duplicate the existing one-branch tests.

- [ ] **Step 2: Build the nested binary with explicit control targets**

Use these PC constants in `nested.rs`:

```rust
const LOAD_PC: &str = "0x80000024";
const OUTER_BRANCH_PC: &str = "0x80000028";
const INNER_BRANCH_PC: &str = "0x8000002c";
const DESCENDANT_PC: &str = "0x80000030";
const WRONG_STORE_PC: &str = "0x80000034";
const INNER_TARGET_PC: &str = "0x8000003c";
const OUTER_TARGET_PC: &str = "0x80000040";
const DATA_ADDRESS: &str = "0x800000c0";
const WRONG_STORE_ADDRESS: &str = "0x800000c8";
```

Create:

```rust
fn nested_control_binary(
    name: &str,
    outer_taken: bool,
    inner_taken: bool,
    dependent_inner: bool,
) -> std::path::PathBuf {
    let data_start = 192_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 5, 0x13),
        i_type(if outer_taken { 1 } else { 2 }, 0, 0x0, 6, 0x13),
        i_type(3, 0, 0x0, 7, 0x13),
        i_type(if inner_taken { 3 } else { 4 }, 0, 0x0, 8, 0x13),
        i_type(6, 0, 0x0, 9, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        b_type(24, 6, 5, 0b000),
        if dependent_inner {
            b_type(16, 0, 12, 0b000)
        } else {
            b_type(16, 8, 7, 0b000)
        },
        r_type(0x01, 9, 7, 0x0, 13, 0x33),
        s_type(8, 13, 10, 0b010),
        i_type(1, 0, 0x0, 14, 0x13),
        i_type(2, 0, 0x0, 15, 0x13),
        i_type(3, 0, 0x0, 16, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
```

Before implementation, verify the branch offsets against the generated PC constants. If a PC differs, correct the constant or immediate before adding assertions; do not weaken the assertions.

- [ ] **Step 3: Add the nested command wrapper**

```rust
fn nested_control_command(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,Fetch,Memory,HostAction",
        "--riscv-branch-lookahead",
        "2",
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        execution_mode,
        "--dump-memory",
        &format!("{DATA_ADDRESS}:16"),
    ]);
    command
}
```

Add `run_nested_control_json` with the same success and JSON parsing assertions as the parent helper.

- [ ] **Step 4: Add the failing correct/correct row**

```rust
#[test]
fn rem6_run_o3_nested_controls_commit_direct() {
    let path = nested_control_binary("o3-nested-control-direct", false, false, false);
    let json = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);

    let load = event_at_pc(&json, LOAD_PC);
    let outer = event_at_pc(&json, OUTER_BRANCH_PC);
    let inner = event_at_pc(&json, INNER_BRANCH_PC);
    let descendant = event_at_pc(&json, DESCENDANT_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");

    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert!(event_u64(inner, "issue_tick") < response_tick);
    assert!(event_u64(descendant, "issue_tick") < response_tick);
    assert!([load, outer, inner, descendant]
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
    for branch in [outer, inner] {
        assert_eq!(
            branch.pointer("/branch_mispredicted").and_then(Value::as_bool),
            Some(false)
        );
    }
    assert_eq!(register_value(&json, "x13"), 18);
    assert_json_stat(&json, "sim.cpu0.o3.max_rob_occupancy", "Count", 4, "monotonic");
    assert_json_stat(&json, "sim.cpu0.o3.max_lsq_occupancy", "Count", 1, "monotonic");
}
```

Add `register_value` locally or change the parent helper to `pub(super)` and reuse it. Prefer reuse if no unrelated visibility is widened.

- [ ] **Step 5: Run the row and inspect the first failure**

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_nested_controls_commit_direct -- --exact --nocapture
```

Expected before Tasks 1-3: FAIL because branch 2 is not admitted. After Tasks 1-3, PASS with four resident rows and exact final values.

- [ ] **Step 6: Commit the first real CLI evidence**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs
git commit -m "test: cover nested predicted control commit"
```

### Task 5: Distinguish Outer and Inner Misprediction Rollback

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Inspect: `crates/rem6-cpu/src/riscv_execute.rs`

- [ ] **Step 1: Add the cache/fabric/DRAM outer-misprediction row**

```rust
#[test]
fn rem6_run_o3_outer_misprediction_discards_nested_control_cache_fabric_dram() {
    let path = nested_control_binary("o3-nested-outer-mispredict", true, false, false);
    let completed = run_nested_control_json(
        &path,
        "cache-fabric-dram",
        2_500,
        "detailed",
        &[],
    );
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    assert!(event_u64(outer, "issue_tick") < event_u64(load, "lsq_data_response_tick"));
    assert_eq!(
        outer.pointer("/branch_mispredicted").and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&completed, INNER_BRANCH_PC).is_none());
    assert!(event_at_pc_if_present(&completed, DESCENDANT_PC).is_none());
    assert!(event_at_pc_if_present(&completed, WRONG_STORE_PC).is_none());
    assert_eq!(register_value(&completed, "x16"), 3);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(completed
            .pointer(pointer)
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0));
    }
}
```

Derive a pre-repair tick from `outer.issue_tick + 1`, rerun with that tick limit, and assert the resident ROB contains `[LOAD_PC, OUTER_BRANCH_PC, INNER_BRANCH_PC, DESCENDANT_PC]` before repair.

- [ ] **Step 2: Add the direct inner-misprediction row**

```rust
#[test]
fn rem6_run_o3_inner_misprediction_preserves_outer_control_direct() {
    let path = nested_control_binary("o3-nested-inner-mispredict", false, true, false);
    let completed = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);

    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let inner = event_at_pc(&completed, INNER_BRANCH_PC);
    assert_eq!(
        outer.pointer("/branch_mispredicted").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        inner.pointer("/branch_mispredicted").and_then(Value::as_bool),
        Some(true)
    );
    assert!(event_at_pc_if_present(&completed, DESCENDANT_PC).is_none());
    assert!(event_at_pc_if_present(&completed, WRONG_STORE_PC).is_none());
    assert_eq!(register_value(&completed, "x15"), 2);
    assert_eq!(register_value(&completed, "x16"), 3);
    assert_json_stat(
        &completed,
        "sim.cpu0.o3.branch_event.squashes",
        "Count",
        1,
        "monotonic",
    );
}
```

The outer event must remain in the final O3 trace. Do not accept a result that removes both branches.

- [ ] **Step 3: Run both rows and fix only sequence-boundary defects**

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_outer_misprediction_discards_nested_control_cache_fabric_dram -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_inner_misprediction_preserves_outer_control_direct -- --exact --nocapture
```

Expected: both PASS. If cleanup is wrong, preserve the existing branch-specific call:

```rust
if let Some(sequence) = live_control_sequence {
    state
        .o3_runtime
        .discard_live_control_descendants_from(sequence);
}
```

and keep rollback sequence-bounded:

```rust
self.snapshot
    .reorder_buffer
    .retain(|entry| !entry.is_live_staged() || entry.sequence() <= branch_sequence);
self.live_scalar_memory_younger_sequences
    .retain(|sequence| *sequence <= branch_sequence);
self.live_speculative_executions
    .retain(|execution| execution.sequence <= branch_sequence);
self.live_control_dependencies
    .retain(|sequence, _| *sequence <= branch_sequence);
```

- [ ] **Step 4: Commit the rollback matrix**

```bash
git add crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/riscv_execute.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs
git commit -m "test: distinguish nested control rollback"
```

### Task 6: Lock Dependency Suppression and Complete Fetch Identity

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Inspect: `crates/rem6-cpu/src/riscv_live_retire_window.rs`

- [ ] **Step 1: Add the dependent-inner-branch CLI row**

```rust
#[test]
fn rem6_run_o3_load_dependent_inner_branch_suppresses_descendant() {
    let path = nested_control_binary("o3-nested-dependent-inner", false, false, true);
    let completed = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let outer = event_at_pc(&completed, OUTER_BRANCH_PC);
    let inner = event_at_pc(&completed, INNER_BRANCH_PC);
    let response_tick = event_u64(load, "lsq_data_response_tick");

    assert!(event_u64(outer, "issue_tick") < response_tick);
    assert!(event_u64(inner, "issue_tick") >= response_tick);
    assert!(event_at_pc_if_present(&completed, DESCENDANT_PC)
        .is_some_and(|event| event_u64(event, "issue_tick") >= response_tick));

    let live_tick = event_u64(outer, "issue_tick") + 1;
    let resident = run_nested_control_json(&path, "direct", live_tick, "detailed", &[]);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        rob.iter()
            .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        [LOAD_PC, OUTER_BRANCH_PC, INNER_BRANCH_PC]
    );
}
```

The completed run may execute the descendant normally after branch 2 resolves. The required suppression is no descendant residency or early issue while the load is unresolved.

- [ ] **Step 2: Add the split inner-branch suffix-replacement regression**

Add `split_inner_branch_suffix_replacement_prunes_nested_chain` in the focused control-window tests. Validate the outer producer, rewrite the inner record's consumed vector to `[prefix, original_suffix]`, then retire the same inner execution with `[prefix, replacement_suffix]`. Assert the outer execution remains, the inner and scalar descendant records disappear, and `live_control_dependencies` is empty. Retain this exact readiness assertion in the existing live-retire split-fetch regression:

```rust
assert_eq!(
    runtime.live_speculative_execution_ready_tick(
        &[prefix_request, replacement_suffix_request],
        &inner_execution,
    ),
    None
);
```

Then retire the replacement fetch vector and assert the inner speculative record is not reused and its descendant chain is invalidated.

- [ ] **Step 3: Preserve exact full-vector matching**

The implementation must retain exact equality in both readiness and retirement paths:

```rust
issued.consumed_requests.as_slice() == consumed_requests
```

Do not revert to prefix-only identity, byte-only identity, or `first()` equality. If invalidation leaves nested dependency edges behind, remove edges whose key or controller belongs to the invalidated sequence set while preserving unrelated outer rows.

Replace `invalidate_live_speculative_execution_chain` with an implementation
that retains the complete invalidated sequence set and prunes its control edges:

```rust
fn invalidate_live_speculative_execution_chain(&mut self, sequence: u64) {
    let mut invalidated = BTreeSet::from([sequence]);
    let mut pending = vec![sequence];
    while let Some(producer) = pending.pop() {
        let mut index = 0;
        while index < self.live_speculative_executions.len() {
            if self.live_speculative_executions[index]
                .producer_sequences
                .contains(&producer)
            {
                let removed = self.live_speculative_executions.remove(index).sequence;
                if invalidated.insert(removed) {
                    pending.push(removed);
                }
            } else {
                index += 1;
            }
        }
    }
    self.live_control_dependencies.retain(|dependent, control| {
        !invalidated.contains(dependent) && !invalidated.contains(control)
    });
}
```

- [ ] **Step 4: Run focused tests and commit**

```bash
cargo test -p rem6-cpu --lib split_suffix_replacement -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_load_dependent_inner_branch_suppresses_descendant -- --exact --nocapture
git add crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_control_window_tests.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs
git commit -m "fix: validate nested control fetch identity"
```

### Task 7: Prove Transfer, Checkpoint, and Timing Boundaries

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs`

- [ ] **Step 1: Add the detailed-to-timing transfer row**

Use the correct/correct direct binary. Derive `switch_tick = descendant.issue_tick + 1` and assert it is before the load response. Add:

```rust
#[test]
fn rem6_run_host_switch_transfers_o3_nested_controls() {
    let path = nested_control_binary("o3-nested-control-switch", false, false, false);
    let baseline = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);
    let switch_tick = event_u64(event_at_pc(&baseline, DESCENDANT_PC), "issue_tick") + 1;
    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let switched = run_nested_control_json(
        &path,
        "direct",
        2_000,
        "detailed",
        &["--host-switch-cpu-mode", &switch_arg],
    );

    let transfer = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| switches.iter().find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
        }))
        .and_then(|switch| switch.pointer("/state_transfer"))
        .unwrap();
    assert_eq!(transfer.pointer("/restorable").and_then(Value::as_bool), Some(false));
    let runtime = transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(runtime.pointer("/snapshot_rob_entries").and_then(Value::as_u64), Some(4));
    assert_eq!(runtime.pointer("/snapshot_lsq_entries").and_then(Value::as_u64), Some(1));
    let handoff = transfer_live_data_handoff_chunk(transfer, "cpu0");
    assert_eq!(handoff.pointer("/schema_version").and_then(Value::as_u64), Some(7));
    assert_eq!(handoff.pointer("/younger_rows").and_then(Value::as_u64), Some(3));
}
```

Compare issue, writeback, and commit ticks for all four PCs against the baseline.

- [ ] **Step 2: Add the live-reject/drained-restore checkpoint row**

```rust
#[test]
fn rem6_run_o3_nested_control_checkpoint_boundary() {
    let path = nested_control_binary("o3-nested-control-checkpoint", false, false, false);
    let baseline = run_nested_control_json(&path, "direct", 2_000, "detailed", &[]);
    let live_tick = event_u64(event_at_pc(&baseline, DESCENDANT_PC), "issue_tick") + 1;
    let live_arg = format!("{live_tick}:nested-control-live");
    let mut command = nested_control_command(&path, "direct", 2_000, "detailed");
    command.args(["--host-checkpoint", &live_arg]);
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("checkpoint component is not quiescent: cpu0"));

    let drained_tick = event_u64(event_at_pc(&baseline, DESCENDANT_PC), "commit_tick") + 1;
    let restore_tick = drained_tick + 1;
    let checkpoint = format!("{drained_tick}:nested-control-drained");
    let restore = format!("{restore_tick}:nested-control-drained");
    let restored = run_nested_control_json(
        &path,
        "direct",
        2_000,
        "detailed",
        &[
            "--host-checkpoint",
            &checkpoint,
            "--host-restore-checkpoint",
            &restore,
        ],
    );
    assert_eq!(
        restored.pointer("/host_actions/checkpoint_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
}
```

Inspect the drained CPU component and assert no `o3-live-data-handoff` chunk and zero decoded ROB/LSQ rows.

- [ ] **Step 3: Add timing-mode suppression**

```rust
#[test]
fn rem6_run_timing_suppresses_o3_nested_controls() {
    let path = nested_control_binary("o3-nested-control-timing", false, false, false);
    let timing = run_nested_control_json(&path, "direct", 2_000, "timing", &[]);

    assert_eq!(register_value(&timing, "x13"), 18);
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert!(timing
        .pointer("/stats")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .all(|path| !path.starts_with("sim.cpu0.o3.") && !path.starts_with("system.cpu.rob.")));
}
```

- [ ] **Step 4: Run the complete nested CLI module and commit**

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested:: -- --nocapture
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs
git commit -m "test: prove nested control lifecycle"
```

Expected: seven nested rows PASS. Verify the test runner reports seven selected tests, not zero filtered tests.

### Task 8: Record Honest Migration Evidence

**Files:**
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add exact CLI anchors**

Append these names to `core_test_anchors.txt` in the CPU O3 group:

```text
rem6_run_o3_nested_controls_commit_direct
rem6_run_o3_outer_misprediction_discards_nested_control_cache_fabric_dram
rem6_run_o3_inner_misprediction_preserves_outer_control_direct
rem6_run_o3_load_dependent_inner_branch_suppresses_descendant
rem6_run_host_switch_transfers_o3_nested_controls
rem6_run_o3_nested_control_checkpoint_boundary
rem6_run_timing_suppresses_o3_nested_controls
```

- [ ] **Step 2: Update the CPU section without changing score or checklist**

Update the CPU heading narrative, `Migrated`, `Not migrated`, and `Next evidence` so they state:

1. one bounded four-row nested direct-conditional window now has executable evidence,
2. outer and inner misprediction boundaries are distinct,
3. direct and hierarchy routes, dependency suppression, transfer, checkpoint, and timing suppression are covered,
4. third/deeper branches, wider control classes, speculative memory descendants, issue-width/resource contention, restorable transport ownership, and a general O3 engine remain open.

Keep:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw,
capped at the 74% representative bucket cap.
```

Do not check the running-O3 item.

- [ ] **Step 3: Update the CPU test-ledger row**

Add the nested-control anchors to the existing CPU row's migrated boundary and narrow its next evidence from generic multi-branch to third/deeper branches plus wider control classes. Do not create a second CPU progress ledger.

- [ ] **Step 4: Preserve exactly 1,200 lines**

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected:

```text
1200 docs/architecture/gem5-to-rem6-migration.md
```

Replace or rewrap existing CPU prose so the ledger remains exactly 1,200 lines. Do not change the source-policy constant.

- [ ] **Step 5: Run source-policy suites and commit**

```bash
cargo test -p rem6 --test source_policy -- --nocapture
cargo test -p rem6-cpu --test source_policy -- --nocapture
git add crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record nested control evidence"
```

### Task 9: Verify, Review, Fix, and Push

**Files:**
- Modify only files required by concrete test or review findings

- [ ] **Step 1: Run every nested CLI row with an exact module-qualified name**

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_nested_controls_commit_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_outer_misprediction_discards_nested_control_cache_fabric_dram -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_inner_misprediction_preserves_outer_control_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_load_dependent_inner_branch_suppresses_descendant -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_host_switch_transfers_o3_nested_controls -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_o3_nested_control_checkpoint_boundary -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested::rem6_run_timing_suppresses_o3_nested_controls -- --exact --nocapture
```

Expected: each command reports exactly one passed test.

- [ ] **Step 2: Run focused and broad suites**

```bash
cargo test -p rem6-cpu --lib --quiet
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control:: -- --nocapture
cargo test -p rem6 --test cli_run --quiet
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
cargo test --workspace --all-targets --quiet
```

Expected: all tests PASS. The known repository clippy warnings are outside this slice; do not widen them.

- [ ] **Step 3: Run formatting and hygiene checks**

```bash
cargo fmt --all -- --check
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short --branch
```

Expected: formatting and diff checks pass, ledger is 1,200 lines, and only intended commits are ahead of `origin/main`.

- [ ] **Step 4: Dispatch an xhigh read-only whole-diff review**

Give the reviewer the pushed-slice base and current head. Require findings first and ask it to verify:

1. two predictions are recorded exactly once,
2. branch 2 owns branch 1 as its immediate control producer,
3. the descendant owns branch 2,
4. outer rollback removes the whole nested suffix,
5. inner rollback preserves the outer branch,
6. complete split-fetch vectors are matched exactly,
7. mode transfer and checkpoint behavior do not add serialized transient state,
8. timing mode remains O3-free,
9. ledger claims and score are honest, and
10. no `temp/` path is tracked.

- [ ] **Step 5: Fix every concrete finding test-first**

For each finding:

1. add or tighten the smallest failing regression,
2. run it and confirm failure,
3. make the smallest production correction,
4. rerun focused and affected broad suites,
5. commit with an English behavior-focused message.

Do not dismiss a recovered fetch-identity, rollback, or test-selection defect as cosmetic.

- [ ] **Step 6: Re-run final evidence and push**

```bash
cargo fmt --all -- --check
git diff --check HEAD~1..HEAD
git status --short --branch
wc -l docs/architecture/gem5-to-rem6-migration.md
git fetch origin
git rev-list --left-right --count origin/main...HEAD
git push origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

Expected: the remote has not diverged before push, `HEAD` equals `origin/main` after push, and the worktree is clean.

## Plan Self-Review

The plan covers every design requirement: bounded two-branch admission, immediate ancestry, correct/correct retirement, outer rollback, inner rollback, dependency suppression, two recorded prediction decisions, split-fetch identity, mode transfer, checkpoint rejection and drained restore, timing suppression, direct and hierarchy evidence, source-policy anchors, ledger honesty, full verification, and independent review.

No task changes the four-row limit, adds memory descendants, creates a predictor or redirect authority, adds issue-width configuration, changes handoff schema v7, makes transport state restorable, checks the running-O3 checklist item, or raises the CPU score.
