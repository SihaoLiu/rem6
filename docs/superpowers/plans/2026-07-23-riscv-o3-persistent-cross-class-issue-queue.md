# RISC-V O3 Persistent Cross-Class Issue Queue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace future-tick live issue simulation with one runtime-owned sequence queue that persists across scheduler turns, services one tick per O3 wake, and exposes exact lifecycle, telemetry, stats, and debug evidence.

**Architecture:** `O3RuntimeState` stores exactly one `O3LiveIssueState` containing sequence membership, requested service tick, same-tick decision state, and transient evidence. Each service turn materializes canonical packet/ROB/dependency/calendar data for resident sequences only, commits selected rows through a bounded rollback transaction, and routes all later progress through the existing O3 writeback wake owner.

**Tech Stack:** Rust workspace, `rem6-cpu` detailed RISC-V O3 runtime, `rem6-system` stats bridge, `rem6` CLI/JSON/text/debug surfaces, real-binary integration tests, source-policy tests, Git.

---

## File Map

Create production owners:

- `crates/rem6-cpu/src/o3_runtime_live_window/issue_identity.rs` - staged fetch identity and exact issue-packet binding, extracted to create live-window headroom.
- `crates/rem6-cpu/src/riscv_o3_writeback_wake/desired.rs` - one shared desired-wake calculation used by external publication and internal refresh.
- `crates/rem6-cpu/src/o3_runtime_issue/state.rs` - resident sequence inventory, issue-service request, same-tick decision, telemetry, debug records, and transaction-active state.
- `crates/rem6-cpu/src/o3_runtime_issue/service.rs` - exactly-one-tick materialization, arbitration, commit, classification, and next-wake calculation.
- `crates/rem6-cpu/src/o3_runtime_issue/transaction.rs` - bounded selected-batch rollback/commit without cloning all of `O3RuntimeState`.
- `crates/rem6/src/debug_output/o3_issue_queue_json.rs` - stable queue telemetry and lifecycle-event JSON.

Create focused tests:

- `crates/rem6-cpu/src/o3_runtime_issue/state_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/transaction_tests.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`

Modify runtime ownership:

- `crates/rem6-cpu/src/o3_runtime.rs`
- `crates/rem6-cpu/src/o3_runtime_error.rs`
- `crates/rem6-cpu/src/o3_runtime_issue.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/queue.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/dependency.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs`
- `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- `crates/rem6-cpu/src/o3_runtime_memory.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_result_admission.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_set.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs`
- `crates/rem6-cpu/src/o3_runtime_handoff.rs`
- `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- `crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs`
- `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`
- `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`
- `crates/rem6-cpu/src/riscv_live_retire_gate.rs`
- `crates/rem6-cpu/src/lib.rs`
- `crates/rem6-cpu/src/public_api.rs`

Modify CPU tests and policy:

- `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_live_window_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_live_window_identity_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs` and focused children
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs` and `replan.rs`
- `crates/rem6-cpu/src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs`
- `crates/rem6-cpu/tests/source_policy.rs`

Modify output and stats plumbing:

- `crates/rem6/src/core_summary.rs`
- `crates/rem6/src/core_summary_json.rs`
- `crates/rem6/src/run_execution_summary.rs`
- `crates/rem6/src/stats_output/o3_runtime.rs`
- `crates/rem6/src/stats_output/o3_runtime_issue.rs`
- `crates/rem6/src/debug_output/o3.rs`
- `crates/rem6-system/src/riscv_run_stats.rs`
- `crates/rem6-system/src/riscv_o3_runtime_stats.rs`
- `crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs`
- `crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs`
- `crates/rem6-system/src/host.rs`

Modify executable evidence and ledger:

- `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue/general_iq.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/general_iq.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs`
- `crates/rem6/tests/source_policy.rs`
- `crates/rem6/tests/source_policy/core_test_anchors.txt`
- `docs/architecture/gem5-to-rem6-migration.md`

## Execution Preconditions

The implementation worktree already exists on branch `o3-persistent-cross-class-issue-queue`. Start from design commit `c788ffecea07eb7fa24109c63b92c003f0c0e7c7` or a later clean commit containing the approved design and this plan.

Use a repository-local temporary directory for every Cargo command and commit hook:

```bash
mkdir -p target/tmp
git status --short --branch
```

Expected: the branch tracks `origin/o3-persistent-cross-class-issue-queue` and the worktree is clean.

Do not edit or commit anything under `temp/`. Do not build or run the read-only gem5 reference.

Before every task commit, run `TMPDIR=$PWD/target/tmp cargo fmt --all`, rerun that task's green commands if formatting changed Rust files, and stage only the paths owned by that task.

### Task 1: Create Focused Headroom and Shared Wake Calculation

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_live_window/issue_identity.rs`
- Create: `crates/rem6-cpu/src/riscv_o3_writeback_wake/desired.rs`
- Create: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs:1-105,585-657`
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs:1-290`
- Modify: `crates/rem6/tests/source_policy.rs:1-45`
- Modify: `crates/rem6-cpu/tests/source_policy.rs:1-80`

- [ ] **Step 1: Add the focused-file ownership RED**

Attach the new CLI policy child near the other O3 ownership modules:

```rust
#[path = "source_policy/o3_persistent_iq_ownership.rs"]
mod o3_persistent_iq_ownership;
```

Create `o3_persistent_iq_ownership.rs` with the initial layout test:

```rust
use super::*;

const MAX_PERSISTENT_IQ_CLI_LINES: usize = 900;
const MAX_PERSISTENT_IQ_POLICY_LINES: usize = 600;

#[test]
#[ignore = "RED until Task 10 creates the persistent-IQ CLI owner"]
fn o3_persistent_iq_focused_owners_exist_and_stay_bounded() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli = crate_dir.join("tests/cli_run/m5_host_actions/o3/persistent_iq.rs");
    let policy = crate_dir.join("tests/source_policy/o3_persistent_iq_ownership.rs");

    assert!(cli.is_file(), "missing {}", cli.display());
    assert!(line_count(&cli) <= MAX_PERSISTENT_IQ_CLI_LINES);
    assert!(line_count(&policy) <= MAX_PERSISTENT_IQ_POLICY_LINES);
}
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_persistent_iq_focused_owners_exist_and_stay_bounded -- --ignored --nocapture
```

Expected: FAIL because `persistent_iq.rs` does not exist yet. Keep this test in place; Task 10 creates the owner and turns it green.

- [ ] **Step 2: Add CPU line caps before moving code**

Add these constants to `crates/rem6-cpu/tests/source_policy.rs`:

```rust
const MAX_O3_RUNTIME_LIVE_ISSUE_IDENTITY_LINES: usize = 350;
const MAX_RISCV_O3_WRITEBACK_WAKE_DESIRED_LINES: usize = 220;
const MAX_O3_RUNTIME_ISSUE_STATE_LINES: usize = 450;
const MAX_O3_RUNTIME_ISSUE_STATE_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_ISSUE_SERVICE_LINES: usize = 600;
const MAX_O3_RUNTIME_ISSUE_SERVICE_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_ISSUE_TRANSACTION_LINES: usize = 450;
const MAX_O3_RUNTIME_ISSUE_TRANSACTION_TEST_LINES: usize = 500;
```

Add this test after the constants:

```rust
#[test]
#[ignore = "RED until Tasks 2, 4, and 5 create all focused issue owners"]
fn o3_persistent_iq_cpu_files_stay_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    for (relative, limit) in [
        (
            "src/o3_runtime_live_window/issue_identity.rs",
            MAX_O3_RUNTIME_LIVE_ISSUE_IDENTITY_LINES,
        ),
        (
            "src/riscv_o3_writeback_wake/desired.rs",
            MAX_RISCV_O3_WRITEBACK_WAKE_DESIRED_LINES,
        ),
        (
            "src/o3_runtime_issue/state.rs",
            MAX_O3_RUNTIME_ISSUE_STATE_LINES,
        ),
        (
            "src/o3_runtime_issue/state_tests.rs",
            MAX_O3_RUNTIME_ISSUE_STATE_TEST_LINES,
        ),
        (
            "src/o3_runtime_issue/service.rs",
            MAX_O3_RUNTIME_ISSUE_SERVICE_LINES,
        ),
        (
            "src/o3_runtime_issue/service_tests.rs",
            MAX_O3_RUNTIME_ISSUE_SERVICE_TEST_LINES,
        ),
        (
            "src/o3_runtime_issue/transaction.rs",
            MAX_O3_RUNTIME_ISSUE_TRANSACTION_LINES,
        ),
        (
            "src/o3_runtime_issue/transaction_tests.rs",
            MAX_O3_RUNTIME_ISSUE_TRANSACTION_TEST_LINES,
        ),
    ] {
        let path = crate_dir.join(relative);
        assert!(path.is_file(), "missing focused owner {}", path.display());
        let lines = line_count(&path);
        assert!(lines <= limit, "{relative} has {lines} lines; limit is {limit}");
    }
}
```

Run it with `--ignored` to prove the layout gap, keep intermediate commits green, and remove the ignore in Task 5 after all eight files exist.

- [ ] **Step 3: Extract staged issue identity without changing behavior**

Move `O3LiveStagedFetchIdentity`, its implementation, and the complete existing definitions named `bind_live_staged_issue_packet`, `bind_live_staged_issue_packet_at_sequence`, `live_staged_issue_packet`, `live_staged_instruction_matches`, `live_staged_fetch_identity_matches`, and `live_staged_sequence_for_fetch_identity` verbatim from `o3_runtime_live_window.rs` into `o3_runtime_live_window/issue_identity.rs`.

Attach and re-export the identity from `o3_runtime_live_window.rs`:

```rust
#[path = "o3_runtime_live_window/issue_identity.rs"]
mod issue_identity;

pub(super) use issue_identity::O3LiveStagedFetchIdentity;
```

The moved file starts with:

```rust
use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvInstruction};

use super::super::o3_runtime_issue::queue::O3LiveIssuePacket;
use super::*;
```

- [ ] **Step 4: Extract one shared wake-demand helper without adding IQ behavior yet**

Attach the helper from `riscv_o3_writeback_wake.rs`:

```rust
#[path = "riscv_o3_writeback_wake/desired.rs"]
mod desired;

use desired::desired_o3_writeback_wake;
```

Create `desired.rs` with the current sources and an optional translated-pair input:

```rust
use rem6_kernel::Tick;

use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvO3WritebackWakeDemand {
    pub(super) desired_tick: Option<Tick>,
    pub(super) allow_current: bool,
}

pub(super) fn desired_o3_writeback_wake(
    state: &RiscvCoreState,
    now: Tick,
    translated_result_pair: Option<Tick>,
) -> RiscvO3WritebackWakeDemand {
    let memory_result = state
        .o3_runtime
        .earliest_unpublished_memory_result_writeback_tick();
    let pending_address = state.o3_runtime.pending_data_address_wake_tick();
    let live_gate_ready_tick = state.live_retire_gate.pending_ready_tick();
    let restored_live_gate = state
        .live_retire_gate
        .owned_scheduler_wakes()
        .is_empty()
        .then_some(live_gate_ready_tick)
        .flatten();
    let forwarded_control = state
        .o3_runtime
        .producer_forwarded_control_target()
        .filter(|forwarded| {
            !state
                .branch_speculations
                .contains_key(&forwarded.fetch_request().sequence())
        })
        .map(|forwarded| forwarded.ready_tick().max(now));
    let translated_result_retry = state.translated_result_pair_retry_wake_tick(now);
    let desired_tick = [
        memory_result,
        pending_address,
        restored_live_gate,
        forwarded_control,
        translated_result_pair,
        translated_result_retry,
    ]
    .into_iter()
    .flatten()
    .min();
    let allow_current = [pending_address, restored_live_gate, forwarded_control]
        .into_iter()
        .flatten()
        .any(|tick| tick == now);
    RiscvO3WritebackWakeDemand {
        desired_tick,
        allow_current,
    }
}
```

Make both current entry points call this helper. `requested_o3_writeback_wake_tick` passes its translated-pair tick; `refresh_o3_writeback_wake` passes `None`.

- [ ] **Step 5: Run behavior-preserving verification**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_writeback_wake_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_packet_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_runtime_live_window -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy source_policy_driver_keeps_anchor_data_out_of_root -- --nocapture
```

Expected: all selected tests PASS. The persistent-IQ focused-owner RED remains intentionally failing until Task 10 and is not included in this green set.

- [ ] **Step 6: Commit the mechanical refactor**

```bash
git add crates/rem6-cpu/src/o3_runtime_live_window.rs crates/rem6-cpu/src/o3_runtime_live_window/issue_identity.rs crates/rem6-cpu/src/riscv_o3_writeback_wake.rs crates/rem6-cpu/src/riscv_o3_writeback_wake/desired.rs crates/rem6-cpu/tests/source_policy.rs crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "refactor: prepare persistent O3 issue ownership"
```

### Task 2: Add the Persistent Live Issue State

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/state.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/state_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs:80-120,236-275,282-410,626-660,1060-1090`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:1-45`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue.rs:1-590`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window/issue_identity.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs:14-125`
- Modify: packet-binding call sites listed in the File Map
- Modify: `crates/rem6-cpu/src/o3_runtime_error.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs:1-25`

- [ ] **Step 1: Attach state tests and write membership RED tests**

Attach the production module from `o3_runtime_issue.rs`:

```rust
#[path = "o3_runtime_issue/state.rs"]
mod state;

pub use state::{O3LiveIssueTelemetry, O3LiveIssueTraceAction, O3LiveIssueTraceClass,
    O3LiveIssueTraceRecord};
pub(in crate::o3_runtime) use state::O3LiveIssueState;
```

Attach the focused tests from `o3_runtime_issue_tests.rs`:

```rust
#[path = "o3_runtime_issue/state_tests.rs"]
mod live_issue_state;
```

Add these tests to `state_tests.rs`:

```rust
#[test]
fn live_issue_state_enqueues_supported_bound_rows_once_and_orders_by_sequence() {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::CrossResource);
    fixture.bind_row_at(2, 31);
    fixture.bind_row_at(0, 31);
    fixture.bind_row_at(1, 31);
    let expected = [BRANCH_PC, SECOND_PC, THIRD_PC].map(|pc| fixture.sequence(pc));
    assert_eq!(fixture.runtime.live_issue.resident_sequences(), expected);

    fixture.bind_row_at(1, 31);
    assert_eq!(fixture.runtime.live_issue.resident_sequences(), expected);
}

#[test]
fn live_issue_state_skips_bound_fp_vector_and_system_rows() {
    let mut runtime = O3RuntimeState::default();
    for (pc, raw, request_sequence) in [
        (BRANCH_PC, 0x0020_81d3, 11),
        (SECOND_PC, 0x0220_81d7, 12),
        (THIRD_PC, 0x0000_0073, 13),
    ] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        runtime
            .stage_live_instruction(Address::new(pc), decoded.instruction(), 0)
            .unwrap();
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            31,
        ));
    }
    assert!(runtime.live_issue.resident_sequences().is_empty());
}

#[test]
fn live_issue_state_requests_current_tick_on_admission() {
    let mut fixture = ScalarIssueFixture::new_unbound(1, ScalarIssueCase::CrossResource);
    fixture.bind_row_at(0, 31);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(31));
    assert_eq!(fixture.runtime.live_issue_telemetry().wake_requests(), 1);
}

#[test]
fn live_issue_state_removes_exact_and_suffix_rows_atomically() {
    let mut state = O3LiveIssueState::default();
    for sequence in 1..=4 {
        assert!(state.enqueue_at(
            sequence,
            Address::new(0x8000 + sequence * 4),
            O3LiveIssueTraceClass::ScalarInteger,
            31,
        ));
    }
    assert!(state.remove_exact_at(
        2,
        O3LiveIssueTraceAction::Retired,
        Address::new(0x8008),
        O3LiveIssueTraceClass::ScalarInteger,
        32,
    ));
    let suffix = [
        (3, Address::new(0x800c), O3LiveIssueTraceClass::ScalarInteger),
        (4, Address::new(0x8010), O3LiveIssueTraceClass::Control),
    ];
    assert_eq!(
        state.remove_suffix_at(
            3,
            O3LiveIssueTraceAction::Squashed,
            &suffix,
            33,
        ),
        2,
    );
    assert_eq!(state.resident_sequences(), [1]);
    assert_eq!(state.telemetry().current_occupancy(), 1);
}

#[test]
fn live_issue_state_stats_reset_preserves_membership_and_requested_wake() {
    let mut state = O3LiveIssueState::default();
    assert!(state.enqueue_at(
        7,
        Address::new(0x8020),
        O3LiveIssueTraceClass::IntegerMulDiv,
        41,
    ));
    state.reset_stats_baseline();
    assert_eq!(state.resident_sequences(), [7]);
    assert_eq!(state.requested_service_tick(), Some(41));
    assert_eq!(state.telemetry().enqueued_rows(), 0);
    assert_eq!(state.telemetry().wake_requests(), 0);
    assert_eq!(state.telemetry().current_occupancy(), 1);
    assert_eq!(state.telemetry().peak_occupancy(), 1);
}

#[test]
fn live_issue_head_binding_enqueues_then_durable_record_removes_exact_row() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let decoded = decoded(instruction);
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &[request(11)],
        20,
    ));
    assert_eq!(runtime.live_issue.resident_sequences(), [sequence]);

    let mut hart = RiscvHartState::new(BRANCH_PC);
    let execution = hart.execute_decoded(decoded).unwrap();
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, 20, instruction);
    assert!(runtime
        .record_live_issue_head_execution(head, &[request(11)], execution)
        .unwrap());
    assert!(runtime.live_issue.resident_sequences().is_empty());

    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &[request(11)],
        20,
    ));
    assert!(runtime.live_issue.resident_sequences().is_empty());
}

#[test]
fn live_issue_active_transaction_is_nonquiescent() {
    let mut state = O3LiveIssueState::default();
    assert!(state.is_quiescent());
    assert!(state.begin_transaction());
    assert!(!state.is_quiescent());
    state.end_transaction();
    assert!(state.is_quiescent());
}
```

Add this fixture helper beside `bind_row` in `o3_runtime_issue_tests.rs` and update `bind_row` to call it with tick 20:

```rust
fn bind_row_at(&mut self, index: usize, admission_tick: u64) {
    let (pc, instruction, request_sequence) = self.rows[index];
    assert!(self.runtime.bind_live_staged_issue_packet(
        Address::new(pc),
        decoded(instruction),
        &[request(request_sequence)],
        admission_tick,
    ));
}
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_state_ -- --nocapture
```

Expected: compilation FAIL because `O3LiveIssueState` and the runtime accessors do not exist.

- [ ] **Step 2: Define the public transient evidence types**

Start `state.rs` with these exact public shapes:

```rust
use std::collections::{BTreeMap, BTreeSet};

use rem6_memory::Address;

use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct O3LiveIssueTelemetry {
    enqueued_rows: u64,
    service_turns: u64,
    wake_requests: u64,
    current_occupancy: u64,
    peak_occupancy: u64,
    scalar_integer_issued_rows: u64,
    integer_mul_div_issued_rows: u64,
    memory_agu_issued_rows: u64,
    control_issued_rows: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LiveIssueTraceClass {
    ScalarInteger,
    IntegerMulDiv,
    MemoryAgu,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LiveIssueTraceAction {
    Queued,
    Selected,
    RetainedResource,
    RetainedDependency,
    Replayed,
    Squashed,
    Retired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3LiveIssueTraceRecord {
    sequence: u64,
    pc: Address,
    action: O3LiveIssueTraceAction,
    issue_class: O3LiveIssueTraceClass,
    service_tick: u64,
    next_wake_tick: Option<u64>,
    raw_writeback_tick: Option<u64>,
    admitted_writeback_tick: Option<u64>,
    cleanup_boundary: Option<u64>,
}
```

Add `const` getters for every field. Add `name()` getters returning these stable strings:

```rust
impl O3LiveIssueTraceClass {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ScalarInteger => "scalar_integer",
            Self::IntegerMulDiv => "integer_mul_div",
            Self::MemoryAgu => "memory_agu",
            Self::Control => "control",
        }
    }
}

impl O3LiveIssueTraceAction {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Selected => "selected",
            Self::RetainedResource => "retained_resource",
            Self::RetainedDependency => "retained_dependency",
            Self::Replayed => "replayed",
            Self::Squashed => "squashed",
            Self::Retired => "retired",
        }
    }
}
```

- [ ] **Step 3: Define sequence-only state and same-tick decision ownership**

Continue `state.rs` with:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveIssueBlockedKind {
    Resource,
    Dependency,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct O3LiveIssueActiveTick {
    tick: u64,
    issued_sequences: BTreeSet<u64>,
    blocked_sequences: BTreeMap<u64, O3LiveIssueBlockedKind>,
    baseline_issued_sequences: BTreeSet<u64>,
    baseline_blocked_sequences: BTreeMap<u64, O3LiveIssueBlockedKind>,
    max_rows_after_reset: usize,
    observed_after_reset: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueState {
    resident_sequences: Vec<u64>,
    requested_service_tick: Option<u64>,
    active_tick: Option<O3LiveIssueActiveTick>,
    transaction_active: bool,
    mutation_generation: u64,
    last_service_generation: Option<(u64, u64)>,
    telemetry: O3LiveIssueTelemetry,
    trace_records: Vec<O3LiveIssueTraceRecord>,
}
```

Implement these focused methods, keeping the resident vector sorted with `binary_search`:

```rust
pub(in crate::o3_runtime) fn resident_sequences(&self) -> &[u64];
pub(in crate::o3_runtime) fn enqueue_at(&mut self, sequence: u64, pc: Address,
    issue_class: O3LiveIssueTraceClass, tick: u64) -> bool;
pub(in crate::o3_runtime) fn remove_exact_at(&mut self, sequence: u64,
    action: O3LiveIssueTraceAction, pc: Address, issue_class: O3LiveIssueTraceClass,
    tick: u64) -> bool;
pub(in crate::o3_runtime) fn remove_suffix_at(&mut self, boundary: u64,
    action: O3LiveIssueTraceAction, rows: &[(u64, Address, O3LiveIssueTraceClass)],
    tick: u64) -> usize;
pub(in crate::o3_runtime) fn request_service_at(&mut self, tick: u64);
pub(in crate::o3_runtime) fn requested_service_tick(&self) -> Option<u64>;
pub(in crate::o3_runtime) fn clear_requested_service_tick(&mut self);
pub(in crate::o3_runtime) fn begin_service_at(&mut self, tick: u64) -> bool;
pub(in crate::o3_runtime) fn mark_mutated(&mut self);
pub(in crate::o3_runtime) fn begin_transaction(&mut self) -> bool;
pub(in crate::o3_runtime) fn end_transaction(&mut self);
pub(in crate::o3_runtime) fn transaction_active(&self) -> bool;
pub(in crate::o3_runtime) fn is_quiescent(&self) -> bool;
pub(in crate::o3_runtime) fn telemetry(&self) -> O3LiveIssueTelemetry;
pub(in crate::o3_runtime) fn trace_records(&self) -> &[O3LiveIssueTraceRecord];
pub(in crate::o3_runtime) fn reset_stats_baseline(&mut self);
```

`request_service_at` must keep the minimum tick and increment `wake_requests` only when the stored minimum changes. `begin_service_at` accepts a stored request less than or equal to `tick`, rejects duplicate `(tick, generation)` calls, clears the due requested tick, and increments `service_turns` only for an accepted turn.

`reset_stats_baseline` must clear enqueue/service/wake/class counters and trace records, set current and peak occupancy to `resident_sequences.len()`, preserve `requested_service_tick`, and retain the active tick while copying its current issued/blocked observations into the reset baseline.

`remove_exact_at` and `remove_suffix_at` must remove matching rows from the active tick's blocked-classification map and update occupancy in the same mutation. They retain already-issued sequence observations because squashed work still consumed issue capacity, but removed rows cannot contribute a later blocked-row count or requested wake.

- [ ] **Step 4: Add exactly one runtime field and remove the legacy issue-cycle set**

In `O3RuntimeState`, replace:

```rust
live_issue_cycle_ticks: BTreeSet<u64>,
```

with:

```rust
live_issue: O3LiveIssueState,
```

Initialize it with `O3LiveIssueState::default()`. In `restore`, assign a fresh default. In `reset_stats`, call `self.live_issue.reset_stats_baseline()` instead of clearing `live_issue_cycle_ticks`.

Add narrow runtime accessors:

```rust
pub(crate) fn live_issue_service_tick(&self) -> Option<u64> {
    self.live_issue.requested_service_tick()
}

pub(crate) fn live_issue_is_quiescent(&self) -> bool {
    self.live_issue.is_quiescent()
}

pub fn live_issue_telemetry(&self) -> O3LiveIssueTelemetry {
    self.live_issue.telemetry()
}

pub fn live_issue_trace_records(&self) -> &[O3LiveIssueTraceRecord] {
    self.live_issue.trace_records()
}
```

Re-export the four public evidence types from `o3_runtime.rs`:

```rust
pub use o3_runtime_issue::{
    O3LiveIssueTelemetry, O3LiveIssueTraceAction, O3LiveIssueTraceClass,
    O3LiveIssueTraceRecord,
};
use o3_runtime_issue::O3LiveIssueState;
```

- [ ] **Step 5: Make exact packet binding conditionally enqueue at an admission tick**

Add `admission_tick: u64` to both packet-binding runtime methods. After an exact identity bind succeeds and its mutable identity borrow ends, call:

```rust
self.enqueue_bound_live_issue_sequence_at(sequence, admission_tick)
```

Implement the helper in `state.rs`:

```rust
impl O3RuntimeState {
    fn enqueue_bound_live_issue_sequence_at(&mut self, sequence: u64, tick: u64) -> bool {
        let Some((index, rob)) = self
            .snapshot
            .reorder_buffer
            .iter()
            .copied()
            .enumerate()
            .find(|(_, entry)| entry.is_live_staged() && entry.sequence() == sequence)
        else {
            return false;
        };
        if self.live_speculative_executions.iter().any(|issued| issued.sequence == sequence) {
            return true;
        }
        let pending = self.pending_data_addresses.find_sequence(sequence);
        if pending.is_some_and(|row| row.materialized.is_some()) {
            return true;
        }
        let Some(packet) = self.live_staged_issue_packet(sequence) else {
            return false;
        };
        let issue_class = if pending.is_some() {
            Some(O3LiveIssueTraceClass::MemoryAgu)
        } else {
            live_issue_trace_class(packet.instruction())
        };
        let Some(issue_class) = issue_class else {
            return true;
        };
        let pc = rob.pc();
        let _ = index;
        self.live_issue.enqueue_at(sequence, pc, issue_class, tick);
        true
    }
}
```

Add `live_issue_trace_class` beside `live_issue_op_class` in `queue.rs`. It returns `None` for FP, vector arithmetic, and system instructions.

For pending addresses, reorder `try_stage_pending_data_address_window` so the canonical `O3PendingDataAddress` is inserted before the packet bind/enqueue. Add `admission_tick` to `stage_pending_data_address_window` and pass the real `issue_tick` from `stage_dependent_result_address_window`; focused unit fixtures may use `0`.

- [ ] **Step 6: Remove exact rows after durable head or batch recording**

After `record_live_issue_head_execution` has appended the durable execution, remove the exact sequence from `live_issue`. Do the same after each successful fixed-FU record and pending-address materialization in the current batch path. Use `Selected` action and preserve raw/admitted writeback ticks for the later debug update.

- [ ] **Step 7: Run state and compatibility tests**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_state_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_packet_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib pending_address_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_persistent_live_issue -- --nocapture
```

Expected: PASS. Existing future-loop scheduling remains behaviorally intact for this commit, but queue membership is now persistent and exact.

- [ ] **Step 8: Commit persistent state ownership**

```bash
git add crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_error.rs crates/rem6-cpu/src/o3_runtime_issue.rs crates/rem6-cpu/src/o3_runtime_issue/state.rs crates/rem6-cpu/src/o3_runtime_issue/state_tests.rs crates/rem6-cpu/src/o3_runtime_issue/queue.rs crates/rem6-cpu/src/o3_runtime_live_window/issue_identity.rs crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs crates/rem6-cpu/src/riscv_live_retire_window.rs crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs crates/rem6-cpu/src/o3_runtime_issue_tests.rs crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: add persistent O3 live issue state"
```

### Task 3: Materialize Resident Sequences and Derive the Calendar

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue.rs:100-175`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs:35-100`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:200-305`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_result_admission.rs:45-75`
- Modify: `crates/rem6-cpu/tests/source_policy.rs:3698-4145`

- [ ] **Step 1: Write resident-materialization RED tests**

Add these tests to `queue_tests.rs`:

```rust
#[test]
fn live_issue_queue_materializes_resident_sequences_without_rob_inventory_scan() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let (_, sequences) = stage_queue_rows(&mut runtime, instructions);
    for (pc, instruction, request_sequence) in [
        (BRANCH_PC, instructions[0], 11),
        (SECOND_PC, instructions[1], 12),
        (THIRD_PC, instructions[2], 13),
    ] {
        bind_queue_row_at(&mut runtime, pc, instruction, request_sequence, 20);
    }
    assert!(runtime.live_issue.remove_exact_at(
        sequences[1],
        O3LiveIssueTraceAction::Retired,
        Address::new(SECOND_PC),
        O3LiveIssueTraceClass::IntegerMulDiv,
        20,
    ));

    let queue = ready_queue(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()).unwrap(),
    );
    assert_eq!(
        queue.sequences().collect::<Vec<_>>(),
        vec![sequences[0], sequences[2]],
    );
    assert!(runtime.live_staged_issue_packet(sequences[1]).is_some());
}

#[test]
fn live_issue_queue_rejects_stale_ordinary_resident_sequence() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let (_, sequences) = stage_queue_rows(&mut runtime, instructions);
    bind_queue_row_at(&mut runtime, BRANCH_PC, instructions[0], 11, 20);
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequences[0]));

    assert!(matches!(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence })
            if sequence == sequences[0]
    ));
}

#[test]
fn live_issue_queue_returns_exact_pending_replay_boundary() {
    let mut runtime = O3RuntimeState::default();
    let (_, sequence) = stage_queue_pending_row(&mut runtime);
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequence));

    assert!(matches!(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()).unwrap(),
        O3LiveIssueQueueCapture::ReplayPending(replay) if replay == sequence
    ));
}

#[test]
fn live_issue_queue_preserves_architectural_sequence_order() {
    let mut runtime = O3RuntimeState::default();
    let instructions = [branch(), mul(14, 2, 3), addi(15, 4, 1)];
    let (_, sequences) = stage_queue_rows(&mut runtime, instructions);
    bind_queue_row_at(&mut runtime, THIRD_PC, instructions[2], 13, 20);
    bind_queue_row_at(&mut runtime, BRANCH_PC, instructions[0], 11, 20);
    bind_queue_row_at(&mut runtime, SECOND_PC, instructions[1], 12, 20);

    let queue = ready_queue(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()).unwrap(),
    );
    assert_eq!(queue.sequences().collect::<Vec<_>>(), sequences);
}
```

Rename the existing `bind_queue_row` helper to `bind_queue_row_at`, add an `admission_tick` parameter, and pass that value to `bind_live_staged_issue_packet`. Keep a four-argument `bind_queue_row` wrapper whose body is `bind_queue_row_at(runtime, pc, instruction, request_sequence, 20);` until all existing tests are migrated.

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_materializes_resident -- --nocapture
```

Expected: FAIL because `O3LiveIssueQueue::capture` still scans the full ROB.

- [ ] **Step 2: Replace capture with sequence-owned materialization**

Replace `capture(runtime, head)` with:

```rust
pub(in crate::o3_runtime) fn materialize(
    runtime: &O3RuntimeState,
    resident_sequences: &[u64],
) -> Result<O3LiveIssueQueueCapture, O3RuntimeError> {
    let mut entries = Vec::with_capacity(resident_sequences.len());
    for &sequence in resident_sequences {
        let index = runtime
            .snapshot
            .reorder_buffer
            .binary_search_by_key(&sequence, O3ReorderBufferEntry::sequence)
            .map_err(|_| O3RuntimeError::InvalidLiveIssueQueueEntry { sequence })?;
        let rob = runtime.snapshot.reorder_buffer[index];
        if !rob.is_live_staged() {
            return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
        }
        let pending = runtime.pending_data_addresses.find_sequence(sequence);
        if pending.is_some_and(|row| row.materialized.is_some()) {
            return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
        }
        let Some(packet) = runtime.live_staged_issue_packet(sequence).cloned() else {
            return if pending.is_some() {
                Ok(O3LiveIssueQueueCapture::ReplayPending(sequence))
            } else {
                Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence })
            };
        };
        let Some(scheduling) =
            runtime.live_issue_scheduling_candidate_from_metadata(index, rob, &packet)
        else {
            return if pending.is_some() {
                Ok(O3LiveIssueQueueCapture::ReplayPending(sequence))
            } else {
                Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence })
            };
        };
        entries.push(O3LiveIssueQueueEntry { packet, scheduling });
    }
    Self::try_from_entries(entries).map(O3LiveIssueQueueCapture::Ready)
}
```

Use `self.live_issue.resident_sequences()` at every scheduler materialization site. Do not iterate the complete ROB to discover queue membership.

- [ ] **Step 3: Write no-head calendar RED tests**

Add:

```rust
#[test]
fn live_issue_calendar_derives_fixed_fu_reservations_without_caller_head() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    stage_live_row(&mut runtime, 1, LOAD_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(1, 20, LOAD_PC, addi(3, 0, 1)));

    let plan = O3LiveIssueCalendar::capture(&runtime)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(2, O3IssueOpClass::Branch)],
        )
        .unwrap();
    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![2]);
}

#[test]
fn live_issue_calendar_derives_memory_head_from_live_data_access() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    assert!(runtime.set_memory_issue_width(1));
    let head = calendar_load_event(LOAD_PC, 10, 5, 2, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &head,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));

    let plan = O3LiveIssueCalendar::capture(&runtime)
        .plan_scoped_at(
            31,
            std::iter::empty::<O3DependencyScopeId>(),
            [
                ready(2, O3IssueOpClass::Memory),
                ready(3, O3IssueOpClass::IntAlu),
            ],
        )
        .unwrap();
    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![3]);
    assert_eq!(sequences(plan.resource_blocked()), vec![2]);
}

#[test]
fn live_issue_calendar_keeps_narrow_head_admission_helper() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(1));
    let head = O3LiveIssueHeadReservation::for_instruction(1, 20, addi(3, 0, 1));

    let plan = O3LiveIssueCalendar::capture_with_head_for_admission(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(2, O3IssueOpClass::IntAlu)],
        )
        .unwrap();
    assert_eq!(plan.reserved_width(), 1);
    assert!(plan.issued().is_empty());
    assert_eq!(sequences(plan.resource_blocked()), vec![2]);
}
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_calendar_derives_ -- --nocapture
```

Expected: FAIL because calendar capture still requires `O3LiveIssueHeadReservation`.

- [ ] **Step 4: Derive all service reservations from canonical runtime state**

Change the calendar API to:

```rust
pub(in crate::o3_runtime) fn capture(runtime: &O3RuntimeState) -> Self;

pub(in crate::o3_runtime) fn capture_with_head_for_admission(
    runtime: &O3RuntimeState,
    head: O3LiveIssueHeadReservation,
) -> Self {
    let mut calendar = Self::capture(runtime);
    calendar.reserve(head.issue_tick, head.op_class);
    calendar
}
```

`capture(runtime)` must reserve:

```rust
for live in &runtime.live_data_accesses {
    calendar.reserve(live.issue_tick, O3IssueOpClass::Memory);
}
for pending in runtime.pending_data_addresses.iter() {
    if let Some(tick) = pending.selected_issue_tick {
        calendar.reserve(tick, O3IssueOpClass::Memory);
    }
}
for issued in &runtime.live_speculative_executions {
    calendar.reserve(
        issued.issue_tick,
        live_issue_op_class(issued.execution.instruction()),
    );
}
```

Use `capture_with_head_for_admission` only in `next_memory_result_issue_tick`; queue service and the compatibility scheduler use `capture(runtime)`.

- [ ] **Step 5: Replace the old source-policy prohibition**

Rename `o3_live_issue_queue_owns_candidate_inventory` to `o3_persistent_live_issue_state_owns_candidate_inventory`. Assert:

```rust
assert_eq!(
    production_struct_named_type_storage(&production_sources, "O3LiveIssueState"),
    vec![(PathBuf::from("src/o3_runtime.rs"), 1)],
);
assert!(!queue.contains("for (index, rob) in runtime.snapshot.reorder_buffer"));
assert!(queue.contains("for &sequence in resident_sequences"));
assert!(queue.contains("binary_search_by_key(&sequence"));
assert!(!production_sources.iter().any(|(_, source)| {
    source.contains("O3LiveIssueDependencyTable,") || source.contains("O3LiveIssueCalendar,")
}));
```

Retain the rule that packet storage exists only in `O3LiveStagedFetchIdentity`.

- [ ] **Step 6: Run focused queue/calendar tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_queue_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_calendar_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_persistent_live_issue -- --nocapture
git add crates/rem6-cpu/src/o3_runtime_issue/queue.rs crates/rem6-cpu/src/o3_runtime_issue/queue_tests.rs crates/rem6-cpu/src/o3_runtime_issue/calendar.rs crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs crates/rem6-cpu/src/o3_runtime_issue.rs crates/rem6-cpu/src/o3_runtime_memory_result_admission.rs crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: materialize persistent O3 issue membership"
```

Expected: all selected tests PASS.

### Task 4: Replace Full-State Clone with a Bounded Issue Transaction

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/transaction.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/transaction_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:15-45,150-205`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs:90-135`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs:99-140,440-490`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Write atomicity and rollback RED tests**

Attach `transaction_tests.rs` from `o3_runtime_issue_tests.rs` and add:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
struct TouchedIssueState {
    pending_state: O3PendingStateSnapshot,
    reorder_buffer: Vec<O3ReorderBufferEntry>,
    live_speculative_executions: Vec<O3LiveSpeculativeExecution>,
    pending_data_addresses: O3PendingDataAddresses,
    writeback_calendar: O3WritebackReservationCalendar,
    live_writeback_counted_sequences: BTreeSet<u64>,
    finalized_writeback_port_stats: O3FinalizedWritebackPortStats,
    live_staged_fetch_identities: BTreeMap<u64, O3LiveStagedFetchIdentity>,
    stats: O3RuntimeStats,
    live_issue: O3LiveIssueState,
}

fn touched(runtime: &O3RuntimeState) -> TouchedIssueState {
    TouchedIssueState {
        pending_state: runtime.snapshot.pending_state.clone(),
        reorder_buffer: runtime.snapshot.reorder_buffer.clone(),
        live_speculative_executions: runtime.live_speculative_executions.clone(),
        pending_data_addresses: runtime.pending_data_addresses.clone(),
        writeback_calendar: runtime.writeback_calendar.clone(),
        live_writeback_counted_sequences: runtime.live_writeback_counted_sequences.clone(),
        finalized_writeback_port_stats: runtime.finalized_writeback_port_stats.clone(),
        live_staged_fetch_identities: runtime.live_staged_fetch_identities.clone(),
        stats: runtime.stats,
        live_issue: runtime.live_issue.clone(),
    }
}

fn prepared_rows(fixture: &ScalarIssueFixture, tick: u64) -> Vec<O3PreparedLiveIssue> {
    let queue = ready_queue(
        O3LiveIssueQueue::materialize(
            &fixture.runtime,
            fixture.runtime.live_issue.resident_sequences(),
        )
        .unwrap(),
    );
    let dependencies =
        O3LiveIssueDependencyTable::new(&fixture.runtime, queue.entries()).unwrap();
    let plan = O3LiveIssueCalendar::capture(&fixture.runtime)
        .plan_at(tick, &dependencies, queue.entries())
        .unwrap();
    match fixture
        .runtime
        .prepare_live_issue_batch(&fixture.hart, &queue, plan.issued(), tick)
        .unwrap()
    {
        O3PreparedLiveIssueBatch::Prepared(rows) => rows,
        O3PreparedLiveIssueBatch::ReplayPending(sequence) => {
            panic!("unexpected pending replay at {sequence}")
        }
    }
}

#[test]
fn live_issue_transaction_failure_records_no_partial_runtime_or_queue_state() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let prepared = prepared_rows(&fixture, 21);
    assert_eq!(prepared.len(), 2);
    let rejected = prepared[1].candidate.sequence();
    assert!(fixture
        .runtime
        .remove_live_staged_issue_identity_for_test(rejected));
    let before = touched(&fixture.runtime);

    assert!(matches!(
        fixture.runtime.record_live_issue_batch(prepared),
        Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence })
            if sequence == rejected
    ));
    assert_eq!(touched(&fixture.runtime), before);
}

#[test]
fn live_issue_transaction_writeback_replan_rollback_restores_ports_and_descendants() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    assert!(fixture.runtime.set_writeback_width(1));
    let prepared = prepared_rows(&fixture, 21);
    assert_eq!(prepared.len(), 2);
    let rejected = prepared[1].candidate.sequence();
    assert!(fixture
        .runtime
        .remove_live_staged_issue_identity_for_test(rejected));
    let calendar = fixture.runtime.writeback_calendar.clone();
    let pending_state = fixture.runtime.snapshot.pending_state.clone();
    let executions = fixture.runtime.live_speculative_executions.clone();

    assert!(fixture.runtime.record_live_issue_batch(prepared).is_err());
    assert_eq!(fixture.runtime.writeback_calendar, calendar);
    assert_eq!(fixture.runtime.snapshot.pending_state, pending_state);
    assert_eq!(fixture.runtime.live_speculative_executions, executions);
}

#[test]
fn live_issue_transaction_pending_replay_commits_only_exact_suffix_cleanup() {
    let mut runtime = O3RuntimeState::default();
    let (_, sequence) = super::queue::stage_queue_pending_row(&mut runtime);
    runtime.set_pending_data_address_resource_blocked_wake_for_test(sequence, 41);
    let mut hart = RiscvHartState::new(BRANCH_PC);
    hart.write(reg(12), 0x9100);
    let queue = ready_queue(
        O3LiveIssueQueue::materialize(&runtime, runtime.live_issue.resident_sequences()).unwrap(),
    );
    let dependencies = O3LiveIssueDependencyTable::new(&runtime, queue.entries()).unwrap();
    let plan = O3LiveIssueCalendar::capture(&runtime)
        .plan_at(40, &dependencies, queue.entries())
        .unwrap();
    let prepared = match runtime
        .prepare_live_issue_batch(&hart, &queue, plan.issued(), 40)
        .unwrap()
    {
        O3PreparedLiveIssueBatch::Prepared(rows) => rows,
        O3PreparedLiveIssueBatch::ReplayPending(replay) => {
            panic!("unexpected early replay at {replay}")
        }
    };
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequence));
    let older_live_data = runtime.live_data_accesses.clone();

    assert_eq!(
        runtime.record_live_issue_batch(prepared).unwrap(),
        O3LiveIssueBatchOutcome::ReplayPending(sequence),
    );
    assert_eq!(runtime.live_data_accesses, older_live_data);
    assert!(runtime.pending_data_addresses.is_empty());
    assert!(runtime.live_issue.resident_sequences().is_empty());
}

#[test]
fn live_issue_transaction_commit_removes_only_durable_selected_sequences() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let prepared = prepared_rows(&fixture, 21);
    let committed = prepared
        .iter()
        .map(|row| row.candidate.sequence())
        .collect::<BTreeSet<_>>();
    assert_eq!(committed.len(), 2);

    assert_eq!(
        fixture.runtime.record_live_issue_batch(prepared).unwrap(),
        O3LiveIssueBatchOutcome::Recorded,
    );
    assert!(committed.iter().all(|sequence| {
        !fixture
            .runtime
            .live_issue
            .resident_sequences()
            .contains(sequence)
    }));
    assert_eq!(fixture.runtime.live_issue.resident_sequences().len(), 1);
}
```

Make `prepare_live_issue_batch` visible as `pub(in crate::o3_runtime)` and make `stage_queue_pending_row` plus `ready_queue` `pub(super)` in `queue_tests.rs` so the focused transaction tests reuse the canonical fixtures rather than duplicating them.

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_transaction_ -- --nocapture
```

Expected: compilation FAIL because the transaction owner is absent.

- [ ] **Step 2: Define the bounded rollback image**

Create `transaction.rs` with exactly the owners mutated by issue recording:

```rust
use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[derive(Clone)]
struct O3LiveIssueRollback {
    pending_state: O3PendingStateSnapshot,
    reorder_buffer: Vec<O3ReorderBufferEntry>,
    live_speculative_executions: Vec<O3LiveSpeculativeExecution>,
    pending_data_addresses: O3PendingDataAddresses,
    writeback_calendar: O3WritebackReservationCalendar,
    live_writeback_counted_sequences: BTreeSet<u64>,
    finalized_writeback_port_stats: O3FinalizedWritebackPortStats,
    live_staged_fetch_identities: BTreeMap<u64, O3LiveStagedFetchIdentity>,
    stats: O3RuntimeStats,
    live_issue: O3LiveIssueState,
}

pub(in crate::o3_runtime) struct O3LiveIssueTransaction;
```

Implement `capture` and `restore` by cloning only these fields. Do not clone `O3RuntimeState`, live data payloads, trace records, control-lineage maps, fetch state, or configuration.

- [ ] **Step 3: Pre-admit selected fixed-FU writebacks as one batch**

Add a helper on `O3PreparedLiveIssue`:

```rust
fn fixed_fu_writeback_ready(&self) -> Result<Option<O3LiveWritebackReady>, O3RuntimeError> {
    if self.candidate.is_pending_data_address() || !self.candidate.consumes_writeback_slot() {
        return Ok(None);
    }
    let issue_tick = self.candidate.issue_tick(self.issue_tick);
    let raw_ready_tick = issue_tick
        .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
            self.execution.instruction(),
        ))
        .ok_or(O3RuntimeError::WritebackTickOverflow { tick: issue_tick })?;
    Ok(Some(O3LiveWritebackReady::fixed_fu(
        self.candidate.sequence(),
        raw_ready_tick,
    )))
}
```

Make `O3WritebackReservation::sequence()` available in production. Reserve all selected fixed-FU rows with one `reserve_writeback_completions` call before appending executions. Refactor `record_live_speculative_execution` to accept an already-admitted reservation, while the head-recording path keeps `reserve_fixed_fu_writeback`.

- [ ] **Step 4: Implement transactional batch recording**

The transaction entry point must follow this shape:

```rust
pub(in crate::o3_runtime) fn record(
    runtime: &mut O3RuntimeState,
    prepared: Vec<O3PreparedLiveIssue>,
) -> Result<O3LiveIssueBatchOutcome, O3RuntimeError> {
    let rollback = O3LiveIssueRollback::capture(runtime);
    if !runtime.live_issue.begin_transaction() {
        return Err(O3RuntimeError::LiveIssueTransactionAlreadyActive);
    }
    let result = record_prepared_batch_in_place(runtime, prepared);
    match result {
        Ok(O3LiveIssueBatchOutcome::Recorded) => {
            runtime.live_issue.end_transaction();
            Ok(O3LiveIssueBatchOutcome::Recorded)
        }
        Ok(O3LiveIssueBatchOutcome::ReplayPending(sequence)) => {
            rollback.restore(runtime);
            runtime.discard_pending_data_address_from(sequence);
            Ok(O3LiveIssueBatchOutcome::ReplayPending(sequence))
        }
        Err(error) => {
            rollback.restore(runtime);
            Err(error)
        }
    }
}
```

`record_prepared_batch_in_place` first reserves all writebacks, then records every row, then removes exact queue membership. If any ordinary row is invalid, return an error and let rollback restore every touched owner. A pending replay does not retain any prior selected-row mutation.

- [ ] **Step 5: Replace the full clone and enforce it mechanically**

Replace the current `let mut staged = self.clone()` implementation with `O3LiveIssueTransaction::record(self, prepared)`.

Add source-policy assertions:

```rust
let batch = rust_function_definition(&issue, "record_live_issue_batch").unwrap();
assert!(!batch.contains("self.clone()"));
assert!(batch.contains("O3LiveIssueTransaction::record(self, prepared)"));
let transaction_storage = production_struct_named_type_storage(
    &[(PathBuf::from("src/o3_runtime_issue/transaction.rs"), transaction.clone())],
    "O3RuntimeState",
);
assert!(transaction_storage.is_empty());
```

The storage helper inspects struct field definitions, so method signatures may still name `O3RuntimeState`.

- [ ] **Step 6: Run transaction and writeback regressions, then commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_transaction_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib selected_issue_batch_failure_records_no_partial_state -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_writeback_replan -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_live_issue_transaction -- --nocapture
git add crates/rem6-cpu/src/o3_runtime_issue.rs crates/rem6-cpu/src/o3_runtime_issue/transaction.rs crates/rem6-cpu/src/o3_runtime_issue/transaction_tests.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_writeback.rs crates/rem6-cpu/src/o3_runtime_issue_tests.rs crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: bound O3 live issue transactions"
```

Expected: all selected tests PASS and the source policy finds no complete runtime clone in issue service.

### Task 5: Implement Exactly-One-Tick Service and Delayed Stats

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/service.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:1-395`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/state.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/dependency.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_stats.rs:1090-1120`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs:325-410`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs:780-920`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Write one-tick and same-tick stats RED tests**

Attach `service_tests.rs` and add:

```rust
#[test]
fn service_live_issue_queue_at_issues_only_the_requested_tick() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::CrossResource);
    let outcome = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 21)
        .unwrap();
    assert_eq!(outcome.issued_rows(), 1);
    assert_eq!(fixture.runtime.live_speculative_executions.len(), 1);
    assert!(fixture
        .runtime
        .live_speculative_executions
        .iter()
        .all(|row| row.issue_tick == 21));
    assert_eq!(fixture.runtime.live_issue.resident_sequences().len(), 2);
}

#[test]
fn service_live_issue_queue_at_retains_resource_blocked_rows_for_next_tick() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::CrossResource);
    let outcome = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 21)
        .unwrap();
    assert_eq!(outcome.next_service_tick(), Some(22));
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(22));
    assert_eq!(fixture.runtime.live_issue.resident_sequences().len(), 2);
}

#[test]
fn service_live_issue_queue_at_requests_earliest_dependency_ready_tick() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);
    let outcome = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 21)
        .unwrap();
    let producer_ready = fixture.execution_at(SECOND_PC).admitted_writeback_tick;
    assert_eq!(outcome.next_service_tick(), Some(producer_ready));
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(producer_ready));
    assert!(fixture
        .runtime
        .live_issue
        .resident_sequences()
        .contains(&fixture.sequence(THIRD_PC)));
}

#[test]
fn service_live_issue_queue_at_allows_capacity_remaining_same_tick_reentry() {
    assert_eq!(
        crate::riscv_fu_latency::riscv_execute_wait_cycles(addi(14, 2, 1)),
        0,
    );
    let mut fixture = ScalarIssueFixture::new(4, ScalarIssueCase::SameTickAluDependency);
    let first = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    assert_eq!(first.issued_rows(), 2);
    assert_eq!(first.next_service_tick(), Some(20));
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(20));

    let second = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    assert_eq!(second.issued_rows(), 1);
    assert!(fixture.runtime.live_issue.resident_sequences().is_empty());
}

#[test]
fn live_issue_stats_same_tick_reentry_projects_once() {
    let mut fixture = ScalarIssueFixture::new(4, ScalarIssueCase::SameTickAluDependency);
    fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    let first_projection = fixture.runtime.stats();
    assert_eq!(fixture.runtime.stats(), first_projection);
    assert_eq!(first_projection.issue_cycles(), 1);
    assert_eq!(first_projection.issued_rows(), 2);

    fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    let second_projection = fixture.runtime.stats();
    assert_eq!(second_projection.issue_cycles(), 1);
    assert_eq!(second_projection.issued_rows(), 3);
    assert_eq!(fixture.runtime.stats(), second_projection);

    fixture.runtime.seal_live_issue_decision_before(21);
    assert_eq!(fixture.runtime.stats(), second_projection);
}

#[test]
fn live_issue_stats_reset_rebases_unsealed_decision() {
    let mut fixture = ScalarIssueFixture::new(4, ScalarIssueCase::SameTickAluDependency);
    fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    assert_eq!(fixture.runtime.stats().issued_rows(), 2);
    fixture.runtime.reset_stats();
    assert_eq!(fixture.runtime.stats().issued_rows(), 0);

    fixture.runtime.live_issue.mark_mutated();
    fixture.runtime.live_issue.request_service_at(20);
    fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    let post_reset = fixture.runtime.stats();
    assert_eq!(post_reset.issue_cycles(), 1);
    assert_eq!(post_reset.issued_rows(), 1);
    assert_eq!(post_reset.resource_blocked_row_cycles(), 0);
    assert_eq!(post_reset.dependency_blocked_row_cycles(), 0);
}
```

Add `SameTickAluDependency` to `ScalarIssueCase` with rows `[addi(14, 2, 1), addi(15, 14, 1), branch()]`. This fixture supplies one zero-latency producer, one dependent ALU, and one independent control row so width four has a legal same-tick second turn after the head reservation and first selected pair.

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib service_live_issue_queue_at_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_stats_ -- --nocapture
```

Expected: compilation FAIL because the one-tick service and projected stats are absent.

- [ ] **Step 2: Define the service outcome**

Create `service.rs` with:

```rust
use rem6_isa_riscv::RiscvHartState;

use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueServiceOutcome {
    issued_rows: usize,
    next_service_tick: Option<u64>,
    replay_boundary: Option<u64>,
}

impl O3LiveIssueServiceOutcome {
    pub(in crate::o3_runtime) const fn issued_rows(self) -> usize { self.issued_rows }
    pub(in crate::o3_runtime) const fn next_service_tick(self) -> Option<u64> { self.next_service_tick }
    pub(in crate::o3_runtime) const fn replay_boundary(self) -> Option<u64> { self.replay_boundary }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct O3LiveIssuePostService {
    resource_blocked_sequences: Vec<u64>,
    dependency_blocked_sequences: Vec<u64>,
    max_rows_at_tick: usize,
    next_service_tick: Option<u64>,
}
```

- [ ] **Step 3: Move preparation into the service owner**

Move `prepare_live_issue_batch` from the issue root into `service.rs` unchanged except for module paths. Keep `O3PreparedLiveIssue`, `O3PreparedLiveIssueBatch`, `O3LiveIssueBatchOutcome`, and head reservation declarations in the thin issue root.

- [ ] **Step 4: Implement one service turn with no later-tick loop**

Implement `O3RuntimeState::service_live_issue_queue_at` in this order:

```rust
pub(crate) fn service_live_issue_queue_at(
    &mut self,
    hart: &RiscvHartState,
    now: u64,
) -> Result<O3LiveIssueServiceOutcome, O3RuntimeError> {
    self.seal_live_issue_decision_before(now);
    if !self.live_issue.begin_service_at(now) {
        return Ok(O3LiveIssueServiceOutcome::default());
    }
    let queue = match O3LiveIssueQueue::materialize(
        self,
        self.live_issue.resident_sequences(),
    )? {
        O3LiveIssueQueueCapture::Ready(queue) => queue,
        O3LiveIssueQueueCapture::ReplayPending(sequence) => {
            self.discard_pending_data_address_from(sequence);
            return Ok(O3LiveIssueServiceOutcome {
                replay_boundary: Some(sequence),
                ..O3LiveIssueServiceOutcome::default()
            });
        }
    };
    if queue.entries().is_empty() {
        self.live_issue.clear_requested_service_tick();
        return Ok(O3LiveIssueServiceOutcome::default());
    }
    let dependencies = O3LiveIssueDependencyTable::new(self, queue.entries())?;
    let calendar = O3LiveIssueCalendar::capture(self);
    let plan = calendar.plan_at(now, &dependencies, queue.entries())?;
    let issued_sequences = plan
        .issued()
        .iter()
        .map(O3ScopedReadyInstruction::sequence)
        .collect::<Vec<_>>();
    let prepared = self.prepare_live_issue_batch(hart, &queue, plan.issued(), now)?;
    let issued_rows = match prepared {
        O3PreparedLiveIssueBatch::Prepared(rows) => {
            O3LiveIssueTransaction::record(self, rows)?;
            plan.issued().len()
        }
        O3PreparedLiveIssueBatch::ReplayPending(sequence) => {
            self.discard_pending_data_address_from(sequence);
            return Ok(O3LiveIssueServiceOutcome {
                replay_boundary: Some(sequence),
                ..O3LiveIssueServiceOutcome::default()
            });
        }
    };
    let mut post = self.classify_live_issue_queue_after_service(now)?;
    post.max_rows_at_tick = post
        .max_rows_at_tick
        .max(plan.reserved_width().saturating_add(issued_rows));
    self.live_issue.observe_sequences(
        now,
        &issued_sequences,
        &post.resource_blocked_sequences,
        &post.dependency_blocked_sequences,
        post.max_rows_at_tick,
    );
    if let Some(tick) = post.next_service_tick {
        self.live_issue.request_service_at(tick);
    }
    Ok(O3LiveIssueServiceOutcome {
        issued_rows,
        next_service_tick: post.next_service_tick,
        replay_boundary: None,
    })
}
```

There must be no `loop`, no assignment that advances `now`, and no speculative execution recorded at a tick greater than the call argument.

- [ ] **Step 5: Compute the next wake from a post-commit view**

`classify_live_issue_queue_after_service(now)` rematerializes the remaining resident rows and replans at `now` using the calendar that now includes rows committed by the first plan. Compute candidates as follows:

```rust
let same_tick = (!post_plan.issued().is_empty()).then_some(now);
let resource_tick = (!post_plan.resource_blocked().is_empty())
    .then(|| now.checked_add(1))
    .flatten();
let dependency_tick = dependency_table
    .earliest_resolution_after(now, post_plan.dependency_blocked());
let pending_tick = post_plan
    .resource_blocked()
    .iter()
    .find_map(|row| self.pending_data_address_sequence_for_replay(row.sequence()))
    .and_then(|sequence| {
        self.record_pending_data_address_resource_blocked(sequence, now);
        self.pending_data_address_wake_tick()
    });
let next_service_tick = [same_tick, resource_tick, dependency_tick, pending_tick]
    .into_iter()
    .flatten()
    .min();

let resource_blocked_sequences = post_plan
    .resource_blocked()
    .iter()
    .map(O3ScopedReadyInstruction::sequence)
    .collect::<Vec<_>>();
let dependency_blocked_sequences = post_plan
    .dependency_blocked()
    .iter()
    .map(O3ScopedReadyInstruction::sequence)
    .collect::<Vec<_>>();
Ok(O3LiveIssuePostService {
    resource_blocked_sequences,
    dependency_blocked_sequences,
    max_rows_at_tick: post_plan
        .reserved_width()
        .saturating_add(post_plan.issued().len()),
    next_service_tick,
})
```

If rows remain and all candidates are `None`, return `O3RuntimeError::LiveIssueQueueHasNoWake { sequence }` for the oldest resident row.

- [ ] **Step 6: Implement active decision projection and sealing**

Add a compact delta type in `state.rs`:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueDecisionDelta {
    pub(in crate::o3_runtime) new_cycle: bool,
    pub(in crate::o3_runtime) issued_rows: usize,
    pub(in crate::o3_runtime) resource_blocked_rows: usize,
    pub(in crate::o3_runtime) dependency_blocked_rows: usize,
    pub(in crate::o3_runtime) max_rows_at_tick: usize,
}
```

`observe_sequences` adds only newly committed sequences, replaces the latest blocked map, records the maximum reserved-plus-issued width, and sets `observed_after_reset`. `reset_stats_baseline` copies current issued/blocked maps into the baseline, sets `max_rows_after_reset` to zero, and clears `observed_after_reset`.

Add runtime methods:

```rust
fn seal_live_issue_decision_before(&mut self, tick: u64) {
    if let Some(delta) = self.live_issue.take_decision_before(tick) {
        self.stats.record_issue_cycle(
            delta.new_cycle,
            delta.issued_rows,
            delta.resource_blocked_rows,
            delta.dependency_blocked_rows,
            delta.max_rows_at_tick,
        );
    }
}

fn seal_live_issue_decision(&mut self) {
    if let Some(delta) = self.live_issue.take_current_decision() {
        self.stats.record_issue_cycle(
            delta.new_cycle,
            delta.issued_rows,
            delta.resource_blocked_rows,
            delta.dependency_blocked_rows,
            delta.max_rows_at_tick,
        );
    }
}

pub fn stats(&self) -> O3RuntimeStats {
    let mut projected = self.stats;
    if let Some(delta) = self.live_issue.projected_decision() {
        projected.record_issue_cycle(
            delta.new_cycle,
            delta.issued_rows,
            delta.resource_blocked_rows,
            delta.dependency_blocked_rows,
            delta.max_rows_at_tick,
        );
    }
    projected
}
```

Remove `record_live_issue_decision`, `flush_live_issue_decision`, and `O3LiveIssueTickDecision` from the old calendar owner.

- [ ] **Step 7: Keep a temporary compatibility driver for existing callers**

Until Task 6 rewires scheduler wake entry, retain `schedule_live_speculative_issues` as a thin compatibility driver that requests `earliest_tick`, repeatedly calls `service_live_issue_queue_at`, and advances only to the returned requested tick. Mark it `pub(crate)` and keep all planning in `service.rs`. Task 6 removes this function and its loop.

- [ ] **Step 8: Run service/stats regressions and commit**

Remove `#[ignore = "RED until Tasks 2, 4, and 5 create all focused issue owners"]` from `o3_persistent_iq_cpu_files_stay_focused`; all eight focused CPU files now exist.

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib service_live_issue_queue_at_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_stats_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib scoped_issue_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_live_issue_service -- --nocapture
git add crates/rem6-cpu/src/o3_runtime_issue.rs crates/rem6-cpu/src/o3_runtime_issue/service.rs crates/rem6-cpu/src/o3_runtime_issue/service_tests.rs crates/rem6-cpu/src/o3_runtime_issue/state.rs crates/rem6-cpu/src/o3_runtime_issue/dependency.rs crates/rem6-cpu/src/o3_runtime_issue/calendar.rs crates/rem6-cpu/src/o3_runtime_stats.rs crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_issue_tests.rs crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: service one O3 issue tick at a time"
```

Expected: all selected tests PASS. Existing top-level behavior remains green through the temporary driver.

### Task 6: Route All Queue Progress Through the O3 Wake

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake/desired.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs:145-290`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs:680-735,880-955`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs:440-480`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/state.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/service.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Write wake aggregation and fired-order RED tests**

Add these tests in `riscv_o3_writeback_wake.rs`:

```rust
#[test]
fn requested_o3_writeback_wake_tick_includes_live_issue_service_tick() {
    let core = core();
    core.state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .live_issue
        .request_service_at(31);
    assert_eq!(core.requested_o3_writeback_wake_tick(30), Some(31));
}

#[test]
fn refresh_o3_writeback_wake_includes_live_issue_service_tick() {
    let core = core();
    let mut state = core.state.lock().expect("riscv core lock");
    state.o3_runtime.live_issue.request_service_at(31);
    state.refresh_o3_writeback_wake(30);
    assert_eq!(state.o3_writeback_wake.desired_tick, Some(31));
}

#[test]
fn requested_and_refresh_share_the_same_live_issue_minimum() {
    let core = core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .live_retire_gate
            .restore_checkpoint(Some(O3LiveRetireGateCheckpointPayload::new(
                memory_request(35),
                35,
            )));
        state.o3_runtime.live_issue.request_service_at(31);
        state.refresh_o3_writeback_wake(30);
        assert_eq!(state.o3_writeback_wake.desired_tick, Some(31));
    }
    assert_eq!(core.requested_o3_writeback_wake_tick(30), Some(31));
}

#[test]
fn writeback_replan_moves_live_issue_wake_earlier() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.live_issue.enqueue_at(
        1,
        Address::new(0x8000),
        O3LiveIssueTraceClass::ScalarInteger,
        50,
    ));
    runtime.live_issue.clear_requested_service_tick();
    runtime.live_issue.request_service_at(50);

    runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(9, 31)])
        .unwrap();
    assert_eq!(runtime.live_issue_service_tick(), Some(31));
}

```

Add this exact structural order test to `crates/rem6-cpu/tests/source_policy.rs`:

```rust
#[test]
fn o3_writeback_wake_fired_services_pending_address_before_issue_queue() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = production_rust_source(
        &fs::read_to_string(crate_dir.join("src/riscv_o3_writeback_wake.rs")).unwrap(),
    );
    let fired = rust_function_definition(&source, "mark_o3_writeback_wake_fired")
        .expect("missing fired O3 wake callback");
    let mark = fired.find("o3_writeback_wake.mark_fired(now)").unwrap();
    let pending = fired
        .find("wake_ready_o3_data_access_younger_window(now, &fetch_events)")
        .unwrap();
    let issue = fired
        .find("service_live_issue_queue_at(&hart, now)")
        .unwrap();
    let refresh = fired.find("refresh_o3_writeback_wake(now)").unwrap();
    assert!(mark < pending && pending < issue && issue < refresh);
}
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib requested_o3_writeback_wake_tick_includes_live_issue_service_tick -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_writeback_wake_fired_services_pending_address_before_issue_queue -- --nocapture
```

Expected: FAIL because issue service is not yet part of desired wake or fired handling.

- [ ] **Step 2: Add live issue to the shared minimum**

In `desired.rs`, add and clamp stale-past runtime requests to the current scheduler tick:

```rust
let live_issue = state
    .o3_runtime
    .live_issue_service_tick()
    .map(|tick| tick.max(now));
```

Include it in the minimum and current-tick rule:

```rust
let desired_tick = [
    memory_result,
    pending_address,
    restored_live_gate,
    forwarded_control,
    translated_result_pair,
    translated_result_retry,
    live_issue,
]
.into_iter()
.flatten()
.min();

let allow_current = [pending_address, restored_live_gate, forwarded_control, live_issue]
    .into_iter()
    .flatten()
    .any(|tick| tick == now);
```

Add this runtime helper in `state.rs`:

```rust
pub(super) fn request_live_issue_after_writeback_change(&mut self, tick: u64) {
    if self.live_issue.resident_sequences().is_empty() {
        return;
    }
    self.live_issue.mark_mutated();
    if !self.live_issue.transaction_active() {
        self.live_issue.request_service_at(tick);
    }
}
```

Add `transaction_active(&self) -> bool` to `O3LiveIssueState`. After `reserve_writeback_completions` commits its writeback replan, request the minimum admitted tick returned by that reservation batch through this helper. During an active issue transaction the post-service classifier remains authoritative; outside issue service this moves an earlier producer wake immediately and permits an old earlier request to remain as a harmless stale-early wake when a replan moves later.

- [ ] **Step 3: Service the queue in deterministic fired-callback order**

Replace `mark_o3_writeback_wake_fired` with this order:

```rust
pub fn mark_o3_writeback_wake_fired(&self, now: Tick) {
    let fetch_events = self.core.fetch_events();
    let mut state = self.state.lock().expect("riscv core lock");
    state.o3_runtime.prune_writeback_calendar_before(now);
    state.o3_writeback_wake.mark_fired(now);
    if state
        .o3_runtime
        .pending_data_address_wake_tick()
        .is_some_and(|tick| tick <= now)
    {
        state.wake_ready_o3_data_access_younger_window(now, &fetch_events);
    }
    let hart = state.hart.clone();
    if let Err(error) = state.o3_runtime.service_live_issue_queue_at(&hart, now) {
        state
            .pending_callback_error
            .get_or_insert(RiscvCpuError::O3Runtime(error));
    }
    state.refresh_o3_writeback_wake(now);
}
```

- [ ] **Step 4: Stop live-window callers from simulating future issue**

Change `schedule_o3_live_speculative_younger_executions` so it only binds packets at `issue_tick` and refreshes the wake:

```rust
fn schedule_o3_live_speculative_younger_executions(
    state: &mut RiscvCoreState,
    younger: &[RiscvCompletedFetchInstruction],
    issue_tick: u64,
) -> Result<bool, RiscvCpuError> {
    let pending_window = state.o3_runtime.has_pending_data_address();
    for younger in younger {
        if !state.o3_runtime.bind_live_staged_issue_packet(
            younger.pc,
            younger.decoded,
            &younger.consumed_requests,
            issue_tick,
        ) {
            if pending_window {
                state.o3_runtime.discard_pending_data_address();
            }
            return Ok(false);
        }
    }
    state.refresh_o3_writeback_wake(issue_tick);
    Ok(true)
}
```

Remove the now-unused head argument from this helper and its call sites. Remove `schedule_live_speculative_issues` entirely. Production callers must not call `service_live_issue_queue_at`; only the O3 wake callback and focused tests may call it.

- [ ] **Step 5: Add source-policy enforcement for sole event ownership**

Assert:

```rust
assert!(!issue.contains("fn schedule_live_speculative_issues("));
assert!(!retire.contains("service_live_issue_queue_at("));
assert_eq!(wake.matches("service_live_issue_queue_at(&hart, now)").count(), 1);
assert_eq!(desired.matches("live_issue_service_tick()").count(), 1);
assert_eq!(wake.matches("desired_o3_writeback_wake(").count(), 2);
assert!(writeback.contains("request_live_issue_after_writeback_change("));
```

Also scan production structs and assert that only `RiscvO3WritebackWakeState` stores scheduler event identity for this path.

- [ ] **Step 6: Run wake, CPU, and scheduler regressions, then commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_writeback_wake_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib writeback_replan_moves_live_issue_wake_earlier -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib service_live_issue_queue_at_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib riscv_live_retire_window -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_writeback_wake_paths_share_live_issue_desired_tick -- --nocapture
git add crates/rem6-cpu/src/riscv_o3_writeback_wake.rs crates/rem6-cpu/src/riscv_o3_writeback_wake/desired.rs crates/rem6-cpu/src/riscv_live_retire_window.rs crates/rem6-cpu/src/o3_runtime_writeback.rs crates/rem6-cpu/src/o3_runtime_issue/state.rs crates/rem6-cpu/src/o3_runtime_issue.rs crates/rem6-cpu/src/o3_runtime_issue/service.rs crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: drive O3 issue through scheduler wake"
```

Expected: all selected tests PASS and no production future-tick driver remains.

### Task 7: Close Cleanup, Checkpoint, Restore, and Handoff Boundaries

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/state.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs:260-560`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_set.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs:282-410`
- Modify: `crates/rem6-cpu/src/o3_runtime_handoff.rs:10-35`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs:800-845`
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs:230-265`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_gate.rs:316-375`
- Modify: `crates/rem6-cpu/src/lib.rs:438-470`
- Modify: focused lifecycle/checkpoint/handoff tests

- [ ] **Step 1: Write lifecycle boundary RED tests**

Add the first three tests to `state_tests.rs`:

```rust
#[test]
fn live_issue_cleanup_retirement_removes_exact_row_before_metadata_finalization() {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::CrossResource);
    fixture.bind_row_at(0, 20);
    let sequence = fixture.sequence(BRANCH_PC);
    let instruction = fixture.rows[0].1;
    let mut hart = fixture.hart.clone();
    hart.set_pc(BRANCH_PC);
    let execution = hart.execute_decoded(decoded(instruction)).unwrap();
    fixture.runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(BRANCH_PC, 11), instruction, execution),
        &[request(11)],
        30,
    );
    assert!(!fixture
        .runtime
        .live_issue
        .resident_sequences()
        .contains(&sequence));
}

#[test]
fn live_issue_cleanup_squash_removes_boundary_and_younger_rows_and_wake() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let first = fixture.sequence(BRANCH_PC);
    let boundary = fixture.sequence(SECOND_PC);
    fixture
        .runtime
        .discard_live_staged_window_from_at(boundary, 30);
    assert_eq!(fixture.runtime.live_issue.resident_sequences(), [first]);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(30));
    assert!(fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .filter(|event| event.action() == O3LiveIssueTraceAction::Squashed)
        .all(|event| event.cleanup_boundary() == Some(boundary)));
}

#[test]
fn live_issue_cleanup_pending_replay_removes_exact_suffix() {
    let mut runtime = O3RuntimeState::default();
    let (_, sequence) = super::queue::stage_queue_pending_row(&mut runtime);
    let older = sequence - 1;
    assert!(runtime.live_issue.enqueue_at(
        older,
        Address::new(BRANCH_PC - 4),
        O3LiveIssueTraceClass::ScalarInteger,
        20,
    ));
    runtime.discard_pending_data_address_from(sequence);
    assert_eq!(runtime.live_issue.resident_sequences(), [older]);
    assert!(runtime.pending_data_addresses.is_empty());
}
```

Add these two tests in `riscv_o3_writeback_wake.rs`, reusing its `core()` helper:

```rust
#[test]
fn live_issue_nonempty_queue_blocks_data_access_lifecycle_quiescence() {
    let core = core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.live_issue.enqueue_at(
            1,
            Address::new(0x8000),
            O3LiveIssueTraceClass::ScalarInteger,
            20,
        ));
    }
    assert!(!core.data_access_lifecycle_is_quiescent());
}

#[test]
fn checkpoint_finalization_seals_stats_only_decision_after_queue_drains() {
    let core = core();
    let projected = {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .live_issue
            .observe_decision_for_test(20, &[1], &[], &[], 1);
        assert!(state.o3_runtime.live_issue_is_quiescent());
        state.o3_runtime.stats()
    };
    core.finalize_quiescent_o3_writeback_for_checkpoint();
    assert_eq!(core.o3_runtime_stats(), projected);
}

#[test]
fn detailed_policy_disable_clears_live_issue_state_and_telemetry() {
    let core = core();
    core.set_detailed_live_retire_gate_enabled(true);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.live_issue.enqueue_at(
            1,
            Address::new(0x8000),
            O3LiveIssueTraceClass::Control,
            20,
        ));
    }
    core.set_detailed_live_retire_gate_enabled(false);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.o3_runtime.live_issue_telemetry(),
        O3LiveIssueTelemetry::default(),
    );
    assert!(state.o3_runtime.live_issue_trace_records().is_empty());
}
```

Add this `#[cfg(test)]` helper to `O3LiveIssueState`; it must call the same internal active-decision update used by service rather than writing counters directly:

```rust
pub(in crate::o3_runtime) fn observe_decision_for_test(
    &mut self,
    tick: u64,
    issued: &[u64],
    resource_blocked: &[u64],
    dependency_blocked: &[u64],
    max_rows_at_tick: usize,
) {
    self.observe_sequences(
        tick,
        issued,
        resource_blocked,
        dependency_blocked,
        max_rows_at_tick,
    );
}
```

Add the restore test to `state_tests.rs`:

```rust
#[test]
fn o3_runtime_restore_clears_live_issue_membership_telemetry_and_wake() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.live_issue.enqueue_at(
        1,
        Address::new(0x8000),
        O3LiveIssueTraceClass::IntegerMulDiv,
        20,
    ));
    let snapshot = runtime.snapshot();
    runtime.restore(snapshot).unwrap();
    assert_eq!(runtime.live_issue_telemetry(), O3LiveIssueTelemetry::default());
    assert!(runtime.live_issue_trace_records().is_empty());
    assert!(runtime.live_issue_is_quiescent());
}
```

Add the handoff test to `o3_runtime_handoff.rs` tests:

```rust
#[test]
fn live_scalar_memory_handoff_rejects_nonempty_issue_queue_without_mutation() {
    for with_live_data in [false, true] {
        let mut runtime = O3RuntimeState::default();
        if with_live_data {
            let load = scalar_load_event(0x8000, 10, 0x9000);
            assert!(runtime.stage_live_data_access_issue_for_test(
                &load,
                memory_request(20),
                31,
            ));
        }
        assert!(runtime.live_issue.enqueue_at(
            99,
            Address::new(0x8010),
            O3LiveIssueTraceClass::Control,
            32,
        ));
        let before = runtime.clone();
        assert!(runtime.live_scalar_memory_handoff().is_none());
        assert_eq!(runtime, before);
    }
}
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_cleanup_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_nonempty_queue_blocks_data_access_lifecycle_quiescence -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_scalar_memory_handoff_rejects_nonempty_issue_queue -- --nocapture
```

Expected: FAIL because canonical cleanup and quiescence do not yet include live issue state.

- [ ] **Step 2: Add runtime-owned exact, suffix, and clear delegates**

Implement in `state.rs`:

```rust
impl O3RuntimeState {
    fn discard_live_issue_exact_at(
        &mut self,
        sequence: u64,
        action: O3LiveIssueTraceAction,
        now: u64,
    ) {
        if let Some((pc, issue_class)) = self.live_issue_identity(sequence) {
            self.live_issue
                .remove_exact_at(sequence, action, pc, issue_class, now);
        }
        self.rearm_live_issue_after_cleanup(now);
    }

    fn discard_live_issue_suffix_at(
        &mut self,
        boundary: u64,
        action: O3LiveIssueTraceAction,
        now: u64,
    ) {
        let rows = self.live_issue_rows_from(boundary);
        self.live_issue
            .remove_suffix_at(boundary, action, &rows, now);
        self.rearm_live_issue_after_cleanup(now);
    }

    fn rearm_live_issue_after_cleanup(&mut self, now: u64) {
        self.live_issue.clear_requested_service_tick();
        if !self.live_issue.resident_sequences().is_empty() {
            self.live_issue.request_service_at(now);
        }
    }
}
```

This deliberately requests a stale-early current tick for remaining rows rather than retaining a possibly stale-late tick derived from a removed producer.

- [ ] **Step 3: Call cleanup from canonical sequence boundaries**

Wire the delegates before canonical metadata is removed:

- `retire_live_staged_instruction`: exact `Retired` removal after identity validation and before ROB/identity retirement mutation.
- `discard_live_staged_window_rows_from_at`: suffix `Squashed` removal.
- `discard_pending_data_address_at_internal` and `discard_pending_data_address_from`: suffix `Replayed` removal.
- `discard_live_data_access_suffix`, `discard_live_data_access_window_rows`, and terminal memory failure/retry paths: matching suffix `Squashed` removal.
- control redirect/invalidation paths: delegate to the existing live-window suffix function rather than maintaining a second IQ cleanup implementation.
- `discard_live_staged_instructions`, reset/restart/trap fallback, HTM abort, and detailed-policy disable: clear queue state and transient telemetry.

Cleanup is idempotent; a second call finds no resident row and emits no second trace event.

- [ ] **Step 4: Add quiescence and stats-only finalization**

In `RiscvCore::data_access_lifecycle_is_quiescent`, add:

```rust
&& state.o3_runtime.live_issue_is_quiescent()
```

At the start of `finalize_quiescent_o3_writeback_for_checkpoint`, return if `!state.o3_runtime.live_issue_is_quiescent()`. After all existing writeback/wake checks pass, call:

```rust
state.o3_runtime.seal_live_issue_decision();
```

Then finalize writeback reservations and detached wakes as before. A stats-only active decision does not make `live_issue_is_quiescent` false.

- [ ] **Step 5: Keep O3RT v23 and restore transient state empty**

Do not add queue fields to `O3RuntimeCheckpointPayload`. Keep:

```rust
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS: u8 = 23;
const O3_RUNTIME_CHECKPOINT_VERSION: u8 =
    O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS;
```

`O3RuntimeState::restore` assigns `O3LiveIssueState::default()`. Add a test that creates queue telemetry, captures a drained checkpoint, restores it, and asserts:

```rust
assert_eq!(restored.live_issue_telemetry(), O3LiveIssueTelemetry::default());
assert!(restored.live_issue_trace_records().is_empty());
assert!(restored.live_issue_is_quiescent());
```

- [ ] **Step 6: Reject nonempty IQ handoff before mutation**

Add this first guard to `live_scalar_memory_handoff`:

```rust
if !self.live_issue.resident_sequences().is_empty() {
    return None;
}
```

Make `capture_o3_live_data_handoff_status` reject a nonempty issue queue even when no live-data row exists. Keep O3DH schema version 7 and do not add an IQ chunk.

- [ ] **Step 7: Run boundary tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_cleanup_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib checkpoint_finalization_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib o3_runtime_restore_clears_live_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_scalar_memory_handoff_rejects_nonempty_issue_queue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_persistent_live_issue_cleanup -- --nocapture
git add crates/rem6-cpu/src/o3_runtime_issue/state.rs crates/rem6-cpu/src/o3_runtime_live_window.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_memory.rs crates/rem6-cpu/src/o3_runtime_pending_address.rs crates/rem6-cpu/src/o3_runtime_pending_address_set.rs crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_handoff.rs crates/rem6-cpu/src/riscv_execution_mode_handoff.rs crates/rem6-cpu/src/riscv_o3_writeback_wake.rs crates/rem6-cpu/src/riscv_live_retire_gate.rs crates/rem6-cpu/src/lib.rs crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: enforce persistent IQ lifecycle boundaries"
```

Expected: all selected tests PASS; live checkpoint and nonempty handoff fail before mutation.

### Task 8: Publish Queue Telemetry Through JSON, Text, and m5 Stats

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime.rs:1060-1095`
- Modify: `crates/rem6-cpu/src/public_api.rs:80-92`
- Modify: `crates/rem6/src/core_summary.rs:100-125`
- Modify: `crates/rem6/src/run_execution_summary.rs:455-480`
- Modify: `crates/rem6/src/core_summary_json.rs:180-210`
- Modify: `crates/rem6/src/stats_output/o3_runtime.rs:920-960`
- Modify: `crates/rem6/src/stats_output/o3_runtime_issue.rs`
- Modify: `crates/rem6-system/src/riscv_run_stats.rs:235-270`
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats.rs:1-170`
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs`
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs`
- Modify: `crates/rem6-system/src/host.rs`
- Modify: output-focused unit tests

- [ ] **Step 1: Write output RED tests for the stable schema**

Add unit tests asserting these exact paths:

```text
/cores/0/o3_runtime/issue/queue/enqueued_rows
/cores/0/o3_runtime/issue/queue/service_turns
/cores/0/o3_runtime/issue/queue/wake_requests
/cores/0/o3_runtime/issue/queue/current_occupancy
/cores/0/o3_runtime/issue/queue/peak_occupancy
/cores/0/o3_runtime/issue/queue/issued_by_class/scalar_integer
/cores/0/o3_runtime/issue/queue/issued_by_class/integer_mul_div
/cores/0/o3_runtime/issue/queue/issued_by_class/memory_agu
/cores/0/o3_runtime/issue/queue/issued_by_class/control
```

Add registry tests for:

```text
sim.cpu0.o3.issue_queue.enqueued_rows
sim.cpu0.o3.issue_queue.service_turns
sim.cpu0.o3.issue_queue.wake_requests
sim.cpu0.o3.issue_queue.current_occupancy
sim.cpu0.o3.issue_queue.peak_occupancy
sim.cpu0.o3.issue_queue.issued_by_class.scalar_integer
sim.cpu0.o3.issue_queue.issued_by_class.integer_mul_div
sim.cpu0.o3.issue_queue.issued_by_class.memory_agu
sim.cpu0.o3.issue_queue.issued_by_class.control
```

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 core_summary_json_o3_issue_queue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system o3_issue_queue_stats -- --nocapture
```

Expected: FAIL because telemetry is not wired into summaries or registries.

- [ ] **Step 2: Expose telemetry from `RiscvCore` and summary construction**

Add:

```rust
pub fn o3_runtime_live_issue_telemetry(&self) -> O3LiveIssueTelemetry {
    self.with_o3_runtime(|runtime| runtime.live_issue_telemetry())
}

pub fn o3_runtime_live_issue_trace_records(&self) -> Vec<O3LiveIssueTraceRecord> {
    self.with_o3_runtime(|runtime| runtime.live_issue_trace_records().to_vec())
}
```

Extend the existing `public_api.rs` re-export to this exact list so `rem6` and `rem6-system` import the evidence types from `rem6_cpu`:

```rust
pub use crate::o3_runtime::{
    O3LiveIssueTelemetry, O3LiveIssueTraceAction, O3LiveIssueTraceClass,
    O3LiveIssueTraceRecord, O3LoadStoreQueueEntry, O3LoadStoreQueueKind,
    O3RenameMapEntry, O3ReorderBufferEntry, O3RuntimeCheckpointPayload, O3RuntimeError,
    O3RuntimeSnapshot, O3RuntimeStats, O3RuntimeWritebackReservation,
    RiscvO3WritebackDebugState,
};
```

Add `o3_runtime_live_issue_telemetry: O3LiveIssueTelemetry` to `Rem6CoreSummary` and populate it next to `o3_runtime` in `run_execution_summary.rs`.

- [ ] **Step 3: Add structured JSON under the existing issue object**

Build a `queue` object in `o3_runtime_issue_json`:

```rust
let queue = summary.o3_runtime_live_issue_telemetry;
let queue = format!(
    "{{\"enqueued_rows\":{},\"service_turns\":{},\"wake_requests\":{},\"current_occupancy\":{},\"peak_occupancy\":{},\"issued_by_class\":{{\"scalar_integer\":{},\"integer_mul_div\":{},\"memory_agu\":{},\"control\":{}}}}}",
    queue.enqueued_rows(),
    queue.service_turns(),
    queue.wake_requests(),
    queue.current_occupancy(),
    queue.peak_occupancy(),
    queue.scalar_integer_issued_rows(),
    queue.integer_mul_div_issued_rows(),
    queue.memory_agu_issued_rows(),
    queue.control_issued_rows(),
);
```

Append `"queue":{queue}` to the existing issue object. Timing mode already omits the full O3 runtime object; preserve that suppression.

- [ ] **Step 4: Extend final text/JSON stats emission**

Change the focused emitter signature to:

```rust
pub(super) fn emit_o3_runtime_issue_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    o3: O3RuntimeStats,
    queue: O3LiveIssueTelemetry,
) -> Result<(), Rem6CliError>
```

Keep existing counters and add the nine stable queue paths. Use `StatResetPolicy::Resettable` for every queue value because the snapshot is explicitly rebased by `m5_reset_stats`; current and peak occupancy are emitted from the rebased snapshot rather than accumulated across epochs.

- [ ] **Step 5: Extend the rem6-system live stats bridge**

Import `O3LiveIssueTelemetry`, add nine `StatId` fields to `RiscvO3RuntimeCpuStats`, and register them under `issue_queue.*`.

Add telemetry parameters to `record_cpu_snapshot`, `sync_cpu_snapshot`, `increment_delta`, and `set_snapshot`. Every caller obtains the current value from:

```rust
let live_issue = core.o3_runtime_live_issue_telemetry();
```

For `increment_delta`, use counter deltas for enqueued/service/wake/class-issued values and direct snapshot assignment for current/peak occupancy. Store a separate previous telemetry map in `RiscvO3RuntimeStats`; clear or rebase it on stats reset exactly where the existing `previous` O3 stats map is reset.

- [ ] **Step 6: Prove telemetry is transient and reset-scoped**

Add tests that:

- reset stats while one row remains resident;
- assert activity counters return to zero;
- assert current and peak occupancy both equal the resident count;
- assert the requested issue wake is unchanged;
- restore a checkpoint and assert all queue telemetry is zero;
- inspect checkpoint encoding/source and assert no `O3LiveIssueTelemetry` field is present.

- [ ] **Step 7: Run output and stats tests, then commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --lib live_issue_state_stats_reset -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 core_summary_json_o3_issue_queue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 stats_output_o3_runtime_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-system o3_issue_queue_stats -- --nocapture
git add crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/public_api.rs crates/rem6/src/core_summary.rs crates/rem6/src/run_execution_summary.rs crates/rem6/src/core_summary_json.rs crates/rem6/src/stats_output/o3_runtime.rs crates/rem6/src/stats_output/o3_runtime_issue.rs crates/rem6-system/src/riscv_run_stats.rs crates/rem6-system/src/riscv_o3_runtime_stats.rs crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs crates/rem6-system/src/host.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: expose persistent IQ telemetry"
```

Expected: all selected tests PASS and O3RT remains version 23.

### Task 9: Add Queue Lifecycle Debug Evidence

**Files:**
- Create: `crates/rem6/src/debug_output/o3_issue_queue_json.rs`
- Modify: `crates/rem6/src/debug_output/o3.rs:1-330`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/state.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/service.rs`
- Modify: `crates/rem6/src/debug_output` focused tests

- [ ] **Step 1: Write debug JSON RED tests**

Add a focused test constructing queued, selected, retained, replayed, squashed, and retired records and assert this exact JSON shape:

```json
{
  "telemetry": {
    "enqueued_rows": 4,
    "service_turns": 3,
    "wake_requests": 3,
    "current_occupancy": 0,
    "peak_occupancy": 4,
    "issued_by_class": {
      "scalar_integer": 1,
      "integer_mul_div": 1,
      "memory_agu": 1,
      "control": 1
    }
  },
  "events": []
}
```

Each event must contain `sequence`, `pc`, `action`, `issue_class`, `service_tick`, `next_wake_tick`, `raw_writeback_tick`, `admitted_writeback_tick`, and `cleanup_boundary`.

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 o3_issue_queue_debug_json -- --nocapture
```

Expected: FAIL because the serializer and trace-record plumbing are absent.

- [ ] **Step 2: Record lifecycle actions at their authority points**

Use state-owned trace records only as evidence:

- enqueue records `Queued` with admission tick;
- successful transaction records `Selected` with raw/admitted writeback ticks;
- post-service blocked rows record `RetainedResource` or `RetainedDependency` with next wake;
- pending replay records `Replayed` with exact boundary;
- suffix/control/memory cleanup records `Squashed` with exact boundary;
- retirement cleanup records `Retired` with exact sequence boundary.

When one service turn computes the next wake, write that same value into all selected/retained events emitted for the turn. Trace mutation must not affect arbitration, membership, or wake calculations.

- [ ] **Step 3: Serialize queue evidence in a focused child**

Create `o3_issue_queue_json.rs` with:

```rust
use rem6_cpu::{O3LiveIssueTelemetry, O3LiveIssueTraceRecord};

use crate::formatting::json_escape;

pub(super) fn o3_issue_queue_to_json(
    telemetry: O3LiveIssueTelemetry,
    events: &[O3LiveIssueTraceRecord],
) -> String {
    let events = events
        .iter()
        .map(|event| {
            format!(
                "{{\"sequence\":{},\"pc\":\"{:#x}\",\"action\":\"{}\",\"issue_class\":\"{}\",\"service_tick\":{},\"next_wake_tick\":{},\"raw_writeback_tick\":{},\"admitted_writeback_tick\":{},\"cleanup_boundary\":{}}}",
                event.sequence(),
                event.pc().get(),
                json_escape(event.action().name()),
                json_escape(event.issue_class().name()),
                event.service_tick(),
                optional_u64_json(event.next_wake_tick()),
                optional_u64_json(event.raw_writeback_tick()),
                optional_u64_json(event.admitted_writeback_tick()),
                optional_u64_json(event.cleanup_boundary()),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"telemetry\":{},\"events\":[{}]}}", telemetry_json(telemetry), events)
}
```

Implement local `optional_u64_json` and `telemetry_json` helpers in the same file.

- [ ] **Step 4: Extend `Rem6O3TraceRecord` and suppress timing mode**

Add telemetry and issue events to the record, constructor, and `to_json`. Publish telemetry below `/debug/o3_trace/0/issue_queue/telemetry`, including the exact leaf `/debug/o3_trace/0/issue_queue/telemetry/peak_occupancy`. Publish lifecycle rows in `/debug/o3_trace/0/issue_queue/events`, including the exact leaf `/debug/o3_trace/0/issue_queue/events/0/action`.

In `o3_trace_records`, determine execution mode before reading queue evidence. Read telemetry/events only when `execution_mode == Some("detailed")`; otherwise use defaults and an empty vector. Timing-from-start and post-switch timing mode therefore expose no detailed queue surface.

- [ ] **Step 5: Run debug tests and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 o3_issue_queue_debug_json -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 debug_output_o3 -- --nocapture
git add crates/rem6-cpu/src/o3_runtime_issue/state.rs crates/rem6-cpu/src/o3_runtime_issue/service.rs crates/rem6/src/debug_output/o3.rs crates/rem6/src/debug_output/o3_issue_queue_json.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: trace persistent IQ lifecycle"
```

Expected: all selected tests PASS and timing-mode records contain no queue object.

### Task 10: Add the Real-Binary Persistent-IQ Matrix

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue/general_iq.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/general_iq.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`

- [ ] **Step 1: Attach the focused CLI owner and move reusable helpers**

Attach from `o3.rs`:

```rust
#[path = "o3/persistent_iq.rs"]
mod persistent_iq;
```

Start `persistent_iq.rs` with:

```rust
use std::collections::BTreeSet;

use super::*;
```

Make the existing general-IQ binary/JSON/event helpers `pub(super)` from `scoped_issue.rs`, and make predicted-control command/checkpoint helpers `pub(super)` from `predicted_control.rs`. Remove the old two-test `scoped_issue/general_iq.rs` and four lifecycle tests from `predicted_control/general_iq.rs` after their renamed versions compile in the new owner.

Expose only the reused nested fixture modules to the `o3` test family:

```rust
// o3/writeback_port.rs
pub(in crate::m5_host_actions::o3) mod dependent_result_address;
pub(in crate::m5_host_actions::o3) mod fixed_fu;

// o3/writeback_port/dependent_result_address.rs
pub(in crate::m5_host_actions::o3) mod two_pending;

// o3/writeback_port/dependent_result_address/two_pending.rs
pub(in crate::m5_host_actions::o3) mod boundaries;
```

Use the same `pub(in crate::m5_host_actions::o3)` visibility for the four extracted JSON fixture functions. Keep all binary builders, constants, and assertion helpers private to their existing focused owner.

- [ ] **Step 2: Add width-one and width-two direct anchors**

Rename and strengthen the existing general-IQ tests:

```rust
#[test]
fn rem6_run_o3_persistent_iq_width_one_oldest_ready_cross_class_direct() {
    assert_persistent_iq_oldest_ready(1);
}

#[test]
fn rem6_run_o3_persistent_iq_width_two_coissues_ready_cross_class_direct() {
    assert_persistent_iq_oldest_ready(2);
}
```

Retain exact final-register, sequence, issue/writeback/commit, and fetch-before-load-issue assertions. Add:

```rust
let queue = json.pointer("/cores/0/o3_runtime/issue/queue").unwrap();
assert!(queue.pointer("/enqueued_rows").and_then(Value::as_u64).unwrap() >= 4);
assert!(queue.pointer("/service_turns").and_then(Value::as_u64).unwrap() >= 2);
assert_eq!(queue.pointer("/current_occupancy").and_then(Value::as_u64), Some(0));
assert_eq!(queue.pointer("/issued_by_class/integer_mul_div").and_then(Value::as_u64), Some(2));
assert_eq!(queue.pointer("/issued_by_class/scalar_integer").and_then(Value::as_u64), Some(2));
```

- [ ] **Step 3: Add width-four hierarchy and wakeup matrix anchors**

Adapt the existing `rem6_run_o3_general_iq_pending_address_and_scalar_hierarchy` fixture into:

```rust
#[test]
fn rem6_run_o3_persistent_iq_width_four_respects_class_caps_hierarchy() {
    let json = super::writeback_port::dependent_result_address::two_pending::
        persistent_iq_width_four_hierarchy_json();
    let queue = json
        .pointer("/cores/0/o3_runtime/issue/queue")
        .expect("persistent-IQ hierarchy queue summary");
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/issue/configured_width")
            .and_then(Value::as_u64),
        Some(4),
    );
    assert!(queue.pointer("/issued_by_class/scalar_integer").and_then(Value::as_u64).unwrap() > 0);
    assert!(queue.pointer("/issued_by_class/integer_mul_div").and_then(Value::as_u64).unwrap() > 0);
    assert!(queue.pointer("/issued_by_class/memory_agu").and_then(Value::as_u64).unwrap() > 0);
    assert!(queue.pointer("/issued_by_class/control").and_then(Value::as_u64).unwrap() > 0);
}
```

Extract `persistent_iq_width_four_hierarchy_json` from the old hierarchy test. It must run `TWO_PENDING_ROWS[1]`, retain `assert_two_pending_resident` and `assert_two_pending_completed`, add one independent multiply and one supported control row to that fixture, set issue width four, and return the completed JSON after its existing exact memory-byte and ordered-retirement assertions.

Add:

```rust
#[test]
fn rem6_run_o3_persistent_iq_cross_class_wakeup_matrix_direct() {
    let general_path = general_iq_oldest_ready_binary("o3-persistent-iq-wakeup-general");
    let producer_and_resource = general_iq_oldest_ready_json(&general_path, 1, 4_000);
    let replanned = super::writeback_port::persistent_iq_writeback_replan_json();
    let pending = super::writeback_port::dependent_result_address::two_pending::
        persistent_iq_width_four_hierarchy_json();

    assert_queue_wakeup_transition(
        &producer_and_resource,
        "retained_dependency",
        "scalar_integer",
    );
    assert_queue_wakeup_transition(
        &producer_and_resource,
        "retained_resource",
        "integer_mul_div",
    );
    assert_queue_wakeup_transition(&replanned, "retained_dependency", "scalar_integer");
    assert_queue_wakeup_transition(&pending, "retained_resource", "memory_agu");
}

fn assert_queue_wakeup_transition(json: &Value, retained_action: &str, issue_class: &str) {
    let events = json
        .pointer("/debug/o3_trace/0/issue_queue/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing persistent-IQ events: {json}"));
    let retained = events
        .iter()
        .find(|event| {
            event.pointer("/action").and_then(Value::as_str) == Some(retained_action)
                && event.pointer("/issue_class").and_then(Value::as_str) == Some(issue_class)
        })
        .unwrap_or_else(|| panic!("missing {retained_action}/{issue_class}: {json}"));
    let sequence = retained.pointer("/sequence").and_then(Value::as_u64).unwrap();
    let wake = retained
        .pointer("/next_wake_tick")
        .and_then(Value::as_u64)
        .expect("retained row next wake");
    let selected = events
        .iter()
        .find(|event| {
            event.pointer("/action").and_then(Value::as_str) == Some("selected")
                && event.pointer("/sequence").and_then(Value::as_u64) == Some(sequence)
        })
        .unwrap_or_else(|| panic!("missing later selection for sequence {sequence}: {json}"));
    assert_eq!(
        selected.pointer("/service_tick").and_then(Value::as_u64),
        Some(wake),
    );
}
```

Expose `persistent_iq_writeback_replan_json` from `writeback_port.rs` by running the existing width-one fixed-FU/load collision fixture that forces a reservation replan and returning its completed JSON after the current writeback assertions. The helper above proves each retained row selects exactly at its advertised wake; the existing wake-owner unit tests prove deduplication, and the same-run stats assertions from Task 5 prove repeated reads do not inflate counters.

- [ ] **Step 4: Add squash/replay cleanup evidence**

Create a predicted wrong-path suffix containing a multiply row and control row plus a separate pending-address replay fixture:

```rust
#[test]
fn rem6_run_o3_persistent_iq_squash_discards_wrong_path_queue_suffix() {
    let (wrong_path, boundary) =
        super::writeback_port::fixed_fu::persistent_iq_wrong_path_cleanup_json();
    let replay = super::writeback_port::dependent_result_address::two_pending::boundaries::
        persistent_iq_first_replay_json();

    let squashed = queue_events(&wrong_path)
        .iter()
        .filter(|event| event.pointer("/action").and_then(Value::as_str) == Some("squashed"))
        .collect::<Vec<_>>();
    assert!(squashed.len() >= 2);
    assert!(squashed.iter().all(|event| {
        event.pointer("/cleanup_boundary").and_then(Value::as_u64) == Some(boundary)
    }));
    assert!(squashed.iter().any(|event| {
        event.pointer("/issue_class").and_then(Value::as_str) == Some("integer_mul_div")
    }));
    assert!(squashed.iter().any(|event| {
        matches!(
            event.pointer("/issue_class").and_then(Value::as_str),
            Some("control") | Some("scalar_integer")
        )
    }));

    let replayed = queue_events(&replay)
        .iter()
        .find(|event| event.pointer("/action").and_then(Value::as_str) == Some("replayed"))
        .expect("pending-address replay event");
    let replay_boundary = replayed
        .pointer("/cleanup_boundary")
        .and_then(Value::as_u64)
        .expect("replay cleanup boundary");
    assert_eq!(
        replayed.pointer("/sequence").and_then(Value::as_u64),
        Some(replay_boundary),
    );
}

fn queue_events(json: &Value) -> &[Value] {
    json.pointer("/debug/o3_trace/0/issue_queue/events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("missing persistent-IQ events: {json}"))
}
```

Extract `persistent_iq_wrong_path_cleanup_json` from the existing wrong-path writeback test and return the final JSON plus the mispredicted branch sequence used as the cleanup boundary. Extract `persistent_iq_first_replay_json` from the existing first-replay boundary test and return its completed JSON after preserving all architectural, request-count, and transport-suppression assertions.

Assert:

- older architectural result remains;
- wrong-path final registers and memory remain unchanged;
- no wrong-path Data/Memory transport event occurs after squash;
- queue trace contains `squashed` for both wrong-path classes with the same boundary;
- replay trace contains `replayed` at the exact pending sequence;
- current occupancy reaches zero and no later selected event names a removed sequence.

- [ ] **Step 5: Add text, dump, and debug anchors**

Reuse the existing scoped-issue text and `m5_dump_stats` programs, renaming tests to:

```rust
rem6_run_o3_persistent_iq_text_stats_expose_queue_counters
rem6_run_o3_persistent_iq_stats_dump_exposes_queue_counters
rem6_run_o3_persistent_iq_debug_exposes_residency_and_cleanup
```

Text must contain the nine `sim.cpu0.o3.issue_queue.*` paths. The dump must contain the same suffixes under `sim.host_actions.stats_dump.cpu0.o3.issue_queue.*`. Debug must contain queued, retained, selected, and cleanup actions with exact sequence/PC/tick/class fields from one run.

Use this descriptor list in `persistent_iq.rs`:

```rust
const PERSISTENT_IQ_QUEUE_STATS: [(&str, &str); 9] = [
    ("enqueued_rows", "enqueued_rows"),
    ("service_turns", "service_turns"),
    ("wake_requests", "wake_requests"),
    ("current_occupancy", "current_occupancy"),
    ("peak_occupancy", "peak_occupancy"),
    ("issued_by_class/scalar_integer", "issued_by_class.scalar_integer"),
    ("issued_by_class/integer_mul_div", "issued_by_class.integer_mul_div"),
    ("issued_by_class/memory_agu", "issued_by_class.memory_agu"),
    ("issued_by_class/control", "issued_by_class.control"),
];
```

Append this exact text assertion loop to the renamed text test:

```rust
let queue = json.pointer("/cores/0/o3_runtime/issue/queue").unwrap();
for (json_field, stat_field) in PERSISTENT_IQ_QUEUE_STATS {
    let value = queue
        .pointer(&format!("/{json_field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing queue field {json_field}: {queue}"));
    let path = format!("sim.cpu0.o3.issue_queue.{stat_field}");
    assert_text_count_stat(&stdout, &path, value);
    assert_text_stat_occurs_once(&stdout, &path);
}
```

Append this exact dump assertion loop to the renamed dump test:

```rust
let queue = json.pointer("/cores/0/o3_runtime/issue/queue").unwrap();
for (json_field, stat_field) in PERSISTENT_IQ_QUEUE_STATS {
    let value = queue
        .pointer(&format!("/{json_field}"))
        .and_then(Value::as_u64)
        .unwrap();
    assert_stats_dump_sample(
        dump,
        &format!("sim.host_actions.stats_dump.cpu0.o3.issue_queue.{stat_field}"),
        "counter",
        "Count",
        value,
        "resettable",
    );
}
```

The debug test must call `queue_events`, collect action names, and assert:

```rust
let actions = queue_events(&json)
    .iter()
    .filter_map(|event| event.pointer("/action").and_then(Value::as_str))
    .collect::<BTreeSet<_>>();
for required in ["queued", "selected", "retained_resource", "retained_dependency"] {
    assert!(actions.contains(required), "missing {required}: {json}");
}
assert!(
    actions.contains("squashed") || actions.contains("replayed") || actions.contains("retired"),
    "missing cleanup action: {json}",
);
for event in queue_events(&json) {
    assert!(event.pointer("/sequence").and_then(Value::as_u64).is_some());
    assert!(event.pointer("/pc").and_then(Value::as_str).is_some());
    assert!(event.pointer("/service_tick").and_then(Value::as_u64).is_some());
    assert!(event.pointer("/issue_class").and_then(Value::as_str).is_some());
}
```

- [ ] **Step 6: Rename and strengthen handoff/checkpoint/timing anchors**

Move the existing positive lifecycle tests under these names:

```rust
rem6_run_host_switch_preserves_o3_persistent_iq_ticks
rem6_run_o3_persistent_iq_checkpoint_boundary
rem6_run_timing_suppresses_o3_persistent_iq_surface
```

Keep the positive switch after the IQ drains but while the scalar live-data row is resident. In the renamed switch test, preserve the existing transfer-chunk and exact inherited-tick assertions, then add this rejection before the successful switch:

```rust
let live_iq_tick = event_u64(load, "issue_tick") + 1;
let queued_before_switch = queue_events(&baseline).iter().any(|event| {
    event.pointer("/action").and_then(Value::as_str) == Some("queued")
        && event
            .pointer("/service_tick")
            .and_then(Value::as_u64)
            .is_some_and(|tick| tick <= live_iq_tick)
        && !queue_events(&baseline).iter().any(|later| {
            later.pointer("/sequence") == event.pointer("/sequence")
                && later.pointer("/action").and_then(Value::as_str) == Some("selected")
                && later
                    .pointer("/service_tick")
                    .and_then(Value::as_u64)
                    .is_some_and(|tick| tick <= live_iq_tick)
        })
});
assert!(queued_before_switch, "fixture must have a resident IQ row at {live_iq_tick}: {baseline}");

let artifact = temp_output("o3-persistent-iq-live-switch.json");
let switch_arg = format!("{live_iq_tick}:cpu0:timing");
let mut command = predicted_control_command(&path, "direct", 1_500, "detailed");
command.args([
    "--host-switch-cpu-mode",
    switch_arg.as_str(),
    "--output",
    artifact.to_str().unwrap(),
]);
let output = command.output().unwrap();
assert_eq!(output.status.code(), Some(2), "live IQ switch: {output:?}");
assert!(output.stdout.is_empty(), "live IQ switch: {output:?}");
assert_eq!(
    String::from_utf8(output.stderr).unwrap(),
    "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
);
assert!(!artifact.exists(), "live IQ switch emitted {}", artifact.display());
```

Use the existing `switch_tick = event_u64(event_at_pc(&baseline, ADD_PC), "issue_tick") + 1` for the successful transfer. Assert `switch_tick < event_u64(load, "lsq_data_response_tick")`, O3DH schema 7, one resident live-data row, three younger rows, and exact issue/writeback/commit equality for `LOAD_PC`, `BRANCH_PC`, `MUL_PC`, and `ADD_PC` against the baseline.

In the renamed checkpoint test, reject the same nonempty-IQ tick with the exact fail-closed contract:

```rust
let live_iq_tick = event_u64(event_at_pc(&baseline, LOAD_PC), "issue_tick") + 1;
let checkpoint_arg = format!("{live_iq_tick}:persistent-iq-live");
let artifact = temp_output("o3-persistent-iq-live-checkpoint.json");
let mut command = predicted_control_command(&path, "direct", 1_500, "detailed");
command.args([
    "--host-checkpoint",
    checkpoint_arg.as_str(),
    "--output",
    artifact.to_str().unwrap(),
]);
let output = command.output().unwrap();
assert_eq!(output.status.code(), Some(2), "live IQ checkpoint: {output:?}");
assert!(output.stdout.is_empty(), "live IQ checkpoint: {output:?}");
assert_eq!(
    String::from_utf8(output.stderr).unwrap(),
    "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n",
);
assert!(
    !artifact.exists(),
    "live IQ checkpoint emitted {}",
    artifact.display(),
);
```

Capture and restore after `ADD_PC` commits, preserving the old register and empty ROB/LSQ assertions. Add:

```rust
assert_eq!(
    runtime.pointer("/checkpoint_version").and_then(Value::as_u64),
    Some(23),
);
assert_eq!(
    restored
        .pointer("/host_actions/checkpoint_restored_count")
        .and_then(Value::as_u64),
    Some(1),
);
let queue = restored
    .pointer("/cores/0/o3_runtime/issue/queue")
    .expect("post-restore transient queue telemetry");
for (field, expected) in [
    ("enqueued_rows", 0),
    ("service_turns", 0),
    ("wake_requests", 0),
    ("current_occupancy", 0),
    ("peak_occupancy", 0),
    ("issued_by_class/scalar_integer", 0),
    ("issued_by_class/integer_mul_div", 0),
    ("issued_by_class/memory_agu", 0),
    ("issued_by_class/control", 0),
] {
    assert_eq!(
        queue.pointer(&format!("/{field}")).and_then(Value::as_u64),
        Some(expected),
        "post-restore queue field {field}: {queue}",
    );
}
```

The renamed timing test keeps the architectural register assertions and adds exact queue-surface suppression:

```rust
assert!(timing.pointer("/cores/0/o3_runtime/issue/queue").is_none());
assert!(timing.pointer("/debug/o3_trace/0/issue_queue").is_none());
let leaked_queue_stats = timing
    .pointer("/stats")
    .and_then(Value::as_array)
    .expect("timing stats")
    .iter()
    .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
    .filter(|path| path.starts_with("sim.cpu0.o3.issue_queue."))
    .collect::<Vec<_>>();
assert!(
    leaked_queue_stats.is_empty(),
    "timing mode leaked persistent-IQ stats: {leaked_queue_stats:?}",
);
```

- [ ] **Step 7: Register all eleven anchors and turn policy green**

Add these exact lines to `core_test_anchors.txt`:

```text
rem6_run_o3_persistent_iq_width_one_oldest_ready_cross_class_direct
rem6_run_o3_persistent_iq_width_two_coissues_ready_cross_class_direct
rem6_run_o3_persistent_iq_width_four_respects_class_caps_hierarchy
rem6_run_o3_persistent_iq_cross_class_wakeup_matrix_direct
rem6_run_o3_persistent_iq_squash_discards_wrong_path_queue_suffix
rem6_run_o3_persistent_iq_text_stats_expose_queue_counters
rem6_run_o3_persistent_iq_stats_dump_exposes_queue_counters
rem6_run_o3_persistent_iq_debug_exposes_residency_and_cleanup
rem6_run_host_switch_preserves_o3_persistent_iq_ticks
rem6_run_o3_persistent_iq_checkpoint_boundary
rem6_run_timing_suppresses_o3_persistent_iq_surface
```

Extend `o3_persistent_iq_ownership.rs` to assert every anchor is defined exactly once in `persistent_iq.rs`, the old `o3_general_iq` lifecycle anchor names are absent, and the owner remains at or below 900 lines.

Remove `#[ignore = "RED until Task 10 creates the persistent-IQ CLI owner"]` from `o3_persistent_iq_focused_owners_exist_and_stay_bounded` before running the green policy command.

- [ ] **Step 8: Run the real-binary matrix and commit**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_persistent_iq_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_host_switch_preserves_o3_persistent_iq_ticks -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_persistent_iq_surface -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_persistent_iq -- --nocapture
git add crates/rem6/tests/cli_run/m5_host_actions/o3.rs crates/rem6/tests/cli_run/m5_host_actions/o3/persistent_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue/general_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/general_iq.rs crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending.rs crates/rem6/tests/source_policy/core_test_anchors.txt crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "test: add persistent O3 issue queue matrix"
```

Expected: all eleven real-binary anchors PASS and the focused ownership RED from Task 1 is now green.

### Task 11: Lock Source Policy, Update the Ledger, Review, and Verify

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Complete persistent-IQ ownership policy**

The final CPU policy must assert all of the following:

```text
O3RuntimeState stores exactly one O3LiveIssueState.
No other production type stores a second resident sequence inventory.
O3LiveIssuePacket remains stored only by O3LiveStagedFetchIdentity.
O3LiveIssueQueue materializes resident sequences and does not scan the full ROB for discovery.
O3LiveIssueDependencyTable and O3LiveIssueCalendar are not persistent fields.
service_live_issue_queue_at contains no later-tick loop.
record_live_issue_batch contains no self.clone() or O3RuntimeState clone.
Both wake entry points call desired_o3_writeback_wake.
Only mark_o3_writeback_wake_fired calls service_live_issue_queue_at in production.
O3RuntimeCheckpointPayload contains neither O3LiveIssueState nor O3LiveIssueTelemetry.
O3RT current version remains 23 and O3DH current version remains 7.
Canonical retirement, suffix, replay, memory, reset, and teardown paths delegate queue cleanup.
All new production and test files stay within the approved caps.
```

Use source parsing helpers already present in `source_policy.rs`; do not use raw substring checks where the file already has exact item-definition helpers.

- [ ] **Step 2: Update CPU evidence without changing score or cap**

In the CPU component prose, replace the limitation phrase:

```text
persistent and cross-class IQ/wakeup/select beyond the derived scalar/control/capacity-three-pending-address live queue
```

with:

```text
FP/vector arithmetic and system issue rows, a general load/store queue scheduler, dependent stores or arbitrary atomics, arbitrary nonadjacent or unbounded dependency graphs, checkpoint-restorable live IQ/transport state, and a general O3 engine
```

Replace the scoped-issue evidence sentence with a sentence that names the bounded per-run persistent cross-class queue, scheduler-turn wakeup/select, widths 1/2/4, direct/hierarchy routes, same-tick projection, bounded transaction, queue telemetry/debug, empty-IQ handoff, live checkpoint rejection, drained v23 restore, timing suppression, and the eleven new anchors. Keep `8 of 10`, raw `80%`, and the `74% representative` cap unchanged.

- [ ] **Step 3: Update Stats evidence without changing score or cap**

In the Stats component and O3 host-action note, add the queue JSON/text/dump/debug paths and state explicitly that telemetry is transient and absent from O3RT v23. Keep `24 of 26`, raw `92%`, and the `74% representative` cap unchanged.

Edit existing lines in place so the ledger remains exactly 1,200 lines.

- [ ] **Step 4: Run formatting and focused policy verification**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_persistent_live_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_writeback_wake_paths_share_live_issue_desired_tick -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_persistent_iq -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy gem5_migration_sections_are_auditable -- --nocapture
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
```

Expected: every command exits 0.

- [ ] **Step 5: Run full workspace verification**

```bash
TMPDIR=$PWD/target/tmp cargo test --workspace --all-targets
```

Expected: all workspace targets PASS with zero failures.

- [ ] **Step 6: Run a high-intensity read-only review**

Dispatch one fresh read-only reviewer over the final diff. Require checks for:

```text
No future issue tick is recorded before its scheduler turn.
No stale-late wake can hide ready bandwidth.
Same-tick stats reads and reset baselines are idempotent.
Rollback covers every owner mutated by writeback replan and selected recording.
Cleanup removes membership, wake, blocked classification, occupancy, and trace authority together.
Checkpoint/handoff rejection occurs before mutation.
Telemetry/debug evidence is real-path derived and timing-suppressed.
Source policy and ledger claims match executable evidence and do not raise either 74% cap.
```

Resolve every Critical or Important finding, rerun the affected focused tests, then rerun the full workspace command.

- [ ] **Step 7: Commit, push, and verify remote parity**

```bash
git add crates/rem6-cpu/tests/source_policy.rs crates/rem6/tests/source_policy/o3_persistent_iq_ownership.rs docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp git commit -m "docs: record persistent O3 issue queue evidence"
git push origin o3-persistent-cross-class-issue-queue
git status --short --branch
git rev-parse HEAD
git rev-parse origin/o3-persistent-cross-class-issue-queue
```

Expected: the worktree is clean and the two hashes match.
