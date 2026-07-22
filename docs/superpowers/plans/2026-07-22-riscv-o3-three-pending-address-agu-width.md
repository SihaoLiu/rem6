# RISC-V O3 Three Pending Addresses And AGU Width Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend detailed RISC-V O3 execution to three unresolved scalar load addresses and configurable memory/AGU issue width, with sibling, chain, mixed-fanout, route, lifecycle, and top-level CLI evidence.

**Architecture:** Keep the live issue queue derived from canonical ROB rows and exact decoded packets. Raise the focused pending-address owner to three rows, let the dependency table retain sequence-owned readiness, and make the live issue calendar subtract per-tick memory reservations from a separately configured memory issue width while total issue width remains the global bound. Addressless rows remain non-restorable; after all three rows bind into ordinary transport-owned scalar loads, the existing live-data handoff can transfer them to timing mode.

**Tech Stack:** Rust 2021 workspace, `rem6-cpu` detailed RISC-V O3 runtime, `rem6` CLI/configuration and structured JSON, source-policy tests, direct and cache/fabric/DRAM memory paths, Cargo, Git.

---

## File Map

Create:

- `crates/rem6/tests/cli_run/validation/o3_memory_issue_width.rs` - CLI/TOML acceptance, precedence, prerequisite, range, and cross-width validation.
- `crates/rem6/tests/source_policy/o3_memory_issue_width_ownership.rs` - configuration/runtime/calendar/JSON ownership and focused line caps.
- `crates/rem6/tests/source_policy/o3_three_pending_address_ownership.rs` - provisional focused CLI module, line-cap, and exact-anchor ownership before ledger registration.
- `crates/rem6-cpu/src/o3_runtime_widths.rs` - total issue, memory issue, and writeback width getters, validation, and mutation policy.
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_three_pending.rs` - sibling, chain, mixed-fanout, depth, fourth-row, and graph authorization tests.
- `crates/rem6-cpu/src/o3_runtime_pending_address_tests/three_pending.rs` - capacity-three collection, staging, scheduling, wake, replay, cleanup, checkpoint, and handoff tests.
- `crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address_three_pending.rs` - exact oldest materialized selection, bind order, request identity, and sequence-suffix replay tests.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs` - six-row sibling/chain/mixed positive matrix and artifact assertions.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/fixture.rs` - ELF program, data layout, command, resident snapshot, and route helpers.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs` - fourth-row, invalid graph, replay, checkpoint, mode-transfer, and timing boundaries.

Modify configuration and runtime:

- `crates/rem6-cpu/src/riscv_defaults.rs`
- `crates/rem6-cpu/src/public_api.rs`
- `crates/rem6-cpu/src/o3_runtime.rs`
- `crates/rem6-cpu/src/o3_runtime_tests.rs`
- `crates/rem6-cpu/src/lib.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`
- `crates/rem6/src/config.rs`
- `crates/rem6/src/config/riscv_timing.rs`
- `crates/rem6/src/config/accessors.rs`
- `crates/rem6/src/cli_error.rs`
- `crates/rem6/src/run_validation.rs`
- `crates/rem6/src/riscv_core_runtime.rs`
- `crates/rem6/src/core_summary.rs`
- `crates/rem6/src/core_summary_json.rs`
- `crates/rem6/src/run_execution_summary.rs`
- `crates/rem6/tests/cli_run/validation.rs`
- `crates/rem6/tests/source_policy.rs`

Modify pending-address production and focused tests:

- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_set.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs`
- `crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs`
- `crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs`
- `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- `crates/rem6-cpu/tests/source_policy.rs`

Modify executable evidence and migration accounting:

- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs`
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs`
- `crates/rem6/tests/source_policy/writeback_ownership.rs`
- `crates/rem6/tests/source_policy/core_test_anchors.txt`
- `docs/architecture/gem5-to-rem6-migration.md`

## Execution Preconditions

Before Task 1, invoke `superpowers:using-git-worktrees` and create an isolated worktree on branch `codex/o3-three-pending-agu-width` from clean `main` containing this plan. Do not alter the pre-existing unrelated worktrees.

The root filesystem and `/tmp` are full. Use repository-local temporary storage for every Cargo and Git command:

```bash
mkdir -p target/tmp
export TMPDIR="$PWD/target/tmp"
```

Do not edit or commit anything under `temp/`. `temp/reference_designs/gem5` remains read-only and must not be built or executed.

Before each commit:

1. Run the focused commands listed in that task.
2. Run `cargo fmt --all -- --check` and `git diff --check`.
3. Dispatch a fresh high-intensity read-only reviewer over the task diff, the approved design, this plan, and `temp/improve-rem6-0.md`.
4. Fix every actionable finding and rerun verification.
5. Commit with the listed English message and push the feature branch.

### Task 1: Configure And Expose Memory Issue Width

**Files:**
- Create: `crates/rem6/tests/cli_run/validation/o3_memory_issue_width.rs`
- Create: `crates/rem6/tests/source_policy/o3_memory_issue_width_ownership.rs`
- Modify: `crates/rem6/tests/cli_run/validation.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_defaults.rs`
- Modify: `crates/rem6-cpu/src/public_api.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_widths.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_tests.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6/src/config.rs`
- Modify: `crates/rem6/src/config/riscv_timing.rs`
- Modify: `crates/rem6/src/config/accessors.rs`
- Modify: `crates/rem6/src/cli_error.rs`
- Modify: `crates/rem6/src/run_validation.rs`
- Modify: `crates/rem6/src/riscv_core_runtime.rs`
- Modify: `crates/rem6/src/core_summary.rs`
- Modify: `crates/rem6/src/core_summary_json.rs`
- Modify: `crates/rem6/src/run_execution_summary.rs`

- [ ] **Step 1: Add CLI and runtime RED tests**

Attach the new validation child in `validation.rs`:

```rust
#[path = "validation/o3_memory_issue_width.rs"]
mod o3_memory_issue_width;
```

Create the child with `use super::*;` and these exact tests:

```text
rem6_run_accepts_riscv_o3_memory_issue_width_cli_min_and_max
rem6_run_accepts_riscv_o3_memory_issue_width_from_config
rem6_run_cli_o3_memory_issue_width_overrides_config
rem6_run_rejects_invalid_riscv_o3_memory_issue_width_values
rem6_run_rejects_memory_issue_width_above_total_issue_width
rem6_run_validates_o3_memory_issue_width_execution_and_riscv_requirements
rem6_run_config_scan_treats_o3_memory_issue_width_as_value_taking
```

Use the same four-byte RISC-V smoke ELF and command pattern as the existing issue-width tests for rejection cases. Positive artifact rows must instead build a short detailed-mode program containing initialized integer operands, one `DIV`, and one younger integer instruction so `/cores/0/o3_runtime` is present. Pass matching total and memory issue widths for the min/max CLI rows; set both TOML keys for the config row and override both from CLI in the precedence row. Parse stdout JSON and assert:

```rust
assert_eq!(
    json.pointer("/cores/0/o3_runtime/issue/configured_width")
        .and_then(Value::as_u64),
    Some(issue_width as u64),
);
assert_eq!(
    json.pointer("/cores/0/o3_runtime/issue/configured_memory_width")
        .and_then(Value::as_u64),
    Some(memory_width as u64),
);
```

The relation failure must require exact stderr text:

```text
RISC-V O3 memory issue width 4 exceeds total issue width 2
```

Add focused `rem6-cpu` tests:

```rust
#[test]
fn o3_runtime_memory_issue_width_defaults_to_one_and_tracks_valid_values() {
    let mut runtime = O3RuntimeState::default();
    assert_eq!(runtime.memory_issue_width(), 1);
    assert!(runtime.set_memory_issue_width(4));
    assert_eq!(runtime.memory_issue_width(), 4);
}

#[test]
fn o3_runtime_rejects_memory_width_outside_total_issue_width() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_issue_width(2));
    assert!(!runtime.set_memory_issue_width(3));
    assert_eq!(runtime.memory_issue_width(), 1);
    assert!(runtime.set_memory_issue_width(2));
    assert!(!runtime.set_issue_width(1));
}
```

- [ ] **Step 2: Run RED**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_memory_issue_width -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_memory_issue_width -- --nocapture
```

Expected: compile failures for missing constants, fields, setters, error variants, and validation module behavior.

- [ ] **Step 3: Extract width policy and add canonical CPU invariants**

In `riscv_defaults.rs` add:

```rust
pub const MIN_RISCV_O3_MEMORY_ISSUE_WIDTH: usize = 1;
pub const DEFAULT_RISCV_O3_MEMORY_ISSUE_WIDTH: usize = 1;
pub const MAX_RISCV_O3_MEMORY_ISSUE_WIDTH: usize = MAX_RISCV_O3_ISSUE_WIDTH;
```

Export them through `public_api.rs`. `o3_runtime.rs` is already at its 1,200-line cap, so attach a focused child:

```rust
#[path = "o3_runtime_widths.rs"]
mod o3_runtime_widths;
```

Move the existing `issue_width`, `set_issue_width`, `writeback_width`, and `set_writeback_width` methods from the root into that child. Add `memory_issue_width: usize` beside `issue_width` in `O3RuntimeState`, initialize it to the default, and add the new methods in the focused child:

```rust
pub(crate) const fn issue_width(&self) -> usize {
    self.issue_width
}

pub(crate) const fn memory_issue_width(&self) -> usize {
    self.memory_issue_width
}

pub(crate) fn set_memory_issue_width(&mut self, width: usize) -> bool {
    if !(MIN_RISCV_O3_MEMORY_ISSUE_WIDTH..=MAX_RISCV_O3_MEMORY_ISSUE_WIDTH)
        .contains(&width)
        || width > self.issue_width
    {
        return false;
    }
    self.memory_issue_width = width;
    true
}
```

Strengthen `set_issue_width` so it rejects a value below the current memory width before mutation. Add `RiscvCore::set_o3_memory_issue_width`, `RiscvCore::o3_issue_width`, and `RiscvCore::o3_memory_issue_width` beside the existing issue/writeback setters, then configure memory width immediately after `set_o3_issue_width` in `riscv_core_runtime.rs`.

- [ ] **Step 4: Parse, validate, and report the new CLI/TOML setting**

Add `riscv_o3_memory_issue_width: Option<usize>` to both `Rem6RunConfig` and `Rem6RunFileConfig`. `config.rs` is already within eleven lines of its cap, so replace the current tuple/free-function width plumbing with this focused owner in `riscv_timing.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvO3WidthOptions {
    issue: Option<usize>,
    memory_issue: Option<usize>,
    writeback: Option<usize>,
}

impl RiscvO3WidthOptions {
    pub(crate) fn new(
        issue: Option<usize>,
        memory_issue: Option<usize>,
        writeback: Option<usize>,
    ) -> Result<Self, Rem6CliError>;
    pub(crate) fn apply_flag(&mut self, flag: &str, value: &str)
        -> Result<(), Rem6CliError>;
    pub(crate) fn validate_resolved(self) -> Result<(), Rem6CliError>;
    pub(crate) const fn issue(self) -> Option<usize>;
    pub(crate) const fn memory_issue(self) -> Option<usize>;
    pub(crate) const fn writeback(self) -> Option<usize>;
}
```

`new` validates every present numeric value and the resolved relation. `apply_flag` owns all three O3 width flag names. `validate_resolved` runs after all CLI overrides:

```rust
riscv_o3_widths.validate_resolved()?;
```

Keep one local `riscv_o3_widths` value in `config.rs` and populate the three final config fields from its accessors. This extraction must leave `config.rs <= 1,699` lines and `o3_runtime.rs <= 1,200` lines after formatting.

Add these `Rem6CliError` variants and exact display strings:

```rust
InvalidRiscvO3MemoryIssueWidth { value: String }
RiscvO3MemoryIssueWidthExceedsIssueWidth {
    memory_issue_width: usize,
    issue_width: usize,
}
RiscvO3MemoryIssueWidthRequiresExecution
RiscvO3MemoryIssueWidthRequiresRiscv
```

Register the flag as value-taking in both the normal parser and config pre-scan. Add accessors mirroring issue width:

```rust
pub fn riscv_o3_memory_issue_width(&self) -> usize;
pub const fn riscv_o3_memory_issue_width_is_explicit(&self) -> bool;
```

Add execute/RISC-V prerequisite checks in `run_validation.rs`.

- [ ] **Step 5: Expose selected widths in the real O3 JSON artifact**

Add `o3_runtime_issue_width` and `o3_runtime_memory_issue_width` to `Rem6CoreSummary`, fill them from the actual `RiscvCore` getters in `run_execution_summary.rs`, and change the issue formatter to consume the summary:

```rust
fn o3_runtime_issue_json(summary: &Rem6CoreSummary) -> String {
    let stats = summary.o3_runtime;
    format!(
        "{{\"configured_width\":{},\"configured_memory_width\":{},\"cycles\":{},\"issued_rows\":{},\"resource_blocked_row_cycles\":{},\"dependency_blocked_row_cycles\":{},\"max_rows_per_cycle\":{}}}",
        summary.o3_runtime_issue_width,
        summary.o3_runtime_memory_issue_width,
        stats.issue_cycles(),
        stats.issued_rows(),
        stats.resource_blocked_row_cycles(),
        stats.dependency_blocked_row_cycles(),
        stats.max_rows_per_cycle(),
    )
}
```

Keep O3 JSON suppression tied to `O3RuntimeStats::has_activity`; selecting a width alone must not create a timing-mode O3 surface.

- [ ] **Step 6: Add ownership policy**

Attach `o3_memory_issue_width_ownership.rs` from `source_policy.rs`. The test must require:

- one CLI field and one TOML field;
- parse/individual validation/relation validation in `config/riscv_timing.rs`;
- one runtime field in `o3_runtime.rs`;
- total/memory/writeback mutation policy only in `o3_runtime_widths.rs`;
- one core setter call in `riscv_core_runtime.rs`;
- JSON keys only in `core_summary_json.rs` and CLI assertions; and
- line caps of 180 for `o3_runtime_widths.rs`, 500 for the new validation child, 1,200 for `o3_runtime.rs`, and 1,699 for `config.rs`.

Task 1 creates only `o3_memory_issue_width_config_and_runtime_ownership`. Task 2 adds the separate calendar-ownership policy test after the calendar implementation exists, so every committed task remains green.

- [ ] **Step 7: Verify GREEN**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_memory_issue_width -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run o3_memory_issue_width -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_memory_issue_width_config_and_runtime_ownership -- --nocapture
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 8: Review, commit, and push**

```bash
git add crates/rem6-cpu/src/riscv_defaults.rs \
  crates/rem6-cpu/src/public_api.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_widths.rs \
  crates/rem6-cpu/src/o3_runtime_tests.rs \
  crates/rem6-cpu/src/lib.rs \
  crates/rem6/src/config.rs \
  crates/rem6/src/config/riscv_timing.rs \
  crates/rem6/src/config/accessors.rs \
  crates/rem6/src/cli_error.rs \
  crates/rem6/src/run_validation.rs \
  crates/rem6/src/riscv_core_runtime.rs \
  crates/rem6/src/core_summary.rs \
  crates/rem6/src/core_summary_json.rs \
  crates/rem6/src/run_execution_summary.rs \
  crates/rem6/tests/cli_run/validation.rs \
  crates/rem6/tests/cli_run/validation/o3_memory_issue_width.rs \
  crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/o3_memory_issue_width_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: configure o3 memory issue width"
git push -u origin codex/o3-three-pending-agu-width
```

### Task 2: Give The Live Calendar Configurable Memory Capacity

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/o3_memory_issue_width_ownership.rs`

- [ ] **Step 1: Add calendar RED tests**

Add these exact tests to `calendar_tests.rs`:

```text
live_issue_calendar_memory_width_one_blocks_younger_ready_memory_rows
live_issue_calendar_memory_width_two_selects_two_oldest_ready_memory_rows
live_issue_calendar_total_width_still_bounds_memory_width_four
live_issue_calendar_rebuild_counts_each_same_tick_memory_reservation
```

The two-slot test uses total width four, memory width two, and three ready memory rows:

```rust
assert!(runtime.set_issue_width(4));
assert!(runtime.set_memory_issue_width(2));
let plan = O3LiveIssueCalendar::capture(&runtime, head)
    .plan_scoped_at(
        40,
        std::iter::empty::<O3DependencyScopeId>(),
        [
            ready(2, O3IssueOpClass::Memory),
            ready(3, O3IssueOpClass::Memory),
            ready(4, O3IssueOpClass::Memory),
        ],
    )
    .unwrap();
assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![2, 3]);
assert_eq!(
    plan.resource_blocked()
        .iter()
        .map(O3ScopedReadyInstruction::sequence)
        .collect::<Vec<_>>(),
    vec![4],
);
```

The rebuild test must create two materialized pending rows with the same selected issue tick, capture a fresh calendar, and prove both memory slots and both total-width slots remain reserved. This is the regression for removing `BTreeSet` deduplication.

- [ ] **Step 2: Run RED**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_calendar_memory_width -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_calendar_rebuild_counts_each_same_tick_memory_reservation -- --nocapture
```

Expected: ready memory rows still serialize behind the hard-coded one-slot capacity, and same-tick reservations collapse to one.

- [ ] **Step 3: Move all memory capacity arithmetic into the calendar**

Add `memory_issue_width` to `O3LiveIssueCalendar`, capture it from the runtime, and pass it into `live_issue_capacities_after_reservations`:

```rust
struct O3LiveIssueCalendar {
    issue_width: usize,
    memory_issue_width: usize,
    by_tick: BTreeMap<u64, O3LiveIssueReservations>,
}
```

Replace the selected-pending `BTreeSet` with one reservation per row:

```rust
for tick in runtime
    .pending_data_addresses
    .iter()
    .filter_map(|pending| pending.selected_issue_tick)
{
    calendar.reserve(tick, O3IssueOpClass::Memory);
}
```

Change the memory capacity row to:

```rust
(
    O3IssueOpClass::Memory,
    memory_issue_width.saturating_sub(reservations.memory),
),
```

Do not change IntALU, IntMult, Branch, dependency, or total-width behavior.

- [ ] **Step 4: Ratchet sole ownership**

Update both source-policy tests so production definitions of the memory subtraction, `memory_issue_width` calendar field, and same-tick reservation loop exist only in `o3_runtime_issue/calendar.rs`. Continue forbidding persistent calendar state in runtime structs.

- [ ] **Step 5: Verify GREEN and existing scheduling behavior**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu live_issue_calendar -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu scoped_issue -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy o3_live_issue_calendar_owns_reservations_and_arbiter -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_memory_issue_width_calendar_ownership -- --nocapture
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 6: Review, commit, and push**

```bash
git add crates/rem6-cpu/src/o3_runtime_issue/calendar.rs \
  crates/rem6-cpu/src/o3_runtime_issue/calendar_tests.rs \
  crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6/tests/source_policy/o3_memory_issue_width_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: widen o3 memory arbitration"
git push
```

### Task 3: Admit And Own Three Pending Addresses

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_three_pending.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/three_pending.rs`
- Create: `crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address_three_pending.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_set.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Replace obsolete capacity-two expectations with stronger RED tests**

Rename the old focused tests without deleting their intent:

```text
dependent_address_two_pending_rejects_third_unresolved_load
  -> dependent_address_depth_three_rejects_a_second_pending_row

two_pending_collection_orders_by_sequence_and_rejects_third
  -> pending_address_collection_orders_by_sequence_and_rejects_fourth
```

The depth-three test must construct its authorizer with row limit three and prove that the existing second-pending admission still requires depth four. The collection test must now insert three ordered rows and reject a fourth.

Attach the three new focused children:

```rust
mod dependent_result_address_three_pending;

#[path = "o3_runtime_pending_address_tests/three_pending.rs"]
mod three_pending;

#[path = "riscv_data_issue_tests/dependent_result_address_three_pending.rs"]
mod dependent_result_address_three_pending;
```

- [ ] **Step 2: Add exact capacity-three RED inventories**

`dependent_result_address_three_pending.rs` owns:

```text
dependent_address_three_pending_authorizes_siblings_at_depth_four
dependent_address_three_pending_authorizes_full_chain_at_depth_four
dependent_address_three_pending_authorizes_mixed_fanout_at_depth_four
dependent_address_three_pending_rejects_fourth_and_nonadjacent_graphs
dependent_address_three_pending_window_records_three_split_fetch_authorizations
dependent_address_three_pending_rejects_late_memory_after_scalar_start
```

The three positive tests call the authorizer directly and assert:

```rust
assert_eq!(authorizer.dependent_rows(), 3);
assert_eq!(
    authorizer.result_destinations(),
    &[reg(5), reg(6), reg(7), reg(8)],
);
```

The mixed graph uses sources `[x5, x5, x7]`. The nonadjacent negative tries `[x5, x5, x6]` for the third row and must leave the authorizer at two dependent rows.

`o3_runtime_pending_address_tests/three_pending.rs` owns:

```text
three_pending_staging_allocates_three_addressless_lsq_rows
three_pending_sibling_width_one_issues_in_sequence
three_pending_sibling_width_two_issues_two_then_one
three_pending_sibling_width_four_issues_all_three_together
three_pending_chain_waits_for_each_admitted_writeback
three_pending_mixed_fanout_coissues_two_and_blocks_third
three_pending_resource_wake_updates_only_the_blocked_suffix
three_pending_replay_from_middle_preserves_older_and_discards_younger
three_pending_interrupt_reset_htm_and_mode_cleanup_remove_all_rows
three_pending_live_checkpoint_and_addressless_handoff_reject
```

Build requests using the existing fixture helpers, adding a third fetch at `0x800c`. Sibling sources are `[x5, x5, x5]`, chain sources are `[x5, x6, x7]`, and mixed sources are `[x5, x5, x7]`. Before the head response, assert four ROB rows, four LSQ rows for a scalar-load head, and exactly three `address: null` LSQ sequences.

`dependent_result_address_three_pending.rs` owns:

```text
three_pending_unissued_selector_returns_oldest_materialized_first
three_pending_bind_removes_exact_rows_in_sequence
three_pending_first_replay_discards_complete_suffix
three_pending_middle_replay_preserves_first_live_access
three_pending_last_replay_preserves_two_older_live_accesses
three_pending_atomic_root_range_applies_to_every_descendant
```

- [ ] **Step 3: Run RED**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_address_three_pending -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu three_pending -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_address_collection_orders_by_sequence_and_rejects_fourth -- --nocapture
```

Expected: the third authorization/staging row is absent, the collection rejects at its current capacity of two, and the retire-window collector stops after two rows.

- [ ] **Step 4: Generalize authorization without opening arbitrary graphs**

In `DependentResultAddressAuthorizer`, replace `first_pending_destination` with `previous_pending_destination`. Permit at most three dependent rows. Preserve the existing depth-four requirement for every row after the first:

```rust
if self.dependent_rows >= 3 || (self.dependent_rows >= 1 && self.row_limit < 4) {
    return None;
}

let allowed_source = if self.dependent_rows == 0 {
    rs1 == self.head_destination
} else {
    rs1 == self.head_destination || Some(rs1) == self.previous_pending_destination
};
```

After acceptance, always set `previous_pending_destination = Some(rd)`. Keep duplicate destination, `rd == rs1`, non-doubleword, compressed, translated, MMIO, and late-scalar rejection unchanged.

- [ ] **Step 5: Raise the focused collection and remove retire-window literals**

Change the sole capacity constant to three and make that focused constant crate-visible:

```rust
pub(crate) const O3_PENDING_DATA_ADDRESS_CAPACITY: usize = 3;
```

In staging, preserve the historical depth rule explicitly:

```rust
if pending.is_empty()
    || pending.len() > O3_PENDING_DATA_ADDRESS_CAPACITY
    || (pending.len() >= 2 && self.scalar_memory_window_limit < 4)
{
    return 0;
}
```

Import that constant in `riscv_live_retire_window/dependent_result_address.rs`. Replace all literal two-row capacities and `for _ in 0..2` with `O3_PENDING_DATA_ADDRESS_CAPACITY`. Size `result_destinations` for `capacity + 1`; size scheduled rows from `scalar_memory_window_limit().saturating_sub(1)`. Keep authorization removal transactional and only after scheduling succeeds.

- [ ] **Step 6: Ratchet focused ownership and test caps**

Add line caps of 450 lines for each new focused CPU test child. Require exact one-time module declarations and exact test-name inventories. Update the old two-pending inventories to the renamed depth/collection tests. Source policy must continue to reject:

- a second collection field;
- `Option<O3PendingDataAddress>` or `pending_data_address_2`;
- a capacity literal in the retire-window collector;
- arbitrary producer search beyond root or immediately older pending row; and
- capacity checks outside the set/staging owners.

- [ ] **Step 7: Verify GREEN and legacy one/two-row behavior**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_address_three_pending -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu three_pending -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu two_pending -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task3_pending_data_address_staging_stays_in_focused_owners -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy task5_dependent_result_address_data_issue_stays_focused -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy riscv_data_access_result_fetch_authority_is_focused -- --nocapture
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 8: Review, commit, and push**

```bash
git add crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_two_pending.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address_three_pending.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_set.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_staging.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/multiple.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/three_pending.rs \
  crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address_three_pending.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: stage three pending o3 addresses"
git push
```

### Task 4: Add The Representative Top-Level CLI Matrix

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/fixture.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs`
- Create: `crates/rem6/tests/source_policy/o3_three_pending_address_ownership.rs`
- Modify: `crates/rem6/tests/source_policy.rs`

- [ ] **Step 1: Attach the focused CLI child and define the six-row matrix**

Attach:

```rust
#[path = "dependent_result_address/three_pending.rs"]
mod three_pending;
```

In `three_pending.rs`, attach `fixture.rs` and define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ThreePendingTopology {
    Sibling,
    Chain,
    MixedFanout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ThreePendingRow {
    topology: ThreePendingTopology,
    memory_system: &'static str,
    issue_width: usize,
    memory_issue_width: usize,
    route_delay: u64,
    max_tick: u64,
}
```

Register these exact tests:

```text
rem6_run_o3_three_pending_sibling_width_one_direct
rem6_run_o3_three_pending_sibling_width_two_direct
rem6_run_o3_three_pending_sibling_width_four_hierarchy
rem6_run_o3_three_pending_chain_width_four_direct
rem6_run_o3_three_pending_chain_width_two_hierarchy
rem6_run_o3_three_pending_mixed_fanout_width_two_hierarchy
```

Use rows `(1,1)`, `(2,2)`, `(4,4)`, `(4,4)`, `(2,2)`, and `(2,2)` respectively.

- [ ] **Step 2: Build one exact ELF/data fixture**

The fixture uses:

```text
head PC     0x80000030: LD x5, 0(x9)
pending 1   0x80000034: LD x6, 0(x5)
pending 2   0x80000038: source x5 for sibling/mixed, x6 for chain
pending 3   0x8000003c: source x5 for sibling, x7 for chain/mixed
witnesses   0x80000040 onward: stores x6, x7, x8 after ordered retirement
```

Use distinct data layouts:

- sibling: `x5 = P0`, loads values at `P0`, `P0+8`, `P0+16`;
- chain: `x5 = P0`, `P0 -> P1`, `P1+8 -> P2`, `P2+8 -> final value`;
- mixed: `x5 = P0`, `P0 -> value1`, `P0+8 -> P2`, `P2+8 -> value3`.

The command starts from `dependent_address_command`, then appends:

```rust
command.args([
    "--riscv-o3-memory-issue-width",
    &row.memory_issue_width.to_string(),
]);
```

Use route delay large enough that every younger request in the width-four hierarchy row is sent before the first younger response.

- [ ] **Step 3: Assert resident ownership before the head response**

At `head.lsq_data_response_tick - 1`, assert:

- ROB count four;
- LSQ count four;
- three addressless LSQ sequences matching pending PCs in order;
- four distinct live integer mappings for x5-x8;
- old architectural x5-x8 values remain visible;
- exactly one data request, the head; and
- no load has touched any pending target.

- [ ] **Step 4: Assert exact scheduling topology**

For sibling rows:

```rust
match (row.issue_width, row.memory_issue_width) {
    (1, 1) => assert!(first_issue < second_issue && second_issue < third_issue),
    (2, 2) => {
        assert_eq!(first_issue, second_issue);
        assert!(third_issue > second_issue);
    }
    (4, 4) => assert_eq!([first_issue, second_issue, third_issue], [first_issue; 3]),
    _ => unreachable!(),
}
```

For chain rows, assert each younger issue tick is at or after the immediately older admitted writeback tick. For mixed fanout, assert the first two issue together and the third waits for the second writeback.

Every row also asserts:

- exact `lsq_load_address` values;
- one Data/Memory load per pending target;
- nondecreasing sequence and commit ticks;
- exact x5-x8 and witness memory bytes;
- direct transport activity with zero cache/fabric/DRAM for direct rows;
- positive cache, transport, fabric, and DRAM activity for hierarchy rows;
- `/cores/0/o3_runtime/issue/configured_width` and `configured_memory_width`;
- resource-blocked deltas for width-limited siblings;
- dependency-blocked cycles for chain and mixed rows; and
- final zero ROB/LSQ live ownership.

- [ ] **Step 5: Run the focused CLI matrix**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_three_pending_ -- --nocapture
```

Expected: all six real-binary rows pass. The width-four hierarchy row must show three younger requests before their first response; do not weaken it to issue-counter-only evidence.

- [ ] **Step 6: Register positive ownership and line caps**

Attach `o3_three_pending_address_ownership.rs` from `source_policy.rs`. The provisional policy requires the parent and fixture modules exactly once, line caps of 550/450, no `include!`, and the six exact positive anchors with no duplicate ownership. It deliberately does not inspect `core_test_anchors.txt`; central registration waits for Task 5 when the completed boundary matrix and ledger change together.

- [ ] **Step 7: Verify and commit**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_three_pending_ -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_three_pending_positive_cli_ownership -- --nocapture
cargo fmt --all -- --check
git diff --check
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/fixture.rs \
  crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/o3_three_pending_address_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "test: add three pending o3 cli matrix"
git push
```

### Task 5: Lock Fault, Checkpoint, Transfer, And Timing Boundaries

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs`
- Modify: `crates/rem6/tests/source_policy/o3_three_pending_address_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/o3_memory_issue_width_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Replace the obsolete third-row CLI boundary**

Remove `rem6_run_o3_two_pending_result_address_rejects_third_unresolved` and its now-unused `ThirdUnresolved` fixture case. Do not delete coverage: replace it with the stronger capacity-three fourth-row test below. Keep first/second replay, atomic overlap, live-action, and timing tests unchanged.

- [ ] **Step 2: Add six exact three-pending boundary tests**

Attach `three_pending/boundaries.rs` and add:

```text
rem6_run_o3_three_pending_rejects_fourth_unresolved
rem6_run_o3_three_pending_rejects_nonadjacent_graph
rem6_run_o3_three_pending_replays_middle_failure
rem6_run_o3_three_pending_checkpoint_boundary
rem6_run_host_switch_preserves_o3_three_pending_transport_ticks
rem6_run_timing_suppresses_o3_three_pending_surface
```

The fourth-row program appends `LD x10, 0(x8)` after the three accepted pending loads. At the pre-head-response snapshot only the first three pending rows are addressless; the fourth is absent from ROB/LSQ and has no request. Final architectural execution must load its target exactly once after fallback.

The nonadjacent graph uses `h -> a`, `h -> b`, `a -> c`. It must stop O3 authorization before `c`, issue no duplicate request, and preserve final architecture.

The middle failure uses a PMA-uncacheable or cross-line second pending target. After the first younger request completes, prove the original second and third sequences are discarded, the first live access/architectural result is preserved, and replay emits each target request at most once.

- [ ] **Step 3: Distinguish addressless rejection from post-bind transfer**

For `rem6_run_o3_three_pending_checkpoint_boundary`:

- schedule a live checkpoint while all three rows are addressless and require the existing non-quiescent failure;
- schedule another checkpoint after addresses bind but requests remain live and require the same failure;
- checkpoint and restore after complete drain, then assert identical registers, memory, and zero pending/ROB/LSQ state.

For `rem6_run_host_switch_preserves_o3_three_pending_transport_ticks`:

- first prove a switch while any addressless pending owner exists is rejected;
- use the cache/fabric/DRAM sibling width-four fixture to find a tick after the head has retired and all three younger requests are transport-owned but before the first younger response;
- switch CPU0 to timing at that tick;
- assert `pending_data_addresses` is empty, all three requests are still represented in `outstanding_data`, and the transfer artifact carries exactly three resident scalar-memory entries;
- compare each inherited row's issue, response/writeback, and commit order with the detailed baseline; and
- prove the next post-window instruction has no O3 event.

All three targets must remain cacheable scalar loads. This uses the existing `RiscvO3LiveDataHandoff`; do not serialize `O3PendingDataAddresses`, add addressless rows to the handoff codec, or accept a switch after any plain load response has arrived.

- [ ] **Step 4: Prove timing suppression**

Run sibling and chain programs in timing mode with the memory-width flag present. Assert identical final registers/memory, one-at-a-time architectural requests as dictated by timing mode, and absence of:

```text
/cores/0/o3_runtime
sim.cpu0.o3.issue_cycles
system.cpu.iq.instsIssued
```

- [ ] **Step 5: Run boundary evidence before documentation changes**

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_three_pending_rejects -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_three_pending_replays_middle_failure -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_o3_three_pending_checkpoint_boundary -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_host_switch_preserves_o3_three_pending_transport_ticks -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_three_pending_surface -- --nocapture
```

Do not change the migration ledger unless all five commands pass.

- [ ] **Step 6: Promote ownership and update the ledger**

Extend the provisional three-pending ownership policy with the six boundary anchors, require the boundary child exactly once, and cap it at 550 lines. Promote the new files into `writeback_ownership.rs`: add parent/fixture/boundary paths, child declarations, line caps, and all twelve exact anchors; remove the obsolete two-pending third-row anchor and test.

Add all six positive and six boundary anchors to `core_test_anchors.txt` in the same commit. Update the CPU migration section at unchanged `74% representative` and `8 of 10` raw evidence. Record capacity-three sibling/chain/mixed graphs, configured memory widths 1/2/4, three addressless rows, same-tick width-four address generation, direct/hierarchy requests, replay, live rejection, post-bind transfer, and timing suppression. Retain arbitrary non-adjacent graphs, fifth/deeper memory requests, translated/MMIO pairs, dependent stores/atomics, FP/vector addresses, addressless state serialization, restorable transport, and the general O3 engine as open. Keep the ledger exactly 1,200 lines.

```bash
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_three_pending_address -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy o3_memory_issue_width -- --nocapture
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy migration -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy core_test_anchors -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 7: Review, commit, and push**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/three_pending/boundaries.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/two_pending/boundaries.rs \
  crates/rem6/tests/source_policy/o3_three_pending_address_ownership.rs \
  crates/rem6/tests/source_policy/o3_memory_issue_width_ownership.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs \
  crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp git commit -m "test: complete wider o3 address evidence"
git push
```

### Task 6: Verify, Review, And Integrate

- [ ] **Step 1: Run final branch verification**

```bash
cargo fmt --all -- --check
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
timeout 120s env TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
timeout 2h env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --all-targets
timeout 2h env TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
timeout 2h env TMPDIR=$PWD/target/tmp cargo test --workspace
git diff --check
git status --short --branch
```

- [ ] **Step 2: Run final read-only review**

Dispatch a fresh high-intensity reviewer over the complete branch diff. Require explicit checks for:

- real width-two/width-four memory selection rather than configuration-only evidence;
- no persistent or serialized derived queue/calendar;
- no addressless pending rows in the handoff codec;
- exact fourth-row fallback and request de-duplication;
- exact sibling/chain/mixed dependency timing;
- direct versus hierarchy activity;
- timing-mode suppression;
- source-policy ownership and line caps; and
- ledger claims no broader than executable evidence.

Fix findings and rerun the affected focused tests plus final verification.

- [ ] **Step 3: Fast-forward main, verify again, and push**

From the primary checkout:

```bash
git fetch origin
git merge --ff-only codex/o3-three-pending-agu-width
timeout 2h env TMPDIR=$PWD/target/tmp cargo test --workspace
git push origin main
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
```

After `HEAD` and `origin/main` match and the primary checkout is clean, remove only the worktree created for this plan, prune worktree metadata, and delete the local feature branch. Leave all pre-existing unrelated worktrees untouched.
