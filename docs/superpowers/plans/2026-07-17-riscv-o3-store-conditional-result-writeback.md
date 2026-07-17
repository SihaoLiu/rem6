# RISC-V O3 Store-Conditional Result Writeback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route successful and failed nonzero-destination RISC-V store-conditional status through the existing detailed-O3 memory-result writeback calendar, with representative CLI evidence and no duplicate SC or test ownership.

**Architecture:** Add a typed SC-failure outcome to `RiscvDataCompletion`, reuse the one live data-access/ROB/LSQ/writeback lifecycle, and consolidate local plus target-reported SC failure handling in a focused issue module. Convert the result CLI text fragments into normal Rust modules before adding a focused SC matrix, so the new evidence does not deepen include-order coupling.

**Tech Stack:** Rust 2021 workspace, `rem6-cpu`, `rem6-system`, `rem6` CLI integration tests, `rem6-kernel` scheduler, `rem6-transport`, source-policy structural tests, JSON/debug/stat artifacts.

---

## File Structure

### New files

- `crates/rem6-cpu/src/riscv_data_issue/store_conditional.rs`
  - Owns SC reservation checks, local-failure scheduling, and the shared failed-status callback.
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests/store_conditional.rs`
  - Owns result classification, calendar, event-kind, retry, and cleanup tests.
- `crates/rem6-cpu/src/riscv_data_issue_tests/store_conditional_result.rs`
  - Owns direct/local/target-reported failure publication tests.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_support.rs`
  - Normal-module shared helpers for result classes, boundaries, and SC evidence.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/store_conditional_result.rs`
  - Owns the top-level SC matrix.

### Modified files

- `crates/rem6-cpu/src/riscv_data_issue.rs`
- `crates/rem6-cpu/src/riscv_data_completion.rs`
- `crates/rem6-cpu/src/o3_runtime_memory.rs`
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- `crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_window/terminal_ownership.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries/support.rs`
- `crates/rem6/tests/source_policy/writeback_ownership.rs`
- `crates/rem6/tests/source_policy/core_test_anchors.txt`
- `docs/architecture/gem5-to-rem6-migration.md`

### Removed file

- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/support.rs`
  - Its generic helpers move to `result_support.rs`; the class-specific record helper moves into `result_classes.rs`.

---

### Task 1: Replace include-order-coupled result tests with normal modules

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_support.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries/support.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Remove: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/support.rs`

- [ ] **Step 1: Run the current result evidence and policy baseline**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_memory_result_writeback --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --exact
```

Expected: both commands pass before the mechanical refactor.

- [ ] **Step 2: Add a red source-policy expectation for normal modules**

Replace include-path constants with module-path constants and assert that the
root owns these module declarations in order:

```rust
const RESULT_SUPPORT_MODULE: &str = "result_support";
const RESULT_CLASSES_MODULE: &str = "result_classes";
const RESULT_BOUNDARIES_MODULE: &str = "result_boundaries";

assert_eq!(
    top_level_module_names(WRITEBACK_ROOT, &root),
    [RESULT_SUPPORT_MODULE, RESULT_CLASSES_MODULE, RESULT_BOUNDARIES_MODULE],
);
```

Add `top_level_module_names` by parsing `syn::Item::Mod`. Remove assertions that
lock `include!` order. Keep the existing test inventories, helper ownership,
rustfmt checks, and line caps.

- [ ] **Step 3: Run the policy test and verify it fails**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --exact
```

Expected: FAIL because `writeback_port.rs` still uses `include!`.

- [ ] **Step 4: Convert the fragments into modules**

At the end of `writeback_port.rs`, replace both includes with:

```rust
#[path = "writeback_port/result_support.rs"]
mod result_support;
#[path = "writeback_port/result_classes.rs"]
mod result_classes;
#[path = "writeback_port/result_boundaries.rs"]
mod result_boundaries;
```

Move generic helper functions from the old class support file into
`result_support.rs`, add `use super::*;`, and mark each helper `pub(super)`.
Move `result_data_record`, which depends on `MemoryResultClass`, into
`result_classes.rs`. Add:

```rust
use super::*;
use super::result_support::*;
```

Mark only the result-class types/functions consumed by sibling modules
`pub(super)`. In `result_boundaries.rs`, import those explicit items plus
`result_support::*`. Replace the boundary support include with:

```rust
#[path = "result_boundaries/support.rs"]
mod support;
use support::*;
```

Add `use super::*;` in the boundary support file and mark its enum/functions
`pub(super)`.

- [ ] **Step 5: Verify the mechanical refactor**

Run:

```bash
cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_memory_result_writeback --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --exact
```

Expected: all pass with unchanged result behavior.

- [ ] **Step 6: Commit the refactor**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_support.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries/support.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs
git add -u crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/support.rs
git commit -m "refactor: modularize O3 memory-result CLI tests"
```

---

### Task 2: Isolate SC issue and failure ownership

**Files:**
- Create: `crates/rem6-cpu/src/riscv_data_issue/store_conditional.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`

- [ ] **Step 1: Run focused SC behavior before moving code**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu store_conditional --quiet
```

Expected: existing SC execution, timing, diagnostic, and checkpoint tests pass.

- [ ] **Step 2: Move the existing SC methods without changing behavior**

Declare:

```rust
mod store_conditional;
```

Move these methods from `riscv_data_issue.rs` into an `impl RiscvCore` block in
the new file:

```rust
fn record_local_store_conditional_failure_issue(&self, issue: OutstandingDataAccess);
pub(crate) fn schedule_store_conditional_failure(
    &self,
    scheduler: &mut PartitionedScheduler,
    issue: OutstandingDataAccess,
) -> Result<PartitionEventId, RiscvCpuError>;
pub(crate) fn schedule_store_conditional_failure_parallel(
    &self,
    scheduler: &mut PartitionedScheduler,
    issue: OutstandingDataAccess,
) -> Result<PartitionEventId, RiscvCpuError>;
pub(crate) fn store_conditional_fails(&self, issue: &OutstandingDataAccess) -> bool;
fn record_store_conditional_failure(&self, request_id: MemoryRequestId, tick: Tick);
```

Use `use super::*;` so call sites and visibility remain unchanged.

- [ ] **Step 3: Verify behavior and line-count relief**

Run:

```bash
rustfmt --edition 2021 crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_data_issue/store_conditional.rs
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu store_conditional --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy --quiet
wc -l crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_data_issue/store_conditional.rs
```

Expected: tests pass and the root issue file moves materially away from its cap.

- [ ] **Step 4: Commit the mechanical extraction**

```bash
git add crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_data_issue/store_conditional.rs
git commit -m "refactor: isolate RISC-V store-conditional issue"
```

---

### Task 3: Add a typed SC completion outcome

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_data_completion.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`

- [ ] **Step 1: Write failing completion tests**

Add focused unit tests in `riscv_data_completion.rs`:

```rust
#[test]
fn successful_store_conditional_completion_writes_zero_and_records_success() {
    // Seed rd with 9 and an active reservation, apply a successful SC
    // completion, then assert rd == 0, reservation == None, and no failure
    // streak remains.
}

#[test]
fn failed_store_conditional_completion_writes_one_and_preserves_failure_tick() {
    // Build a failed SC completion at tick 37, apply it, then assert rd == 1,
    // reservation == None, and the failure streak begins at tick 37.
}

#[test]
fn failed_store_conditional_completion_rejects_non_sc_access() {
    // The failed-status constructor must panic or return None for a load.
}
```

- [ ] **Step 2: Run the tests and verify red**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu data_completion::tests::failed_store_conditional -- --nocapture
```

Expected: FAIL because no typed failed outcome exists.

- [ ] **Step 3: Implement the outcome and semantic rename**

Add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvDataCompletionOutcome {
    Completed,
    StoreConditionalFailed { tick: Tick },
}
```

Store it in `RiscvDataCompletion`. Keep `from_issued_response` as the successful
constructor and add:

```rust
pub(crate) fn store_conditional_failed(
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    physical_address: Address,
    size: AccessSize,
    request_byte_offset: usize,
    tick: Tick,
) -> Self;

pub(crate) const fn data_event_kind(&self) -> RiscvDataAccessEventKind;
```

Rename `apply_completed_data_access` to `apply_data_completion`. For SC, match
the outcome and apply zero/success or one/failure-at-recorded-tick. Update every
production and test call site in the same change; do not add a deprecated alias.

- [ ] **Step 4: Run focused and full completion tests**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_data_completion --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu load_reserved_completion --quiet
```

Expected: all pass.

- [ ] **Step 5: Commit the typed completion**

```bash
git add crates/rem6-cpu/src/riscv_data_completion.rs \
  crates/rem6-cpu/src/riscv_data_issue.rs crates/rem6-cpu/src/o3_runtime.rs
git commit -m "refactor: type RISC-V data completion outcomes"
```

---

### Task 4: Admit SC status into the O3 result calendar

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_memory_result_tests/store_conditional.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`

- [ ] **Step 1: Write failing O3 policy and calendar tests**

Add the child module declaration and tests for:

```rust
#[test]
fn nonzero_store_conditional_is_one_integer_memory_result() { /* one ROB, one LSQ, one rename */ }

#[test]
fn zero_destination_store_conditional_has_no_result_reservation() { /* x0 boundary */ }

#[test]
fn failed_store_conditional_reserves_response_plus_one_writeback() { /* ConditionalFailed */ }

#[test]
fn width_one_serializes_store_conditional_against_older_fu() { /* older sequence wins */ }

#[test]
fn mismatched_store_conditional_completion_kind_is_rejected() { /* fail closed */ }

#[test]
fn retry_discards_unpublished_store_conditional_reservation() { /* no stale calendar row */ }
```

Remove the nonzero SC tuple from `unsupported_results()` and add SC to the
supported destination matrix.

- [ ] **Step 2: Run the tests and verify red**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_memory_result_tests::store_conditional -- --nocapture
```

Expected: FAIL because SC is not classified and `ConditionalFailed` is not a
completion outcome in the live lifecycle.

- [ ] **Step 3: Implement O3 classification and event validation**

Extend `o3_memory_result_destination`:

```rust
MemoryAccessKind::StoreConditional { rd, .. } if !rd.is_zero() => {
    Some((O3RegisterClass::Integer, u32::from(rd.index())))
}
```

In `complete_live_data_access`, accept `Completed` and `ConditionalFailed` only
when the typed completion reports the same event kind. Require a completion for
nonzero SC failure, mark the LSQ row completed, reserve one memory-result slot
at `response_tick + 1`, and retain the existing retry/failed discard behavior.

- [ ] **Step 4: Run focused O3 tests**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_memory_result_tests --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_writeback_tests --quiet
```

Expected: all pass.

- [ ] **Step 5: Commit O3 status-result admission**

```bash
git add crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs \
  crates/rem6-cpu/src/o3_runtime_memory_result_tests/store_conditional.rs
git commit -m "feat: arbitrate O3 store-conditional results"
```

---

### Task 5: Unify local and target-reported SC failure callbacks

**Files:**
- Create: `crates/rem6-cpu/src/riscv_data_issue_tests/store_conditional_result.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/store_conditional.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`

- [ ] **Step 1: Write failing data-issue tests**

Add tests for:

```rust
#[test]
fn detailed_local_sc_failure_waits_for_admitted_writeback() {
    // No LR reservation: no transport request; rd keeps its old value before
    // admission and becomes one at admitted writeback.
}

#[test]
fn detailed_target_sc_failure_waits_for_admitted_writeback() {
    // Seed a reservation, return MemoryResponse::store_conditional_failed,
    // keep memory unchanged, and defer rd=1.
}

#[test]
fn sc_failure_retry_or_redirect_never_publishes_stale_status() {
    // Remove the owner before admission and prove rd/checker/progress unchanged.
}
```

- [ ] **Step 2: Run the tests and verify red**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu store_conditional_result -- --nocapture
```

Expected: FAIL because both failure callbacks still publish immediately or do
not hand a typed completion to O3.

- [ ] **Step 3: Add one shared failure recorder**

In `store_conditional.rs`, add:

```rust
fn record_store_conditional_failure_outcome(
    &self,
    state: &mut RiscvCoreState,
    access: IssuedDataAccess,
    tick: Tick,
) {
    // Construct failed completion, choose cloned/retired event based on
    // deferred_o3_live_data_access_retirement, record O3 outcome, apply only
    // when no deferred result owns publication, sync checker once, and emit one
    // ConditionalFailed data event.
}
```

Make both the local scheduled callback and the transport
`ResponseStatus::StoreConditionalFailed` branch remove the outstanding access
and call this function. Delete both direct `hart.write(rd, 1)` paths and their
duplicate reservation/progress/checker code.

- [ ] **Step 4: Verify failure and regression coverage**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu store_conditional_result --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu store_conditional --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --test riscv_data_access_probes store_conditional --quiet
```

Expected: all pass.

- [ ] **Step 5: Commit the unified callback**

```bash
git add crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_data_issue/store_conditional.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/store_conditional_result.rs
git commit -m "fix: defer O3 store-conditional status publication"
```

---

### Task 6: Cover fixed-FU terminal ownership and lifecycle boundaries

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_window/terminal_ownership.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs` only if the new classifier is insufficient

- [ ] **Step 1: Write failing terminal SC tests**

Add:

```rust
#[test]
fn fixed_fu_head_can_provision_terminal_store_conditional_status() {
    // Seed a valid reservation, place DIV then SC, observe pending terminal SC,
    // and prove rd remains old until its own admitted writeback.
}

#[test]
fn failed_terminal_store_conditional_is_squashed_without_status_publication() {
    // No reservation, redirect before admission, then prove rd and SC progress
    // remain unchanged and both owners are quiescent.
}
```

- [ ] **Step 2: Run tests and verify red**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu terminal_ownership::fixed_fu_head_can_provision_terminal_store_conditional_status -- --exact
```

Expected: FAIL before SC classification and typed failure publication are fully
wired.

- [ ] **Step 3: Apply the smallest terminal-owner adjustment if required**

The expected implementation is no new branch: the provisioner should accept SC
through `o3_memory_result_destination`. If a special case is required, stop and
revisit the design rather than adding a parallel SC pending object.

- [ ] **Step 4: Run the complete terminal-result module**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu result_younger_window --quiet
```

Expected: all terminal ownership, interrupt, rollback, and dependency tests pass.

- [ ] **Step 5: Commit focused terminal coverage**

```bash
git add crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_window/terminal_ownership.rs
git commit -m "test: lock terminal O3 store-conditional ownership"
```

---

### Task 7: Add the top-level SC result matrix

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/store_conditional_result.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`

- [ ] **Step 1: Add red source-policy ownership for the SC child**

Require a fourth normal module named `store_conditional_result`, a focused line
cap, and these exact anchors:

```rust
const STORE_CONDITIONAL_RESULT_ANCHORS: [&str; 6] = [
    "rem6_run_o3_store_conditional_result_width_one_serializes_direct",
    "rem6_run_o3_store_conditional_result_width_two_exact_fit_direct",
    "rem6_run_o3_store_conditional_result_cache_fabric_dram",
    "rem6_run_o3_store_conditional_failure_is_local_and_deferred",
    "rem6_run_o3_store_conditional_result_live_actions_reject",
    "rem6_run_timing_suppresses_o3_store_conditional_result_surface",
];
```

- [ ] **Step 2: Run policy and verify red**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --exact
```

Expected: FAIL because the module and tests do not exist.

- [ ] **Step 3: Add table-driven SC fixtures and assertions**

Create a fixture that emits `LR.D`, a delayed DIV collision, `SC.D`, a result
store, and host stop. Parameterize route and writeback width. Assert:

- width one orders older DIV before SC admitted writeback;
- width two admits both at the same tick;
- `rd` is old or absent before admission and zero afterward;
- the success memory dump contains the SC value;
- the hierarchy row has nonzero cache/data-transport/fabric/DRAM activity;
- local failure emits no ordinary Data or Memory request, preserves memory, and
  publishes one only at admission;
- live checkpoint and mode switch fail with the exact cpu0 non-quiescent error;
- timing mode preserves final memory/registers and omits O3 writeback surfaces.

Replace the old nonzero SC boundary case with an x0 SC program and keep the
boundary assertion that x0 has no result reservation.

- [ ] **Step 4: Run the focused CLI matrix**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_store_conditional_result -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_memory_result_writeback --quiet
```

Expected: all new and existing writeback-result tests pass.

- [ ] **Step 5: Commit CLI evidence**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/store_conditional_result.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs \
  crates/rem6/tests/source_policy/core_test_anchors.txt
git commit -m "test: cover O3 store-conditional result matrix"
```

---

### Task 8: Record evidence without changing the score

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update the CPU evidence paragraph**

Add the representative SC status-result matrix to the bounded O3 memory-result
evidence, including success/failure, direct/hierarchy, width one/two, local
no-request suppression, x0, lifecycle rejection, and timing suppression.
Remove `SC result arbitration` from the open CPU list.

Keep these values unchanged:

```text
CPU Execution Models - 74% representative
8 of 10 items
80% raw
74% representative cap
```

Compact existing prose so the ledger remains exactly 1,200 lines.

- [ ] **Step 2: Run ledger policy and verify**

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy --quiet
```

Expected: exactly 1,200 lines and 54 passing policy tests.

- [ ] **Step 3: Commit the ledger update**

```bash
git add docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record O3 store-conditional result writeback"
```

---

### Task 9: Full verification and review

**Files:**
- Review all files changed since the design commit.

- [ ] **Step 1: Run focused gates**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu store_conditional --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu result_younger_window --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_store_conditional_result --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_memory_result_writeback --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy --quiet
```

Expected: all pass.

- [ ] **Step 2: Run full workspace gates**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6-system --quiet
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run --quiet
TMPDIR=$PWD/target/tmp cargo test --workspace --all-targets --quiet
cargo fmt --all -- --check
git diff --check
```

Expected: all pass with no formatting or whitespace errors.

- [ ] **Step 3: Audit boundaries and artifacts**

Verify:

```bash
wc -l crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs \
  docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp
git diff -U0 | rg '^\+.*(dbg!|eprintln!|println!|todo!|unimplemented!)'
```

Expected: all source-policy line caps hold, the ledger is 1,200 lines, `temp/`
is unchanged, and the debug scan has no production additions.

- [ ] **Step 4: Dispatch read-only spec and quality reviews**

The spec reviewer must verify every supported and boundary row in the design.
After spec approval, the quality reviewer must inspect typed outcome ownership,
SC progress timing, callback symmetry, stale reservation cleanup, normal-module
test ownership, dead code, and ledger honesty. Fix and re-review every finding.

- [ ] **Step 5: Push the verified commits**

```bash
git push origin main
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
```

Expected: clean `main`, and local/remote hashes match.
