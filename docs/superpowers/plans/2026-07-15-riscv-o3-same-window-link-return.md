# RISC-V O3 Same-Window Link Return Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Admit one bounded same-window linked call/return pair and one return-target descendant behind a delayed scalar-memory head, using staged link forwarding and RAS-only target authority with real CLI evidence.

**Architecture:** Track the latest live call-produced `x1`/`x5` provenance in the existing scalar-memory window policy and return a distinct RAS-required predicted-control decision. Carry that authority through detailed fetch selection so a same-window return may use the speculative RAS top but may never fall back to stale committed link state; keep runtime rename, source forwarding, issue, writeback, retire, RAS, repair, mode-transfer, and checkpoint owners unchanged.

**Tech Stack:** Rust workspace, `rem6-cpu`, top-level `rem6 run --execute` integration tests, generated RV64 ELF fixtures, JSON/debug artifacts, source-policy tests, Cargo.

---

## File Map

- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: own forwardable-link provenance and the RAS-required admission decision.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`: carry normal versus RAS-required target authority on detailed predicted-control candidates.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`: pass target authority into the existing fetch-ahead decision owner.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead.rs`: fail closed when a RAS-required return has no RAS target; preserve normal committed-return fallback.
- Modify `crates/rem6-cpu/src/o3_runtime_live_window.rs`: treat the RAS-required decision as a predicted control for row staging and control dependencies.
- Modify `crates/rem6-cpu/src/riscv_live_retire_window.rs`: traverse recorded predicted PCs for the RAS-required decision during completed-fetch replay.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`: replace the terminal same-window return test with positive RAS chaining and missing-RAS suppression tests.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`: prove return candidate dependency and forwarded link ownership.
- Modify `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`: prove call/return branch serialization, admitted-writeback dependency, and descendant ordering.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_return.rs`: own the eight-row direct/hierarchy/suppression/rollback/transfer/checkpoint/timing matrix.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`: register the focused `link_return` module.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs`: own shared linked-control assertions now needed by two modules.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_kind.rs`: consume shared assertions and delete duplicate local helpers.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: add the eight new top-level anchors atomically with the ledger update.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record the executable boundary without changing 74%, 8/10, the unchecked general-O3 item, or the 1,200-line invariant.

Do not modify `o3_source_operands.rs`, `return_address_stack.rs`, `riscv_branch_kind.rs`, `riscv_execute.rs`, checkpoint payload versions, execution-mode handoff schemas, or public configuration/stat APIs. Coroutine forms, same-link indirect calls, and general live-produced indirect targets remain outside this increment.

### Task 1: Add Forwardable-Link Provenance And RAS-Required Fetch Authority

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`

- [ ] **Step 1: Add failing policy tests for call-produced return provenance**

Replace `same_window_call_followed_by_return_is_terminal` with these exact tests in `riscv_o3_window_policy.rs`:

```rust
#[test]
fn same_window_link_return_requires_ras_prediction() {
    for (call, return_jump) in [
        (jal_with_destination(1), jalr_with_registers(0, 1)),
        (jalr_with_registers(5, 9), jalr_with_registers(0, 5)),
    ] {
        let mut window = scalar_load_window(4);

        assert_eq!(
            window.classify_younger(call),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedControl,
            "{call:?}"
        );
        assert_eq!(
            window.classify_younger(return_jump),
            RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl,
            "{return_jump:?}"
        );
        assert_eq!(
            window.classify_younger(addi(8, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert!(window.is_full());
    }
}

#[test]
fn scalar_overwrite_after_call_keeps_same_window_return_terminal() {
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
        window.classify_younger(jalr_with_registers(0, 1)),
        RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
    );
    assert!(window.is_full());
}
```

Keep `frontend_sensitive_indirect_controls_remain_terminal_when_target_is_unresolved`, `frontend_sensitive_indirect_controls_remain_terminal_when_target_is_live`, and `scalar_memory_prefix_rejects_unsupported_link_forms` unchanged. They are the negative boundary.

- [ ] **Step 2: Add failing detailed frontend tests**

Replace `detailed_scalar_window_keeps_same_window_return_terminal_after_direct_call` in `detailed_o3_control.rs` with:

```rust
#[test]
fn detailed_scalar_window_forwards_call_ras_to_same_window_return() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let fallthrough = i_type(1, 0, 0x0, 7, 0x13);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
        (3, 0x8008, fallthrough.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(call_decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision).unwrap(),
    );

    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(return_decision.pc(), Address::new(0x8008));
    let speculation = return_decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x800c));
    assert_eq!(speculation.target(), Some(Address::new(0x8008)));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.return_address_stack.stack_entries().is_empty());
    assert_eq!(state.return_address_stack.pending_operation_count(), 2);
    assert_eq!(state.return_address_stack_operations.len(), 2);
    assert_eq!(
        state
            .branch_speculation_summary
            .target_provider()
            .value(BranchTargetProvider::RAS),
        1
    );
}

#[test]
fn detailed_same_window_return_does_not_fall_back_without_ras() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision).unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.discard_return_address_stack_speculations();
        state.hart.write(Register::new(1).unwrap(), 0x9000);
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
```

- [ ] **Step 3: Run the policy and frontend tests to verify RED**

Run:

```text
cargo test -p rem6-cpu same_window_link_return -- --nocapture
cargo test -p rem6-cpu detailed_scalar_window_forwards_call_ras_to_same_window_return -- --nocapture
```

Expected RED:

1. the policy test does not compile because `AdmitPredictedRasControl` does not exist, or returns `AdmitTerminalControl` before the variant is added;
2. the detailed frontend test returns `None` for the return decision;
3. the missing-RAS test must fail if normal architectural fallback is accidentally used.

- [ ] **Step 4: Add the exact policy provenance field and decision**

Add the field and variant:

```rust
pub(crate) struct RiscvScalarIntegerLiveWindow {
    unresolved_destinations: Vec<Register>,
    live_destinations: Vec<Register>,
    forwardable_link_destinations: Vec<Register>,
    rows: usize,
    row_limit: usize,
    admits_terminal_control: bool,
    control_depth: usize,
    control_closed: bool,
}

pub(crate) enum RiscvScalarIntegerYoungerDecision {
    AdmitContinue,
    AdmitStop,
    AdmitTerminalControl,
    AdmitPredictedControl,
    AdmitPredictedRasControl,
    Reject,
}
```

Initialize `forwardable_link_destinations` to `Vec::new()` in `new`.

Before the current terminal decision, derive:

```rust
let forwardable_live_return = control.kind() == BranchTargetKind::Return
    && control.sources().len() == 1
    && self
        .forwardable_link_destinations
        .contains(&control.sources()[0]);
```

Change the terminal condition to:

```rust
if depends_on_unresolved || (indirect_target_is_live && !forwardable_live_return) {
    self.control_closed = true;
    return RiscvScalarIntegerYoungerDecision::AdmitTerminalControl;
}
```

For a control destination, keep the one shadowing owner and then mark calls:

```rust
if let Some(destination) = control
    .destination()
    .filter(|destination| !destination.is_zero())
{
    self.record_shadowing_destination(destination);
    if matches!(
        control.kind(),
        BranchTargetKind::CallDirect | BranchTargetKind::CallIndirect
    ) {
        self.record_forwardable_link_destination(destination);
    }
}
```

Return the distinct decision after incrementing control depth:

```rust
self.control_depth += 1;
if forwardable_live_return {
    RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
} else {
    RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
}
```

Make every generic destination overwrite clear link provenance:

```rust
fn record_shadowing_destination(&mut self, destination: Register) {
    self.unresolved_destinations
        .retain(|unresolved| *unresolved != destination);
    self.forwardable_link_destinations
        .retain(|link| *link != destination);
    self.record_live_destination(destination);
}

fn record_forwardable_link_destination(&mut self, destination: Register) {
    if !self.forwardable_link_destinations.contains(&destination) {
        self.forwardable_link_destinations.push(destination);
    }
}
```

- [ ] **Step 5: Carry typed target authority through detailed O3 selection**

Add to `detailed_o3.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PredictedControlTargetAuthority {
    Normal,
    RasRequired,
}
```

Add `target_authority: PredictedControlTargetAuthority` to `ReadyPredictedControl`.

In the first scalar-window loop, replace the current `match window.classify_younger(...)` opening with:

```rust
let decision = window.classify_younger(younger.decoded().instruction());
match decision {
```

In the second loop, replace the current `match alu_window.classify_younger(...)` opening with:

```rust
let decision = alu_window.classify_younger(next.decoded().instruction());
match decision {
```

Use this complete predicted arm in the first loop:

```rust
RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
| RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl => {
    let target_authority = if matches!(
        decision,
        RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    ) {
        PredictedControlTargetAuthority::RasRequired
    } else {
        PredictedControlTargetAuthority::Normal
    };
    let prediction_request = younger.first_consumed_request();
    previous_request = younger.last_consumed_request();
    let sequential_pc = Address::new(
        younger
            .pc()
            .get()
            .wrapping_add(u64::from(younger.decoded().bytes())),
    );
    next_pc = match recorded_predicted_pc(state, prediction_request, sequential_pc) {
        Some(predicted_pc) => predicted_pc,
        None if state.branch_speculations.len() < state.branch_lookahead => {
            return DetailedFetchAheadCandidate::ReadyPredictedControl {
                request: prediction_request,
                pc: younger.pc(),
                sequential_pc,
                instruction: younger.decoded().instruction(),
                target_authority,
            };
        }
        None => return DetailedFetchAheadCandidate::Blocked,
    };
    continue;
}
```

Use this complete predicted arm in the second loop:

```rust
RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
| RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl => {
    let target_authority = if matches!(
        decision,
        RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    ) {
        PredictedControlTargetAuthority::RasRequired
    } else {
        PredictedControlTargetAuthority::Normal
    };
    let prediction_request = next.first_consumed_request();
    let previous_request = next.last_consumed_request();
    let sequential_pc = Address::new(
        next.pc()
            .get()
            .wrapping_add(u64::from(next.decoded().bytes())),
    );
    let next_pc = match recorded_predicted_pc(state, prediction_request, sequential_pc) {
        Some(predicted_pc) => predicted_pc,
        None if state.branch_speculations.len() < state.branch_lookahead => {
            return DetailedFetchAheadCandidate::ReadyPredictedControl {
                request: prediction_request,
                pc: next.pc(),
                sequential_pc,
                instruction: next.decoded().instruction(),
                target_authority,
            };
        }
        None => return DetailedFetchAheadCandidate::Blocked,
    };
    return scalar_integer_window_candidate_from(
        state,
        fetch_events,
        previous_request,
        next_pc,
        alu_window,
    );
}
```

Do not change `AdmitTerminalControl`, `AdmitStop`, or `Reject` handling.

- [ ] **Step 6: Route the new decision through staging and replay**

In `o3_runtime_live_window.rs`, include the new variant everywhere predicted controls own a live control row or become the latest control dependency:

```rust
matches!(
    decision,
    RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
)
```

Use that expression in both the `live_control_window_sequences` insertion condition and the `control_sequence = Some(sequence)` condition.

In `riscv_live_retire_window.rs`, add `AdmitPredictedRasControl` beside `AdmitPredictedControl` in `accepted_scalar_integer_younger_window`. In `completed_scalar_integer_younger_window`, replace the equality check with:

```rust
let next_pc = if matches!(
    decision,
    RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
        | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
) {
    let Some(next_pc) = crate::riscv_fetch_ahead::recorded_predicted_pc(
        state,
        prediction_request,
        sequential_pc,
    ) else {
        break;
    };
    next_pc
} else {
    sequential_pc
};
```

Do not treat the new variant as terminal in either file.

- [ ] **Step 7: Enforce RAS-only target selection**

In `driver.rs`, pass the candidate authority to `fetch_ahead_decision`. Pass `PredictedControlTargetAuthority::Normal` from the ordinary completed-fetch path at the second call site.

Change the signatures in `riscv_fetch_ahead.rs`:

```rust
fn fetch_ahead_decision(
    state: &mut RiscvCoreState,
    completed_fetches: &[&CpuFetchEvent],
    request: MemoryRequestId,
    fetch_pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
    target_authority: detailed_o3::PredictedControlTargetAuthority,
    translated: detailed_o3::TranslatedMemoryFetchAhead,
) -> Option<RiscvFetchAheadDecision>
```

```rust
fn direct_jump_fetch_ahead_target(
    state: &mut RiscvCoreState,
    fetch_pc: Address,
    instruction: RiscvInstruction,
    target_authority: detailed_o3::PredictedControlTargetAuthority,
) -> Option<(
    Address,
    BranchTargetKind,
    BranchTargetPrediction,
    BranchTargetProvider,
)>
```

At the existing lookup in `fetch_ahead_decision`, call
`direct_jump_fetch_ahead_target(state, fetch_pc, instruction, target_authority)`;
leave the branch-decision construction below it unchanged.

Implement the fail-closed target choice:

```rust
let ras_required =
    target_authority == detailed_o3::PredictedControlTargetAuthority::RasRequired;
if ras_required && kind != BranchTargetKind::Return {
    return None;
}
let ras_target = (kind == BranchTargetKind::Return)
    .then(|| state.return_address_stack.top())
    .flatten();
if ras_required && ras_target.is_none() {
    return None;
}
let target = match instruction {
    RiscvInstruction::Jal { offset, .. } => {
        checked_add_signed(fetch_pc.get(), offset.value()).map(Address::new)
    }
    RiscvInstruction::Jalr { rs1, offset, .. } => ras_target.or_else(|| {
        if ras_required {
            None
        } else {
            checked_add_signed(state.hart.read(rs1), offset.value())
                .map(|target| Address::new(target & !1))
        }
    }),
    _ => None,
}?;
```

Keep target-provider selection unchanged; a RAS-required success therefore records `BranchTargetProvider::RAS`.

- [ ] **Step 8: Run focused GREEN gates**

Run:

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
cargo test -p rem6-cpu detailed_o3_control -- --nocapture
cargo test -p rem6-cpu riscv_fetch_ahead -- --nocapture
cargo test -p rem6-cpu o3_runtime_live_window -- --nocapture
cargo test -p rem6-cpu riscv_live_retire_window -- --nocapture
cargo test -p rem6-cpu --test source_policy --quiet
```

Expected: all pass. Confirm the policy count increases by two tests and the detailed frontend now records two pending RAS operations.

- [ ] **Step 9: Commit policy and frontend behavior**

```bash
git add crates/rem6-cpu/src/riscv_o3_window_policy.rs crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs crates/rem6-cpu/src/riscv_fetch_ahead.rs crates/rem6-cpu/src/o3_runtime_live_window.rs crates/rem6-cpu/src/riscv_live_retire_window.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs
git commit -m "cpu: classify same-window link returns"
```

### Task 2: Prove Existing Runtime Forwarding, Issue, And Cleanup

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`

No production runtime file is changed in this task. The existing generic staged-rename and source-forwarding path is the behavior under test.

- [ ] **Step 1: Add a focused return-candidate forwarding test**

Add a local helper to `o3_runtime_control_window_tests.rs`:

```rust
fn jalr_return(rs1: u8) -> RiscvInstruction {
    RiscvInstruction::Jalr {
        rd: reg(0),
        rs1: reg(rs1),
        offset: Immediate::new(0),
    }
}
```

Add:

```rust
#[test]
fn same_window_return_candidate_uses_link_call_forwarding() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let call = jal_link(1, 8);
    let return_jump = jalr_return(1);
    let descendant = addi(8, 0, 7);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), call),
                (Address::new(0x800c), return_jump),
                (Address::new(0x8008), descendant),
            ],
        ),
        3
    );

    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), call)
        .expect("linked call candidate");
    let call_sequence = call_candidate.sequence;
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

    let return_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x800c), return_jump)
        .expect("same-window return candidate");
    assert!(return_candidate.producer_sequences.contains(&call_sequence));
    assert_eq!(
        return_candidate.forwarded_register_writes,
        vec![RegisterWrite::new(reg(1), 0x8008)]
    );
}
```

Use the existing `request`, `reg`, `scalar_load_event`, and `jal_link` helpers. The test module can inspect the candidate's private fields through its parent-module scope.

- [ ] **Step 2: Add a dedicated issue fixture case**

Extend `ScalarIssueCase`:

```rust
SameWindowLinkReturn,
```

Map it in `ScalarIssueFixture::new`:

```rust
ScalarIssueCase::SameWindowLinkReturn => [
    jal_link(1),
    jalr_return(1),
    addi(14, 0, 7),
],
```

Add:

```rust
#[test]
fn scoped_issue_orders_same_window_call_return_and_descendant() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::SameWindowLinkReturn);
    assert!(fixture.runtime.set_writeback_width(1));

    fixture.schedule_all(20);

    let call = fixture.execution_at(BRANCH_PC);
    let return_control = fixture.execution_at(SECOND_PC);
    let descendant = fixture.execution_at(THIRD_PC);
    assert_eq!(call.issue_tick, 20);
    assert_eq!(call.admitted_writeback_tick, 20);
    assert_eq!(call.writeback_slot, Some(0));
    assert_eq!(return_control.issue_tick, call.admitted_writeback_tick + 1);
    assert_eq!(return_control.writeback_slot, None);
    assert_eq!(descendant.issue_tick, return_control.admitted_writeback_tick + 1);
    assert!(return_control.producer_sequences.contains(&call.sequence));
    assert!(descendant
        .producer_sequences
        .contains(&return_control.sequence));
}
```

- [ ] **Step 3: Run runtime proof gates**

Run:

```text
cargo test -p rem6-cpu same_window_return_candidate_uses_link_call_forwarding -- --nocapture
cargo test -p rem6-cpu scoped_issue_orders_same_window_call_return_and_descendant -- --nocapture
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue -- --nocapture
```

Expected: all pass without production runtime changes. A failure is an ownership defect; reduce it before changing runtime code and keep any fix in the focused runtime module that owns the missing behavior.

- [ ] **Step 4: Commit runtime proof**

```bash
git add crates/rem6-cpu/src/o3_runtime_control_window_tests.rs crates/rem6-cpu/src/o3_runtime_issue_tests.rs
git commit -m "test: prove same-window link return runtime"
```

### Task 3: Add The Eight-Row Real CLI Matrix

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_return.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_kind.rs`

- [ ] **Step 1: Move shared linked-control assertions to `window_support.rs`**

Move these existing helpers from `link_kind.rs` to `window_support.rs`, make them `pub(super)`, and import them back into `link_kind.rs`:

```rust
pub(super) fn assert_stopped_by_host(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
}

pub(super) fn assert_branch_kind_and_link(event: &Value, kind: &str, link_write: bool) {
    assert_eq!(
        event.pointer("/branch_kind").and_then(Value::as_str),
        Some(kind),
        "unexpected branch kind: {event}"
    );
    assert_eq!(
        event
            .pointer("/branch_link_register_write")
            .and_then(Value::as_bool),
        Some(link_write),
        "unexpected branch link-write flag: {event}"
    );
}

pub(super) fn assert_ordered_commits<const N: usize>(events: [&Value; N]) {
    assert!(events
        .windows(2)
        .all(|events| event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")));
}

pub(super) fn assert_register_absent_or_zero(json: &Value, register: &str) {
    let registers = json
        .pointer("/cores/0/registers")
        .and_then(Value::as_object)
        .unwrap_or_else(|| panic!("missing register object: {json}"));
    match registers.get(register) {
        None => {}
        Some(value) if value.as_str() == Some("0x0") => {}
        Some(value) => {
            panic!("expected {register} to be absent or explicitly zero, got {value}: {json}")
        }
    }
}

pub(super) fn assert_link_rename_maps_to_call_destination(
    json: &Value,
    call_pc: &str,
    register: u64,
) {
    let call_entry = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(call_pc))
        })
        .unwrap_or_else(|| panic!("missing resident linked call row {call_pc}: {json}"));
    let destination = call_entry
        .pointer("/destination")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("linked call row should own a destination: {call_entry}"));
    let rename_entry = json
        .pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.pointer("/register_class").and_then(Value::as_str) == Some("integer")
                    && entry.pointer("/architectural").and_then(Value::as_u64)
                        == Some(register)
            })
        })
        .unwrap_or_else(|| panic!("missing live rename for x{register}: {json}"));
    assert_eq!(
        rename_entry.pointer("/physical").and_then(Value::as_u64),
        Some(destination),
        "x{register} should map to the linked call destination"
    );
}

pub(super) fn assert_pointer_u64_gt(json: &Value, pointer: &str, minimum: u64) {
    assert!(
        json.pointer(pointer)
            .and_then(Value::as_u64)
            .is_some_and(|value| value > minimum),
        "expected {pointer} > {minimum}: {json}"
    );
}

pub(super) fn assert_hierarchy_activity(json: &Value) {
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert_pointer_u64_gt(json, pointer, 0);
    }
}

pub(super) fn assert_final_execution_mode(json: &Value, expected_mode: &str) {
    let execution_modes = json
        .pointer("/host_actions/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing final execution mode: {json}"));
    assert_eq!(execution_modes.len(), 1);
    assert_eq!(
        execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0")
    );
    assert_eq!(
        execution_modes[0].pointer("/mode").and_then(Value::as_str),
        Some(expected_mode)
    );
}

pub(super) fn assert_drained_control_runtime(json: &Value) {
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/rob/count")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(0)
    );
}

pub(super) fn assert_no_o3_stats(json: &Value) {
    let unexpected = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("timing control-window stats")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                    "system.cpu.fetch.",
                    "system.cpu.bac.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "timing mode leaked control-window O3 stats: {unexpected:?}"
    );
}
```

Rename only `assert_drained_link_kind_runtime` to `assert_drained_control_runtime`. Move `assert_register_absent_or_zero` as well. Delete the local copies from `link_kind.rs`. This is a mechanical ownership move; run the existing `link_kind` filter before adding new tests.

- [ ] **Step 2: Register the focused module**

Add to `predicted_control.rs` after `link_kind`:

```rust
#[path = "predicted_control/link_return.rs"]
mod link_return;
```

Do not add source-policy anchors yet. Anchors and ledger prose are committed atomically in Task 4.

- [ ] **Step 3: Create shared constants and runner in `link_return.rs`**

Start the file with:

```rust
use super::window_support::{
    assert_branch_kind_and_link, assert_drained_control_runtime,
    assert_final_execution_mode, assert_hierarchy_activity,
    assert_link_rename_maps_to_call_destination, assert_no_data_address,
    assert_no_fetch_pc, assert_no_o3_stats, assert_ordered_commits,
    assert_pointer_u64_gt, assert_register_absent_or_zero,
    assert_stopped_by_host, control_window_command, resident_rob_pcs,
    run_control_window_json,
};
use super::*;

const DATA_START: i32 = 0x100;
const DATA_ADDRESS: &str = "0x80000100";
const WRONG_STORE_ADDRESS: &str = "0x80000108";
const DIRECT_WIDTH_ARGS: [&str; 4] = [
    "--riscv-o3-issue-width",
    "4",
    "--riscv-o3-writeback-width",
    "1",
];

fn run_link_return_json(
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

fn finish_link_return_binary(
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

- [ ] **Step 4: Build the direct same-window call/return fixture**

Use this exact layout:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c lw x12, 0(x18)
10 jal x1, +12          # target 0x1c, link/return target 0x14
14 addi x13, x0, 42     # return-target descendant
18 jal x0, +16          # exit store at 0x28
1c jalr x0, 0(x1)
20 sw x7, 8(x18)        # wrong return-fallthrough witness
24 m5_fail
28 sw x13, 4(x18)
2c m5_exit
30 m5_fail
```

Initialize data as `[42, 0, 0, 0]`. The live row order is `[load@0c, call@10, return@1c, addi@14]`.

- [ ] **Step 5: Add direct positive assertions**

Implement `rem6_run_o3_same_window_link_return_commits_direct`. Run detailed direct memory with branch lookahead two and `DIRECT_WIDTH_ARGS`.

Assert:

```rust
assert_stopped_by_host(&completed);
assert_eq!(register_value(&completed, "x1"), 0x8000_0014);
assert_eq!(register_value(&completed, "x13"), 42);
assert_eq!(
    completed.pointer("/memory/0/hex").and_then(Value::as_str),
    Some("2a0000002a0000000000000000000000")
);
assert_no_data_address(&completed, WRONG_STORE_ADDRESS);
```

Use `event_at_pc` for all four PCs. Assert call kind/link `call_direct/true`, return kind/link `return/false`, return target provider `ras`, call/return/descendant issue before load response, return issue no earlier than call admitted writeback, descendant issue no earlier than return issue, and ordered commits.

At `descendant.issue_tick + 1`, assert the tick remains before load response, exact resident PCs `[0x0c, 0x10, 0x1c, 0x14]`, LSQ count one, `assert_register_absent_or_zero(&resident, "x1")`, and the live rename entry for architectural register 1 matches the call ROB destination. Assert RAS pushes, pops, used, correct, and target-provider RAS counters are positive.

- [ ] **Step 6: Build and test the hierarchy indirect-call/return fixture**

Use:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c auipc x11, 0
10 addi x11, x11, 28    # target 0x28
14 lw x12, 0(x18)
18 jalr x5, 0(x11)      # link/return target 0x1c
1c addi x13, x0, 42
20 jal x0, +20          # exit store at 0x34
24 m5_fail
28 jalr x0, 0(x5)
2c sw x7, 8(x18)
30 m5_fail
34 sw x13, 4(x18)
38 m5_exit
3c m5_fail
```

Implement `rem6_run_o3_same_window_indirect_link_return_commits_cache_fabric_dram`. Assert final `x5=0x8000_001c`, exact memory, no wrong store, exact live PCs `[0x14, 0x18, 0x28, 0x1c]`, call kind `call_indirect`, return kind `return`, call link write only, RAS target provider, ordered issue/commit, one LSQ row, and nonzero cache/transport/fabric/DRAM activity.

- [ ] **Step 7: Add lookahead-one suppression**

Implement `rem6_run_o3_same_window_link_return_requires_branch_lookahead_two` with the direct fixture and branch lookahead one.

Assert final architecture still succeeds after ordinary execution, but at a live tick between load issue and response:

```rust
assert_eq!(resident_rob_pcs(&resident), ["0x8000000c", "0x80000010", "0x8000001c"]);
assert_no_fetch_pc(&resident, "0x80000014");
```

The call may be predicted; the return-target descendant must not open without the second lookahead slot.

- [ ] **Step 8: Add overwritten-link suppression**

Use:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c lw x12, 0(x18)
10 jal x1, +16          # target 0x20, initial link 0x14
14 sw x7, 8(x18)        # stale call-return target witness
18 m5_fail
1c nop
20 addi x1, x1, 28      # overwrite link with valid target 0x30
24 jalr x0, 0(x1)
28 sw x7, 12(x18)
2c m5_fail
30 addi x13, x0, 42
34 sw x13, 4(x18)
38 m5_exit
3c m5_fail
```

Implement `rem6_run_o3_same_window_overwritten_link_return_stays_terminal`. Assert final `x1=0x8000_0030`, `x13=42`, exact success memory, no stores at offsets 8 or 12, live PCs `[0x0c, 0x10, 0x20, 0x24]`, and no fetch at stale target `0x14` or live target `0x30` before the load response.

- [ ] **Step 9: Add older-branch rollback with call and return**

Use branch lookahead three and this layout:

```text
00 m5_switch_cpu
04 auipc x18, 0
08 addi x18, x18, DATA_START - 4
0c addi x7, x0, 1
10 addi x1, x0, 0x11
14 lw x12, 0(x18)
18 beq x7, x7, +24      # correct target 0x30, predicted fallthrough
1c jal x1, +12          # wrong-path call target 0x28, link 0x20
20 sw x7, 8(x18)
24 m5_fail
28 jalr x0, 0(x1)
2c sw x7, 12(x18)
30 addi x15, x0, 0x33
34 sw x15, 4(x18)
38 m5_exit
3c m5_fail
```

Implement `rem6_run_o3_older_branch_discards_same_window_link_return_chain`. Assert resident PCs `[0x14, 0x18, 0x1c, 0x28]` before repair, final `x1=0x11`, `x15=0x33`, exact memory `2a000000330000000000000000000000`, no wrong-path Data/Memory stores, branch misprediction/squash evidence, no retired call/return rows, and RAS push plus pop squashes.

- [ ] **Step 10: Add mode-transfer row**

Implement `rem6_run_host_switch_transfers_o3_same_window_link_return` with the direct fixture.

First run a detailed baseline. Select `switch_tick = descendant.issue_tick + 1` and assert it is before the load response. Run the switch with:

```rust
let switch_arg = format!("{switch_tick}:cpu0:timing");
let mut switch_args = DIRECT_WIDTH_ARGS.to_vec();
switch_args.extend(["--host-switch-cpu-mode", switch_arg.as_str()]);
let switched = run_link_return_json(
    &path,
    "direct",
    2_500,
    "detailed",
    2,
    &switch_args,
);
```

Assert:

1. final mode is timing;
2. final `x1`, `x13`, and memory match baseline;
3. transfer is non-restorable and carries four ROB/one LSQ rows;
4. all four events preserve baseline issue, admitted-writeback, writeback, and commit ticks;
5. no wrong store occurs;
6. inherited call/return RAS and control state drains exactly once.

- [ ] **Step 11: Add checkpoint boundary**

Implement `rem6_run_o3_same_window_link_return_checkpoint_boundary` with the direct fixture.

Run a live checkpoint at the same live tick:

```rust
let live_arg = format!("{live_tick}:link-return-live");
let mut live_command =
    control_window_command(&path, "direct", 2_500, "detailed", 2, DATA_ADDRESS, 16);
let mut live_args = DIRECT_WIDTH_ARGS.to_vec();
live_args.extend(["--host-checkpoint", live_arg.as_str()]);
live_command.args(live_args.iter().copied());
let output = live_command.output().unwrap();
assert!(!output.status.success());
assert!(output.stdout.is_empty());
```

Assert stderr contains:

```text
checkpoint component is not quiescent: cpu0
```

Then checkpoint after the success store commits and restore one tick later:

```rust
let checkpoint_tick = event_u64(event_at_pc(&baseline, "0x80000028"), "commit_tick") + 1;
let restore_tick = checkpoint_tick + 1;
let checkpoint_arg = format!("{checkpoint_tick}:link-return-drained");
let restore_arg = format!("{restore_tick}:link-return-drained");
let mut restore_args = DIRECT_WIDTH_ARGS.to_vec();
restore_args.extend([
    "--host-checkpoint",
    checkpoint_arg.as_str(),
    "--host-restore-checkpoint",
    restore_arg.as_str(),
]);
let restored = run_link_return_json(
    &path,
    "direct",
    2_500,
    "detailed",
    2,
    &restore_args,
);
```

Assert final architecture, checkpoint and restore counts one, zero ROB/LSQ rows, and no `o3-live-data-handoff` chunk. Use `assert_drained_control_runtime` for restored JSON.

- [ ] **Step 12: Add timing suppression**

Implement `rem6_run_timing_suppresses_o3_same_window_link_return` with the direct fixture in timing mode.

Assert final `x1`, `x13`, and memory match detailed mode, `/cores/0/o3_runtime` is absent, `/debug/o3_trace` is empty, and `assert_no_o3_stats` finds no O3, ROB, LSQ, rename, IQ, IEW, commit, FTQ, fetch, or BAC aliases.

- [ ] **Step 13: Run CLI RED/GREEN and neighboring regressions**

Run:

```text
cargo test -p rem6 --test cli_run same_window_link_return -- --nocapture
cargo test -p rem6 --test cli_run predicted_control -- --nocapture
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
```

Expected: eight new tests pass, the existing 42-test predicted-control matrix grows accordingly, and source policy passes because no anchors have been added yet.

- [ ] **Step 14: Commit executable evidence**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_return.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/link_kind.rs
git commit -m "test: cover same-window link returns"
```

### Task 4: Update The Honest Migration Boundary

**Files:**
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add all eight anchors**

Add after the existing linked-control anchor block:

```text
rem6_run_o3_same_window_link_return_commits_direct
rem6_run_o3_same_window_indirect_link_return_commits_cache_fabric_dram
rem6_run_o3_same_window_link_return_requires_branch_lookahead_two
rem6_run_o3_same_window_overwritten_link_return_stays_terminal
rem6_run_o3_older_branch_discards_same_window_link_return_chain
rem6_run_host_switch_transfers_o3_same_window_link_return
rem6_run_o3_same_window_link_return_checkpoint_boundary
rem6_run_timing_suppresses_o3_same_window_link_return
```

- [ ] **Step 2: Update the CPU evidence paragraph in place**

In the CPU migrated prose, add one bounded same-window link-return sentence that names all eight anchors and states:

1. direct `JAL x1` and independent `JALR x5` calls feed same-window returns;
2. the return target is RAS-provided and never architectural-fallback-provided;
3. direct and hierarchy rows hold exactly four ROB and one LSQ row;
4. call link rename/writeback forwards to return issue;
5. one RAS push and pop remain pending behind the load;
6. lookahead one and overwritten-link cases suppress descendants;
7. older repair squashes call/return and both RAS operations;
8. switch, checkpoint, and timing boundaries are covered.

Do not claim general live indirect-target forwarding.

- [ ] **Step 3: Narrow the open boundary consistently**

Remove only `same-window call-to-return link forwarding` from:

1. the main CPU incomplete list;
2. CPU `Not migrated` prose;
3. CPU `Next evidence`;
4. the `tests/gem5/cpu_tests` next-evidence cell.

Retain verbatim or equivalently precise gaps for:

```text
coroutine pop-then-push forms
linked indirect calls whose target source is x1/x5
producer-forwarded indirect targets for descendant fetch
fourth-and-deeper linked/control chains
arbitrary broader mixed windows
general IQ/wakeup/select beyond bounded scoped issue authority
a general O3 engine
```

- [ ] **Step 4: Preserve score and line-count invariants**

Verify the document still contains:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw,
- [ ] A running O3 engine owns ROB, LSQ, rename map, commit, store-to-load forwarding, and FU latency.
```

Keep the file at exactly 1,200 lines by editing existing CPU lines in place rather than appending a new section.

- [ ] **Step 5: Run documentation policy gates**

Run:

```text
cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --nocapture
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
wc -l docs/architecture/gem5-to-rem6-migration.md
git diff --check
```

Expected: all tests pass, line count is exactly 1,200, score remains 74%/8-of-10, and the general O3 item remains unchecked.

- [ ] **Step 6: Commit anchors and ledger atomically**

```bash
git add crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record same-window link return evidence"
```

### Task 5: Full Verification, Review, Push, And Continue The Active Goal

**Files:**
- Read only unless a review finding requires a focused fix.

- [ ] **Step 1: Run focused gates from the design**

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu riscv_o3_window_policy -- --nocapture
cargo test -p rem6-cpu detailed_o3_control -- --nocapture
cargo test -p rem6-cpu o3_runtime_control_window -- --nocapture
cargo test -p rem6-cpu o3_runtime_issue -- --nocapture
cargo test -p rem6-cpu riscv_fetch_ahead -- --nocapture
cargo test -p rem6 --test cli_run same_window_link_return -- --nocapture
cargo test -p rem6 --test cli_run predicted_control -- --nocapture
```

- [ ] **Step 2: Run full completion gates**

```text
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
cargo test --workspace --all-targets --quiet
wc -l docs/architecture/gem5-to-rem6-migration.md
git diff --check origin/main...HEAD
git diff --name-only origin/main...HEAD -- temp
```

Expected: all pass; ledger is 1,200 lines; no `temp/` file is changed.

- [ ] **Step 3: Dispatch mandatory high-intensity read-only reviews**

Use five independent `gpt-5.5:xhigh` reviewers:

1. policy provenance and latest-writer semantics;
2. RAS-required frontend target authority and missing-RAS fail-closed behavior;
3. runtime rename/source-forwarding/issue/writeback/rollback ownership;
4. real CLI matrix strength, tick assertions, RAS and architectural witnesses;
5. Slop/dead-code/duplication/source-policy/ledger honesty and score invariants.

Each reviewer reports findings first with file/line references. Fix every substantive finding through a red test where applicable, send the fix back to the finding reviewer, and rerun affected plus full gates.

- [ ] **Step 4: Verify commit series and remote freshness**

Run:

```text
git status --short --branch
git log --oneline --stat origin/main..HEAD
git fetch origin main
git status --short --branch
```

Expected: clean branch, only the intended self-contained commits, and no remote divergence.

- [ ] **Step 5: Push and verify remote identity**

```text
git push origin main
git rev-parse HEAD
git ls-remote origin refs/heads/main
git status --short --branch
```

Expected: local and remote hashes match and `main...origin/main` is clean.

- [ ] **Step 6: Keep the broad repository goal active**

Do not mark the persistent goal complete. Recalibrate the remaining CPU `Next evidence` after push and select the next bounded cap-breaking or root-cause cleanup increment.
