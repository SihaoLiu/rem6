# O3 Writeback Test Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove O3 writeback test mutation authority from production-owned source while preserving direct private invariant coverage and existing test setup behavior.

**Architecture:** A source-policy RED test rejects four named writeback test helpers in non-test Rust paths. The three corruption-backed tests move beside the private replan validator and construct impossible reservations without public helpers; the legitimate fixed-FU core setup extension moves unchanged into the existing test-only writeback module family.

**Tech Stack:** Rust workspace, `rem6-cpu`, source-policy parsing helpers, private module unit tests, and real `rem6` CLI tests.

---

### Task 1: Add the RED production-source policy

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs:867-1165`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add the focused helper-location policy**

Add this test beside `o3_runtime_writeback_lives_in_focused_module`:

```rust
#[test]
fn o3_writeback_test_helpers_live_only_in_test_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = [
        "force_test_writeback_reservation_to_memory_result",
        "force_test_writeback_reservation_to_fixed_fu",
        "force_test_writeback_reservation_raw_ready_tick",
        "reserve_test_fixed_fu_writeback",
    ];
    let mut offenders = Vec::new();

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir).unwrap();
        if is_test_only_rust_source(relative) {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let code = rust_code_without_comments_and_literals(&source);
        for helper in forbidden {
            if production_defines_exact_function(&code, helper) {
                offenders.push(format!("{} defines {helper}(", relative.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "O3 writeback test helpers must live in test-only modules, not production source files: {}",
        offenders.join(", ")
    );
}
```

Use raw comment/literal-stripped source rather than `production_rust_source`,
because the latter deliberately removes `#[cfg(test)]` items.

- [ ] **Step 2: Run the exact policy test and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --test source_policy o3_writeback_test_helpers_live_only_in_test_modules -- --exact --nocapture
```

Expected: FAIL and report all four helper definitions in
`src/o3_runtime_writeback.rs`.

### Task 2: Move legitimate core setup into the test-only module family

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback_tests.rs:21-25`
- Create: `crates/rem6-cpu/src/o3_runtime_writeback_tests/core.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs:739-759`

- [ ] **Step 1: Declare the test-only core extension module**

Before the existing `ownership` child declaration, add:

```rust
#[path = "o3_runtime_writeback_tests/core.rs"]
mod core;
```

- [ ] **Step 2: Move the core setup extension unchanged**

Create `crates/rem6-cpu/src/o3_runtime_writeback_tests/core.rs`:

```rust
use super::*;

impl crate::RiscvCore {
    pub(crate) fn reserve_test_fixed_fu_writeback(
        &self,
        sequence: u64,
        raw_ready_tick: u64,
    ) -> Result<(), O3RuntimeError> {
        let mut state = self.state.lock().expect("riscv core lock");
        state
            .o3_runtime
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
                sequence,
                raw_ready_tick,
            )])
            .map(|_| ())
    }
}
```

Delete the identical `#[cfg(test)] impl crate::RiscvCore` block from
`o3_runtime_writeback.rs`.

- [ ] **Step 3: Verify existing core setup consumers still compile and pass**

Run:

```bash
cargo test -p rem6-cpu --lib checkpoint_finalization_clears_consumed_calendar_history -- --nocapture
cargo test -p rem6-cpu --lib riscv_in_order_drive_tests -- --nocapture
```

Expected: PASS. The source-policy test remains RED only because the three force
mutators still live in production source.

### Task 3: Move impossible-state coverage beside the private replan authority

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback/replan.rs:1-8`
- Create: `crates/rem6-cpu/src/o3_runtime_writeback/replan_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback_tests.rs:239-315,698-723`

- [ ] **Step 1: Declare the private child test module**

After `use super::*;` in `replan.rs`, add:

```rust
#[cfg(test)]
#[path = "replan_tests.rs"]
mod tests;
```

- [ ] **Step 2: Add direct fixed-FU owner validation coverage**

Create `replan_tests.rs` with the required imports and this focused test:

```rust
use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RegisterWrite,
    RiscvExecutionRecord, RiscvInstruction,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use crate::riscv_data_completion::RiscvDataCompletion;
use crate::{CpuFetchEvent, CpuFetchRecord, RiscvCpuExecutionEvent};

use super::*;

#[test]
fn fixed_fu_owner_rejects_memory_result_reservation_source() {
    let sequence = 9;
    let raw_ready_tick = 12;
    let instruction = addi(3, 0, 42);
    let mut transaction = O3WritebackReplanTransaction::capture(&O3RuntimeState::default());
    transaction
        .live_speculative_executions
        .push(O3LiveSpeculativeExecution {
            consumed_requests: Vec::new(),
            sequence,
            producer_sequences: Vec::new(),
            issue_tick: 10,
            raw_ready_tick,
            admitted_writeback_tick: raw_ready_tick,
            writeback_slot: Some(0),
            execution: RiscvExecutionRecord::new(
                instruction,
                0x8000,
                0x8004,
                vec![RegisterWrite::new(reg(3), 42)],
                None,
            ),
        });
    let before = transaction.live_speculative_executions.clone();

    let error = transaction
        .sync_live_fixed_fu_writeback_owner(O3WritebackReservation::new(
            sequence,
            raw_ready_tick,
            raw_ready_tick,
            0,
            O3LiveWritebackReadySource::MemoryResult,
            true,
        ))
        .unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackOwnerSourceMismatch {
            sequence,
            owner: "fixed-FU speculative execution",
            reservation_source: "MemoryResult",
        }
    );
    assert_eq!(transaction.live_speculative_executions, before);
}
```

- [ ] **Step 3: Add direct memory-result owner validation coverage**

Add:

```rust
#[test]
fn memory_result_owner_rejects_fixed_fu_reservation_source() {
    let owner = completed_memory_result_owner(7, 39);
    let error = O3WritebackReplanTransaction::validate_live_memory_result_writeback_owner(
        std::slice::from_ref(&owner),
        O3WritebackReservation::new(
            7,
            40,
            40,
            0,
            O3LiveWritebackReadySource::FixedFu,
            true,
        ),
    )
    .unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackOwnerSourceMismatch {
            sequence: 7,
            owner: "live data access",
            reservation_source: "FixedFu",
        }
    );
}

#[test]
fn memory_result_owner_rejects_changed_raw_ready_tick() {
    let owner = completed_memory_result_owner(7, 39);
    let error = O3WritebackReplanTransaction::validate_live_memory_result_writeback_owner(
        std::slice::from_ref(&owner),
        O3WritebackReservation::new(
            7,
            41,
            41,
            0,
            O3LiveWritebackReadySource::MemoryResult,
            true,
        ),
    )
    .unwrap_err();

    assert_eq!(
        error,
        O3RuntimeError::WritebackOwnerReservationMismatch {
            sequence: 7,
            owner: "live data access",
            owner_raw_ready_tick: 40,
            reservation_raw_ready_tick: 41,
        }
    );
}
```

Add these private local constructors:

```rust
fn completed_memory_result_owner(sequence: u64, response_tick: u64) -> O3LiveDataAccess {
    let fetch_request = memory_request(10);
    let data_request = memory_request(20);
    let instruction = RiscvInstruction::Load {
        rd: reg(13),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(13),
        address: 0x9000,
        width: MemoryWidth::Word,
        signed: false,
    };
    let execution = RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 10),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            0x8000,
            0x8004,
            Vec::new(),
            Some(access.clone()),
        ),
    );
    let load_data = vec![0x2a, 0, 0, 0];

    O3LiveDataAccess {
        fetch_request,
        data_request,
        execution,
        sequence,
        lsq_sequence_span: 1,
        issue_tick: 31,
        issue_rob_occupancy: 1,
        issue_lsq_occupancy: 1,
        younger_window_policy: O3DataAccessWindowPolicy::None,
        response_tick: Some(response_tick),
        latency_ticks: Some(response_tick - 31),
        commit_tick: None,
        load_data: Some(load_data.clone()),
        memory_result: Some(RiscvDataCompletion::from_issued_response(
            fetch_request,
            access,
            Address::new(0x9000),
            AccessSize::new(4).unwrap(),
            0,
            Some(load_data),
        )),
        forwarding_plan: None,
        outcome: O3LiveDataAccessOutcome::Completed,
        event_taken: false,
    }
}

fn addi(rd: u8, rs1: u8, immediate: i64) -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(rd),
        rs1: reg(rs1),
        imm: Immediate::new(immediate),
    }
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            memory_request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0013_u32.to_le_bytes().to_vec(),
    )
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}
```

Keep every helper private to this test module.

- [ ] **Step 4: Remove the indirect corruption tests and obsolete fixture**

Delete these tests from `o3_runtime_writeback_tests.rs`:

```text
owner_source_mismatch_error_leaves_writeback_state_unchanged
memory_result_owner_source_mismatch_leaves_writeback_state_unchanged
memory_result_owner_raw_ready_mismatch_leaves_writeback_state_unchanged
```

Delete `runtime_with_completed_memory_result_writeback`, which has no callers
after those tests move. Retain
`owner_validation_error_leaves_writeback_state_unchanged`; it continues to
exercise transaction no-commit behavior through the `WritebackOwnerMissing`
branch.

- [ ] **Step 5: Run the private invariant tests**

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime::o3_runtime_writeback::replan::tests -- --nocapture
```

Expected: all three tests PASS with the exact source and raw-ready mismatch
errors.

### Task 4: Remove the corruption helpers and turn the policy GREEN

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs:447-475`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Delete the three `O3RuntimeState` force mutators**

Remove:

```text
force_test_writeback_reservation_to_memory_result
force_test_writeback_reservation_to_fixed_fu
force_test_writeback_reservation_raw_ready_tick
```

Do not replace them with renamed setters, wider reservation fields, or a
crate-wide corruption enum.

- [ ] **Step 2: Run the exact source policy and confirm GREEN**

Run:

```bash
cargo test -p rem6-cpu --test source_policy o3_writeback_test_helpers_live_only_in_test_modules -- --exact --nocapture
cargo test -p rem6-cpu --test source_policy o3_runtime_writeback_lives_in_focused_module -- --exact --nocapture
```

Expected: both PASS. The first test ignores the relocated core extension only
because its path is test-only.

- [ ] **Step 3: Search for removed corruption authority**

Run:

```bash
rg -n "fn force_test_writeback_reservation_(to_memory_result|to_fixed_fu|raw_ready_tick)" crates/rem6-cpu/src crates/rem6/src
```

Expected: no matches.

### Task 5: Verify runtime behavior, review, commit, and push

**Files:**
- Verify only: `docs/architecture/gem5-to-rem6-migration.md`
- Verify only: `temp/improve-rem6-0.md`

- [ ] **Step 1: Run all affected CPU tests**

Run:

```bash
cargo test -p rem6-cpu --all-targets
```

Expected: PASS, including every existing caller of the relocated core setup
extension and the new private replan invariant tests.

- [ ] **Step 2: Run representative real CLI rows**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_scalar_load_fu_collision_cache_fabric_dram -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::writeback_port::rem6_run_o3_writeback_wrong_path_reservation_never_publishes -- --exact --nocapture
```

Expected: all PASS with unchanged shared-port admission, hierarchy activity,
and wrong-path suppression evidence.

- [ ] **Step 3: Run final workspace and hygiene verification**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets -q
git diff --check
rg -n "reserve_test_fixed_fu_writeback" crates/rem6-cpu/src
wc -l crates/rem6-cpu/src/o3_runtime_writeback.rs docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: every command exits successfully; the remaining setup helper is
defined only in `src/o3_runtime_writeback_tests/core.rs`; the writeback root
remains below 800 lines; the ledger remains exactly 1,200 lines; protected
paths are untouched.

- [ ] **Step 4: Request independent read-only review**

Reviewers must check private invariant equivalence, absence of production test
mutation authority, module privacy, all relocated helper callers, source-policy
false positives, runtime/CLI compatibility, dead code, and ledger honesty.
Address actionable findings and rerun affected tests.

- [ ] **Step 5: Commit and push**

```bash
git add \
  docs/superpowers/specs/2026-07-19-o3-writeback-test-authority-design.md \
  docs/superpowers/plans/2026-07-19-o3-writeback-test-authority.md \
  crates/rem6-cpu/src/o3_runtime_writeback.rs \
  crates/rem6-cpu/src/o3_runtime_writeback/replan.rs \
  crates/rem6-cpu/src/o3_runtime_writeback/replan_tests.rs \
  crates/rem6-cpu/src/o3_runtime_writeback_tests.rs \
  crates/rem6-cpu/src/o3_runtime_writeback_tests/core.rs \
  crates/rem6-cpu/tests/source_policy.rs
git commit -m "refactor: isolate O3 writeback test authority"
git push origin main
```

Verify `git status --short --branch` is clean and `git rev-parse HEAD` equals
`git rev-parse origin/main`.
