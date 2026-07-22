# RISC-V O3 Live Issue Calendar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract detailed RISC-V O3 issue reservations and scoped arbitration into a fresh-per-pass live issue calendar while preserving all existing runtime, stats, replay, checkpoint, and CLI behavior.

**Architecture:** A focused `o3_runtime_issue/calendar.rs` derives per-tick reservations from canonical runtime state, reduces live issue capacities, and wraps `O3ScopedIssueScheduler` results. `o3_runtime_issue.rs` remains the orchestration and mutation owner; it rebuilds the calendar before every arbitration pass so writeback replanning or replay cleanup cannot leave stale local reservations.

**Tech Stack:** Rust workspace, `rem6-cpu` O3 runtime and scoped scheduler, source-policy tests, private unit tests, and real `rem6 run --execute` CLI integration tests.

---

## File Map

- Create `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`: derived reservation inventory, capacity reduction, scoped scheduler invocation, cycle-plan wrapper, and tick-decision aggregation.
- Create `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`: focused private tests for reservation capture, stale-row removal, pending-memory reservation, blocking classification, and same-tick stats aggregation.
- Modify `crates/rem6-cpu/src/o3_runtime_issue.rs`: declare the calendar child, delegate planning, consume typed decisions, and remove duplicate reservation/scheduler ownership.
- Modify `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`: compile the focused calendar test child beside existing issue fixtures.
- Modify `crates/rem6-cpu/tests/source_policy.rs`: add calendar/test caps, require the focused owner, migrate the old scheduler-location assertion, and reject duplicate authority.
- Read-only verification target `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`: retain the existing real CLI matrix without adding a synthetic fixture.
- Read-only documentation target `docs/architecture/gem5-to-rem6-migration.md`: verify it remains exactly 1,200 lines and do not change its score or checklist state.

### Task 1: Add Focused Calendar Tests And Confirm RED

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:12-18`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs:10-16`
- Create: `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`
- Test: `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`

- [ ] **Step 1: Declare the production and focused test modules**

Add the calendar declaration after the dependency child in `o3_runtime_issue.rs`:

```rust
#[path = "o3_runtime_issue/calendar.rs"]
pub(in crate::o3_runtime) mod calendar;
```

Create `o3_runtime_issue/calendar.rs` as a compile-visible skeleton:

```rust
use super::*;
```

Add the focused test declaration after `dependency_scopes` in `o3_runtime_issue_tests.rs`:

```rust
#[path = "o3_runtime_issue/calendar_tests.rs"]
mod calendar;
```

- [ ] **Step 2: Add the failing focused calendar tests**

Create `o3_runtime_issue/calendar_tests.rs` with this complete test module:

```rust
use rem6_isa_riscv::{RiscvExecutionRecord, RiscvInstruction};

use crate::o3_pipeline::{
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueId, O3ScopedReadyInstruction,
};

use super::super::o3_runtime_issue::calendar::{
    O3LiveIssueCalendar, O3LiveIssueTickDecision,
};
use super::*;

const LIVE_QUEUE: O3IssueQueueId = O3IssueQueueId::new(0);

#[test]
fn live_issue_calendar_head_and_recorded_head_consume_one_slot() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let instruction = addi(3, 0, 1);
    let head = O3LiveIssueHeadReservation::for_instruction(1, 20, instruction);
    stage_live_row(&mut runtime, 1, LOAD_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(1, 20, LOAD_PC, instruction));

    let plan = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(2, O3IssueOpClass::Branch)],
        )
        .unwrap();

    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![2]);
    assert!(plan.resource_blocked().is_empty());
}

#[test]
fn live_issue_calendar_rebuild_releases_removed_prior_row() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(1));
    stage_live_row(&mut runtime, 2, BRANCH_PC);
    runtime
        .live_speculative_executions
        .push(live_execution(2, 20, BRANCH_PC, addi(3, 0, 1)));
    let head = O3LiveIssueHeadReservation::for_instruction(1, 10, addi(4, 0, 1));

    let blocked = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(3, O3IssueOpClass::Branch)],
        )
        .unwrap();
    assert_eq!(blocked.reserved_width(), 1);
    assert_eq!(
        blocked
            .resource_blocked()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>(),
        vec![3]
    );

    runtime.snapshot.reorder_buffer.clear();
    let released = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(3, O3IssueOpClass::Branch)],
        )
        .unwrap();
    assert_eq!(released.reserved_width(), 0);
    assert_eq!(released.issued_sequences().collect::<Vec<_>>(), vec![3]);
}

#[test]
fn live_issue_calendar_selected_pending_tick_reserves_memory() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    assert!(runtime.set_window_depths(4, 4));
    let head_event = calendar_load_event(LOAD_PC, 10, 5, 2, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &head_event,
        request(20),
        31,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    ));
    let pending_raw = load_raw(6, 5, 0);
    let pending = O3PendingDataAddressRequest::new(
        request(10),
        calendar_fetch_event(BRANCH_PC, 11, pending_raw),
        vec![request(11)],
        RiscvInstruction::decode_with_length(pending_raw).unwrap(),
        reg(5),
    );
    assert_eq!(
        runtime.stage_pending_data_address_window(
            head_event.fetch().request_id(),
            vec![pending],
            vec![(Address::new(SECOND_PC), addi(7, 5, 8))],
        ),
        2
    );
    runtime.set_pending_data_address_materialized_for_test(
        40,
        calendar_load_event(BRANCH_PC, 11, 6, 5, 0x9100),
    );
    let head = runtime
        .live_data_access_head_reservation(head_event.fetch().request_id())
        .unwrap();

    let plan = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            40,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(99, O3IssueOpClass::Memory)],
        )
        .unwrap();

    assert_eq!(plan.reserved_width(), 1);
    assert_eq!(
        plan.resource_blocked()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>(),
        vec![99]
    );
}

#[test]
fn live_issue_calendar_separates_resource_and_dependency_blocks() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    let head = O3LiveIssueHeadReservation::for_instruction(1, 20, mul(3, 1, 2));
    let unresolved = O3DependencyScopeId::new(7);

    let plan = O3LiveIssueCalendar::capture(&runtime, head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [
                ready(2, O3IssueOpClass::IntMult),
                ready(3, O3IssueOpClass::Branch).with_waits_on([unresolved]),
            ],
        )
        .unwrap();

    assert!(plan.issued().is_empty());
    assert_eq!(
        plan.resource_blocked()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(
        plan.dependency_blocked()
            .iter()
            .map(O3ScopedReadyInstruction::sequence)
            .collect::<Vec<_>>(),
        vec![3]
    );
}

#[test]
fn live_issue_tick_decision_aggregates_same_tick_attempts() {
    let mut first_runtime = O3RuntimeState::default();
    assert!(first_runtime.set_issue_width(2));
    let first_head =
        O3LiveIssueHeadReservation::for_instruction(1, 20, addi(3, 0, 1));
    let first = O3LiveIssueCalendar::capture(&first_runtime, first_head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [
                ready(2, O3IssueOpClass::Branch),
                ready(3, O3IssueOpClass::IntMult),
            ],
        )
        .unwrap();

    let mut second_runtime = O3RuntimeState::default();
    assert!(second_runtime.set_issue_width(2));
    let second_head =
        O3LiveIssueHeadReservation::for_instruction(9, 10, addi(4, 0, 1));
    let second = O3LiveIssueCalendar::capture(&second_runtime, second_head)
        .plan_scoped_at(
            20,
            std::iter::empty::<O3DependencyScopeId>(),
            [ready(4, O3IssueOpClass::IntAlu)],
        )
        .unwrap();

    let mut decision = O3LiveIssueTickDecision::default();
    decision.observe(&first, 1);
    decision.observe(&second, 1);
    let sample = decision.take().unwrap();

    assert_eq!(sample.issued_rows(), 2);
    assert_eq!(sample.resource_blocked_rows(), 0);
    assert_eq!(sample.dependency_blocked_rows(), 0);
    assert_eq!(sample.max_rows_at_tick(), 2);
    assert!(decision.take().is_none());
}

fn ready(sequence: u64, op_class: O3IssueOpClass) -> O3ScopedReadyInstruction {
    O3ScopedReadyInstruction::new(sequence, LIVE_QUEUE, op_class)
}

fn stage_live_row(runtime: &mut O3RuntimeState, sequence: u64, pc: u64) {
    runtime.snapshot.reorder_buffer.push(
        O3ReorderBufferEntry::new(sequence, Address::new(pc), None)
            .with_live_staged_rename_destination(None),
    );
}

fn live_execution(
    sequence: u64,
    issue_tick: u64,
    pc: u64,
    instruction: RiscvInstruction,
) -> O3LiveSpeculativeExecution {
    O3LiveSpeculativeExecution {
        consumed_requests: Vec::new(),
        sequence,
        producer_sequences: Vec::new(),
        issue_tick,
        raw_ready_tick: issue_tick,
        admitted_writeback_tick: issue_tick,
        writeback_slot: None,
        execution: RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            Vec::new(),
            None,
        ),
    }
}

fn calendar_load_event(
    pc: u64,
    sequence: u64,
    rd: u8,
    rs1: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let raw = load_raw(rd, rs1, 0);
    let instruction = RiscvInstruction::decode_with_length(raw)
        .unwrap()
        .instruction();
    let access = MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        calendar_fetch_event(pc, sequence, raw),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            Vec::new(),
            Some(access),
        ),
    )
}

fn calendar_fetch_event(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn load_raw(rd: u8, rs1: u8, offset: i64) -> u32 {
    i_type(offset, rs1, 0b011, rd, 0x03)
}
```

- [ ] **Step 3: Run the focused filter and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --lib live_issue_calendar -- --nocapture
```

Expected: compilation fails because `O3LiveIssueCalendar` and `O3LiveIssueTickDecision` do not yet exist in the calendar skeleton. Do not weaken or remove the tests.

### Task 2: Implement The Derived Calendar Owner

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- Test: `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`

- [ ] **Step 1: Replace the calendar skeleton with the complete focused owner**

Replace `calendar.rs` with:

```rust
use std::collections::{BTreeMap, BTreeSet};

use crate::o3_pipeline::{
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueCapacity, O3IssueQueueId,
    O3ScopedIssuePlan, O3ScopedIssueScheduler, O3ScopedReadyInstruction,
};

use super::super::o3_runtime_control_window::live_issue_op_class;
use super::*;

const LIVE_ISSUE_QUEUE: O3IssueQueueId = O3IssueQueueId::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueCalendar {
    issue_width: usize,
    by_tick: BTreeMap<u64, O3LiveIssueReservations>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueCyclePlan {
    plan: O3ScopedIssuePlan,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueTickDecision {
    issued_rows: usize,
    resource_blocked_rows: usize,
    dependency_blocked_rows: usize,
    max_rows_at_tick: usize,
    observed: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3LiveIssueReservations {
    width: usize,
    int_alu: usize,
    int_mult: usize,
    branch: usize,
    memory: usize,
}

impl O3LiveIssueCalendar {
    pub(in crate::o3_runtime) fn capture(
        runtime: &O3RuntimeState,
        head: O3LiveIssueHeadReservation,
    ) -> Self {
        let mut calendar = Self {
            issue_width: runtime.issue_width,
            by_tick: BTreeMap::new(),
        };
        calendar.reserve(head.issue_tick, head.op_class);

        let pending_ticks = runtime
            .pending_data_addresses
            .iter()
            .filter_map(|pending| pending.selected_issue_tick)
            .collect::<BTreeSet<_>>();
        for tick in pending_ticks {
            calendar.reserve(tick, O3IssueOpClass::Memory);
        }

        for issued in runtime.live_speculative_executions.iter().filter(|issued| {
            issued.sequence != head.sequence
                && runtime.snapshot.reorder_buffer.iter().any(|entry| {
                    entry.is_live_staged() && entry.sequence() == issued.sequence
                })
        }) {
            calendar.reserve(
                issued.issue_tick,
                live_issue_op_class(issued.execution.instruction()),
            );
        }
        calendar
    }

    pub(super) fn plan_at(
        &self,
        tick: u64,
        dependency_table: &O3LiveIssueDependencyTable,
        candidates: &[O3LiveIssueSchedulingCandidate],
    ) -> Result<O3LiveIssueCyclePlan, O3RuntimeError> {
        self.plan_scoped_at(
            tick,
            dependency_table.resolved_scopes_at(tick),
            candidates
                .iter()
                .map(|candidate| dependency_table.scoped_instruction(candidate)),
        )
    }

    pub(in crate::o3_runtime) fn plan_scoped_at<R, I>(
        &self,
        tick: u64,
        resolved_scopes: R,
        ready: I,
    ) -> Result<O3LiveIssueCyclePlan, O3RuntimeError>
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        let reservations = self.by_tick.get(&tick).copied().unwrap_or_default();
        let scheduler = O3ScopedIssueScheduler::new(
            self.issue_width,
            live_issue_capacities_after_reservations(self.issue_width, reservations),
        )
        .expect("configured live O3 issue width is nonzero");
        scheduler
            .try_plan_with_reserved_width(reservations.width, resolved_scopes, ready)
            .map(|plan| O3LiveIssueCyclePlan { plan })
            .map_err(|error| O3RuntimeError::InvalidLiveIssuePlan { error })
    }

    fn reserve(&mut self, tick: u64, op_class: O3IssueOpClass) {
        self.by_tick.entry(tick).or_default().reserve(op_class);
    }
}

impl O3LiveIssueCyclePlan {
    pub(in crate::o3_runtime) fn issued(&self) -> &[O3ScopedReadyInstruction] {
        self.plan.issued()
    }

    pub(in crate::o3_runtime) fn resource_blocked(&self) -> &[O3ScopedReadyInstruction] {
        self.plan.resource_blocked()
    }

    pub(in crate::o3_runtime) fn dependency_blocked(&self) -> &[O3ScopedReadyInstruction] {
        self.plan.dependency_blocked()
    }

    pub(in crate::o3_runtime) const fn reserved_width(&self) -> usize {
        self.plan.reserved_width()
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn issued_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.plan.issued_sequences()
    }
}

impl O3LiveIssueTickDecision {
    pub(in crate::o3_runtime) fn observe(
        &mut self,
        plan: &O3LiveIssueCyclePlan,
        issued_rows: usize,
    ) {
        debug_assert!(issued_rows <= plan.issued().len());
        self.issued_rows = self.issued_rows.saturating_add(issued_rows);
        self.resource_blocked_rows = plan.resource_blocked().len();
        self.dependency_blocked_rows = plan.dependency_blocked().len();
        self.max_rows_at_tick = self
            .max_rows_at_tick
            .max(plan.reserved_width().saturating_add(issued_rows));
        self.observed = true;
    }

    pub(in crate::o3_runtime) fn take(&mut self) -> Option<Self> {
        self.observed.then(|| std::mem::take(self))
    }

    pub(in crate::o3_runtime) const fn issued_rows(self) -> usize {
        self.issued_rows
    }

    pub(in crate::o3_runtime) const fn resource_blocked_rows(self) -> usize {
        self.resource_blocked_rows
    }

    pub(in crate::o3_runtime) const fn dependency_blocked_rows(self) -> usize {
        self.dependency_blocked_rows
    }

    pub(in crate::o3_runtime) const fn max_rows_at_tick(self) -> usize {
        self.max_rows_at_tick
    }
}

impl O3LiveIssueReservations {
    fn reserve(&mut self, op_class: O3IssueOpClass) {
        self.width = self.width.saturating_add(1);
        match op_class {
            O3IssueOpClass::IntAlu => self.int_alu = self.int_alu.saturating_add(1),
            O3IssueOpClass::IntMult => self.int_mult = self.int_mult.saturating_add(1),
            O3IssueOpClass::Branch => self.branch = self.branch.saturating_add(1),
            O3IssueOpClass::Memory => self.memory = self.memory.saturating_add(1),
            O3IssueOpClass::Float | O3IssueOpClass::System => {}
        }
    }
}

fn live_issue_capacities_after_reservations(
    issue_width: usize,
    reservations: O3LiveIssueReservations,
) -> Vec<O3IssueQueueCapacity> {
    [
        (
            O3IssueOpClass::IntAlu,
            issue_width.saturating_sub(reservations.int_alu),
        ),
        (
            O3IssueOpClass::IntMult,
            1_usize.saturating_sub(reservations.int_mult),
        ),
        (
            O3IssueOpClass::Branch,
            1_usize.saturating_sub(reservations.branch),
        ),
        (
            O3IssueOpClass::Memory,
            1_usize.saturating_sub(reservations.memory),
        ),
    ]
    .into_iter()
    .filter(|(_, slots)| *slots != 0)
    .map(|(op_class, slots)| {
        O3IssueQueueCapacity::new(LIVE_ISSUE_QUEUE, op_class, slots)
            .expect("live O3 issue capacities are nonzero")
    })
    .collect()
}
```

- [ ] **Step 2: Run the focused calendar tests and confirm GREEN**

Run:

```bash
cargo test -p rem6-cpu --lib live_issue_calendar -- --nocapture
```

Expected: 5 focused calendar tests pass. Existing issue code still uses its old inline scheduler and reservations, so this task changes no runtime behavior.

- [ ] **Step 3: Run formatting and the existing scoped scheduler unit tests**

Run:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo test -p rem6-cpu --test o3_pipeline scoped_issue -- --nocapture
```

Expected: both commands pass.

- [ ] **Step 4: Commit the isolated calendar owner**

```bash
git add crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar.rs \
  crates/rem6-cpu/src/o3_runtime_issue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs
git commit -m "refactor: add live o3 issue calendar"
```

### Task 3: Add The RED Ownership Policy

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs:1-10,2940-3000`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add calendar production and test caps**

Add beside the existing issue constants:

```rust
const MAX_O3_RUNTIME_ISSUE_CALENDAR_LINES: usize = 450;
const MAX_O3_RUNTIME_ISSUE_CALENDAR_TEST_LINES: usize = 450;
```

- [ ] **Step 2: Add the focused ownership policy**

Add immediately after `o3_runtime_issue_lives_in_focused_module`:

```rust
#[test]
fn o3_live_issue_calendar_owns_reservations_and_arbiter() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let runtime_root = fs::read_to_string(crate_dir.join("src/o3_runtime.rs")).unwrap();
    let issue_path = crate_dir.join("src/o3_runtime_issue.rs");
    let calendar_path = crate_dir.join("src/o3_runtime_issue/calendar.rs");
    let calendar_tests_path = crate_dir.join("src/o3_runtime_issue/calendar_tests.rs");
    let issue = fs::read_to_string(&issue_path).unwrap();
    let calendar = fs::read_to_string(&calendar_path).unwrap();
    let calendar_tests = fs::read_to_string(&calendar_tests_path).unwrap();

    assert!(issue.lines().count() <= MAX_O3_RUNTIME_ISSUE_LINES);
    assert!(calendar.lines().count() <= MAX_O3_RUNTIME_ISSUE_CALENDAR_LINES);
    assert!(
        calendar_tests.lines().count() <= MAX_O3_RUNTIME_ISSUE_CALENDAR_TEST_LINES
    );
    assert!(issue.contains("#[path = \"o3_runtime_issue/calendar.rs\"]"));
    assert!(issue.contains("mod calendar;"));

    for anchor in [
        "struct O3LiveIssueCalendar",
        "struct O3LiveIssueReservations",
        "struct O3LiveIssueCyclePlan",
        "struct O3LiveIssueTickDecision",
        "O3ScopedIssueScheduler::new(",
        "fn live_issue_capacities_after_reservations(",
        "const LIVE_ISSUE_QUEUE",
    ] {
        assert!(
            calendar.contains(anchor),
            "live issue calendar is missing authority `{anchor}`"
        );
    }

    for removed in [
        "fn live_issue_reservations_at(",
        "struct O3LiveIssueReservations",
        "struct O3LiveIssueTickDecision",
        "fn live_issue_capacities_after_reservations(",
        "O3ScopedIssueScheduler::new(",
        "const LIVE_ISSUE_QUEUE",
    ] {
        assert!(
            !issue.contains(removed),
            "src/o3_runtime_issue.rs retains calendar authority `{removed}`"
        );
    }

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if relative == Path::new("src/o3_runtime_issue/calendar.rs")
            || is_test_only_rust_source(relative)
        {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        for removed in [
            "fn live_issue_reservations_at(",
            "fn live_issue_capacities_after_reservations(",
            "pending_data_address_selected_issue_tick_for_reservation",
        ] {
            assert!(
                !source.contains(removed),
                "{} retains live calendar authority `{removed}`",
                relative.display()
            );
        }
    }

    assert!(issue.contains("O3LiveIssueCalendar::capture("));
    assert!(issue.contains(".plan_at("));
    assert!(!runtime_root.contains("live_issue_calendar:"));

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if relative == Path::new("src/o3_runtime_issue/calendar.rs")
            || is_test_only_rust_source(relative)
        {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        assert!(
            !source.contains("O3ScopedIssueScheduler::new("),
            "{} duplicates live scheduler construction",
            relative.display()
        );
    }

    for anchor in [
        "fn live_issue_calendar_head_and_recorded_head_consume_one_slot(",
        "fn live_issue_calendar_rebuild_releases_removed_prior_row(",
        "fn live_issue_calendar_selected_pending_tick_reserves_memory(",
        "fn live_issue_calendar_separates_resource_and_dependency_blocks(",
        "fn live_issue_tick_decision_aggregates_same_tick_attempts(",
    ] {
        assert!(
            calendar_tests.contains(anchor),
            "calendar tests are missing `{anchor}`"
        );
    }
}
```

- [ ] **Step 3: Run the exact ownership policy and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --test source_policy o3_live_issue_calendar_owns_reservations_and_arbiter -- --exact --nocapture
```

Expected: FAIL because `o3_runtime_issue.rs` still defines `live_issue_reservations_at`, `O3LiveIssueReservations`, `O3LiveIssueTickDecision`, capacity reduction, and scheduler construction, while `o3_runtime_issue/pending_address.rs` still exposes the obsolete selected-tick reservation adapter.

### Task 4: Delegate Runtime Arbitration And Remove Duplicate Authority

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs:1-20,240-380,480-622`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs:155-168`
- Modify: `crates/rem6-cpu/tests/source_policy.rs:2940-3000`
- Test: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Narrow the issue-root imports**

Replace the current pipeline import with:

```rust
use crate::o3_pipeline::{O3IssueOpClass, O3ScopedReadyInstruction};
```

Add this calendar import after the child declarations:

```rust
use calendar::{O3LiveIssueCalendar, O3LiveIssueTickDecision};
```

Keep `BTreeSet`, `live_issue_op_class`, dependency, pending-address, request, preparation, and head-reservation imports unchanged.

- [ ] **Step 2: Replace inline reservation and scheduler construction**

Inside `schedule_live_speculative_issues`, replace:

```rust
let dependency_table = O3LiveIssueDependencyTable::new(self, &candidates)?;
let reservations = self.live_issue_reservations_at(head, tick);
let scheduler = O3ScopedIssueScheduler::new(
    self.issue_width,
    live_issue_capacities_after_reservations(self.issue_width, reservations),
)
.expect("configured live O3 issue width is nonzero");
let plan = scheduler
    .try_plan_with_reserved_width(
        reservations.width,
        dependency_table.resolved_scopes_at(tick),
        candidates
            .iter()
            .map(|candidate| dependency_table.scoped_instruction(candidate)),
    )
    .map_err(|error| O3RuntimeError::InvalidLiveIssuePlan { error })?;
```

with:

```rust
let dependency_table = O3LiveIssueDependencyTable::new(self, &candidates)?;
let calendar = O3LiveIssueCalendar::capture(self, head);
let plan = calendar.plan_at(tick, &dependency_table, &candidates)?;
```

This capture occurs inside the loop immediately before every plan. Do not move it outside the loop or append selected rows to it after recording.

- [ ] **Step 3: Make decision aggregation consume typed plans**

In the replay outcome branch, replace:

```rust
tick_decision.observe(
    0,
    plan.resource_blocked().len(),
    plan.dependency_blocked().len(),
    plan.reserved_width(),
);
```

with:

```rust
tick_decision.observe(&plan, 0);
```

Replace the normal observation:

```rust
tick_decision.observe(
    issued_rows,
    plan.resource_blocked().len(),
    plan.dependency_blocked().len(),
    plan.reserved_width().saturating_add(issued_rows),
);
```

with:

```rust
tick_decision.observe(&plan, issued_rows);
```

- [ ] **Step 4: Update stats flushing to take one typed decision**

Replace `flush_live_issue_decision` with:

```rust
fn flush_live_issue_decision(
    &mut self,
    tick: u64,
    decision: &mut O3LiveIssueTickDecision,
) {
    let Some(decision) = decision.take() else {
        return;
    };
    self.record_live_issue_decision(
        tick,
        decision.issued_rows(),
        decision.resource_blocked_rows(),
        decision.dependency_blocked_rows(),
        decision.max_rows_at_tick(),
    );
}
```

- [ ] **Step 5: Delete the old issue-root calendar implementation**

Delete all of these from `o3_runtime_issue.rs`:

```text
const LIVE_ISSUE_QUEUE
O3RuntimeState::live_issue_reservations_at
O3LiveIssueReservations
O3LiveIssueTickDecision
O3LiveIssueTickDecision::observe
O3LiveIssueReservations::reserve
live_issue_capacities_after_reservations
```

Retain `record_live_issue_decision`; it remains the only runtime stats mutation owner.

Delete this obsolete adapter from `o3_runtime_issue/pending_address.rs`, because the calendar now derives selected ticks directly from canonical pending rows:

```rust
pub(super) fn pending_data_address_selected_issue_tick_for_reservation(
    &self,
    tick: u64,
) -> bool {
    self.pending_data_addresses
        .iter()
        .any(|pending| pending.selected_issue_tick == Some(tick))
}
```

- [ ] **Step 6: Migrate the existing focused-module source policy**

In `o3_runtime_issue_lives_in_focused_module`, remove `"O3ScopedIssueScheduler::new("` from `issue_authority_patterns`. After the existing line-cap assertion, add:

```rust
assert!(
    module.contains("O3LiveIssueCalendar::capture("),
    "src/o3_runtime_issue.rs must delegate live reservation capture"
);
assert!(
    module.contains(".plan_at("),
    "src/o3_runtime_issue.rs must delegate live scoped arbitration"
);
assert!(
    !module.contains("O3ScopedIssueScheduler::new("),
    "src/o3_runtime_issue.rs must not construct the scoped scheduler directly"
);
```

Keep all other root authority and live-retire delegation assertions intact.

- [ ] **Step 7: Run focused policy and runtime tests and confirm GREEN**

Run:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo test -p rem6-cpu --test source_policy o3_live_issue_calendar_owns_reservations_and_arbiter -- --exact --nocapture
cargo test -p rem6-cpu --test source_policy o3_runtime_issue_lives_in_focused_module -- --exact --nocapture
cargo test -p rem6-cpu --lib live_issue_calendar -- --nocapture
cargo test -p rem6-cpu --lib scoped_issue -- --nocapture
```

Expected: all commands pass. In particular, the existing partial-reentry, return-owner, dependency, rollback, and stats-reset tests remain green.

- [ ] **Step 8: Commit the authority transfer**

```bash
git add crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/pending_address.rs \
  crates/rem6-cpu/tests/source_policy.rs
git commit -m "refactor: delegate live o3 issue arbitration"
```

### Task 5: Verify Real CLI Behavior And Full Workspace

**Files:**
- Verify: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`
- Verify: `docs/architecture/gem5-to-rem6-migration.md`
- Verify: repository worktree and branch state

- [ ] **Step 1: Prepare the repository-local temporary directory**

Run:

```bash
mkdir -p target/tmp
```

Use `TMPDIR=$PWD/target/tmp` on the remaining cargo commands if the host temporary filesystem is constrained.

- [ ] **Step 2: Run the complete existing scoped-issue CLI matrix**

Run:

```bash
cargo test -p rem6 --test cli_run o3_scoped_issue -- --nocapture
```

Expected: all filtered tests pass, including width-one serialization, width-two cross-resource co-issue, same-MUL contention, long-FU dependency, JSON/text/dump stats, timing suppression, checkpoint boundary, and host switch.

- [ ] **Step 3: Run source-policy and line-bound checks**

Run:

```bash
cargo test -p rem6-cpu --test source_policy o3_runtime_issue -- --nocapture
wc -l crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs \
  docs/architecture/gem5-to-rem6-migration.md
```

Expected:

```text
o3_runtime_issue.rs <= 800
calendar.rs <= 450
calendar_tests.rs <= 450
gem5-to-rem6-migration.md = 1200
```

- [ ] **Step 4: Run crate and workspace verification**

Run in this order:

```bash
cargo fmt --all -- --check
git diff --check
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --all-targets
cargo test --workspace
```

Expected: every command exits zero. Do not claim completion from the targeted tests alone.

- [ ] **Step 5: Inspect the final diff and branch state**

Run:

```bash
git status --short --branch
git diff --stat HEAD~2..HEAD
git log -2 --oneline
```

Expected: only the planned calendar, tests, issue-root, and source-policy files changed across the two implementation commits; no migration score or checkpoint schema changed.

- [ ] **Step 6: Push the verified commits**

Run:

```bash
git push origin main
git rev-parse HEAD origin/main
```

Expected: push succeeds and both printed commit IDs are identical.
