# RISC-V O3 Linked Control Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Admit bounded direct calls, independent indirect calls, and committed-source returns into the existing four-row scalar-memory O3 live window with real rename, writeback, RAS, rollback, transfer, checkpoint, and timing evidence.

**Architecture:** Extend the single typed live-control descriptor with an optional integer link destination, then let the existing scalar-memory window policy decide whether indirect target sources are committed or terminal. Generalize the current control candidate so linked calls use the already-staged rename destination and fixed-FU writeback calendar; keep frontend target selection, RAS speculation, architectural branch resolution, ROB retirement, mode transfer, and checkpoint ownership unchanged.

**Tech Stack:** Rust workspace, `rem6-cpu`, top-level `rem6 run --execute` integration tests, generated RV64 ELF fixtures, JSON/debug artifacts, source-policy tests, Cargo.

---

## File Map

- Modify `crates/rem6-cpu/src/o3_source_operands.rs`: remain the only live-control opcode inventory and expose optional link destination metadata.
- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: admit supported linked forms, record link destinations, and make live-produced indirect targets terminal.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window.rs`: carry an optional staged destination on generic control candidates, validate the exact link write, and reserve writeback capacity.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`: prove candidate shape, exact-write validation, ordered rename ownership, and rollback cleanup.
- Modify `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`: prove branch-port serialization and fixed-FU writeback arbitration for linked controls.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`: prove direct-call target following and same-window call-to-return suppression through the real detailed frontend.
- Modify `crates/rem6-cpu/tests/source_policy.rs`: require destination metadata in the one typed owner and prevent linked opcode inventories in consumers.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_kind.rs`: own the eight-row direct/hierarchy/RAS/rollback/suppression/transfer/checkpoint/timing matrix.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`: register the focused `link_kind` module.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs`: own the shared no-fetch assertion.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/mixed_kind.rs`: consume the shared no-fetch assertion and remove the duplicate local helper.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: anchor all eight new top-level tests.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record the executable boundary in place while preserving 74% and exactly 1,200 lines.

Do not modify `o3_runtime_live_window.rs`, `o3_runtime_retire.rs`, `riscv_fetch_ahead.rs`, `riscv_execute.rs`, `return_address_stack.rs`, checkpoint payload versions, or handoff schema versions unless a failing test demonstrates an existing ownership bug. Their current generic paths are part of the behavior under test, not parallel implementation targets.

### Task 1: Extend The Typed Descriptor And Admission Policy

**Files:**
- Modify: `crates/rem6-cpu/src/o3_source_operands.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing descriptor tests**

Add a test module to `o3_source_operands.rs` with these exact supported rows:

```rust
#[test]
fn live_control_descriptor_owns_link_kind_sources_and_destination() {
    for (instruction, kind, sources, destination) in [
        (jal(0), BranchTargetKind::DirectUnconditional, vec![], None),
        (jal(1), BranchTargetKind::CallDirect, vec![], Some(reg(1))),
        (jal(5), BranchTargetKind::CallDirect, vec![], Some(reg(5))),
        (
            jalr(0, 9),
            BranchTargetKind::IndirectUnconditional,
            vec![reg(9)],
            None,
        ),
        (
            jalr(1, 9),
            BranchTargetKind::CallIndirect,
            vec![reg(9)],
            Some(reg(1)),
        ),
        (
            jalr(5, 9),
            BranchTargetKind::CallIndirect,
            vec![reg(9)],
            Some(reg(5)),
        ),
        (jalr(0, 1), BranchTargetKind::Return, vec![reg(1)], None),
        (jalr(0, 5), BranchTargetKind::Return, vec![reg(5)], None),
    ] {
        let control = o3_live_control_operands(instruction)
            .unwrap_or_else(|| panic!("missing live-control descriptor for {instruction:?}"));
        assert_eq!(control.kind(), kind);
        assert_eq!(control.sources(), sources);
        assert_eq!(control.destination(), destination);
    }
}
```

Use these local builders:

```rust
fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn jal(rd: u8) -> RiscvInstruction {
    RiscvInstruction::Jal {
        rd: reg(rd),
        offset: rem6_isa_riscv::Immediate::new(8),
    }
}

fn jalr(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Jalr {
        rd: reg(rd),
        rs1: reg(rs1),
        offset: rem6_isa_riscv::Immediate::new(0),
    }
}
```

- [ ] **Step 2: Add failing descriptor rejection tests**

```rust
#[test]
fn live_control_descriptor_rejects_non_link_destinations_and_link_source_calls() {
    for instruction in [
        jal(2),
        jalr(2, 9),
        jalr(1, 1),
        jalr(1, 5),
        jalr(5, 1),
        jalr(5, 5),
    ] {
        assert_eq!(o3_live_control_operands(instruction), None, "{instruction:?}");
    }
}
```

- [ ] **Step 3: Replace the obsolete policy rejection test with linked admission tests**

Delete `scalar_memory_prefix_rejects_link_writing_and_return_controls`. Add:

```rust
#[test]
fn scalar_memory_prefix_admits_bounded_linked_controls() {
    for instruction in [
        jal_with_destination(1),
        jal_with_destination(5),
        jalr_with_registers(1, 9),
        jalr_with_registers(5, 9),
        jalr_with_registers(0, 1),
        jalr_with_registers(0, 5),
    ] {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window.classify_younger(instruction),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl,
            "{instruction:?}"
        );
    }
}

#[test]
fn live_target_sources_keep_linked_controls_terminal() {
    let mut load_target = scalar_load_window(4);
    assert_eq!(
        load_target.classify_younger(jalr_with_registers(1, 4)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );

    let mut alu_target = scalar_load_window(4);
    assert_eq!(
        alu_target.classify_younger(addi(9, 0)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );
    assert_eq!(
        alu_target.classify_younger(jalr_with_registers(5, 9)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );

    let mut live_return = scalar_load_window(4);
    assert_eq!(
        live_return.classify_younger(jal_with_destination(1)),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        live_return.classify_younger(jalr_with_registers(0, 1)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );
}
```

Keep explicit `Reject` coverage for `JAL x2`, `JALR x2, x9`, coroutine forms, and same-link indirect calls. Update older tests that used `JAL x1` as an unsupported row to use `JAL x2` instead.

- [ ] **Step 4: Add failing detailed-frontend tests**

Add one direct-call positive and one same-window return negative to `detailed_o3_control.rs`:

```rust
#[test]
fn detailed_scalar_window_follows_direct_call_and_pushes_ras() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let call = j_type(8, 1);
    let target = r_type(0x01, 7, 8, 0x0, 9, 0x33);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, target.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&decision).unwrap(),
    );

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.return_address_stack.stack_entries(),
        &[Address::new(0x8008)]
    );
}

#[test]
fn detailed_scalar_window_keeps_same_window_return_terminal() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let call = j_type(8, 1);
    let ret = i_type(0, 1, 0x0, 0, 0x67);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, ret.to_le_bytes().to_vec()),
        (3, 0x8008, i_type(1, 0, 0x0, 9, 0x13).to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(4);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision).unwrap(),
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
```

- [ ] **Step 5: Tighten the source-policy test before implementation**

Add these anchors to `o3_live_control_operands_have_one_typed_owner`:

```rust
"destination: Option<Register>",
"pub(crate) const fn destination(&self) -> Option<Register>",
```

Keep the existing consumer opcode-inventory ban unchanged. The descriptor owner may use `is_riscv_link_register`; policy, issue, staging, and retirement consumers may not recreate that inventory.

- [ ] **Step 6: Run focused tests and verify RED**

Run:

```text
cargo test -p rem6-cpu o3_source_operands -- --nocapture
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
cargo test -p rem6-cpu detailed_scalar_window -- --nocapture
cargo test -p rem6-cpu --test source_policy o3_live_control_operands_have_one_typed_owner -- --nocapture
```

Expected: descriptor tests fail to compile because `destination()` does not exist; after adding only the test accessor signature, linked forms still fail because the helper and policy reject them.

- [ ] **Step 7: Implement the descriptor**

Change the type to:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveControlOperands {
    kind: BranchTargetKind,
    sources: Vec<Register>,
    destination: Option<Register>,
}

impl O3LiveControlOperands {
    pub(crate) const fn kind(&self) -> BranchTargetKind {
        self.kind
    }

    pub(crate) fn sources(&self) -> &[Register] {
        &self.sources
    }

    pub(crate) const fn destination(&self) -> Option<Register> {
        self.destination
    }
}
```

Replace the helper's supported-form match with:

```rust
let destination = match instruction {
    RiscvInstruction::Beq { .. }
    | RiscvInstruction::Bne { .. }
    | RiscvInstruction::Blt { .. }
    | RiscvInstruction::Bge { .. }
    | RiscvInstruction::Bltu { .. }
    | RiscvInstruction::Bgeu { .. } => None,
    RiscvInstruction::Jal { rd, .. } if rd.is_zero() => None,
    RiscvInstruction::Jal { rd, .. } if is_riscv_link_register(rd) => Some(rd),
    RiscvInstruction::Jalr { rd, .. } if rd.is_zero() => None,
    RiscvInstruction::Jalr { rd, rs1, .. }
        if is_riscv_link_register(rd) && !is_riscv_link_register(rs1) =>
    {
        Some(rd)
    }
    _ => return None,
};
Some(O3LiveControlOperands {
    kind: riscv_branch_target_kind(instruction),
    sources: o3_scalar_integer_source_registers(&instruction),
    destination,
})
```

- [ ] **Step 8: Implement deterministic linked admission**

In `classify_younger`, classify every frontend-sensitive indirect target through the branch kind:

```rust
let indirect_target_is_live = matches!(
    control.kind(),
    BranchTargetKind::IndirectUnconditional
        | BranchTargetKind::CallIndirect
        | BranchTargetKind::Return
) && control
    .sources()
    .iter()
    .any(|source| self.live_destinations.contains(source));

if depends_on_unresolved || indirect_target_is_live {
    self.control_closed = true;
    return RiscvScalarIntegerYoungerDecision::AdmitTerminalControl;
}
if let Some(destination) = control.destination() {
    self.record_live_destination(destination);
}
self.control_depth += 1;
RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
```

Do not inspect the RAS and do not special-case opcodes outside `o3_source_operands.rs`.

- [ ] **Step 9: Run focused tests and verify GREEN**

Run the four commands from Step 6. Expected: all pass.

- [ ] **Step 10: Commit the typed boundary**

```bash
git add crates/rem6-cpu/src/o3_source_operands.rs crates/rem6-cpu/src/riscv_o3_window_policy.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs crates/rem6-cpu/tests/source_policy.rs
git commit -m "cpu: classify linked O3 controls"
```

### Task 2: Generalize Control Execution, Writeback, And Rollback

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`

- [ ] **Step 1: Add failing linked candidate tests**

Add builders that accept both destination and source:

```rust
fn jal_with_destination(rd: u8, offset: i64) -> RiscvInstruction {
    RiscvInstruction::Jal {
        rd: reg(rd),
        offset: Immediate::new(offset),
    }
}

fn jalr_with_registers(rd: u8, rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Jalr {
        rd: reg(rd),
        rs1: reg(rs1),
        offset: Immediate::new(0),
    }
}
```

Add a test that stages a delayed load followed by `JAL x1` and asserts:

```rust
let call = jal_with_destination(1, 8);
let runtime = scalar_load_runtime_with_branch(call);
let candidate = runtime
    .live_speculative_issue_candidate(Address::new(0x8004), call)
    .expect("linked call should expose a generic control candidate");
let destination = candidate.destination().expect("linked call destination");
assert_eq!(destination.register_class(), O3RegisterClass::Integer);
assert_eq!(destination.architectural(), 1);
assert_eq!(
    Some(destination.physical()),
    runtime.snapshot().reorder_buffer()[1].destination()
);
```

- [ ] **Step 2: Add exact-write validation tests**

Clone the candidate and assert `record_live_speculative_execution` returns `false` for:

```rust
RiscvExecutionRecord::new(call, 0x8004, 0x800c, Vec::new(), None)
RiscvExecutionRecord::new(
    call,
    0x8004,
    0x800c,
    vec![RegisterWrite::new(reg(5), 0x8008)],
    None,
)
RiscvExecutionRecord::new(
    call,
    0x8004,
    0x800c,
    vec![
        RegisterWrite::new(reg(1), 0x8008),
        RegisterWrite::new(reg(5), 0x8008),
    ],
    None,
)
```

Assert the exact one-write record succeeds and owns one writeback reservation:

```rust
let valid = RiscvExecutionRecord::new(
    call,
    0x8004,
    0x800c,
    vec![RegisterWrite::new(reg(1), 0x8008)],
    None,
);
assert!(runtime
    .record_live_speculative_execution(candidate, &[request(11)], 11, valid)
    .unwrap());
let call_sequence = runtime.snapshot().reorder_buffer()[1].sequence();
assert!(runtime.writeback_reservation(call_sequence).is_some());
```

Add a return test proving `JALR x0, 0(x1)` has no destination, accepts zero writes, rejects a link write, and creates no writeback reservation.

- [ ] **Step 3: Add failing rollback coverage**

Stage `[load, older BEQ, JAL x1, ADDI]`, record speculative executions, then call:

```rust
runtime.discard_live_control_descendants_from_at(branch_sequence, 12);
```

Assert all of the following:

```rust
assert_eq!(
    runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .map(|entry| entry.pc())
        .collect::<Vec<_>>(),
    [Address::new(0x8000), Address::new(0x8004)]
);
assert!(runtime.writeback_reservation(call_sequence).is_none());
assert!(runtime
    .snapshot()
    .rename_map()
    .iter()
    .all(|entry| entry.architectural() != 1));
```

Seed a committed `x1` rename before staging and assert rollback restores that physical register rather than deleting it.

- [ ] **Step 4: Add failing issue and writeback arbitration coverage**

Add `ScalarIssueCase::LinkedControls` with these rows:

```rust
[
    jal_with_destination(1, 8),
    addi(14, 0, 7),
    jalr_with_registers(0, 5),
]
```

Seed `x5` in the hart, use issue width three and writeback width one, then assert:

```rust
assert_eq!(fixture.issue_tick(BRANCH_PC), 20);
assert_eq!(fixture.issue_tick(MUL_PC), 20);
assert_eq!(fixture.issue_tick(THIRD_PC), 21);

let call = fixture.execution_at(BRANCH_PC);
let scalar = fixture.execution_at(MUL_PC);
let ret = fixture.execution_at(THIRD_PC);
assert_eq!(call.writeback_slot, Some(0));
assert_eq!(call.raw_ready_tick, 20);
assert_eq!(call.admitted_writeback_tick, 20);
assert_eq!(scalar.raw_ready_tick, 20);
assert_eq!(scalar.admitted_writeback_tick, 21);
assert_eq!(ret.writeback_slot, None);
```

- [ ] **Step 5: Run focused runtime tests and verify RED**

Run:

```text
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue -- --nocapture
```

Expected: linked candidates are absent because the control variant currently requires no staged destination; writeback and rollback assertions therefore fail.

- [ ] **Step 6: Generalize the control candidate**

Change the enum to:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveSpeculativeIssueKind {
    Scalar { destination: O3RenameMapEntry },
    Control {
        kind: BranchTargetKind,
        destination: Option<O3RenameMapEntry>,
    },
}
```

Make `destination()` return the destination from either variant.

When constructing a candidate, use `control.destination()` as the expected architectural destination. Build `Control { destination: Some(staged_rename_entry(entry)...) }` only when the staged ROB metadata matches the descriptor; keep no-link controls and returns on `destination: None`. Return `None` for any mismatch.

- [ ] **Step 7: Validate exact linked writes and reserve writeback**

Replace the control validation arm with:

```rust
O3LiveSpeculativeIssueKind::Control { kind, destination } => {
    let control_matches = o3_live_control_operands(execution.instruction())
        .is_some_and(|control| control.kind() == kind);
    let writes_match = match destination {
        Some(destination) => {
            execution.register_writes().len() == 1
                && execution_writes_rename_destination(&execution, destination)
        }
        None => execution.register_writes().is_empty(),
    };
    control_matches && writes_match
}
```

Reserve fixed-FU writeback for both scalar rows and linked controls:

```rust
let consumes_writeback_slot = matches!(
    candidate.kind,
    O3LiveSpeculativeIssueKind::Scalar { .. }
        | O3LiveSpeculativeIssueKind::Control {
            destination: Some(_),
            ..
        }
);
```

Do not add a second call-specific candidate or writeback path.

- [ ] **Step 8: Run focused runtime tests and verify GREEN**

Run the commands from Step 5. Expected: all pass, including existing no-link, dependency, rollback, and issue tests.

- [ ] **Step 9: Run the full CPU crate test gate**

Run:

```text
cargo test -p rem6-cpu --quiet
```

Expected: zero failures.

- [ ] **Step 10: Commit runtime behavior**

```bash
git add crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_control_window_tests.rs crates/rem6-cpu/src/o3_runtime_issue_tests.rs
git commit -m "cpu: execute linked O3 controls"
```

### Task 3: Add The Eight-Row Real CLI Matrix

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_kind.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/mixed_kind.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`

- [ ] **Step 1: Move the duplicate no-fetch assertion to shared support**

Add this function to `window_support.rs`:

```rust
pub(super) fn assert_no_fetch_pc(json: &Value, pc: &str) {
    assert!(
        json.pointer("/debug/fetch_trace")
            .and_then(Value::as_array)
            .is_some_and(|records| records.iter().all(|record| {
                record.pointer("/pc").and_then(Value::as_str) != Some(pc)
            })),
        "unexpected fetch at {pc}: {json}"
    );
}
```

Import it from `mixed_kind.rs` and delete that file's local copy.

- [ ] **Step 2: Register the focused module and anchors**

Add to `predicted_control.rs`:

```rust
#[path = "predicted_control/link_kind.rs"]
mod link_kind;
```

Add exactly these test anchors to `core_test_anchors.txt` after the mixed-kind block:

```text
rem6_run_o3_link_kind_direct_call_commits_direct
rem6_run_o3_link_kind_indirect_call_commits_cache_fabric_dram
rem6_run_o3_link_kind_return_uses_committed_ras_entry
rem6_run_o3_link_kind_older_branch_discards_linked_call
rem6_run_o3_link_kind_live_target_sources_stay_terminal
rem6_run_host_switch_transfers_o3_link_kind_window
rem6_run_o3_link_kind_checkpoint_boundary
rem6_run_timing_suppresses_o3_link_kind_window
```

- [ ] **Step 3: Build deterministic direct-call fixture**

Use this exact instruction layout from base `0x8000_0000`:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c addi x1, x0, 0x11
10 lw x12, 0(x18)
14 jal x1, +12
18 sw x7, 8(x18)        # skipped fallthrough witness
1c m5_fail
20 addi x13, x0, 42
24 addi x14, x13, 3
28 sw x14, 4(x18)
2c m5_exit
30 m5_fail
```

Set `DATA_START=0x100`; initialize the first word to `42`. The live window is `[load@10, call@14, addi@20, addi@24]`, and the architectural link result is `x1=0x8000_0018`.

- [ ] **Step 4: Add direct-call positive assertions**

Run detailed direct memory with issue width four and writeback width one. Assert:

```rust
assert_eq!(register_value(&completed, "x1"), 0x8000_0018);
assert_eq!(register_value(&completed, "x13"), 42);
assert_eq!(register_value(&completed, "x14"), 45);
assert_eq!(
    completed.pointer("/memory/0/hex").and_then(Value::as_str),
    Some("2a0000002d0000000000000000000000")
);
```

Assert the call event has `branch_kind=call_direct`, `branch_link_register_write=true`, issues before the load response, writes back before ordered commit, and causes one RAS push. At a tick after both target descendants issue but before the load response, assert exact four-ROB/one-LSQ residency, architectural `x1=0x11`, a non-null call ROB destination, and an integer rename-map entry for architectural register 1 with the same physical ID. For the width-one writeback run, assert:

```rust
let call = event_at_pc(&completed, DIRECT_CALL_PC);
let first_target = event_at_pc(&completed, DIRECT_TARGET_FIRST_PC);
assert_eq!(
    event_u64(call, "issue_tick"),
    event_u64(first_target, "issue_tick")
);
assert!(
    event_u64(first_target, "writeback_tick")
        > event_u64(call, "writeback_tick")
);
assert!(
    completed
        .pointer("/cores/0/o3_runtime/writeback_port/deferred_rows")
        .and_then(Value::as_u64)
        .is_some_and(|rows| rows > 0)
);
```

- [ ] **Step 5: Build and test the hierarchy indirect-call fixture**

Use this layout:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c auipc x11, 0
10 addi x11, x11, 0x20  # target 0x2c
14 addi x5, x0, 0x55
18 lw x12, 0(x18)
1c jalr x5, 0(x11)
20 sw x7, 8(x18)        # skipped fallthrough witness
24 m5_fail
28 nop
2c addi x13, x0, 42
30 addi x14, x13, 3
34 sw x14, 4(x18)
38 m5_exit
3c m5_fail
```

Run `cache-fabric-dram`. Assert `branch_kind=call_indirect`, one link write, final `x5=0x8000_0020`, exact four-ROB/one-LSQ residency before the response, ordered commit, skipped-store suppression, and nonzero cache data, transport data, fabric, and DRAM activity.

- [ ] **Step 6: Build and test the committed-RAS return fixture**

Use this layout:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c jal x1, +24          # committed call to function at 0x24
10 addi x13, x0, 42     # return target descendant 1
14 addi x14, x13, 3     # return target descendant 2
18 jal x0, +28           # jump to exit at 0x34
1c m5_fail
20 nop
24 lw x12, 0(x18)
28 jalr x0, 0(x1)
2c sw x7, 8(x18)        # skipped return fallthrough witness
30 m5_fail
34 sw x14, 4(x18)
38 m5_exit
3c m5_fail
```

Assert the committed call seeds `x1=0x8000_0010` and the RAS before the load window. Assert `[load@24, return@28, addi@10, addi@14]` is resident, the return event has `branch_kind=return` and no link write, both target descendants issue before the load response, and JSON stats show at least one RAS push, pop, use, and correct prediction with `target_provider.ras > 0`.

- [ ] **Step 7: Build and test older-branch rollback**

Use this layout:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c addi x7, x0, 1
10 addi x1, x0, 0x11
14 lw x12, 0(x18)
18 beq x7, x7, +24      # correct target 0x30, predicted fallthrough
1c jal x1, +12          # wrong-path call target 0x28
20 sw x7, 12(x18)       # wrong call-fallthrough witness
24 m5_fail
28 addi x13, x0, 42     # wrong call-target descendant
2c sw x13, 8(x18)       # wrong call-target memory witness
30 addi x15, x0, 0x33   # repairing branch target
34 sw x15, 4(x18)
38 m5_exit
3c m5_fail
```

The resident rows are `[load@14, branch@18, call@1c, addi@28]`. Assert final `x1=0x11`, `x13=0`, `x15=0x33`, and memory `2a000000330000000000000000000000`. Both wrong-path addresses must be absent from Data and Memory traces. The branch event must be mispredicted and squashing; the linked call must have no retired O3 event. JSON must record at least one RAS push and one RAS squash. Use `branch.issue_tick + 2` for the resident run, assert it is before the load response, and assert all four PCs are present before repair.

- [ ] **Step 8: Add the three-case target suppression test**

Run three deterministic binaries inside `rem6_run_o3_link_kind_live_target_sources_stay_terminal`:

1. Load-produced target:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c addi x5, x0, 0x55
10 lwu x11, 0(x18)      # data word contains 0x8000_0020
14 jalr x5, 0(x11)
18 sw x7, 8(x18)
1c m5_fail
20 addi x13, x0, 42
24 sw x13, 4(x18)
28 m5_exit
2c m5_fail
```

Assert call issue is at or after the load response. Run again at `load.issue_tick + 2`, assert that tick is before response, assert resident PCs `[0x10, 0x14]`, and assert no fetch at `0x20`. The completed run must end with `x5=0x8000_0018`, `x13=42`, and the success store.

2. Live-ALU-produced target:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c addi x5, x0, 0x55
10 lw x12, 0(x18)
14 auipc x11, 0
18 addi x11, x11, 24    # target 0x2c
1c jalr x5, 0(x11)
20 sw x7, 8(x18)
24 m5_fail
28 nop
2c addi x13, x0, 42
30 sw x13, 4(x18)
34 m5_exit
38 m5_fail
```

Assert the call is the terminal fourth row. Run at `call.issue_tick + 1`, assert the tick remains before the load response, assert resident PCs `[0x10, 0x14, 0x18, 0x1c]`, and assert no fetch at `0x2c`. The completed run must end with `x5=0x8000_0020`, `x13=42`, and the success store.

3. Same-window call/return:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c lw x12, 0(x18)
10 jal x1, +12          # function at 0x1c, return target 0x14
14 addi x13, x0, 42
18 jal x0, +16          # exit store at 0x28
1c jalr x0, 0(x1)
20 sw x7, 8(x18)
24 m5_fail
28 sw x13, 4(x18)
2c m5_exit
30 m5_fail
```

Run at `return.issue_tick + 1`, assert the tick remains before the load response, assert resident PCs `[0x0c, 0x10, 0x1c]`, and assert no fetch at `0x14`. The completed run must end with `x1=0x8000_0014`, `x13=42`, one success store, and no wrong return-fallthrough store.

For each case, assert final register/memory witnesses so suppression cannot pass through premature termination.

- [ ] **Step 9: Add mode-transfer, checkpoint, and timing rows**

Reuse the direct-call fixture.

For transfer, switch to timing one tick after the younger target ADDI issues and before the load response. Assert the transfer is non-restorable, carries four ROB and one LSQ row, preserves baseline issue/writeback/commit ticks for all four events, and reaches the same final link register and memory.

For checkpoint, assert a live capture fails with `checkpoint component is not quiescent: cpu0`; then checkpoint and restore after the result store commits. Assert no `o3-live-data-handoff` chunk, zero ROB/LSQ rows, unchanged existing schema versions, final `x1`, and final memory.

For timing, assert architectural parity plus absence of `/cores/0/o3_runtime`, an empty O3 trace, and no `sim.cpu0.o3.*` or gem5-style ROB/LSQ/rename/IQ/IEW/commit/FTQ aliases.

- [ ] **Step 10: Run the new matrix and fix only demonstrated integration defects**

Run:

```text
cargo test -p rem6 --test cli_run link_kind -- --nocapture
```

Expected before any integration fix: the direct, indirect, and return rows exercise the new runtime path; any failure must be reduced to an exact ownership defect rather than bypassed by weakening an assertion. Expected final result: all eight tests pass.

- [ ] **Step 11: Run neighboring control-window regressions**

Run:

```text
cargo test -p rem6 --test cli_run predicted_control -- --nocapture
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
```

Expected: all pass, including mixed no-link controls and anchored test ownership.

- [ ] **Step 12: Commit the executable matrix**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_kind.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/mixed_kind.rs crates/rem6/tests/source_policy/core_test_anchors.txt
git commit -m "test: cover linked O3 controls"
```

### Task 4: Update The Honest Migration Boundary

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update CPU prose in place**

Keep the heading and score exactly:

```text
### CPU Execution Models - 74% representative
```

In the migrated evidence paragraph, add the eight linked-control test anchors and state the exact supported boundary:

```text
one delayed scalar load plus a direct call, independent indirect call, or committed-RAS return; link destinations use staged integer rename and fixed-FU writeback ownership, returns remain no-write branch rows, older repairs remove younger link rename/writeback/RAS state, direct and cache/fabric/DRAM positives expose exact four-ROB/one-LSQ residency, and transfer/checkpoint/timing rows preserve the existing schema and suppression boundaries
```

Replace the broad open gap `link-writing calls and returns` with:

```text
same-window call-to-return link forwarding, coroutine pop-then-push forms, linked indirect calls whose target is a link register, producer-forwarded indirect targets, fourth-and-deeper linked chains, and arbitrary broader mixed windows
```

- [ ] **Step 2: Update `Next evidence` and the `tests/gem5/cpu_tests` row**

Use the same remaining-boundary wording in both places. Do not check the general O3 item, change 8/10 raw coverage, or change the 74% cap.

- [ ] **Step 3: Preserve the mechanical ledger contract**

Run:

```text
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected:

```text
1200 docs/architecture/gem5-to-rem6-migration.md
```

- [ ] **Step 4: Run documentation/source-policy gates**

Run:

```text
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
git diff --check
```

Expected: all pass with no score-calculation or line-count drift.

- [ ] **Step 5: Commit the ledger update**

```bash
git add docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record linked O3 control evidence"
```

### Task 5: Full Verification, Read-Only Review, And Push

**Files:**
- Review all files changed by Tasks 1-4.
- Do not edit `temp/` or `temp/reference_designs/gem5`.

- [ ] **Step 1: Run formatting and focused gates**

Run:

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu o3_source_operands -- --nocapture
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue -- --nocapture
cargo test -p rem6-cpu riscv_fetch_ahead -- --nocapture
cargo test -p rem6 --test cli_run link_kind -- --nocapture
cargo test -p rem6 --test cli_run predicted_control -- --nocapture
```

Expected: all pass.

- [ ] **Step 2: Run completion gates**

Run:

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

Expected: zero failures, ledger line count 1,200, and no whitespace errors.

- [ ] **Step 3: Dispatch four high-intensity read-only reviews**

Use four independent `gpt-5.5:xhigh` reviewers with these non-overlapping emphases:

1. Descriptor and policy fail-closed behavior, including coroutine/same-link rejection and live-target terminal semantics.
2. Runtime rename/writeback/rollback correctness, including exact one-write validation and reservation cleanup.
3. CLI matrix strength, real binary execution, RAS evidence, wrong-path Data/Memory suppression, transfer/checkpoint/timing boundaries.
4. Slop/ownership and migration honesty, including duplicate inventories, dead code, thin-module boundaries, test anchors, 74% cap, and 1,200-line ledger.

Reviewers are read-only. The main process applies any fixes serially and reruns every affected gate.

- [ ] **Step 4: Inspect final repository state**

Run:

```text
git status --short --branch
git log -5 --oneline
git diff origin/main...HEAD --stat
git diff origin/main...HEAD --check
git ls-files temp
```

Expected: only intended commits are ahead, no uncommitted changes, no whitespace errors, and no tracked `temp/` files added by this increment.

- [ ] **Step 5: Push and verify the remote head**

Run:

```text
git push origin main
git status --short --branch
git ls-remote origin refs/heads/main
git rev-parse HEAD
```

Expected: local `HEAD`, `origin/main`, and `ls-remote` all match.
