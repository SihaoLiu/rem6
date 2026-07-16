# RISC-V O3 Same-Window Coroutine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Admit one bounded same-window RISC-V coroutine transfer that consumes an exact linked-call RAS push, writes the other link register, and feeds one scalar descendant, with fail-closed lineage and real CLI evidence.

**Architecture:** Extend the existing live-control descriptor only for distinct-link `JALR` forms, then reuse the current latest-link policy, rename map, speculative source forwarding, branch issue, and fixed-FU writeback owners. Strengthen `RasRequired` target authority so recorded traversal distinguishes a plain `Pop` from `PopThenPush` and binds the replacement address exactly; derive link-write trace evidence from actual register writes rather than branch-kind labels.

**Tech Stack:** Rust workspace, `rem6-cpu`, generated RV64 ELF fixtures, `rem6 run --execute`, JSON O3/branch/RAS/debug evidence, Cargo source-policy tests.

---

## File Map

- Modify `crates/rem6-cpu/src/o3_source_operands.rs`: admit only distinct-link coroutine descriptors and retain same-link rejection.
- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: prove committed-source, exact same-window, overwrite-terminal, destination-shadowing, and provenance-consumption behavior.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`: add explicit expected RAS consumer authority, centralize authority construction, and validate exact recorded `PopThenPush` lineage.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead.rs`: re-export the shared authority helper and ignore the expected-consumer field only during the initial unconsumed-top lookup.
- Modify `crates/rem6-cpu/src/riscv_live_retire_window.rs`: consume the shared authority builder rather than duplicating push-sequence/address construction.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`: prove fresh and recorded coroutine target selection plus wrong-kind, wrong-address, and discarded-lineage suppression.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`: prove coroutine rename destination, call forwarding, issue dependency, and writeback reservation.
- Modify `crates/rem6-cpu/src/o3_runtime.rs`: remove branch-kind filtering from link-write evidence.
- Modify `crates/rem6-cpu/src/o3_runtime_tests.rs`: prove actual link-register writes are authoritative for coroutine trace classification.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`: own nine direct/hierarchy/suppression/repair/transfer/checkpoint/timing rows.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`: register the coroutine module.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: add all nine CLI anchors.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record the bounded capability while preserving `74% representative`, `8/10`, the unchecked general-O3 item, and exactly 1,200 lines.

Do not modify `temp/reference_designs/gem5`, any file under `temp/`, RAS checkpoint wire formats, handoff schema versions, public CLI/config APIs, or the general branch-kind classifier.

### Task 1: Admit Distinct-Link Coroutine Descriptors And Policy

**Files:**
- Modify: `crates/rem6-cpu/src/o3_source_operands.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`

- [ ] **Step 1: Write failing live-control descriptor tests**

Replace `live_control_descriptor_rejects_unsupported_link_forms` with two focused tests while preserving the existing non-link rejects:

```rust
#[test]
fn live_control_descriptor_classifies_coroutine_jalr_forms() {
    for (rd, rs1) in [(5, 1), (1, 5)] {
        assert_live_control(
            jalr(rd, rs1),
            BranchTargetKind::Return,
            &[register(rs1)],
            Some(register(rd)),
        );
    }
}

#[test]
fn live_control_descriptor_rejects_unsupported_link_forms() {
    for instruction in [jal(2), jalr(2, 9), jalr(2, 1), jalr(1, 1), jalr(5, 5)] {
        assert_eq!(
            o3_live_control_operands(instruction),
            None,
            "{instruction:?}"
        );
    }
}
```

- [ ] **Step 2: Write failing policy tests**

Add these tests beside the existing same-window return policy tests:

```rust
#[test]
fn scalar_memory_prefix_admits_coroutines_with_committed_targets() {
    for (destination, source) in [(5, 1), (1, 5)] {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window.classify_younger(jalr_with_registers(destination, source)),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        assert_eq!(
            window.classify_younger(addi(8, destination)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
    }
}

#[test]
fn same_window_coroutine_requires_exact_call_ras_prediction() {
    for (call, coroutine, destination) in [
        (jal_with_destination(1), jalr_with_registers(5, 1), 5),
        (jalr_with_registers(5, 9), jalr_with_registers(1, 5), 1),
    ] {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window.classify_sequenced_younger(call, 51).decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );
        let coroutine = window.classify_sequenced_younger(coroutine, 52);
        assert_eq!(
            coroutine.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        );
        assert_eq!(coroutine.ras_push_sequence(), Some(51));
        assert_eq!(
            window.classify_younger(addi(8, destination)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert!(window.is_full());
    }
}

#[test]
fn overwritten_coroutine_source_remains_terminal() {
    let mut window = scalar_load_window(4);
    assert_eq!(
        window.classify_younger(jal_with_destination(1)),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(addi(1, 1)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );
    assert_eq!(
        window.classify_younger(jalr_with_registers(5, 1)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );
    assert!(window.is_full());
}

#[test]
fn admitted_coroutine_does_not_publish_its_replacement_push() {
    let mut window = scalar_load_window(4);
    assert_eq!(
        window.classify_younger(jal_with_destination(1)),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(jalr_with_registers(5, 1)),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    assert_eq!(
        window.classify_younger(jalr_with_registers(0, 5)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );
    assert!(window.is_full());
}
```

Update both unsupported-form loops so only `jalr_with_registers(1, 1)` and `jalr_with_registers(5, 5)` remain rejected among link/link forms.

- [ ] **Step 3: Run the red tests**

Run:

```text
cargo test -p rem6-cpu live_control_descriptor_classifies_coroutine_jalr_forms -- --nocapture
cargo test -p rem6-cpu scalar_memory_prefix_admits_coroutines_with_committed_targets -- --nocapture
```

Expected: both fail because `o3_live_control_operands` returns `None` for distinct-link forms.

- [ ] **Step 4: Implement the minimal descriptor change**

Change the `Jalr` destination match in `o3_live_control_operands` to:

```rust
RiscvInstruction::Jalr { rd, rs1, .. }
    if is_riscv_link_register(rd)
        && (!is_riscv_link_register(rs1) || rd.index() != rs1.index()) =>
{
    Some(Some(rd))
}
```

Do not add a coroutine-specific policy branch. The existing `Return` handling must supply exact latest-call admission, destination shadowing, and provenance consumption.

- [ ] **Step 5: Run focused and package tests**

Run:

```text
cargo test -p rem6-cpu live_control_descriptor -- --nocapture
cargo test -p rem6-cpu coroutine -- --nocapture
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
```

Expected: all pass; same-link forms remain rejected.

- [ ] **Step 6: Commit**

```text
git add crates/rem6-cpu/src/o3_source_operands.rs crates/rem6-cpu/src/riscv_o3_window_policy.rs
git commit -m "cpu: admit bounded coroutine controls"
```

### Task 2: Bind Exact Pop-Then-Push Target Authority

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`

- [ ] **Step 1: Add failing fresh coroutine frontend coverage**

Add:

```rust
#[test]
fn detailed_scalar_window_forwards_call_ras_to_same_window_coroutine() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let coroutine = i_type(0, 1, 0x0, 5, 0x67);
    let descendant = i_type(0, 5, 0x0, 7, 0x13);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, coroutine.to_le_bytes().to_vec()),
        (3, 0x8008, descendant.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision).unwrap(),
    );
    let coroutine_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(coroutine_decision.pc(), Address::new(0x8008));
    assert_eq!(
        coroutine_decision.branch_speculation().unwrap().target(),
        Some(Address::new(0x8008))
    );
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&coroutine_decision)
            .unwrap(),
    );

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.return_address_stack.stack_entries(),
        &[Address::new(0x8010)]
    );
    assert_eq!(state.return_address_stack.pending_operation_count(), 2);
    assert_eq!(
        state.return_address_stack.pending_operations()[1].kind(),
        ReturnAddressStackOperationKind::PopThenPush
    );
    assert_eq!(
        state.return_address_stack.pending_operations()[1].pushed_address(),
        Some(Address::new(0x8010))
    );
}
```

- [ ] **Step 2: Add failing recorded-lineage tests**

Add a local setup helper and three tests:

```rust
fn recorded_same_window_coroutine_core() -> RiscvCore {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let coroutine = i_type(0, 1, 0x0, 5, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, coroutine.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision).unwrap(),
    );
    let coroutine_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&coroutine_decision)
            .unwrap(),
    );
    core
}

#[test]
fn detailed_recorded_coroutine_accepts_exact_pop_then_push() {
    let core = recorded_same_window_coroutine_core();
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_recorded_coroutine_rejects_wrong_replacement_address() {
    let core = recorded_same_window_coroutine_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let coroutine_sequence = 2;
        state.squash_return_address_stack_speculation(coroutine_sequence).unwrap();
        let wrong = state
            .return_address_stack
            .pop_then_push_speculative(Address::new(0x9000));
        state
            .return_address_stack_operations
            .insert(coroutine_sequence, wrong.id());
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_recorded_coroutine_rejects_plain_pop_consumer() {
    let core = recorded_same_window_coroutine_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let coroutine_sequence = 2;
        state.squash_return_address_stack_speculation(coroutine_sequence).unwrap();
        let wrong = state.return_address_stack.pop_speculative();
        state
            .return_address_stack_operations
            .insert(coroutine_sequence, wrong.id());
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_invalid_recorded_coroutine_does_not_retry_as_fresh_prediction() {
    let core = recorded_same_window_coroutine_core();
    core.set_branch_lookahead(3);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let coroutine_sequence = 2;
        state
            .squash_return_address_stack_speculation(coroutine_sequence)
            .unwrap();
        assert!(state.return_address_stack_operations.contains_key(&1));
        assert!(!state
            .return_address_stack_operations
            .contains_key(&coroutine_sequence));
        assert_eq!(state.return_address_stack.top(), Some(Address::new(0x8008)));
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
```

Retain `detailed_invalid_recorded_return_does_not_retry_as_fresh_prediction` as
the plain-`Pop` counterpart.

- [ ] **Step 3: Run the red frontend tests**

Run:

```text
cargo test -p rem6-cpu detailed_scalar_window_forwards_call_ras_to_same_window_coroutine -- --nocapture
cargo test -p rem6-cpu detailed_recorded_coroutine -- --nocapture
```

Expected: the fresh test reaches the coroutine after Task 1, but recorded traversal fails because `recorded_ras_required_target` requires `Pop`.

- [ ] **Step 4: Add the expected-consumer type and shared authority builder**

In `detailed_o3.rs`, import `RiscvSequencedScalarIntegerYoungerDecision` and add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequiredRasConsumer {
    Pop,
    PopThenPush { pushed_address: Address },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PredictedControlTargetAuthority {
    Normal,
    RasRequired {
        push_sequence: u64,
        pushed_address: Address,
        consumer: RequiredRasConsumer,
    },
}

pub(crate) fn predicted_control_target_authority(
    instruction: RiscvInstruction,
    sequential_pc: Address,
    classification: RiscvSequencedScalarIntegerYoungerDecision,
    sequenced_return_addresses: &[(u64, Address)],
) -> Option<PredictedControlTargetAuthority> {
    if classification.decision()
        != RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    {
        return Some(PredictedControlTargetAuthority::Normal);
    }
    let push_sequence = classification.ras_push_sequence()?;
    let pushed_address = sequenced_return_addresses
        .iter()
        .rev()
        .find_map(|(sequence, address)| (*sequence == push_sequence).then_some(*address))?;
    let consumer = match super::return_address_stack_action(instruction, sequential_pc)? {
        super::ReturnAddressStackAction::Pop => RequiredRasConsumer::Pop,
        super::ReturnAddressStackAction::PopThenPush(pushed_address) => {
            RequiredRasConsumer::PopThenPush { pushed_address }
        }
        super::ReturnAddressStackAction::Push(_) => return None,
    };
    Some(PredictedControlTargetAuthority::RasRequired {
        push_sequence,
        pushed_address,
        consumer,
    })
}
```

Replace both duplicated constructors in `detailed_o3.rs` with:

```rust
let Some(target_authority) = predicted_control_target_authority(
    younger.decoded().instruction(),
    sequential_pc,
    classification,
    &sequenced_return_addresses,
) else {
    return DetailedFetchAheadCandidate::Blocked;
};
```

Use `next.decoded().instruction()` in the second constructor.

- [ ] **Step 5: Validate the exact recorded consumer**

Pass `consumer` from `recorded_predicted_pc` into `recorded_ras_required_target`. Replace the hard-coded pop check with:

```rust
let consumer_matches = match consumer {
    RequiredRasConsumer::Pop => {
        pop.kind() == ReturnAddressStackOperationKind::Pop
            && pop.pushed_address().is_none()
    }
    RequiredRasConsumer::PopThenPush { pushed_address } => {
        pop.kind() == ReturnAddressStackOperationKind::PopThenPush
            && pop.pushed_address() == Some(pushed_address)
            && pop.stack_after().last().copied() == Some(pushed_address)
    }
};
if push.kind() != ReturnAddressStackOperationKind::Push
    || !consumer_matches
    || push.pushed_address() != Some(pushed_address)
    || push.stack_after() != pop.stack_before()
    || pop.predicted_return() != push.pushed_address()
{
    return None;
}
```

Keep adjacency, branch-kind, sequence-to-operation-id, prediction-taken, and target-equality checks unchanged.

- [ ] **Step 6: Share authority construction with live retirement**

Re-export the helper and consumer type in `riscv_fetch_ahead.rs`:

```rust
pub(crate) use detailed_o3::{
    predicted_control_target_authority, recorded_predicted_pc,
    PredictedControlTargetAuthority, RecordedPredictedPc, RequiredRasConsumer,
};
```

In `riscv_live_retire_window.rs`, replace its manual `RasRequired` construction with:

```rust
let Some(target_authority) =
    crate::riscv_fetch_ahead::predicted_control_target_authority(
        instruction.decoded.instruction(),
        sequential_pc,
        classification,
        &sequenced_return_addresses,
    )
else {
    break;
};
```

In `direct_jump_fetch_ahead_target`, match `RasRequired { push_sequence, pushed_address, .. }` because fresh target lookup only needs the exact producer push.

- [ ] **Step 7: Run focused and package tests**

Run:

```text
cargo test -p rem6-cpu detailed_recorded_coroutine -- --nocapture
cargo test -p rem6-cpu detailed_same_window_return -- --nocapture
cargo test -p rem6-cpu riscv_fetch_ahead -- --nocapture
```

Expected: coroutine and ordinary-return lineage tests pass; wrong kind/address and invalid no-retry tests remain blocked.

- [ ] **Step 8: Commit**

```text
git add crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_fetch_ahead.rs crates/rem6-cpu/src/riscv_live_retire_window.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs
git commit -m "cpu: bind coroutine RAS consumer authority"
```

### Task 3: Prove Runtime Rename, Forwarding, Writeback, And Link Evidence

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_tests.rs`

- [ ] **Step 1: Add a failing coroutine runtime candidate test**

Add beside `same_window_return_candidate_uses_link_call_forwarding`:

```rust
#[test]
fn same_window_coroutine_uses_call_forwarding_and_link_destination() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(runtime.set_writeback_width(1));
    let load = scalar_load_event();
    let call = jal_link(1, 8);
    let coroutine = jalr_link(5, 1);
    let descendant = addi(8, 5, 0);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), call),
                (Address::new(0x800c), coroutine),
                (Address::new(0x8008), descendant),
            ],
        ),
        3
    );

    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), call)
        .expect("linked call candidate");
    let call_sequence = call_candidate.sequence();
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                call,
                0x8004,
                0x800c,
                vec![RegisterWrite::new(reg(1), 0x8008)],
                None,
            ),
        )
        .unwrap());

    let coroutine_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), coroutine)
        .expect("same-window coroutine candidate");
    assert_eq!(
        coroutine_candidate.destination().unwrap().architectural(),
        5
    );
    assert!(coroutine_candidate
        .producer_sequences()
        .contains(&call_sequence));
    assert_eq!(
        coroutine_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(1), 0x8008)]
    );
    assert_eq!(coroutine_candidate.issue_tick(1), 20);
    assert!(runtime
        .record_live_speculative_execution(
            coroutine_candidate,
            &[request(12)],
            1,
            RiscvExecutionRecord::new(
                coroutine,
                0x800c,
                0x8008,
                vec![RegisterWrite::new(reg(5), 0x8010)],
                None,
            ),
        )
        .unwrap());
    let issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.execution.instruction() == coroutine)
        .expect("recorded coroutine execution");
    assert_eq!(issued.writeback_slot, Some(0));
    assert!(issued.admitted_writeback_tick >= 20);
}
```

- [ ] **Step 2: Add failing link-write authority tests**

Add to `o3_runtime_tests.rs`:

```rust
#[test]
fn branch_link_write_uses_actual_coroutine_register_write() {
    let coroutine = RiscvInstruction::Jalr {
        rd: rem6_isa_riscv::Register::new(5).unwrap(),
        rs1: rem6_isa_riscv::Register::new(1).unwrap(),
        offset: rem6_isa_riscv::Immediate::new(0),
    };
    let record = RiscvExecutionRecord::new(
        coroutine,
        0x800c,
        0x8008,
        vec![rem6_isa_riscv::RegisterWrite::new(
            rem6_isa_riscv::Register::new(5).unwrap(),
            0x8010,
        )],
        None,
    );

    assert!(o3_branch_link_register_write(&record));
}

#[test]
fn branch_link_write_keeps_plain_return_false() {
    let plain_return = RiscvInstruction::Jalr {
        rd: rem6_isa_riscv::Register::new(0).unwrap(),
        rs1: rem6_isa_riscv::Register::new(1).unwrap(),
        offset: rem6_isa_riscv::Immediate::new(0),
    };
    let record = RiscvExecutionRecord::new(plain_return, 0x800c, 0x8008, vec![], None);

    assert!(!o3_branch_link_register_write(&record));
}
```

- [ ] **Step 3: Run the red runtime tests**

Run:

```text
cargo test -p rem6-cpu same_window_coroutine_uses_call_forwarding_and_link_destination -- --nocapture
cargo test -p rem6-cpu branch_link_write_uses_actual_coroutine_register_write -- --nocapture
```

Expected: the candidate test passes after Tasks 1-2; the link-write test fails because the helper filters out `return` branch kinds.

- [ ] **Step 4: Remove the legacy branch-kind filter**

Replace `o3_branch_link_register_write` with:

```rust
fn o3_branch_link_register_write(record: &rem6_isa_riscv::RiscvExecutionRecord) -> bool {
    record
        .register_writes()
        .iter()
        .any(|write| is_riscv_link_register(write.register()))
}
```

Remove the now-unused `riscv_branch_target_kind` import from `o3_runtime.rs`. Do not change branch-kind classification or stats indexing.

- [ ] **Step 5: Run focused and package tests**

Run:

```text
cargo test -p rem6-cpu same_window_coroutine -- --nocapture
cargo test -p rem6-cpu branch_link_write -- --nocapture
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
```

Expected: all pass; plain returns remain link-write false.

- [ ] **Step 6: Commit**

```text
git add crates/rem6-cpu/src/o3_runtime_control_window_tests.rs crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_tests.rs
git commit -m "cpu: report coroutine link write ownership"
```

### Task 4: Add CLI Scaffold And Direct Positive Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`

- [ ] **Step 1: Register the focused module**

Add before `link_kind`:

```rust
#[path = "predicted_control/coroutine.rs"]
mod coroutine;
```

- [ ] **Step 2: Add shared constants, runner, and direct fixture**

Create `coroutine.rs` with these imports and helpers:

```rust
use super::window_support::{
    assert_branch_kind_and_link, assert_direct_memory_activity,
    assert_drained_control_runtime, assert_final_execution_mode, assert_hierarchy_activity,
    assert_integer_rename_maps_to_row_destination, assert_no_data_address, assert_no_fetch_pc,
    assert_no_o3_stats, assert_ordered_commits, assert_register_absent_or_zero,
    assert_stopped_by_host, control_window_command, resident_rob_pcs,
    run_control_window_json,
};
use super::*;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const WRONG_STORE_ADDRESS: &str = "0x80000108";
const WRONG_STORE_12_ADDRESS: &str = "0x8000010c";
const DIRECT_WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "1",
];

fn run_coroutine_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
    extra_args: &[&str],
) -> Value {
    run_control_window_json(
        path,
        memory_system,
        max_tick,
        execution_mode,
        branch_lookahead,
        DATA_ADDRESS,
        16,
        extra_args,
    )
}

fn direct_coroutine_binary(name: &str, exit_padding_words: usize) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(12, 1),
        i_type(0, 5, 0x0, 13, 0x13),
        j_type(16, 0),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        s_type(4, 13, 18, 0b010),
    ]);
    words.extend(std::iter::repeat_n(
        i_type(0, 0, 0x0, 0, 0x13),
        exit_padding_words,
    ));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    finish_coroutine_binary(name, words, [42, 0, 0, 0])
}

fn finish_coroutine_binary(
    name: &str,
    mut words: Vec<u32>,
    data_words: [u32; 4],
) -> std::path::PathBuf {
    while words.len() * 4 < DATA_START as usize {
        words.push(0);
    }
    words.extend(data_words);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
```

- [ ] **Step 3: Write the direct red CLI test**

Add:

```rust
#[test]
fn rem6_run_o3_same_window_coroutine_commits_direct() {
    let path = direct_coroutine_binary("o3-same-window-coroutine-direct", 0);
    let completed = run_coroutine_json(&path, "direct", 2_500, "detailed", 2, &DIRECT_WIDTH_ARGS);

    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), 0x8000_0014);
    assert_eq!(register_value(&completed, "x5"), 0x8000_0020);
    assert_eq!(register_value(&completed, "x13"), 0x8000_0020);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000200000800000000000000000")
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, "0x8000000c");
    let call = event_at_pc(&completed, "0x80000010");
    let coroutine = event_at_pc(&completed, "0x8000001c");
    let descendant = event_at_pc(&completed, "0x80000014");
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, "call_direct", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    for event in [call, coroutine, descendant] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert!(event_u64(coroutine, "issue_tick") > event_u64(call, "writeback_tick"));
    assert!(event_u64(descendant, "issue_tick") > event_u64(coroutine, "writeback_tick"));
    assert_ordered_commits([load, call, coroutine, descendant]);
    assert_eq!(
        completed
            .pointer("/cores/0/o3_runtime/writeback_port/admitted_rows")
            .and_then(Value::as_u64),
        Some(4)
    );

    let live_tick = event_u64(descendant, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_coroutine_json(
        &path,
        "direct",
        live_tick,
        "detailed",
        2,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        ["0x8000000c", "0x80000010", "0x8000001c", "0x80000014"]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_register_absent_or_zero(&resident, "x1");
    assert_register_absent_or_zero(&resident, "x5");
    assert_integer_rename_maps_to_row_destination(&resident, "0x80000010", 1);
    assert_integer_rename_maps_to_row_destination(&resident, "0x8000001c", 5);

    for (pointer, expected) in [
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 1),
        ("/cores/0/branch_predictor/ras/used", 1),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 0),
        ("/cores/0/branch_predictor/target_provider/ras", 1),
    ] {
        assert_eq!(completed.pointer(pointer).and_then(Value::as_u64), Some(expected));
    }
    assert_direct_memory_activity(&completed);
}
```

- [ ] **Step 4: Run the direct CLI test red then green**

Run before Tasks 1-3 are applied to verify the original boundary:

```text
cargo test -p rem6 --test cli_run rem6_run_o3_same_window_coroutine_commits_direct -- --nocapture
```

Expected before core implementation: FAIL because the coroutine is not resident and its descendant does not issue before the load response. Expected after Tasks 1-3: PASS.

- [ ] **Step 5: Commit**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs
git commit -m "test: prove direct same-window coroutine"
```

### Task 5: Add Reverse-Link Hierarchy Positive Evidence

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`

- [ ] **Step 1: Add the reverse-link indirect-call fixture**

```rust
fn indirect_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0x24 - target_auipc_pc, 11, 0x0, 11, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 11, 0x0, 5, 0x67),
        i_type(0, 1, 0x0, 13, 0x13),
        j_type(16, 0),
        i_type(0, 5, 0x0, 1, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_coroutine_binary(name, words, [42, 0, 0, 0])
}
```

The key PCs are load `0x14`, indirect call `0x18`, coroutine `0x24`, and descendant `0x1c`.

- [ ] **Step 2: Add the hierarchy test**

Create `rem6_run_o3_same_window_indirect_coroutine_commits_cache_fabric_dram` with this complete positive body:

```rust
let path = indirect_coroutine_binary("o3-same-window-indirect-coroutine");
let completed = run_coroutine_json(
    &path,
    "cache-fabric-dram",
    3_000,
    "detailed",
    2,
    &DIRECT_WIDTH_ARGS,
);
assert_stopped_by_host(&completed);
assert_eq!(register_value(&completed, "x5"), 0x8000_001c);
assert_eq!(register_value(&completed, "x1"), 0x8000_0028);
assert_eq!(register_value(&completed, "x13"), 0x8000_0028);
assert_eq!(
    completed.pointer("/memory/0/hex").and_then(Value::as_str),
    Some("2a000000280000800000000000000000")
);
let load = event_at_pc(&completed, "0x80000014");
let call = event_at_pc(&completed, "0x80000018");
let coroutine = event_at_pc(&completed, "0x80000024");
let descendant = event_at_pc(&completed, "0x8000001c");
let response_tick = event_u64(load, "lsq_data_response_tick");
assert_branch_kind_and_link(call, "call_indirect", true);
assert_branch_kind_and_link(coroutine, "return", true);
for event in [call, coroutine, descendant] {
    assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
}
assert!(event_u64(coroutine, "issue_tick") > event_u64(call, "writeback_tick"));
assert!(event_u64(descendant, "issue_tick") > event_u64(coroutine, "writeback_tick"));
assert_ordered_commits([load, call, coroutine, descendant]);
assert_eq!(
    completed
        .pointer("/cores/0/o3_runtime/writeback_port/admitted_rows")
        .and_then(Value::as_u64),
    Some(4)
);
let live_tick = event_u64(descendant, "issue_tick") + 1;
assert!(live_tick < response_tick);
let resident = run_coroutine_json(
    &path,
    "cache-fabric-dram",
    live_tick,
    "detailed",
    2,
    &DIRECT_WIDTH_ARGS,
);
assert_eq!(
    resident_rob_pcs(&resident),
    ["0x80000014", "0x80000018", "0x80000024", "0x8000001c"]
);
assert_eq!(
    resident
        .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
        .and_then(Value::as_u64),
    Some(1)
);
assert_register_absent_or_zero(&resident, "x1");
assert_register_absent_or_zero(&resident, "x5");
assert_integer_rename_maps_to_row_destination(&resident, "0x80000018", 5);
assert_integer_rename_maps_to_row_destination(&resident, "0x80000024", 1);
assert_eq!(
    completed
        .pointer("/cores/0/branch_predictor/target_provider/indirect")
        .and_then(Value::as_u64),
    Some(1)
);
for (pointer, expected) in [
    ("/cores/0/branch_predictor/ras/pushes", 2),
    ("/cores/0/branch_predictor/ras/pops", 1),
    ("/cores/0/branch_predictor/ras/used", 1),
    ("/cores/0/branch_predictor/ras/correct", 1),
    ("/cores/0/branch_predictor/ras/incorrect", 0),
    ("/cores/0/branch_predictor/target_provider/ras", 1),
] {
    assert_eq!(completed.pointer(pointer).and_then(Value::as_u64), Some(expected));
}
assert_hierarchy_activity(&completed);
```

- [ ] **Step 3: Run both positives**

```text
cargo test -p rem6 --test cli_run same_window_coroutine_commits -- --nocapture
```

Expected: both direct and hierarchy rows pass.

- [ ] **Step 4: Commit**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs
git commit -m "test: cover reverse-link coroutine hierarchy"
```

### Task 6: Add Lookahead And Overwrite Suppression

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`

- [ ] **Step 1: Add lookahead-one suppression**

Implement `rem6_run_o3_same_window_coroutine_requires_branch_lookahead_two` with the direct fixture and lookahead one. Assert:

```rust
let load = event_at_pc(&completed, "0x8000000c");
let call = event_at_pc(&completed, "0x80000010");
let coroutine = event_at_pc(&completed, "0x8000001c");
let descendant = event_at_pc(&completed, "0x80000014");
let response_tick = event_u64(load, "lsq_data_response_tick");
assert!(event_u64(call, "issue_tick") < response_tick);
assert!(event_u64(descendant, "issue_tick") > response_tick);
let resident = run_coroutine_json(
    &path,
    "direct",
    response_tick - 1,
    "detailed",
    1,
    &DIRECT_WIDTH_ARGS,
);
assert_eq!(resident_rob_pcs(&resident), ["0x8000000c", "0x80000010"]);
assert_eq!(
    resident
        .pointer("/cores/0/branch_predictor/ras/pushes")
        .and_then(Value::as_u64),
    Some(1)
);
for pointer in [
    "/cores/0/branch_predictor/lookups/return",
    "/cores/0/branch_predictor/ras/pops",
    "/cores/0/branch_predictor/target_provider/ras",
] {
    assert_eq!(resident.pointer(pointer).and_then(Value::as_u64), Some(0));
}
assert_no_fetch_pc(&resident, "0x80000014");
assert!(event_u64(coroutine, "commit_tick") <= event_u64(descendant, "commit_tick"));
```

- [ ] **Step 2: Add the overwrite fixture**

```rust
fn overwritten_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(16, 1),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(32, 1, 0x0, 1, 0x13),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 5, 0x0, 13, 0x13),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_coroutine_binary(name, words, [42, 0, 0, 0])
}
```

- [ ] **Step 3: Add overwrite-terminal evidence**

Implement `rem6_run_o3_same_window_overwritten_coroutine_source_stays_terminal`. The completed witnesses are `x1=0x80000034`, `x5=0x80000028`, `x13=0x80000028`, and memory `2a000000280000800000000000000000`. Before response, assert resident PCs:

```rust
["0x8000000c", "0x80000010", "0x80000020", "0x80000024"]
```

Assert live `x1` maps to the overwrite row `0x80000020`, live `x5` maps to the terminal coroutine row `0x80000024`, coroutine issue is at or after overwrite writeback, and neither `0x80000014` nor `0x80000034` appears in fetch trace before response.

- [ ] **Step 4: Run suppression rows**

```text
cargo test -p rem6 --test cli_run same_window_coroutine_requires -- --nocapture
cargo test -p rem6 --test cli_run overwritten_coroutine -- --nocapture
```

Expected: both pass with exact resident rows and no descendant target fetch.

- [ ] **Step 5: Commit**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs
git commit -m "test: bound coroutine frontend suppression"
```

### Task 7: Add Older Repair And Wrong-Target Repair

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`

- [ ] **Step 1: Add the older-branch fixture**

Use this exact wrong-path shape:

```rust
fn older_branch_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(0x11, 0, 0x0, 1, 0x13),
        i_type(0x55, 0, 0x0, 5, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        b_type(28, 7, 7, 0b000),
        j_type(12, 1),
        i_type(0, 5, 0x0, 13, 0x13),
        s_type(8, 7, 18, 0b010),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(12, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0x33, 0, 0x0, 15, 0x13),
        s_type(4, 15, 18, 0b010),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    finish_coroutine_binary(name, words, [42, 0, 0, 0])
}
```

The exact resident PC order is load `0x80000018`, older branch `0x8000001c`, call `0x80000020`, and coroutine `0x8000002c`.

- [ ] **Step 2: Add exact older-repair assertions**

Implement `rem6_run_o3_older_branch_discards_same_window_coroutine_chain` with hierarchy memory and lookahead three. Assert final `x1=0x11`, `x5=0x55`, absent/zero `x13`, `x15=0x33`, exact memory `2a000000330000000000000000000000`, and no accesses to either wrong-store address. Assert one call and one coroutine lookup, zero commits for those branch kinds, one squash for each, and:

```rust
for (pointer, expected) in [
    ("/cores/0/branch_predictor/ras/pushes", 3),
    ("/cores/0/branch_predictor/ras/pops", 3),
    ("/cores/0/branch_predictor/ras/squashes", 2),
    ("/cores/0/branch_predictor/ras/used", 0),
    ("/cores/0/branch_predictor/ras/correct", 0),
    ("/cores/0/branch_predictor/ras/incorrect", 0),
] {
    assert_eq!(completed.pointer(pointer).and_then(Value::as_u64), Some(expected));
}
```

At the pre-repair live tick, assert both wrong-path link rename mappings belong to the call and coroutine rows.

- [ ] **Step 3: Add the wrong-target fixture**

```rust
fn wrong_target_coroutine_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(12, 1),
        i_type(99, 0, 0x0, 14, 0x13),
        s_type(8, 7, 18, 0b010),
        i_type(20, 1, 0x0, 5, 0x67),
        s_type(4, 13, 18, 0b010),
        m5op(M5_EXIT),
        i_type(0, 5, 0x0, 13, 0x13),
        i_type(0, 5, 0x0, 0, 0x67),
        m5op(M5_FAIL),
    ]);
    finish_coroutine_binary(name, words, [42, 0, 0, 0])
}
```

The call at `0x10` pushes `0x14`. The coroutine at `0x1c` predicts `0x14`, writes `x5=0x20`, resolves to `0x28`, squashes the `x14` descendant, and the later plain return at `0x2c` consumes `0x20`.

- [ ] **Step 4: Add wrong-target end-to-end evidence**

Implement `rem6_run_o3_same_window_coroutine_wrong_target_repairs_descendants`. Assert final `x1=0x80000014`, `x5=0x80000020`, `x13=0x80000020`, absent/zero `x14`, exact memory `2a000000200000800000000000000000`, and no wrong store. For the coroutine event assert `branch_kind=return`, `link_write=true`, predicted target `0x80000014`, resolved target `0x80000028`, mispredicted and squashed. Assert the later return at `0x8000002c` has `link_write=false`, target provider RAS, and reaches `0x80000020`. Lock totals:

```rust
for (pointer, expected) in [
    ("/cores/0/branch_predictor/ras/pushes", 2),
    ("/cores/0/branch_predictor/ras/pops", 2),
    ("/cores/0/branch_predictor/ras/used", 2),
    ("/cores/0/branch_predictor/ras/correct", 1),
    ("/cores/0/branch_predictor/ras/incorrect", 1),
    ("/cores/0/branch_predictor/target_provider/ras", 2),
] {
    assert_eq!(completed.pointer(pointer).and_then(Value::as_u64), Some(expected));
}
```

- [ ] **Step 5: Run both repair tests**

```text
cargo test -p rem6 --test cli_run older_branch_discards_same_window_coroutine -- --nocapture
cargo test -p rem6 --test cli_run coroutine_wrong_target -- --nocapture
```

Expected: exact row, rename, branch, RAS, and wrong-path suppression assertions pass. If a PC differs, inspect the generated word layout and correct the fixture or assertion; do not weaken to presence-only checks.

- [ ] **Step 6: Commit**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs
git commit -m "test: prove coroutine repair lifecycles"
```

### Task 8: Add Switch, Checkpoint, And Timing Boundaries

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`

- [ ] **Step 1: Add detailed-to-timing transfer evidence**

Implement `rem6_run_host_switch_transfers_o3_same_window_coroutine` using `direct_coroutine_binary("o3-same-window-coroutine-switch", 0)`. Choose `switch_tick = descendant.issue_tick + 1`, require it before load response, and add `--host-switch-cpu-mode {switch_tick}:cpu0:timing`. Assert final mode, registers, memory, no wrong stores, `restorable=false`, O3 snapshot counts `ROB=4` and `LSQ=1`, live-data handoff schema `7`, one outstanding request, one resident row, three younger rows, first operation `load`, target `memory`, source partition `0`, address `DATA_ADDRESS`, and width `4`. Compare baseline versus switched `issue_tick`, `writeback_tick`, and `commit_tick` for all four PCs. Assert exact call/return/RAS counters match baseline and finish with `assert_drained_control_runtime`.

- [ ] **Step 2: Add live-reject and drained checkpoint evidence**

Implement `rem6_run_o3_same_window_coroutine_checkpoint_boundary` using `direct_coroutine_binary("o3-same-window-coroutine-checkpoint", 8)`. At the live tick, run `control_window_command` with `--host-checkpoint {live_tick}:coroutine-live` and assert failure, empty stdout, and stderr containing:

```text
checkpoint component is not quiescent: cpu0
```

For the drained path, checkpoint one tick after the padded exit-side commit and restore one tick later. Assert one checkpoint and one restore, exact final registers/memory, no live-data handoff chunk, decoded runtime `snapshot_rob_entries=0`, `snapshot_lsq_entries=0`, and drained runtime.

- [ ] **Step 3: Add timing suppression**

Implement `rem6_run_timing_suppresses_o3_same_window_coroutine` with the direct fixture in timing mode. Assert exact final architectural witnesses, no wrong store, no `/cores/0/o3_runtime`, empty `/debug/o3_trace`, and `assert_no_o3_stats`.

- [ ] **Step 4: Run lifecycle rows**

```text
cargo test -p rem6 --test cli_run transfers_o3_same_window_coroutine -- --nocapture
cargo test -p rem6 --test cli_run coroutine_checkpoint_boundary -- --nocapture
cargo test -p rem6 --test cli_run timing_suppresses_o3_same_window_coroutine -- --nocapture
```

Expected: all pass with exact transfer and checkpoint payload assertions.

- [ ] **Step 5: Commit**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs
git commit -m "test: cover coroutine lifecycle boundaries"
```

### Task 9: Register Source-Policy Anchors And Update The Ledger

**Files:**
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add all nine anchors together**

Insert after the same-window link-return anchors:

```text
rem6_run_o3_same_window_coroutine_commits_direct
rem6_run_o3_same_window_indirect_coroutine_commits_cache_fabric_dram
rem6_run_o3_same_window_coroutine_requires_branch_lookahead_two
rem6_run_o3_same_window_overwritten_coroutine_source_stays_terminal
rem6_run_o3_older_branch_discards_same_window_coroutine_chain
rem6_run_host_switch_transfers_o3_same_window_coroutine
rem6_run_o3_same_window_coroutine_checkpoint_boundary
rem6_run_timing_suppresses_o3_same_window_coroutine
rem6_run_o3_same_window_coroutine_wrong_target_repairs_descendants
```

- [ ] **Step 2: Update all ledger surfaces atomically**

Keep the CPU heading `### CPU Execution Models - 74% representative`, raw score `8/10`, and general O3 checkbox unchanged. Update:

1. the migrated CPU narrative with one bounded call-to-coroutine-to-scalar-descendant window, both link directions, exact PopThenPush lineage, link-write rename/writeback, direct/hierarchy, suppression, repair, switch, checkpoint, and timing evidence;
2. the CPU `Not migrated` text so broad `coroutine pop-then-push forms` becomes the narrower open boundary of another same-window control consuming the coroutine replacement push and deeper coroutine chains;
3. `Next evidence` with the same narrowed boundary;
4. the `tests/gem5/cpu_tests` mapping row with the nine test names and no score inflation.

Preserve linked indirect calls whose target source is `x1`/`x5`, producer-forwarded indirect targets, fourth-and-deeper chains, and general O3 as open.

- [ ] **Step 3: Preserve exactly 1,200 lines**

Run:

```text
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: exactly `1200`. Reflow existing paragraphs rather than adding blank-line churn.

- [ ] **Step 4: Run source-policy tests**

```text
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
```

Expected: `49 passed` and `25 passed` or the current exact totals with zero failures. Confirm the heading, score calculation, checklist, migrated/not-migrated/next-evidence text, anchors, and line count are mechanically accepted.

- [ ] **Step 5: Commit**

```text
git add crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record bounded O3 coroutine evidence"
```

### Task 10: Verify, Review, Harden, And Push

**Files:**
- Review every file changed by Tasks 1-9.
- Do not modify `temp/` or `temp/reference_designs/gem5`.

- [ ] **Step 1: Run focused feature verification**

```text
cargo test -p rem6-cpu coroutine -- --nocapture
cargo test -p rem6-cpu detailed_same_window_return -- --nocapture
cargo test -p rem6 --test cli_run same_window_coroutine -- --nocapture
```

Expected: all descriptor, policy, exact-RAS, runtime, and nine CLI rows pass.

- [ ] **Step 2: Run package and policy verification**

```text
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
cargo fmt --all -- --check
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: all tests and format/diff checks pass; ledger output is `1200`.

- [ ] **Step 3: Run full-workspace verification**

```text
cargo test --workspace --all-targets --quiet
```

Expected: exit zero. If a test fails, run that exact test in isolation, diagnose it with `superpowers:systematic-debugging`, then rerun the complete workspace. Do not label a failure flaky from one rerun.

- [ ] **Step 4: Audit policy and worktree boundaries**

```text
git status --short --branch
git diff -- temp temp/reference_designs/gem5
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

Expected: no `temp` diff, no untracked generated artifacts, only intended commits, and no unrelated user changes staged.

- [ ] **Step 5: Request independent read-only review**

Dispatch focused reviewers for:

1. descriptor/policy scope and no accidental same-link/general forwarding;
2. exact RAS producer/consumer/address lineage and invalid no-retry behavior;
3. runtime rename/writeback/link-write ownership and redirect cleanup;
4. CLI assertion strength, direct/hierarchy route evidence, and lifecycle coverage;
5. migration-ledger honesty, score/cap invariants, dead code, and slop.

Each reviewer must return file/line findings ordered by severity. Fix every valid issue with a red test before implementation changes, rerun affected tests, and request a final PASS.

- [ ] **Step 6: Commit review hardening separately**

When review finds an exact-RAS lineage issue, use this focused commit shape:

```text
git add crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs
git commit -m "cpu: harden coroutine RAS lineage"
```

For a finding in another owner, return to that owner's red-test, implementation,
focused-test, and commit steps above. Do not squash unrelated lifecycle fixes
into the feature commits.

- [ ] **Step 7: Push and verify remote identity**

```text
git push origin main
git fetch origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

Expected: local and remote commit IDs are identical and status is clean.

## Completion Checklist

- [ ] Both `x1 -> x5` and `x5 -> x1` coroutine directions execute through bounded detailed O3.
- [ ] Same-window target selection requires the exact latest call push.
- [ ] Recorded traversal distinguishes `Pop` from `PopThenPush` and binds the replacement PC.
- [ ] Wrong kind, wrong address, stale lineage, and discarded lineage fail closed with no retry.
- [ ] Coroutine `return` owns integer rename, source forwarding, one fixed-FU writeback slot, and link-write trace evidence.
- [ ] Direct and hierarchy positives expose exact four-ROB/one-LSQ residency and route activity.
- [ ] Lookahead, overwrite, older repair, wrong-target repair, switch, checkpoint, and timing boundaries pass.
- [ ] Rollback RAS accounting is exactly pushes `3`, pops `3`, squashes `2`.
- [ ] The wrong-target row proves the replacement push through a later ordinary return.
- [ ] CPU remains `74% representative`, `8/10`, and general O3 remains unchecked.
- [ ] Migration ledger remains exactly 1,200 lines.
- [ ] Independent review passes with no dead API, weak assertion, accidental scope widening, or score inflation.
- [ ] Full verification passes and `origin/main` equals `HEAD`.
