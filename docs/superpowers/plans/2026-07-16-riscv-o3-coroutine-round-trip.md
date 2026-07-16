# RISC-V O3 Coroutine Round-Trip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow one adjacent ordinary return to consume a same-window distinct-link coroutine's replacement RAS push inside the existing four-row detailed O3 window.

**Architecture:** Replace the call-only forwardable-link fields with one typed pending RAS-push producer that records whether authority came from a call `Push` or coroutine `PopThenPush`; only the latter may feed one ordinary `Pop`. Share exact producer/consumer validation across live and recorded RAS paths, keep the four-row and lookahead-three caps unchanged, and prove both link directions through focused CPU and real CLI evidence.

**Tech Stack:** Rust workspace, Cargo tests, RISC-V instruction encoders, detailed O3 runtime snapshots, structured `rem6 run --execute` JSON, source-policy ownership checks, migration ledger.

---

## Scope And Invariants

- Approved design: `docs/superpowers/specs/2026-07-16-riscv-o3-coroutine-round-trip-design.md`.
- Ledger target: `### CPU Execution Models - 74% representative` in `docs/architecture/gem5-to-rem6-migration.md`.
- CPU score remains `74% representative`, raw `8/10`; general O3 remains unchecked.
- The resident positive window is exactly `load -> call -> coroutine -> ordinary return`.
- `O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS` remains `4`.
- `MAX_RISCV_BRANCH_LOOKAHEAD` remains `3`.
- Supported chains are only:

```text
JAL x1        -> JALR x5, 0(x1) -> JALR x0, 0(x5)
JALR x5, ... -> JALR x1, 0(x5) -> JALR x0, 0(x1)
```

- Same-link forms, a second linked coroutine consuming the replacement, other link-sourced indirect controls, arbitrary live target forwarding, a scalar descendant plus return, and a fourth control remain open or rejected.
- `temp/` and `temp/reference_designs/gem5` remain read-only and uncommitted.
- No checkpoint schema change is expected or allowed without a focused red test.
- The migration ledger remains exactly 1,200 lines.

## File Ownership

- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: pending RAS-push producer and admission tests.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead.rs`: pass the exact requested RAS consumer into live producer validation.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`: exact call/coroutine producer and consumer validation.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`: recorded round-trip positive and malformed-provenance negatives.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests/coroutine.rs`: four-row runtime ordering and cleanup evidence.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`: ordered includes only; preserve the 500-line root ratchet.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip.rs`: case data, base fixtures, and positives.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_repair.rs`: suppression and repair fixtures/tests.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_lifecycle.rs`: switch, checkpoint, and timing evidence.
- Modify `crates/rem6/tests/source_policy/coroutine_ownership.rs`: fifth include, split exact test ownership, line ratchets.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: eight exact CLI anchors.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: bounded evidence and remaining boundary.
- Do not modify another production runtime file unless Task 3 produces a focused red failure that proves generic runtime ownership is insufficient.

### Task 1: Publish The Coroutine Replacement In Window Policy

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`

- [ ] **Step 1: Replace the terminal policy test with a failing round-trip test**

Replace `admitted_coroutine_does_not_publish_its_replacement_push` with:

```rust
#[test]
fn admitted_coroutine_publishes_replacement_push_to_one_adjacent_return() {
    for (call, coroutine, return_jump) in [
        (
            jal_with_destination(1),
            jalr_with_registers(5, 1),
            jalr_with_registers(0, 5),
        ),
        (
            jalr_with_registers(5, 9),
            jalr_with_registers(1, 5),
            jalr_with_registers(0, 1),
        ),
    ] {
        let mut window = scalar_load_window(4);
        let call = window.classify_sequenced_younger(call, 51);
        assert_eq!(
            call.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        );

        let coroutine = window.classify_sequenced_younger(coroutine, 52);
        assert_eq!(
            coroutine.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        );
        assert_eq!(coroutine.ras_push_sequence(), Some(51));

        let return_jump = window.classify_sequenced_younger(return_jump, 53);
        assert_eq!(
            return_jump.decision(),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
        );
        assert_eq!(return_jump.ras_push_sequence(), Some(52));
        assert!(window.is_full());
    }
}
```

- [ ] **Step 2: Run the policy test red**

Run:

```text
cargo test -p rem6-cpu admitted_coroutine_publishes_replacement_push_to_one_adjacent_return -- --nocapture
```

Expected: compile failure because `forwardable_ras_push` does not exist, or assertion failure because the third return is `AdmitTerminalControl` instead of `AdmitPredictedRasControl`.

- [ ] **Step 3: Replace paired optional fields with one typed producer**

Add above `RiscvScalarIntegerLiveWindow`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ForwardableRasPushKind {
    Call,
    CoroutineReplacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ForwardableRasPush {
    destination: Register,
    sequence: Option<u64>,
    kind: ForwardableRasPushKind,
}
```

Replace:

```rust
forwardable_link_destination: Option<Register>,
forwardable_link_push_sequence: Option<u64>,
```

with:

```rust
forwardable_ras_push: Option<ForwardableRasPush>,
```

Initialize it as `None` in `new`.

Replace the current forwardable-return calculation with:

```rust
let consumer_writes_link = control
    .destination()
    .is_some_and(|destination| !destination.is_zero());
let forwardable_ras_push = (control.kind() == BranchTargetKind::Return
    && control.sources().len() == 1)
    .then(|| self.forwardable_ras_push)
    .flatten()
    .filter(|push| {
        push.destination == control.sources()[0]
            && match push.kind {
                ForwardableRasPushKind::Call => true,
                ForwardableRasPushKind::CoroutineReplacement => !consumer_writes_link,
            }
    });
let forwardable_live_return = forwardable_ras_push.is_some();
```

In the existing terminal dependency branch, clear pending authority before closing the window:

```rust
if depends_on_unresolved || (indirect_target_is_live && !forwardable_live_return) {
    self.forwardable_ras_push = None;
    self.control_closed = true;
    return RiscvSequencedScalarIntegerYoungerDecision {
        decision: RiscvScalarIntegerYoungerDecision::AdmitTerminalControl,
        ras_push_sequence: None,
    };
}
```

Set the reported producer sequence with:

```rust
let ras_push_sequence = forwardable_ras_push.and_then(|push| push.sequence);
```

Use this exact destination transition after dependency checks:

```rust
let destination = control
    .destination()
    .filter(|destination| !destination.is_zero());
if let Some(destination) = destination {
    self.record_shadowing_destination(destination);
}

match control.kind() {
    BranchTargetKind::CallDirect | BranchTargetKind::CallIndirect => {
        if let Some(destination) = destination {
            self.record_forwardable_ras_push(
                destination,
                instruction_sequence,
                ForwardableRasPushKind::Call,
            );
        }
    }
    BranchTargetKind::Return if forwardable_live_return => {
        if let Some(destination) = destination {
            self.record_forwardable_ras_push(
                destination,
                instruction_sequence,
                ForwardableRasPushKind::CoroutineReplacement,
            );
        } else {
            self.forwardable_ras_push = None;
        }
    }
    BranchTargetKind::Return => {
        self.forwardable_ras_push = None;
    }
    _ => {
        self.forwardable_ras_push = None;
    }
}
```

Replace `record_forwardable_link_destination` with:

```rust
fn record_forwardable_ras_push(
    &mut self,
    destination: Register,
    sequence: Option<u64>,
    kind: ForwardableRasPushKind,
) {
    self.forwardable_ras_push = Some(ForwardableRasPush {
        destination,
        sequence,
        kind,
    });
}
```

Change `record_shadowing_destination` to clear the typed producer only when its destination is shadowed:

```rust
if self
    .forwardable_ras_push
    .is_some_and(|push| push.destination == destination)
{
    self.forwardable_ras_push = None;
}
```

Finish the test from Step 1 by changing the loop tuples to carry the expected destination:

```rust
for (call, coroutine, return_jump, expected_destination) in [
    (
        jal_with_destination(1),
        jalr_with_registers(5, 1),
        jalr_with_registers(0, 5),
        5,
    ),
    (
        jalr_with_registers(5, 9),
        jalr_with_registers(1, 5),
        jalr_with_registers(0, 1),
        1,
    ),
] {
```

Assert the typed producer immediately after classifying the coroutine, before classifying the return:

```rust
assert_eq!(
    window.forwardable_ras_push,
    Some(ForwardableRasPush {
        destination: Register::new(expected_destination).unwrap(),
        sequence: Some(52),
        kind: ForwardableRasPushKind::CoroutineReplacement,
    })
);
```

Then classify the return and assert the producer is `None`.

- [ ] **Step 4: Add fail-closed policy negatives**

Add:

```rust
#[test]
fn committed_source_coroutine_does_not_publish_replacement() {
    let mut window = scalar_load_window(4);
    let coroutine = window.classify_sequenced_younger(jalr_with_registers(1, 5), 52);
    assert_eq!(
        coroutine.decision(),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(coroutine.ras_push_sequence(), None);
    let return_jump = window.classify_sequenced_younger(jalr_with_registers(0, 1), 53);
    assert_eq!(
        return_jump.decision(),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );
    assert_eq!(return_jump.ras_push_sequence(), None);
    assert_eq!(window.forwardable_ras_push, None);
}

#[test]
fn intervening_control_clears_coroutine_replacement() {
    let mut window = scalar_load_window(4);
    assert_eq!(
        window
            .classify_sequenced_younger(jal_with_destination(1), 51)
            .decision(),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window
            .classify_sequenced_younger(jalr_with_registers(5, 1), 52)
            .decision(),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    assert_eq!(
        window
            .classify_sequenced_younger(jalr_with_registers(0, 9), 53)
            .decision(),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(window.forwardable_ras_push, None);
}

#[test]
fn coroutine_replacement_rejects_linked_consumer() {
    let mut window = scalar_load_window(4);
    window.classify_sequenced_younger(jal_with_destination(1), 51);
    window.classify_sequenced_younger(jalr_with_registers(5, 1), 52);
    let linked_consumer =
        window.classify_sequenced_younger(jalr_with_registers(1, 5), 53);
    assert_eq!(
        linked_consumer.decision(),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );
    assert_eq!(linked_consumer.ras_push_sequence(), None);
    assert_eq!(window.forwardable_ras_push, None);
}

#[test]
fn ordinary_return_consumes_coroutine_replacement_once() {
    let mut window = scalar_load_window(4);
    window.classify_sequenced_younger(jal_with_destination(1), 51);
    window.classify_sequenced_younger(jalr_with_registers(5, 1), 52);
    let return_jump = window.classify_sequenced_younger(jalr_with_registers(0, 5), 53);
    assert_eq!(
        return_jump.decision(),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    assert_eq!(return_jump.ras_push_sequence(), Some(52));
    assert_eq!(window.forwardable_ras_push, None);
}
```

The committed-source test proves that `BranchTargetKind::Return` plus a nonzero link destination is insufficient: publication requires consuming an exact prior same-window producer. The linked-consumer test prevents an alternating coroutine chain from widening the approved ordinary-return boundary. The intervening-control test proves adjacency and one-shot ownership; it uses the fourth and final row, so no later return is added to that window.

- [ ] **Step 5: Run policy filters green**

Run:

```text
cargo test -p rem6-cpu admitted_coroutine_publishes -- --nocapture
cargo test -p rem6-cpu coroutine_replacement -- --nocapture
cargo test -p rem6-cpu same_window_link_return -- --nocapture
cargo test -p rem6-cpu same_window_coroutine -- --nocapture
```

Expected: all selected policy tests pass; existing same-window call/return and call/coroutine behavior is unchanged.

- [ ] **Step 6: Commit policy authority**

```text
git add crates/rem6-cpu/src/riscv_o3_window_policy.rs
git commit -m "feat: publish coroutine replacement push"
```

### Task 2: Validate Coroutine Producers In Both Frontend Phases

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`

- [ ] **Step 1: Add live and recorded round-trip helpers with failing positives**

Add beside `recorded_same_window_coroutine_core`:

```rust
fn unconsumed_same_window_coroutine_round_trip_core() -> RiscvCore {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let coroutine = i_type(0, 1, 0x0, 5, 0x67);
    let return_jump = i_type(0, 5, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, coroutine.to_le_bytes().to_vec()),
        (3, 0x8008, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(3);

    for expected_pc in [0x800c, 0x8008] {
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(expected_pc));
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&decision).unwrap(),
        );
    }
    core
}

fn recorded_same_window_coroutine_round_trip_core() -> RiscvCore {
    let core = unconsumed_same_window_coroutine_round_trip_core();
    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(return_decision.pc(), Address::new(0x8010));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );
    core
}

fn recorded_same_window_round_trip_target_authority() -> PredictedControlTargetAuthority {
    let call = RiscvInstruction::decode(j_type(8, 1)).unwrap();
    let coroutine = RiscvInstruction::decode(i_type(0, 1, 0x0, 5, 0x67)).unwrap();
    let return_jump = RiscvInstruction::decode(i_type(0, 5, 0x0, 0, 0x67)).unwrap();
    let mut window =
        crate::riscv_o3_window_policy::RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(6).unwrap()],
            1,
            4,
        )
        .unwrap();
    window.classify_sequenced_younger(call, 1);
    window.classify_sequenced_younger(coroutine, 2);
    let return_classification = window.classify_sequenced_younger(return_jump, 3);
    assert_eq!(
        return_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    assert_eq!(return_classification.ras_push_sequence(), Some(2));
    predicted_control_target_authority(
        return_jump,
        Address::new(0x800c),
        return_classification,
        &[(1, Address::new(0x8008)), (2, Address::new(0x8010))],
    )
    .unwrap()
}

fn recorded_same_window_round_trip_pc(core: &RiscvCore) -> RecordedPredictedPc {
    let state = core.state.lock().expect("riscv core lock");
    recorded_predicted_pc(
        &state,
        request(3),
        Address::new(0x800c),
        recorded_same_window_round_trip_target_authority(),
    )
}

#[test]
fn detailed_unconsumed_coroutine_round_trip_opens_exact_replacement_pop() {
    let core = unconsumed_same_window_coroutine_round_trip_core();
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8010));
}

#[test]
fn detailed_recorded_coroutine_round_trip_accepts_exact_replacement_pop() {
    let core = recorded_same_window_coroutine_round_trip_core();
    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Ready(Address::new(0x8010))
    );
}
```

- [ ] **Step 2: Run the frontend positive red**

Run:

```text
cargo test -p rem6-cpu detailed_unconsumed_coroutine_round_trip_opens_exact_replacement_pop -- --nocapture
```

Expected: FAIL before the third decision because `unconsumed_ras_required_target` rejects a producer whose branch kind is `Return` and whose RAS operation is `PopThenPush`.

- [ ] **Step 3: Share exact producer validation across live and recorded paths**

Add one private producer predicate, preserving the existing `RasRequired` field names to avoid schema/API churn:

```rust
fn ras_push_producer_matches(
    producer_kind: BranchTargetKind,
    operation: &crate::ReturnAddressStackOperation,
    pushed_address: Address,
    consumer: RequiredRasConsumer,
) -> bool {
    if producer_kind == BranchTargetKind::Return
        && !matches!(consumer, RequiredRasConsumer::Pop)
    {
        return false;
    }
    let mut expected_after = operation.stack_before().to_vec();
    match (producer_kind, operation.kind()) {
        (
            BranchTargetKind::CallDirect | BranchTargetKind::CallIndirect,
            ReturnAddressStackOperationKind::Push,
        ) => {
            if operation.predicted_return().is_some() {
                return false;
            }
            expected_after.push(pushed_address);
        }
        (BranchTargetKind::Return, ReturnAddressStackOperationKind::PopThenPush) => {
            let Some(predicted_return) = expected_after.pop() else {
                return false;
            };
            if operation.predicted_return() != Some(predicted_return) {
                return false;
            }
            expected_after.push(pushed_address);
        }
        _ => return false,
    }
    operation.pushed_address() == Some(pushed_address)
        && operation.stack_after() == expected_after.as_slice()
}
```

Replace `unconsumed_ras_required_target` with:

```rust
pub(super) fn unconsumed_ras_required_target(
    state: &RiscvCoreState,
    push_sequence: u64,
    pushed_address: Address,
    consumer: RequiredRasConsumer,
) -> Option<Address> {
    let producer_kind = *state.branch_speculation_kinds.get(&push_sequence)?;
    let producer_id = state.return_address_stack_operations.get(&push_sequence)?;
    let producer_operation = state.return_address_stack.pending_operations().last()?;
    if producer_operation.id() != *producer_id
        || !ras_push_producer_matches(
            producer_kind,
            producer_operation,
            pushed_address,
            consumer,
        )
        || producer_operation.stack_after() != state.return_address_stack.stack_entries()
        || state.return_address_stack.top() != Some(pushed_address)
    {
        return None;
    }
    Some(pushed_address)
}
```

In `direct_jump_fetch_ahead_target` inside `riscv_fetch_ahead.rs`, preserve the requested consumer instead of discarding it:

```rust
PredictedControlTargetAuthority::RasRequired {
    push_sequence,
    pushed_address,
    consumer,
} => {
    if kind != BranchTargetKind::Return {
        return None;
    }
    Some(detailed_o3::unconsumed_ras_required_target(
        state,
        push_sequence,
        pushed_address,
        consumer,
    )?)
}
```

Replace `recorded_ras_required_target` with:

```rust
fn recorded_ras_required_target(
    state: &RiscvCoreState,
    push_sequence: u64,
    pushed_address: Address,
    consumer: RequiredRasConsumer,
    return_sequence: u64,
) -> Option<Address> {
    let producer_kind = *state.branch_speculation_kinds.get(&push_sequence)?;
    if state.branch_speculation_kinds.get(&return_sequence) != Some(&BranchTargetKind::Return) {
        return None;
    }

    let producer_id = state.return_address_stack_operations.get(&push_sequence)?;
    let consumer_id = state.return_address_stack_operations.get(&return_sequence)?;
    let operations = state.return_address_stack.pending_operations();
    let producer_index = operations
        .iter()
        .position(|operation| operation.id() == *producer_id)?;
    let consumer_index = operations
        .iter()
        .position(|operation| operation.id() == *consumer_id)?;
    if consumer_index != producer_index + 1 {
        return None;
    }
    let producer_operation = &operations[producer_index];
    let consumer_operation = &operations[consumer_index];
    if !ras_push_producer_matches(
        producer_kind,
        producer_operation,
        pushed_address,
        consumer,
    )
        || producer_operation.stack_after() != consumer_operation.stack_before()
    {
        return None;
    }

    let mut expected_after = consumer_operation.stack_before().to_vec();
    let consumed_address = expected_after.pop()?;
    if consumed_address != pushed_address
        || consumer_operation.predicted_return() != Some(consumed_address)
    {
        return None;
    }
    match consumer {
        RequiredRasConsumer::Pop => {
            if consumer_operation.kind() != ReturnAddressStackOperationKind::Pop
                || consumer_operation.pushed_address().is_some()
            {
                return None;
            }
        }
        RequiredRasConsumer::PopThenPush { pushed_address } => {
            if consumer_operation.kind() != ReturnAddressStackOperationKind::PopThenPush
                || consumer_operation.pushed_address() != Some(pushed_address)
            {
                return None;
            }
            expected_after.push(pushed_address);
        }
    }
    if consumer_operation.stack_after() != expected_after {
        return None;
    }
    Some(consumed_address)
}
```

- [ ] **Step 4: Add malformed coroutine-producer negatives**

Add live and recorded fail-closed tests using the two helpers:

```rust
#[test]
fn detailed_unconsumed_coroutine_round_trip_rejects_newer_ras_operation() {
    let core = unconsumed_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_unconsumed_coroutine_round_trip_rejects_linked_consumer() {
    let core = unconsumed_same_window_coroutine_round_trip_core();
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        crate::riscv_fetch_ahead::detailed_o3::unconsumed_ras_required_target(
            &state,
            2,
            Address::new(0x8010),
            crate::riscv_fetch_ahead::detailed_o3::RequiredRasConsumer::PopThenPush {
                pushed_address: Address::new(0x8014),
            },
        ),
        None
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_plain_push_producer() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(3).unwrap();
        state.squash_return_address_stack_speculation(2).unwrap();
        let malformed = state
            .return_address_stack
            .push_speculative(Address::new(0x8010));
        state
            .return_address_stack_operations
            .insert(2, malformed.id());
        let consumer = state.return_address_stack.pop_speculative();
        state
            .return_address_stack_operations
            .insert(3, consumer.id());
    }
    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_wrong_replacement_address() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(3).unwrap();
        state.squash_return_address_stack_speculation(2).unwrap();
        let malformed = state
            .return_address_stack
            .pop_then_push_speculative(Address::new(0x9000));
        state
            .return_address_stack_operations
            .insert(2, malformed.id());
        let consumer = state.return_address_stack.pop_speculative();
        state
            .return_address_stack_operations
            .insert(3, consumer.id());
    }
    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_intervening_ras_operation() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(3).unwrap();
        state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
        let consumer = state.return_address_stack.pop_speculative();
        state
            .return_address_stack_operations
            .insert(3, consumer.id());
    }
    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_stale_producer_stack() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let producer_id = *state.return_address_stack_operations.get(&2).unwrap();
        let snapshot = state.return_address_stack.snapshot();
        let pending_operations = snapshot
            .pending_operations()
            .iter()
            .map(|operation| {
                if operation.id() != producer_id {
                    return operation.clone();
                }
                crate::ReturnAddressStackOperation::from_checkpoint_parts(
                    operation.id(),
                    operation.kind(),
                    operation.pushed_address(),
                    operation.predicted_return(),
                    operation.stack_before().to_vec(),
                    vec![Address::new(0x9000)],
                )
            })
            .collect();
        let malformed_snapshot = crate::ReturnAddressStackSnapshot::from_checkpoint_parts(
            snapshot.config().clone(),
            snapshot.stack_entries().to_vec(),
            snapshot.next_operation(),
            pending_operations,
        );
        state
            .return_address_stack
            .restore(&malformed_snapshot)
            .unwrap();
    }
    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}
```

- [ ] **Step 5: Run frontend filters green**

Run:

```text
cargo test -p rem6-cpu detailed_unconsumed_coroutine_round_trip -- --nocapture
cargo test -p rem6-cpu detailed_recorded_coroutine_round_trip -- --nocapture
cargo test -p rem6-cpu detailed_recorded_coroutine -- --nocapture
cargo test -p rem6-cpu detailed_recorded_same_window_return_requires_live_ras_lineage -- --nocapture
cargo test -p rem6-cpu detailed_invalid_recorded_return_does_not_retry_as_fresh_prediction -- --nocapture
```

Expected: new round-trip positive and negatives pass; existing call-to-coroutine and call-to-return validation remains green.

- [ ] **Step 6: Commit frontend lineage**

```text
git add crates/rem6-cpu/src/riscv_fetch_ahead.rs crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs
git commit -m "feat: validate coroutine replacement lineage"
```

### Task 3: Prove Four-Row Runtime Ordering

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests/coroutine.rs`
- Modify only on a proven red: focused generic runtime owner under `crates/rem6-cpu/src/o3_runtime_*`

- [ ] **Step 1: Add the runtime round-trip test**

Add:

```rust
#[test]
fn same_window_coroutine_round_trip_serializes_three_controls() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    assert!(runtime.set_writeback_width(1));
    let load = scalar_load_event();
    let call = jal_link(1, 8);
    let coroutine = jalr_link(5, 1);
    let return_jump = jalr_return(5);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), call),
                (Address::new(0x800c), coroutine),
                (Address::new(0x8008), return_jump),
            ],
        ),
        3
    );

    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), call)
        .expect("round-trip call candidate");
    let call_sequence = call_candidate.sequence();
    runtime
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
        .unwrap();
    let call_writeback = runtime
        .writeback_reservation(call_sequence)
        .unwrap()
        .admitted_tick();

    let coroutine_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), coroutine)
        .expect("round-trip coroutine candidate");
    let coroutine_sequence = coroutine_candidate.sequence();
    assert_eq!(
        coroutine_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(1), 0x8008)]
    );
    assert_eq!(coroutine_candidate.issue_tick(1), call_writeback);
    runtime
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
        .unwrap();
    let coroutine_writeback = runtime
        .writeback_reservation(coroutine_sequence)
        .unwrap()
        .admitted_tick();

    let return_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), return_jump)
        .expect("round-trip return candidate");
    let return_sequence = return_candidate.sequence();
    assert_eq!(return_candidate.destination(), None);
    assert_eq!(
        return_candidate.producer_sequences(),
        &[coroutine_sequence]
    );
    assert_eq!(
        return_candidate.forwarded_register_writes(),
        &[RegisterWrite::new(reg(5), 0x8010)]
    );
    assert_eq!(return_candidate.issue_tick(1), coroutine_writeback);
    runtime
        .record_live_speculative_execution(
            return_candidate,
            &[request(13)],
            1,
            RiscvExecutionRecord::new(
                return_jump,
                0x8008,
                0x8010,
                Vec::new(),
                None,
            ),
        )
        .unwrap();

    let return_issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == return_sequence)
        .unwrap();
    assert_eq!(return_issued.issue_tick, coroutine_writeback);
    assert_eq!(return_issued.raw_ready_tick, coroutine_writeback);
    assert_eq!(return_issued.writeback_slot, None);
    assert!(runtime.writeback_reservation(return_sequence).is_none());
    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x800c, 0x8008].map(Address::new)
    );
    assert!(runtime.has_live_control_descendants(coroutine_sequence));

    runtime.discard_live_control_descendants_from_at(
        coroutine_sequence,
        coroutine_writeback,
    );
    assert_eq!(
        runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.pc())
            .collect::<Vec<_>>(),
        [0x8000, 0x8004, 0x800c].map(Address::new)
    );
    assert!(!runtime.has_live_control_descendants(coroutine_sequence));
}
```

- [ ] **Step 2: Run the runtime test**

Run:

```text
cargo test -p rem6-cpu same_window_coroutine_round_trip_serializes_three_controls -- --nocapture
```

Expected after Tasks 1-2: PASS using existing generic runtime ownership. If it fails, preserve the exact failure and modify only the owner responsible for the failed assertion. Do not add a coroutine-specific runtime branch when generic control dependency or destination handling can express the behavior.

- [ ] **Step 3: Run adjacent runtime tests**

Run:

```text
cargo test -p rem6-cpu o3_runtime_control_window_tests::coroutine -- --nocapture
cargo test -p rem6-cpu scoped_issue_orders_same_window_call_coroutine_and_descendant -- --nocapture
```

Expected: all coroutine runtime tests pass; call/coroutine/scalar-descendant behavior remains unchanged.

- [ ] **Step 4: Commit runtime evidence**

```text
git add crates/rem6-cpu/src/o3_runtime_control_window_tests/coroutine.rs
git commit -m "test: prove coroutine round-trip ordering"
```

If a production runtime owner was required by a focused red test, include that exact file in the same commit and describe the proven generic defect in the commit body.

### Task 4: Add Focused CLI Ownership, Fixtures, And Positives

**Files:**
- Modify: `crates/rem6/tests/source_policy/coroutine_ownership.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip.rs`

- [ ] **Step 1: Extend the ownership policy red**

Change the include array to:

```rust
const COROUTINE_INCLUDES: [&str; 4] = [
    "coroutine/suppression.rs",
    "coroutine/repair.rs",
    "coroutine/lifecycle.rs",
    "coroutine/round_trip.rs",
];
```

Add a fourth `CoroutineConcern` with the two positive names initially:

```rust
CoroutineConcern {
    relative: "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip.rs",
    anchors: &[
        "rem6_run_o3_same_window_coroutine_round_trip_commits_direct",
        "rem6_run_o3_same_window_indirect_coroutine_round_trip_commits_cache_fabric_dram",
    ],
},
```

Update the array length to four.

- [ ] **Step 2: Run the ownership policy red**

Run:

```text
cargo test -p rem6 --test source_policy coroutine_cli_evidence_uses_focused_same_namespace_includes -- --nocapture
```

Expected: FAIL because the include and child file do not exist.

- [ ] **Step 3: Create focused round-trip case data and exact fixtures**

Create `coroutine/round_trip.rs` with:

```rust
#[derive(Clone, Copy)]
struct CoroutineRoundTripCase {
    label: &'static str,
    binary: fn(&str, usize) -> std::path::PathBuf,
    memory_system: &'static str,
    max_tick: u64,
    load_pc: &'static str,
    call_pc: &'static str,
    coroutine_pc: &'static str,
    return_pc: &'static str,
    success_store_pc: &'static str,
    call_kind: &'static str,
    call_destination: u8,
    coroutine_destination: u8,
    final_x1: u64,
    final_x5: u64,
    memory_hex: &'static str,
    provider_no_target: u64,
    provider_indirect: u64,
}

const COROUTINE_ROUND_TRIP_CASES: [CoroutineRoundTripCase; 2] = [
    CoroutineRoundTripCase {
        label: "forward-direct",
        binary: direct_coroutine_round_trip_binary,
        memory_system: "direct",
        max_tick: 2_500,
        load_pc: "0x80000010",
        call_pc: "0x80000014",
        coroutine_pc: "0x80000020",
        return_pc: "0x80000018",
        success_store_pc: "0x80000024",
        call_kind: "call_direct",
        call_destination: 1,
        coroutine_destination: 5,
        final_x1: 0x8000_0018,
        final_x5: 0x8000_0024,
        memory_hex: "2a000000240000800000000000000000",
        provider_no_target: 1,
        provider_indirect: 0,
    },
    CoroutineRoundTripCase {
        label: "reverse-indirect",
        binary: indirect_coroutine_round_trip_binary,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
        load_pc: "0x80000018",
        call_pc: "0x8000001c",
        coroutine_pc: "0x8000002c",
        return_pc: "0x80000020",
        success_store_pc: "0x80000030",
        call_kind: "call_indirect",
        call_destination: 5,
        coroutine_destination: 1,
        final_x1: 0x8000_0030,
        final_x5: 0x8000_0020,
        memory_hex: "2a000000300000800000000000000000",
        provider_no_target: 0,
        provider_indirect: 1,
    },
];
```

Add the direct fixture:

```rust
fn direct_coroutine_round_trip_binary(
    name: &str,
    exit_padding_words: usize,
) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        j_type(12, 1),
        i_type(0, 5, 0x0, 0, 0x67),
        s_type(8, 7, 18, 0b010),
        i_type(0, 1, 0x0, 5, 0x67),
        s_type(4, 5, 18, 0b010),
    ]);
    assert_eq!(words.len() * 4, 0x28);
    words.extend(std::iter::repeat_n(
        i_type(0, 0, 0x0, 0, 0x13),
        exit_padding_words,
    ));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}
```

Add the reverse fixture:

```rust
fn indirect_coroutine_round_trip_binary(
    name: &str,
    exit_padding_words: usize,
) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
    ]);
    let target_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0x2c - target_auipc_pc, 11, 0x0, 11, 0x13),
        i_type(0, 18, 0b010, 12, 0x03),
        i_type(0, 11, 0x0, 5, 0x67),
        i_type(0, 1, 0x0, 0, 0x67),
        s_type(8, 7, 18, 0b010),
        m5op(M5_FAIL),
        i_type(0, 5, 0x0, 1, 0x67),
        s_type(4, 1, 18, 0b010),
    ]);
    assert_eq!(words.len() * 4, 0x34);
    words.extend(std::iter::repeat_n(
        i_type(0, 0, 0x0, 0, 0x13),
        exit_padding_words,
    ));
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}
```

Add after the existing includes in `coroutine.rs`:

```rust
include!("coroutine/round_trip.rs");
```

- [ ] **Step 4: Append the positive CLI helper and tests**

Append to `coroutine/round_trip.rs`:

```rust
fn assert_coroutine_round_trip_positive(case: CoroutineRoundTripCase) {
    let path = (case.binary)(
        &format!("o3-coroutine-round-trip-{}", case.label),
        0,
    );
    let completed = run_coroutine_json(
        &path,
        case.memory_system,
        case.max_tick,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );
    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), case.final_x1);
    assert_eq!(register_value(&completed, "x5"), case.final_x5);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(case.memory_hex)
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let load = event_at_pc(&completed, case.load_pc);
    let call = event_at_pc(&completed, case.call_pc);
    let coroutine = event_at_pc(&completed, case.coroutine_pc);
    let return_jump = event_at_pc(&completed, case.return_pc);
    let success_store = event_at_pc(&completed, case.success_store_pc);
    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert_branch_kind_and_link(call, case.call_kind, true);
    assert_branch_kind_and_link(coroutine, "return", true);
    assert_branch_kind_and_link(return_jump, "return", false);
    for event in [call, coroutine, return_jump] {
        assert!(event_u64(event, "issue_tick") < response_tick, "{event}");
    }
    assert!(event_u64(coroutine, "issue_tick") > event_u64(call, "writeback_tick"));
    assert!(
        event_u64(return_jump, "issue_tick") > event_u64(coroutine, "writeback_tick")
    );
    assert_ordered_commits([load, call, coroutine, return_jump, success_store]);
    assert_eq!(
        completed
            .pointer("/cores/0/o3_runtime/writeback_port/admitted_rows")
            .and_then(Value::as_u64),
        Some(3)
    );

    let live_tick = event_u64(return_jump, "issue_tick") + 1;
    assert!(live_tick < response_tick);
    let resident = run_coroutine_json(
        &path,
        case.memory_system,
        live_tick,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );
    assert_eq!(
        resident_rob_pcs(&resident),
        [case.load_pc, case.call_pc, case.coroutine_pc, case.return_pc]
    );
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_register_absent_or_zero(&resident, "x1");
    assert_register_absent_or_zero(&resident, "x5");
    assert_integer_rename_maps_to_row_destination(
        &resident,
        case.call_pc,
        u64::from(case.call_destination),
    );
    assert_integer_rename_maps_to_row_destination(
        &resident,
        case.coroutine_pc,
        u64::from(case.coroutine_destination),
    );
    let resident_return = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.pointer("/pc").and_then(Value::as_str) == Some(case.return_pc)
            })
        })
        .unwrap_or_else(|| {
            panic!(
                "{}: missing resident ordinary-return row {}: {resident}",
                case.label, case.return_pc
            )
        });
    assert!(
        resident_return
            .pointer("/destination")
            .is_some_and(Value::is_null),
        "{}: ordinary return must not own an integer destination: {resident_return}",
        case.label
    );

    for (pointer, expected) in [
        ("/cores/0/branch_predictor/target_provider/no_target", case.provider_no_target),
        ("/cores/0/branch_predictor/target_provider/indirect", case.provider_indirect),
        ("/cores/0/branch_predictor/target_provider/ras", 2),
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 2),
        ("/cores/0/branch_predictor/ras/used", 2),
        ("/cores/0/branch_predictor/ras/correct", 2),
        ("/cores/0/branch_predictor/ras/incorrect", 0),
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "{}: expected {pointer}={expected}: {completed}",
            case.label
        );
    }

    if case.memory_system == "direct" {
        assert_direct_memory_activity(&resident);
    } else {
        let response_resident = run_coroutine_json(
            &path,
            case.memory_system,
            response_tick,
            "detailed",
            3,
            &DIRECT_WIDTH_ARGS,
        );
        assert_no_data_address(&response_resident, SUCCESS_STORE_ADDRESS);
        assert_hierarchy_activity(&response_resident);
    }
}

#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_commits_direct() {
    assert_coroutine_round_trip_positive(COROUTINE_ROUND_TRIP_CASES[0]);
}

#[test]
fn rem6_run_o3_same_window_indirect_coroutine_round_trip_commits_cache_fabric_dram() {
    assert_coroutine_round_trip_positive(COROUTINE_ROUND_TRIP_CASES[1]);
}
```

- [ ] **Step 5: Run ownership and positives**

Run:

```text
cargo test -p rem6 --test source_policy coroutine_cli_evidence_uses_focused_same_namespace_includes -- --nocapture
cargo test -p rem6 --test cli_run coroutine_round_trip_commits -- --nocapture
```

Expected: ownership passes; both real CLI positives pass with four ROB rows, one LSQ row, three writeback rows, and exact RAS totals.

- [ ] **Step 6: Commit CLI positives**

```text
git add crates/rem6/tests/source_policy/coroutine_ownership.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip.rs
git commit -m "test: prove coroutine round-trip positives"
```

### Task 5: Add Suppression And Repair Rows

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_repair.rs`
- Modify: `crates/rem6/tests/source_policy/coroutine_ownership.rs`
- Modify: `docs/superpowers/specs/2026-07-16-riscv-o3-coroutine-round-trip-design.md`
- Modify: `docs/superpowers/plans/2026-07-16-riscv-o3-coroutine-round-trip.md`

- [ ] **Step 1: Add the lookahead-two suppression row**

Append the focused repair include in `coroutine.rs`:

```rust
include!("coroutine/round_trip_repair.rs");
```

Change `COROUTINE_INCLUDES` to length five by appending
`"coroutine/round_trip_repair.rs"`. Change `COROUTINE_CONCERNS` to length
five and append:

```rust
CoroutineConcern {
    relative: "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_repair.rs",
    anchors: &[
        "rem6_run_o3_same_window_coroutine_round_trip_requires_branch_lookahead_three",
        "rem6_run_o3_same_window_coroutine_round_trip_middle_repair_discards_return",
        "rem6_run_o3_same_window_coroutine_round_trip_terminal_return_repairs_direction",
    ],
},
```

Create `coroutine/round_trip_repair.rs` and add:

```rust
#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_requires_branch_lookahead_three() {
    for case in COROUTINE_ROUND_TRIP_CASES {
        let path = (case.binary)(
            &format!("o3-coroutine-round-trip-lookahead-two-{}", case.label),
            0,
        );
        let completed = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            2,
            &DIRECT_WIDTH_ARGS,
        );
        assert_stopped_by_host(&completed);
        assert_eq!(register_value(&completed, "x1"), case.final_x1);
        assert_eq!(register_value(&completed, "x5"), case.final_x5);
        assert_eq!(
            completed.pointer("/memory/0/hex").and_then(Value::as_str),
            Some(case.memory_hex)
        );

        let load = event_at_pc(&completed, case.load_pc);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        let live_tick = response_tick - 1;
        let resident = run_coroutine_json(
            &path,
            case.memory_system,
            live_tick,
            "detailed",
            2,
            &DIRECT_WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [case.load_pc, case.call_pc, case.coroutine_pc],
            "{}: unexpected lookahead-two resident ROB: {resident}",
            case.label
        );
        assert_round_trip_fetch_count(&resident, case.label, case.return_pc, 1);
        assert_round_trip_fetch_count(&resident, case.label, case.success_store_pc, 0);
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1),
            "{}: lookahead two must retain one LSQ row: {resident}",
            case.label
        );
        assert_eq!(
            resident
                .pointer("/cores/0/branch_predictor/lookups/return")
                .and_then(Value::as_u64),
            Some(1),
            "{}: lookahead two must record only the coroutine return lookup: {resident}",
            case.label
        );
        assert_eq!(
            resident
                .pointer("/cores/0/branch_predictor/target_provider/ras")
                .and_then(Value::as_u64),
            Some(1),
            "{}: lookahead two must use one RAS target: {resident}",
            case.label
        );
        for pointer in [
            "/cores/0/branch_predictor/lookups/total",
            "/cores/0/branch_predictor/target_provider/total",
        ] {
            assert_eq!(
                resident.pointer(pointer).and_then(Value::as_u64),
                Some(2),
                "{}: lookahead two must record exactly two control predictions at {pointer}: {resident}",
                case.label
            );
        }
    }
}
```

At `response_tick - 1`, keep the exact three-row ROB and one-row LSQ. The
ordinary-return instruction must appear exactly once in the fetch trace as the
coroutine's predicted target, while the return row remains unadmitted, causes
no second return lookup or RAS use, and does not fetch the success-store target.

- [ ] **Step 2: Add exact middle-repair fixture**

Add to `coroutine/round_trip_repair.rs`:

```rust
fn middle_round_trip_repair_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
    ]);

    assert_eq!((words.len() * 4) as i32, 0x10);
    words.push(i_type(0, 18, 0b010, 12, 0x03));
    assert_eq!((words.len() * 4) as i32, 0x14);
    words.push(j_type(12, 1));
    assert_eq!((words.len() * 4) as i32, 0x18);
    words.push(i_type(0, 5, 0x0, 0, 0x67));
    assert_eq!((words.len() * 4) as i32, 0x1c);
    words.push(s_type(8, 7, 18, 0b010));
    assert_eq!((words.len() * 4) as i32, 0x20);
    words.push(i_type(24, 1, 0x0, 5, 0x67));
    assert_eq!((words.len() * 4) as i32, 0x24);
    words.push(s_type(4, 5, 18, 0b010));
    assert_eq!((words.len() * 4) as i32, 0x28);
    words.push(m5op(M5_EXIT));
    assert_eq!((words.len() * 4) as i32, 0x2c);
    words.push(m5op(M5_FAIL));
    assert_eq!((words.len() * 4) as i32, 0x30);
    words.push(i_type(0, 5, 0x0, 13, 0x13));
    assert_eq!((words.len() * 4) as i32, 0x34);
    words.push(i_type(0, 5, 0x0, 0, 0x67));
    assert_eq!((words.len() * 4) as i32, 0x38);
    words.push(m5op(M5_FAIL));

    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}
```

The first ordinary return at `0x18` is admitted through the coroutine's predicted target and speculatively opens `0x24`. The coroutine resolves to `0x30`, discarding that return and its target traffic. The repaired descendant at `0x30` reaches the later ordinary return at `0x34`, which consumes the preserved replacement and reaches the same `0x24` store on the committed path.

- [ ] **Step 3: Add the middle-repair test**

Add:

```rust
#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_middle_repair_discards_return() {
    let path = middle_round_trip_repair_binary(
        "o3-coroutine-round-trip-middle-repair",
    );
    let completed = run_coroutine_json(
        &path,
        "direct",
        3_000,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );
    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), 0x8000_0018);
    assert_eq!(register_value(&completed, "x5"), 0x8000_0024);
    assert_eq!(register_value(&completed, "x13"), 0x8000_0024);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000240000800000000000000000")
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let call = event_at_pc(&completed, "0x80000014");
    let coroutine = event_at_pc(&completed, "0x80000020");
    let witness = event_at_pc(&completed, "0x80000024");
    let repaired = event_at_pc(&completed, "0x80000030");
    let later_return = event_at_pc(&completed, "0x80000034");
    assert_branch_kind_and_link(call, "call_direct", true);
    assert_branch_kind_and_link(coroutine, "return", true);
    assert_branch_kind_and_link(later_return, "return", false);
    for (pointer, expected) in [
        ("/branch_predicted_target", "0x80000018"),
        ("/branch_resolved_target", "0x80000030"),
        ("/branch_squashed_target", "0x80000018"),
    ] {
        assert_eq!(
            coroutine.pointer(pointer).and_then(Value::as_str),
            Some(expected)
        );
    }
    let speculative_return_fetches =
        round_trip_fetches_at_pc(&completed, "middle round-trip repair", "0x80000018");
    assert_eq!(speculative_return_fetches.len(), 1);
    let target_fetches =
        round_trip_fetches_at_pc(&completed, "middle round-trip repair", "0x80000024");
    assert_eq!(target_fetches.len(), 2);
    assert!(
        event_u64(speculative_return_fetches[0], "tick")
            < event_u64(target_fetches[0], "tick")
            && event_u64(target_fetches[0], "tick") < event_u64(coroutine, "issue_tick")
    );
    assert_eq!(
        event_u64(target_fetches[1], "tick"),
        event_u64(later_return, "issue_tick")
    );
    assert!(event_at_pc_if_present(&completed, "0x80000018").is_none());
    assert!(event_u64(repaired, "issue_tick") > event_u64(coroutine, "commit_tick"));
    assert!(event_u64(later_return, "issue_tick") > event_u64(repaired, "writeback_tick"));
    assert!(event_u64(witness, "issue_tick") > event_u64(later_return, "issue_tick"));
    assert_ordered_commits([call, coroutine, repaired, later_return, witness]);

    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/call_direct", 1),
        ("/cores/0/branch_predictor/lookups/return", 3),
        ("/cores/0/branch_predictor/committed/call_direct", 1),
        ("/cores/0/branch_predictor/committed/return", 2),
        ("/cores/0/branch_predictor/squashes/return", 1),
        ("/cores/0/branch_predictor/target_provider/ras", 3),
        ("/cores/0/branch_predictor/ras/pushes", 3),
        ("/cores/0/branch_predictor/ras/pops", 3),
        ("/cores/0/branch_predictor/ras/squashes", 1),
        ("/cores/0/branch_predictor/ras/used", 2),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 1),
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "unexpected middle-repair counter {pointer}: {completed}"
        );
    }
}
```

If the exact inverse-operation counters differ, first prove the operation sequence from the RAS trace/state and then lock the observed exact balanced totals. Do not weaken them to lower bounds.

- [ ] **Step 4: Add terminal ordinary-return direction-only fixture and test**

Add to `coroutine/round_trip_repair.rs`:

```rust
fn terminal_round_trip_direction_only_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let data_auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 18, 0x17),
        i_type(DATA_START - data_auipc_pc, 18, 0x0, 18, 0x13),
        i_type(99, 0, 0x0, 7, 0x13),
    ]);

    assert_eq!((words.len() * 4) as i32, 0x10);
    words.push(i_type(0, 18, 0b010, 12, 0x03));
    assert_eq!((words.len() * 4) as i32, 0x14);
    words.push(j_type(12, 1));
    assert_eq!((words.len() * 4) as i32, 0x18);
    words.push(i_type(8, 5, 0x0, 0, 0x67));
    assert_eq!((words.len() * 4) as i32, 0x1c);
    words.push(s_type(8, 7, 18, 0b010));
    assert_eq!((words.len() * 4) as i32, 0x20);
    words.push(i_type(0, 1, 0x0, 5, 0x67));
    assert_eq!((words.len() * 4) as i32, 0x24);
    words.push(s_type(8, 7, 18, 0b010));
    assert_eq!((words.len() * 4) as i32, 0x28);
    words.push(m5op(M5_FAIL));
    assert_eq!((words.len() * 4) as i32, 0x2c);
    words.push(s_type(4, 5, 18, 0b010));
    assert_eq!((words.len() * 4) as i32, 0x30);
    words.push(m5op(M5_EXIT));
    assert_eq!((words.len() * 4) as i32, 0x34);
    words.push(m5op(M5_FAIL));

    finish_control_window_binary(name, words, DATA_START as usize, [42, 0, 0, 0])
}
```

The frontend RAS lookup still predicts replacement address `0x24` and records
one incorrect use. The terminal nonzero-offset ordinary return intentionally
retains basic-update O3 trace semantics: predicted target is null, predicted
taken is false, resolved target is `0x2c`, squashed fallthrough is `0x1c`, and
repair is direction-only. The stores at `0x24` and `0x1c` both target
`WRONG_STORE_ADDRESS` and must be squashed before data access.

Add:

```rust
#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_terminal_return_repairs_direction() {
    let path = terminal_round_trip_direction_only_binary(
        "o3-coroutine-round-trip-terminal-return-direction-only",
    );
    let completed = run_coroutine_json(
        &path,
        "direct",
        3_000,
        "detailed",
        3,
        &DIRECT_WIDTH_ARGS,
    );
    assert_stopped_by_host(&completed);
    assert_eq!(register_value(&completed, "x1"), 0x8000_0018);
    assert_eq!(register_value(&completed, "x5"), 0x8000_0024);
    assert_eq!(
        completed.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000240000800000000000000000")
    );
    assert_no_data_address(&completed, WRONG_STORE_ADDRESS);

    let return_jump = event_at_pc(&completed, "0x80000018");
    assert_branch_kind_and_link(return_jump, "return", false);
    assert!(
        return_jump
            .pointer("/branch_predicted_target")
            .is_some_and(Value::is_null)
    );
    for (pointer, expected) in [
        ("/branch_resolved_target", "0x8000002c"),
        ("/branch_squashed_target", "0x8000001c"),
        ("/branch_repair", "direction_only"),
    ] {
        assert_eq!(
            return_jump.pointer(pointer).and_then(Value::as_str),
            Some(expected)
        );
    }
    for (field, expected) in [
        ("branch_predicted_taken", false),
        ("branch_resolved_taken", true),
        ("branch_wrong_target", false),
        ("branch_mispredicted", true),
        ("branch_squash", true),
    ] {
        assert_eq!(
            return_jump
                .pointer(&format!("/{field}"))
                .and_then(Value::as_bool),
            Some(expected)
        );
    }
    let return_fetches = round_trip_fetches_at_pc(
        &completed,
        "terminal round-trip direction-only repair",
        "0x80000018",
    );
    assert_eq!(return_fetches.len(), 1);
    let target_fetches = round_trip_fetches_at_pc(
        &completed,
        "terminal round-trip direction-only repair",
        "0x80000024",
    );
    assert_eq!(target_fetches.len(), 1);
    assert!(
        event_u64(return_fetches[0], "tick") < event_u64(target_fetches[0], "tick")
            && event_u64(target_fetches[0], "tick") < event_u64(return_jump, "issue_tick")
    );
    for (pointer, expected) in [
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 2),
        ("/cores/0/branch_predictor/ras/squashes", 0),
        ("/cores/0/branch_predictor/ras/used", 2),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 1),
        ("/cores/0/branch_predictor/target_provider/ras", 2),
        (
            "/cores/0/o3_runtime/branch_repair/direction_only_mismatches",
            2,
        ),
        (
            "/cores/0/o3_runtime/branch_repair/direction_only_kind/return",
            1,
        ),
        ("/cores/0/o3_runtime/branch_repair/wrong_targets", 0),
    ] {
        assert_eq!(
            completed.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "unexpected terminal-repair counter {pointer}: {completed}"
        );
    }
}
```

- [ ] **Step 5: Run suppression and repair filters**

Run:

```text
cargo test -p rem6 --test cli_run coroutine_round_trip_requires -- --nocapture
cargo test -p rem6 --test cli_run coroutine_round_trip_middle_repair -- --nocapture
cargo test -p rem6 --test cli_run coroutine_round_trip_terminal_return -- --nocapture
```

Expected: all three real CLI rows pass with exact lookahead suppression,
middle wrong-target repair, terminal direction-only repair, RAS, and wrong-path
suppression evidence.

- [ ] **Step 6: Commit suppression and repair**

```text
git add \
  crates/rem6/tests/source_policy/coroutine_ownership.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_repair.rs \
  docs/superpowers/specs/2026-07-16-riscv-o3-coroutine-round-trip-design.md \
  docs/superpowers/plans/2026-07-16-riscv-o3-coroutine-round-trip.md
git commit -m "test: cover coroutine round-trip repair"
```

### Task 6: Add Switch, Checkpoint, And Timing Lifecycle

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_lifecycle.rs`
- Modify: `crates/rem6/tests/source_policy/coroutine_ownership.rs`

- [ ] **Step 1: Add lifecycle ownership and a shared final-state helper**

Append in `coroutine.rs`:

```rust
include!("coroutine/round_trip_lifecycle.rs");
```

Change the ownership include array to:

```rust
const COROUTINE_INCLUDES: [&str; 6] = [
    "coroutine/suppression.rs",
    "coroutine/repair.rs",
    "coroutine/lifecycle.rs",
    "coroutine/round_trip.rs",
    "coroutine/round_trip_repair.rs",
    "coroutine/round_trip_lifecycle.rs",
];
```

Change `COROUTINE_CONCERNS` to length six and append this concern after the
three-anchor `round_trip_repair.rs` concern:

```rust
CoroutineConcern {
    relative: "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_lifecycle.rs",
    anchors: &[
        "rem6_run_host_switch_transfers_o3_same_window_coroutine_round_trip",
        "rem6_run_o3_same_window_coroutine_round_trip_checkpoint_boundary",
        "rem6_run_timing_suppresses_o3_same_window_coroutine_round_trip",
    ],
},
```

Create `coroutine/round_trip_lifecycle.rs` with:

```rust
fn assert_coroutine_round_trip_final_state(
    json: &Value,
    case: CoroutineRoundTripCase,
    context: &str,
) {
    for (register, expected) in [("x1", case.final_x1), ("x5", case.final_x5)] {
        assert_eq!(
            register_value(json, register),
            expected,
            "{}: unexpected {context} {register}: {json}",
            case.label
        );
    }
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(case.memory_hex),
        "{}: unexpected {context} memory: {json}",
        case.label
    );
    assert_no_data_address(json, WRONG_STORE_ADDRESS);
}
```

- [ ] **Step 2: Add the two-direction mode-switch test**

Add:

```rust
#[test]
fn rem6_run_host_switch_transfers_o3_same_window_coroutine_round_trip() {
    for case in COROUTINE_ROUND_TRIP_CASES {
        let path = (case.binary)(
            &format!("o3-coroutine-round-trip-switch-{}", case.label),
            0,
        );
        let baseline = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &DIRECT_WIDTH_ARGS,
        );
        let load = event_at_pc(&baseline, case.load_pc);
        let return_jump = event_at_pc(&baseline, case.return_pc);
        let switch_tick = event_u64(return_jump, "issue_tick") + 1;
        assert!(
            switch_tick < event_u64(load, "lsq_data_response_tick"),
            "{}: round-trip switch tick must precede load response: load={load}, switch_tick={switch_tick}",
            case.label
        );

        let resident = run_coroutine_json(
            &path,
            case.memory_system,
            switch_tick,
            "detailed",
            3,
            &DIRECT_WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [case.load_pc, case.call_pc, case.coroutine_pc, case.return_pc],
            "{}: unexpected resident round-trip ROB: {resident}",
            case.label
        );
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1),
            "{}: expected one resident LSQ row: {resident}",
            case.label
        );
        for register in ["x1", "x5"] {
            assert_register_absent_or_zero_with_context(&resident, register, case.label);
        }
        for (row_pc, register) in [
            (case.call_pc, case.call_destination),
            (case.coroutine_pc, case.coroutine_destination),
        ] {
            assert_coroutine_lifecycle_rename_maps_to_row_destination(
                &resident,
                row_pc,
                u64::from(register),
                case.label,
            );
        }
        let resident_return = resident
            .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries.iter().find(|entry| {
                    entry.pointer("/pc").and_then(Value::as_str) == Some(case.return_pc)
                })
            })
            .unwrap_or_else(|| {
                panic!(
                    "{}: missing resident round-trip return {}: {resident}",
                    case.label, case.return_pc
                )
            });
        assert!(resident_return
            .pointer("/destination")
            .is_some_and(Value::is_null));

        let switch_arg = format!("{switch_tick}:cpu0:timing");
        let mut switch_args = DIRECT_WIDTH_ARGS.to_vec();
        switch_args.extend(["--host-switch-cpu-mode", switch_arg.as_str()]);
        let switched = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &switch_args,
        );

        assert_coroutine_lifecycle_stopped_by_host(&switched, case.label);
        assert_coroutine_lifecycle_execution_mode(&switched, "timing", case.label);
        assert_coroutine_round_trip_final_state(&baseline, case, "baseline");
        assert_coroutine_round_trip_final_state(&switched, case, "switched");
        for register in ["x1", "x5"] {
            assert_eq!(
                register_value(&switched, register),
                register_value(&baseline, register),
                "{}: switch must preserve {register}: baseline={baseline}, switched={switched}",
                case.label
            );
        }
        assert_eq!(
            switched.pointer("/memory/0/hex").and_then(Value::as_str),
            baseline.pointer("/memory/0/hex").and_then(Value::as_str),
            "{}: switch must preserve final memory: baseline={baseline}, switched={switched}",
            case.label
        );

        let timing_switch = switched
            .pointer("/host_actions/execution_mode_switches")
            .and_then(Value::as_array)
            .and_then(|switches| {
                switches.iter().find(|switch| {
                    switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                        && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                        && switch.pointer("/previous_mode").and_then(Value::as_str)
                            == Some("detailed")
                })
            })
            .unwrap_or_else(|| {
                panic!("{}: missing round-trip mode switch: {switched}", case.label)
            });
        let transfer = timing_switch
            .pointer("/state_transfer")
            .unwrap_or_else(|| panic!("{}: missing round-trip state transfer", case.label));
        assert_eq!(
            transfer.pointer("/restorable").and_then(Value::as_bool),
            Some(false),
            "{}: live round-trip transfer must not be restorable: {transfer}",
            case.label
        );
        let runtime = transfer_o3_runtime_chunk_with_context(transfer, "cpu0", case.label);
        assert_eq!(
            runtime
                .pointer("/snapshot_rob_entries")
                .and_then(Value::as_u64),
            Some(4),
            "{}: unexpected transferred ROB snapshot: {runtime}",
            case.label
        );
        assert_eq!(
            runtime
                .pointer("/snapshot_lsq_entries")
                .and_then(Value::as_u64),
            Some(1),
            "{}: unexpected transferred LSQ snapshot: {runtime}",
            case.label
        );
        let handoff =
            transfer_live_data_handoff_chunk_with_context(transfer, "cpu0", case.label);
        for (pointer, expected) in [
            ("/schema_version", 7),
            ("/outstanding_requests", 1),
            ("/resident_rows", 1),
            ("/younger_rows", 3),
            ("/first_target/source_partition", 0),
            ("/first_bytes", 4),
        ] {
            assert_eq!(
                handoff.pointer(pointer).and_then(Value::as_u64),
                Some(expected),
                "{}: unexpected handoff field {pointer}: {handoff}",
                case.label
            );
        }
        for (pointer, expected) in [
            ("/first_operation", "load"),
            ("/first_target/kind", "memory"),
            ("/first_address", DATA_ADDRESS),
        ] {
            assert_eq!(
                handoff.pointer(pointer).and_then(Value::as_str),
                Some(expected),
                "{}: unexpected handoff field {pointer}: {handoff}",
                case.label
            );
        }
        for pc in [case.load_pc, case.call_pc, case.coroutine_pc, case.return_pc] {
            let expected = event_at_pc(&baseline, pc);
            let actual = event_at_pc(&switched, pc);
            for field in ["issue_tick", "writeback_tick", "commit_tick"] {
                assert_eq!(
                    event_u64(actual, field),
                    event_u64(expected, field),
                    "{}: switch must preserve {field} for {pc}: expected={expected} actual={actual}",
                    case.label
                );
            }
        }

        let opposite_call_kind = match case.call_kind {
            "call_direct" => "call_indirect",
            "call_indirect" => "call_direct",
            other => panic!("{}: unsupported call kind {other}", case.label),
        };
        let predictor_expectations = [
            (format!("/cores/0/branch_predictor/lookups/{}", case.call_kind), 1),
            (format!("/cores/0/branch_predictor/lookups/{opposite_call_kind}"), 0),
            ("/cores/0/branch_predictor/lookups/return".to_owned(), 2),
            (format!("/cores/0/branch_predictor/committed/{}", case.call_kind), 1),
            (format!("/cores/0/branch_predictor/committed/{opposite_call_kind}"), 0),
            ("/cores/0/branch_predictor/committed/return".to_owned(), 2),
            (format!("/cores/0/branch_predictor/squashes/{}", case.call_kind), 0),
            (format!("/cores/0/branch_predictor/squashes/{opposite_call_kind}"), 0),
            ("/cores/0/branch_predictor/squashes/return".to_owned(), 0),
            ("/cores/0/branch_predictor/target_provider/no_target".to_owned(), case.provider_no_target),
            ("/cores/0/branch_predictor/target_provider/indirect".to_owned(), case.provider_indirect),
            ("/cores/0/branch_predictor/target_provider/btb".to_owned(), 0),
            ("/cores/0/branch_predictor/target_provider/ras".to_owned(), 2),
            ("/cores/0/branch_predictor/target_provider/total".to_owned(), 3),
            ("/cores/0/branch_predictor/ras/pushes".to_owned(), 2),
            ("/cores/0/branch_predictor/ras/pops".to_owned(), 2),
            ("/cores/0/branch_predictor/ras/squashes".to_owned(), 0),
            ("/cores/0/branch_predictor/ras/used".to_owned(), 2),
            ("/cores/0/branch_predictor/ras/correct".to_owned(), 2),
            ("/cores/0/branch_predictor/ras/incorrect".to_owned(), 0),
            ("/cores/0/branch_predictor/indirect_hits".to_owned(), case.provider_indirect),
        ];
        for (pointer, expected) in predictor_expectations {
            let baseline_value = baseline.pointer(&pointer).and_then(Value::as_u64);
            assert_eq!(
                baseline_value,
                Some(expected),
                "{}: unexpected baseline counter {pointer}: {baseline}",
                case.label
            );
            assert_eq!(
                switched.pointer(&pointer).and_then(Value::as_u64),
                baseline_value,
                "{}: switch must preserve {pointer}: baseline={baseline}, switched={switched}",
                case.label
            );
        }
        assert_coroutine_lifecycle_runtime_drained(&switched, case.label);
    }
}
```

- [ ] **Step 3: Add the two-direction checkpoint test**

Add:

```rust
#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_checkpoint_boundary() {
    for case in COROUTINE_ROUND_TRIP_CASES {
        let path = (case.binary)(
            &format!("o3-coroutine-round-trip-checkpoint-{}", case.label),
            8,
        );
        let baseline = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &DIRECT_WIDTH_ARGS,
        );
        assert_coroutine_round_trip_final_state(&baseline, case, "baseline");
        let load = event_at_pc(&baseline, case.load_pc);
        let live_tick = event_u64(event_at_pc(&baseline, case.return_pc), "issue_tick") + 1;
        assert!(
            live_tick < event_u64(load, "lsq_data_response_tick"),
            "{}: live checkpoint tick must precede load response: load={load}, live_tick={live_tick}",
            case.label
        );

        let live_arg = format!("{live_tick}:coroutine-round-trip-live");
        let mut live_command = control_window_command(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            DATA_ADDRESS,
            16,
        );
        let mut live_args = DIRECT_WIDTH_ARGS.to_vec();
        live_args.extend(["--host-checkpoint", live_arg.as_str()]);
        live_command.args(live_args.iter().copied());
        let output = live_command.output().unwrap_or_else(|error| {
            panic!("{}: failed to run live checkpoint: {error}", case.label)
        });
        assert!(
            !output.status.success(),
            "{}: live round-trip checkpoint unexpectedly succeeded",
            case.label
        );
        assert!(
            output.stdout.is_empty(),
            "{}: live checkpoint emitted stdout: {}",
            case.label,
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("checkpoint component is not quiescent: cpu0"),
            "{}: live checkpoint should fail closed: {stderr}",
            case.label
        );

        let checkpoint_tick =
            event_u64(event_at_pc(&baseline, case.success_store_pc), "commit_tick") + 1;
        let restore_tick = checkpoint_tick + 1;
        let checkpoint_arg = format!("{checkpoint_tick}:coroutine-round-trip-drained");
        let restore_arg = format!("{restore_tick}:coroutine-round-trip-drained");
        let mut restore_args = DIRECT_WIDTH_ARGS.to_vec();
        restore_args.extend([
            "--host-checkpoint",
            checkpoint_arg.as_str(),
            "--host-restore-checkpoint",
            restore_arg.as_str(),
        ]);
        let restored = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &restore_args,
        );

        assert_coroutine_lifecycle_stopped_by_host(&restored, case.label);
        assert_coroutine_round_trip_final_state(&restored, case, "restored");
        for register in ["x1", "x5"] {
            assert_eq!(
                register_value(&restored, register),
                register_value(&baseline, register),
                "{}: restore must preserve {register}: baseline={baseline}, restored={restored}",
                case.label
            );
        }
        assert_eq!(
            restored.pointer("/memory/0/hex").and_then(Value::as_str),
            baseline.pointer("/memory/0/hex").and_then(Value::as_str),
            "{}: restore must preserve memory: baseline={baseline}, restored={restored}",
            case.label
        );
        assert_eq!(
            restored
                .pointer("/host_actions/checkpoint_count")
                .and_then(Value::as_u64),
            Some(1),
            "{}: expected one checkpoint: {restored}",
            case.label
        );
        assert_eq!(
            restored
                .pointer("/host_actions/checkpoint_restored_count")
                .and_then(Value::as_u64),
            Some(1),
            "{}: expected one restore: {restored}",
            case.label
        );
        let checkpoint = restored
            .pointer("/host_actions/checkpoints/0")
            .unwrap_or_else(|| panic!("{}: missing drained checkpoint", case.label));
        let cpu0 = checkpoint_component_with_context(checkpoint, "cpu0", case.label);
        let chunks = checkpoint_component_chunks_with_context(cpu0, case.label);
        assert!(
            chunks.iter().all(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str)
                    != Some("o3-live-data-handoff")
            }),
            "{}: drained checkpoint retained live handoff: {cpu0}",
            case.label
        );
        let runtime_chunk =
            checkpoint_component_chunk_with_context(chunks, "o3-runtime-state", case.label);
        let runtime = runtime_chunk
            .pointer("/o3_runtime")
            .unwrap_or_else(|| panic!("{}: missing decoded O3 runtime: {cpu0}", case.label));
        assert_eq!(
            runtime
                .pointer("/snapshot_rob_entries")
                .and_then(Value::as_u64),
            Some(0),
            "{}: drained checkpoint retained ROB rows: {runtime}",
            case.label
        );
        assert_eq!(
            runtime
                .pointer("/snapshot_lsq_entries")
                .and_then(Value::as_u64),
            Some(0),
            "{}: drained checkpoint retained LSQ rows: {runtime}",
            case.label
        );
        assert_coroutine_lifecycle_runtime_drained(&restored, case.label);
    }
}
```

- [ ] **Step 4: Add the two-direction timing suppression test**

Add:

```rust
#[test]
fn rem6_run_timing_suppresses_o3_same_window_coroutine_round_trip() {
    for case in COROUTINE_ROUND_TRIP_CASES {
        let path = (case.binary)(
            &format!("o3-coroutine-round-trip-timing-{}", case.label),
            0,
        );
        let timing = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "timing",
            3,
            &[],
        );

        assert_coroutine_lifecycle_stopped_by_host(&timing, case.label);
        assert_coroutine_lifecycle_execution_mode(&timing, "timing", case.label);
        assert_coroutine_round_trip_final_state(&timing, case, "timing");
        assert!(
            timing.pointer("/cores/0/o3_runtime").is_none(),
            "{}: timing mode exposed an O3 runtime: {timing}",
            case.label
        );
        assert!(
            timing
                .pointer("/debug/o3_trace")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "{}: timing mode must keep an empty O3 trace: {timing}",
            case.label
        );
        assert_no_o3_stats_with_context(&timing, case.label);
    }
}
```

- [ ] **Step 5: Run lifecycle and ownership**

Run:

```text
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_o3_same_window_coroutine_round_trip -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_same_window_coroutine_round_trip_checkpoint_boundary -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_same_window_coroutine_round_trip -- --nocapture
cargo test -p rem6 --test source_policy coroutine_cli_evidence_uses_focused_same_namespace_includes -- --nocapture
```

Expected: three named tests pass both directions; ownership confirms two
positive definitions in `round_trip.rs`, three suppression/repair definitions
in `round_trip_repair.rs`, and three lifecycle definitions in
`round_trip_lifecycle.rs`, each exactly once.

- [ ] **Step 6: Commit lifecycle evidence**

```text
git add crates/rem6/tests/source_policy/coroutine_ownership.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/round_trip_lifecycle.rs
git commit -m "test: cover coroutine round-trip lifecycle"
```

### Task 7: Update Anchors And Migration Ledger

**Files:**
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`
- Modify: `docs/superpowers/plans/2026-07-16-riscv-o3-coroutine-round-trip.md`

- [ ] **Step 1: Add exact CLI anchors**

Insert after the existing coroutine anchors:

```text
rem6_run_o3_same_window_coroutine_round_trip_commits_direct
rem6_run_o3_same_window_indirect_coroutine_round_trip_commits_cache_fabric_dram
rem6_run_o3_same_window_coroutine_round_trip_requires_branch_lookahead_three
rem6_run_o3_same_window_coroutine_round_trip_middle_repair_discards_return
rem6_run_o3_same_window_coroutine_round_trip_terminal_return_repairs_direction
rem6_run_host_switch_transfers_o3_same_window_coroutine_round_trip
rem6_run_o3_same_window_coroutine_round_trip_checkpoint_boundary
rem6_run_timing_suppresses_o3_same_window_coroutine_round_trip
```

- [ ] **Step 2: Update only bounded CPU prose and test-map evidence**

State the exact adjacent call `Push` -> distinct-link coroutine `PopThenPush`
-> ordinary return `Pop` evidence matrix without widening any row:

- The forward `x1->x5` positive runs on direct memory.
- The reverse `x5->x1` positive runs on `cache-fabric-dram` hierarchy memory.
- Lookahead-two suppression, mode switch, live/drained checkpoint, and timing
  suppression iterate both cases and directions.
- Middle wrong-target repair and terminal direction-only repair are focused
  direct-only fixtures.

Keep the positive routes and focused repair coverage case-specific; do not
widen either row to the other direction or memory route.

Remove only:

```text
another same-window linked control consuming replacement push
```

Preserve open:

```text
same-link forms
second linked coroutine consuming the replacement
other link-sourced indirect controls outside exact adjacent lineage
producer-forwarded target for a further control descendant
scalar descendant plus later return / fourth-and-deeper chains
arbitrary broader mixed windows
general O3
```

Keep heading `74% representative`, raw `8/10`, and general O3 unchecked.

- [ ] **Step 3: Run policy and ledger gates**

Run:

```text
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: rem6 and CPU source-policy suites pass; ledger is exactly `1200` lines.

- [ ] **Step 4: Commit documentation evidence**

```text
git add crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md docs/superpowers/plans/2026-07-16-riscv-o3-coroutine-round-trip.md
git commit -m "docs: record coroutine round trip"
```

### Task 8: Final Verification, Review, And Push

**Files:**
- Verify all files in this plan.

- [ ] **Step 1: Run focused verification**

```text
cargo test -p rem6-cpu coroutine_round_trip -- --nocapture
cargo test -p rem6-cpu replacement_push -- --nocapture
cargo test -p rem6-cpu coroutine -- --nocapture
cargo test -p rem6 --test cli_run coroutine_round_trip -- --nocapture
cargo test -p rem6 --test cli_run coroutine -- --nocapture
```

- [ ] **Step 2: Run full verification**

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

- [ ] **Step 3: Audit the worktree**

```text
git status --short --branch
git diff -- temp temp/reference_designs/gem5
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

Expected: clean worktree, no temp diff, only scoped commits.

- [ ] **Step 4: Run independent read-only review**

Use separate high-intensity reviewers for:

1. policy one-shot authority and accidental forwarding;
2. exact RAS operation/stack lineage and repair counters;
3. fixture PC/register/memory arithmetic plus runtime timing;
4. lifecycle, ownership, dead-code/slop, anchors, and ledger honesty.

Fix valid findings with a focused red test and a separate commit.

- [ ] **Step 5: Push and prove remote equality**

```text
git push origin main
git fetch origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

Expected: local and remote hashes match; branch is clean and synchronized.

## Completion Criteria

1. Both link directions execute a four-row load/call/coroutine/return round trip.
2. The policy publishes only an exact same-window coroutine replacement, allows it to feed only one ordinary return, and rejects a second linked coroutine.
3. Live and recorded frontend validation accept call `Push` and coroutine `PopThenPush` producers only with exact consumer shape, adjacency, address, and stack state.
4. Runtime evidence proves two link destinations, no ordinary-return destination, serialized issue, three writeback rows, and descendant cleanup.
5. Real CLI positives cover direct and cache/fabric/DRAM routes with exact architecture, memory, ROB/LSQ, predictor, RAS, and resource assertions.
6. Lookahead-two, middle repair, terminal repair, switch, checkpoint, and timing boundaries pass.
7. Same-link, general live target forwarding, five-row chains, older fourth-control wrapping, and fourth/deeper consumers remain rejected or open.
8. Source ownership passes, ledger stays 1,200 lines, CPU remains 74% and raw 8/10, and general O3 stays unchecked.
9. Full workspace verification and four independent reviews pass.
10. Commits are pushed to `origin/main` and local/remote HEADs match.
