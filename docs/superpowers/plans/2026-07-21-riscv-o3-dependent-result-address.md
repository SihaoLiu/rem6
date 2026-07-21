# RISC-V O3 Dependent Result-Address Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add one bounded detailed-O3 window where an integer memory result supplies a younger scalar load address, with addressless LSQ residency, scheduler-owned issue, normal-path request binding, exact replay, and top-level direct/hierarchy evidence.

**Architecture:** Fetch-ahead records a typed `YoungerDependentRead` authorization without inventing a physical range. The runtime stages one `O3PendingDataAddress` as an existing ROB/rename/LSQ row with `address=None`; the scoped scheduler materializes it from the admitted producer value, and the normal data path validates and binds the existing row before any request becomes visible. Unsupported dynamic addresses discard the pending row and suffix, then replay through the architectural path after the producer commits.

**Tech Stack:** Rust 2021 workspace, `rem6-cpu` O3 runtime and RISC-V fetch/data paths, `rem6` CLI integration tests, Cargo, structured JSON/text stats, direct and cache/fabric/DRAM memory routes.

---

## Per-Task Push Gate

Before every `git commit` and `git push` block below, dispatch a fresh
high-intensity read-only reviewer (`gpt-5.5`, `xhigh`) over that task's diff,
the design spec, and `temp/improve-rem6-0.md`. Fix every actionable finding,
rerun the task's listed verification, run `git diff --check`, close the
reviewer, then commit and push. A task is not complete merely because its
focused tests pass.

Use `TMPDIR=$PWD/target/tmp` for Cargo and Git commit commands because the host
temporary filesystem may be full.

**Execution prerequisite:** Commit and push this plan document before starting
Task 1. It is the tracked execution baseline.

## File Map

- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`: thin shared fixture facade after extracting its 11 root tests.
- New `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/fixed_fu.rs`: existing fixed-FU/writeback-port tests moved without behavior changes.
- `crates/rem6-cpu/src/riscv_fetch_ahead/memory_result_authorization.rs`: typed resolved-range versus dependent-source authority.
- New `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs`: exact static dependent-load admission.
- `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`: narrow delegation to the new candidate.
- New `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address.rs`: focused authorization matrix.
- New `crates/rem6-cpu/src/o3_runtime_pending_address.rs`: sole pending-address runtime owner.
- New `crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs`: shared pending-address test fixtures and focused child declarations.
- New `crates/rem6-cpu/src/o3_runtime_pending_address_tests/staging.rs`, `scheduling.rs`, and `lifecycle.rs`: bounded runtime evidence by responsibility.
- New `crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs`: head-issue orchestration that consumes authorization and stages the four-row window.
- `crates/rem6-cpu/src/o3_runtime_issue.rs`, `o3_runtime_control_window.rs`, `o3_runtime_issue/dependency.rs`: memory-class candidate scheduling and materialization.
- `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`: actual-tick wake ownership for resource-blocked pending addresses.
- New `crates/rem6-cpu/src/riscv_translation/unissued_data.rs`: extracted unissued-data selection with materialized pending-address discovery.
- New `crates/rem6-cpu/src/riscv_data_issue/dependent_result_address.rs`: pre-submit validation, replay, and bind.
- New `crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address.rs`: serial, parallel, forwarded, and dynamic replay tests.
- `crates/rem6-cpu/src/o3_runtime_memory.rs`, `o3_runtime_live_window.rs`, `o3_runtime_authority.rs`, `riscv_data_issue/o3_callback.rs`, `lib.rs`, `riscv_fetch.rs`: cleanup, quiescence, event ownership, and retry/failure integration.
- New `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs` and `dependent_result_address/boundaries.rs`: top-level matrix and lifecycle evidence.
- `crates/rem6/tests/source_policy/writeback_ownership.rs`, `crates/rem6-cpu/tests/source_policy.rs`: focused ownership and line caps.
- `crates/rem6/tests/source_policy/core_test_anchors.txt`: mechanically register final CLI anchors.
- `docs/architecture/gem5-to-rem6-migration.md`: executable-evidence update at exactly 1,200 lines and unchanged CPU score.

### Task 1: Extract Existing Writeback-Port Root Tests

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/fixed_fu.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`

- [ ] **Step 1: Make source policy require the focused child**

Add these constants and module declaration metadata in
`writeback_ownership.rs`:

```rust
const FIXED_FU: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/fixed_fu.rs";
const FIXED_FU_MAX_LINES: usize = 800;
const WRITEBACK_ROOT_MAX_LINES: usize = 550;

const FIXED_FU_ANCHORS: [&str; 11] = [
    "rem6_run_o3_writeback_width_one_serializes_direct_fu_dependent_collision",
    "rem6_run_o3_writeback_width_two_exact_fit_direct_fu_dependent_collision",
    "rem6_run_o3_writeback_port_json_exposes_counters",
    "rem6_run_o3_writeback_port_text_stats_expose_counters",
    "rem6_run_o3_writeback_port_stats_dump_exposes_counters",
    "rem6_run_o3_writeback_scalar_load_fu_collision_blocks_architecture_until_admission",
    "rem6_run_o3_writeback_scalar_load_fu_collision_cache_fabric_dram",
    "rem6_run_timing_suppresses_o3_writeback_port_surface",
    "rem6_run_o3_writeback_wrong_path_reservation_never_publishes",
    "rem6_run_o3_writeback_port_checkpoint_boundary",
    "rem6_run_host_switch_preserves_o3_writeback_port_ticks",
];
```

Change `WRITEBACK_ROOT_MODULES` from length 5 to length 6 and add this entry:

```rust
ExpectedModuleDeclaration {
    name: "fixed_fu",
    path: "writeback_port/fixed_fu.rs",
},
```

Read `FIXED_FU`, assert it is a leaf, assert both files remain below their
exclusive ceiling constants, and assert
`top_level_test_names(FIXED_FU, &fixed_fu) == FIXED_FU_ANCHORS`.

- [ ] **Step 2: Run policy to verify the extraction requirement fails**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --nocapture
```

Expected: FAIL because `fixed_fu.rs` and its module declaration do not exist
and the root still exceeds the new 550-line cap.

- [ ] **Step 3: Move the existing tests without semantic edits**

Create `writeback_port/fixed_fu.rs` with:

```rust
use super::*;
```

Move the complete 11 test functions listed in `FIXED_FU_ANCHORS` from
`writeback_port.rs` into the child in the same order. Keep all constants,
fixtures, encoders, commands, and assertion helpers in the root.

Add at the root module-declaration block:

```rust
#[path = "writeback_port/fixed_fu.rs"]
mod fixed_fu;
```

- [ ] **Step 4: Verify unchanged behavior and the ratcheted facade**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run writeback_port::fixed_fu -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --nocapture
wc -l crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/fixed_fu.rs
cargo fmt --all -- --check
```

Expected: all 11 moved tests pass, source policy passes, root is below 550
lines, and child is at or below 800 lines.

- [ ] **Step 5: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/fixed_fu.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "test: extract o3 writeback core coverage"
git push origin main
```

### Task 2: Add Typed Dependent-Address Fetch Authorization

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/dependent_result_address.rs`
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/memory_result_authorization.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_pair_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_effect_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result_pair.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_memory_result_window.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing focused authorization tests**

Add to `riscv_fetch_ahead/tests.rs`:

```rust
mod dependent_result_address;
```

Create the child with `use super::*;`. Add a local `completed_result_pair`
fixture that accepts two encoded `u32` instructions, builds the core with
`core_with_completed_fetches`, enables detailed mode and depth four, initializes
the head base/source registers, and reconstructs both exact
`RiscvCompletedFetchInstruction` values with
`completed_fetch_instruction_starting_with`. Also add a local
`resolved_head_authorization` helper that calls
`detailed_o3::data_access_result_fetch_ahead_authorization` with the head's
first consumed request, decoded instruction and byte count, and
`TranslatedMemoryFetchAhead::Disabled`, then requires a `Head` authorization.
Then add these tests using those local fixtures plus the existing `completed`,
`request`, `i_type`, and atomic helpers:

```rust
#[test]
fn dependent_scalar_ld_authorizes_addressless_younger_read() {
    let (core, head, younger) = completed_result_pair(
        i_type(0, 10, 0b011, 5, 0x03),
        i_type(8, 5, 0b011, 6, 0x03),
    );
    let state = core.state.lock().unwrap();
    let head_authorization = resolved_head_authorization(&state, &head);
    let authorization = dependent_result_address_authorization(
        &state,
        &head,
        &younger,
        head_authorization,
        4,
    )
    .expect("exact dependent LD should be authorized");
    assert_eq!(authorization.role(), O3MemoryResultWindowRole::YoungerDependentRead);
    assert_eq!(
        authorization.dependent_source(),
        Some((
            Register::new(5).unwrap(),
            MemoryWidth::Doubleword,
            Immediate::new(8),
        ))
    );
    assert!(authorization.resolved_range().is_none());
}
```

Add table-driven negatives named:

```text
dependent_address_fetch_rejects_non_exact_load_shapes
dependent_address_authorization_requires_integer_result_head
dependent_address_atomic_head_rejects_ordering_and_allows_unordered
dependent_address_authorization_rejects_translation_and_mmio_heads
dependent_address_counts_as_second_result_and_blocks_third_result
dependent_address_window_remains_four_rows_at_scalar_live_depth_eight
retained_unissued_head_preserves_dependent_address_authority
```

The shape table must include younger store, atomic, LR/SC, FP load, vector
load, `rd=x0`, non-doubleword load, wrong `rs1`, compressed/two-byte load, and
a second dependent result.

Split the existing pair test that treats every dependent second result as
invalid: keep resolved `YoungerRead` dependency rejection, but move exact
`head_rd -> younger LD rs1` acceptance to the new typed role.

- [ ] **Step 2: Run the focused tests to verify RED**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address -- --nocapture
```

Expected: compile failure for missing `YoungerDependentRead`, typed authority,
and focused authorization helper.

- [ ] **Step 3: Replace range-only authorization with a typed authority**

In `memory_result_authorization.rs`, add:

```rust
use rem6_isa_riscv::{Immediate, MemoryWidth, Register};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3MemoryResultWindowAddressAuthority {
    ResolvedRange(AddressRange),
    DependentSource {
        register: Register,
        width: MemoryWidth,
        immediate: Immediate,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3MemoryResultWindowRole {
    Head,
    YoungerRead,
    YoungerBufferedEffect,
    YoungerDependentRead,
}
```

Change `O3MemoryResultWindowAuthorization` to store
`address_authority: O3MemoryResultWindowAddressAuthority` while retaining its
`route` field. `dependent` always sets `route` to
`O3MemoryResultWindowRoute::Memory` and role to `YoungerDependentRead`.
Supply exact constructors and accessors:

```rust
pub(in crate::riscv_fetch_ahead) const fn resolved(
    integer_destination: Option<Register>,
    route: O3MemoryResultWindowRoute,
    physical_range: AddressRange,
    role: O3MemoryResultWindowRole,
) -> Self;

pub(in crate::riscv_fetch_ahead) const fn dependent(
    integer_destination: Register,
    register: Register,
    width: MemoryWidth,
    immediate: Immediate,
) -> Self;

pub(crate) const fn resolved_range(self) -> Option<AddressRange>;
pub(crate) const fn dependent_source(self) -> Option<(Register, MemoryWidth, Immediate)>;
pub(crate) const fn route(self) -> O3MemoryResultWindowRoute;
pub(crate) fn matches_resolved_range(
    self,
    route: O3MemoryResultWindowRoute,
    physical_address: Address,
    size: AccessSize,
) -> bool;
```

`is_younger()` must include `YoungerDependentRead`; `is_buffered_effect()` must
not. Replace all existing `.physical_range()` and `.matches(...)` calls with
resolved-only accessors, and require every resolved-only policy to unwrap or
match `resolved_range()` only after confirming its existing resolved role.
Do not use `Option<AddressRange>` as the stored authority.

- [ ] **Step 4: Implement exact dependent fetch admission in the child**

Create `dependent_result_address.rs` with this public entry point:

```rust
pub(in crate::riscv_fetch_ahead) fn dependent_result_address_authorization(
    state: &RiscvCoreState,
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    row_limit: usize,
) -> Option<O3MemoryResultWindowAuthorization> {
    if !state.live_retire_gate.detailed_policy_enabled()
        || state.data_translation.is_some()
        || row_limit < 2
        || head_authorization.role() != O3MemoryResultWindowRole::Head
        || head_authorization.route() != O3MemoryResultWindowRoute::Memory
        || head_authorization.resolved_range().is_none()
    {
        return None;
    }
    let head_destination = result_head_integer_destination(head.decoded().instruction())?;
    let (rd, rs1, offset, width) = dependent_scalar_ld(younger)?;
    if rd.is_zero() || rs1 != head_destination || width != MemoryWidth::Doubleword {
        return None;
    }
    Some(O3MemoryResultWindowAuthorization::dependent(
        rd,
        rs1,
        width,
        offset,
    ))
}
```

`result_head_integer_destination` accepts nonzero scalar `LD` and unordered
atomic heads only. `dependent_scalar_ld` requires exactly four instruction
bytes and rejects every non-`LD` shape.

Declare the child in `detailed_o3.rs`. In
`data_access_result_window_candidate`, attempt this helper before the existing
resolved younger authorization. On success, append the dependent
authorization, set `result_rows = 2`, rebuild
`RiscvScalarIntegerLiveWindow::from_memory_results(...)`, advance the exact
fetch identity, and continue scanning the scalar suffix. Keep the added root
logic within the existing 450-line cap.

The untranslated scalar-memory prepass in `additional_fetch_candidate` must
return a data-result candidate when its authorization inventory contains
either `YoungerBufferedEffect` or `YoungerDependentRead`. This lets the exact
dependent pair override ordinary scalar-memory-prefix selection while leaving
every unrelated scalar-load head on the existing path.

Update the current resolved-range consumers in `riscv_data_issue.rs` and
`riscv_memory_result_window.rs` to use `matches_resolved_range`. A dependent
authorization must return false there until Task 5's exact pending bind path
handles it; no generic caller may treat it as a physical-range claim.

- [ ] **Step 5: Ratchet focused fetch ownership**

In `crates/rem6-cpu/tests/source_policy.rs`, add:

```rust
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_LINES: usize = 200;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_FETCH_TEST_LINES: usize = 450;
```

Require the module declaration, child file, `dependent_result_address_authorization`,
`YoungerDependentRead`, `O3MemoryResultWindowAddressAuthority`,
`ResolvedRange(AddressRange)`, and `DependentSource`. Assert the large
`detailed_o3.rs` and `data_access_result.rs` roots do not define the helper.
Require the focused test declaration, leaf ownership, and the exact test-name
inventory under the 450-line ceiling.

- [ ] **Step 6: Verify GREEN and compatibility**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu data_access_result_pair -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_memory_result_window -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_data_issue_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy riscv_data_access_result_fetch_authority_is_focused -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy riscv_memory_result_authorization_has_focused_ownership -- --nocapture
cargo fmt --all -- --check
```

Expected: focused and existing pair tests pass; resolved pair policy still
rejects dependent sources unless the exact dependent role is selected.

- [ ] **Step 7: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/riscv_fetch_ahead \
  crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_memory_result_window.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: authorize dependent o3 result addresses"
git push origin main
```

### Task 3: Stage One Addressless Pending Data Row

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/staging.rs`
- Create: `crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_snapshot_entries.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing runtime staging tests**

Declare the test facade in `o3_runtime.rs`, put shared constructors in the
facade, declare `mod staging;`, and create these tests in `staging.rs` with
`use super::*;`:

```text
pending_address_stages_addressless_lsq_and_live_rename_once
pending_address_window_stages_two_scalar_suffix_rows
pending_address_rejects_a_second_owner
pending_address_window_stays_four_rows_at_scalar_live_depth_eight
pending_address_discard_restores_prior_rename_and_removes_lsq
```

The positive fixture must stage a live scalar-load head producing `x5`, then
request this exact suffix:

```rust
let dependent = RiscvInstruction::Load {
    rd: Register::new(6).unwrap(),
    rs1: Register::new(5).unwrap(),
    offset: Immediate::new(0),
    width: MemoryWidth::Doubleword,
    signed: true,
};
let head_dependent = RiscvInstruction::Addi {
    rd: Register::new(7).unwrap(),
    rs1: Register::new(5).unwrap(),
    imm: Immediate::new(8),
};
let fan_in = RiscvInstruction::Add {
    rd: Register::new(8).unwrap(),
    rs1: Register::new(6).unwrap(),
    rs2: Register::new(7).unwrap(),
};
```

Assert ROB count 4, LSQ count 2, pending LSQ address `None`, distinct live
rename rows for `x5` and `x6`, no second `O3LiveDataAccess`, and no duplicate
physical allocation after repeated staging.

- [ ] **Step 2: Run tests to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_address_stages -- --nocapture
```

Expected: compile failure for missing pending owner and stage method.

- [ ] **Step 3: Add the sole pending-address runtime owner**

Create `o3_runtime_pending_address.rs` with:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3PendingDataAddress {
    sequence: u64,
    fetch: CpuFetchEvent,
    consumed_requests: Vec<MemoryRequestId>,
    decoded: RiscvDecodedInstruction,
    producer_register: Register,
    producer_sequence: u64,
    destination: O3RenameMapEntry,
    expected_lsq_bytes: u32,
    head_range: AddressRange,
    atomic_head: bool,
    requested_wake_tick: Option<u64>,
    selected_issue_tick: Option<u64>,
    materialized: Option<RiscvCpuExecutionEvent>,
}

#[derive(Clone, Debug)]
pub(crate) struct O3PendingDataAddressRequest {
    pub(crate) fetch: CpuFetchEvent,
    pub(crate) consumed_requests: Vec<MemoryRequestId>,
    pub(crate) decoded: RiscvDecodedInstruction,
    pub(crate) producer_register: Register,
}
```

Add these methods with one `Option<O3PendingDataAddress>` field on
`O3RuntimeState`:

```rust
pub(crate) fn has_pending_data_address(&self) -> bool;
pub(crate) fn pending_data_address_owns_fetch(&self, fetch: MemoryRequestId) -> bool;
pub(crate) fn stage_pending_data_address_window(
    &mut self,
    head_fetch: MemoryRequestId,
    pending: O3PendingDataAddressRequest,
    suffix: impl IntoIterator<Item = (Address, RiscvInstruction)>,
) -> usize;
pub(super) fn discard_pending_data_address_from(&mut self, sequence: u64);
pub(crate) fn discard_pending_data_address(&mut self);
```

Staging must allocate one sequence, one integer physical destination, one
live-staged ROB row, and:

```rust
self.snapshot
    .load_store_queue
    .push(O3LoadStoreQueueEntry::load(sequence, None, 8));
```

Refactor `stage_live_instruction` through one private
`stage_live_instruction_with_rename_destination(...)` helper so scalar rows
and the pending load share allocation without teaching general scalar policy
to accept loads.

Insert pending and suffix sequences into
`live_data_access_younger_sequences`. Build suffix classification from
`RiscvScalarIntegerLiveWindow::from_memory_results([head_rd, younger_rd], 2,
row_limit)` and never exceed four total rows.

- [ ] **Step 4: Add focused head-issue orchestration**

Create `riscv_live_retire_window/dependent_result_address.rs` with:

```rust
pub(super) fn stage_dependent_result_address_window(
    state: &mut RiscvCoreState,
    head: &RiscvCpuExecutionEvent,
    issue_tick: u64,
    fetch_events: &[CpuFetchEvent],
) -> bool;
```

The helper must find the exact next completed fetch carrying
`YoungerDependentRead`, collect up to two scalar suffix instructions, call
`stage_pending_data_address_window`, bind every consumed fetch identity, and
invoke `schedule_o3_live_speculative_younger_executions` with the dependent
row first and the accepted scalar suffix after it. The initial call may record
dependency blocking before the producer completes. Remove the consumed
dependent authorization only after staging, identity binding, and scheduler
registration all succeed; otherwise discard the newly staged pending suffix.

Declare the child and call it at the start of
`stage_o3_data_access_younger_window`; return immediately when it stages the
specialized window so generic scalar staging cannot duplicate rows.

- [ ] **Step 5: Make discard and quiescence pending-aware**

Add `resolve_address` to `O3LoadStoreQueueEntry` for later binding:

```rust
pub(super) fn resolve_address(&mut self, address: Address) -> bool {
    if self.address.is_some() {
        return self.address == Some(address);
    }
    self.address = Some(address);
    true
}
```

For now, pending discard must remove the pending sequence's LSQ entry before
calling `discard_live_staged_window_from`. Extend
`live_data_access_lifecycle_is_quiescent`, pending-retirement count, fetch
ownership, and live-window predicates to include the pending owner.

- [ ] **Step 6: Ratchet runtime ownership**

In CPU source policy add caps:

```rust
const MAX_O3_RUNTIME_PENDING_ADDRESS_LINES: usize = 650;
const MAX_O3_RUNTIME_PENDING_ADDRESS_TEST_FACADE_LINES: usize = 300;
const MAX_O3_RUNTIME_PENDING_ADDRESS_STAGING_TEST_LINES: usize = 450;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_STAGE_LINES: usize = 250;
```

Require exactly one `Option<O3PendingDataAddress>` in production, focused
stage/discard methods in the child, exact facade/staging module declarations,
leaf ownership, and no pending-address struct or map in `o3_runtime.rs`,
`riscv_live_retire_window.rs`, or `lib.rs`.

- [ ] **Step 7: Verify GREEN**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_pending_address_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
```

Expected: addressless staging, four-row cap, duplicate suppression, and cleanup
tests pass; existing data-result tests remain green.

- [ ] **Step 8: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/staging.rs \
  crates/rem6-cpu/src/o3_runtime_snapshot_entries.rs \
  crates/rem6-cpu/src/o3_runtime_live_window.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/riscv_live_retire_window.rs \
  crates/rem6-cpu/src/riscv_live_retire_window/dependent_result_address.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: stage unresolved o3 data addresses"
git push origin main
```

### Task 4: Schedule and Materialize the Pending Memory Row

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/scheduling.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue/dependency.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_writeback_wake.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing scheduler and materialization tests**

Declare `mod scheduling;` in the test facade and add these exact tests in
`scheduling.rs` with `use super::*;`:

```text
pending_address_scheduler_waits_for_head_writeback
pending_address_scheduler_width_one_orders_memory_before_scalar
pending_address_scheduler_width_two_coissues_memory_and_scalar
pending_address_materialization_uses_admitted_producer_value
pending_address_materialization_stale_identity_replays_and_discards_suffix
pending_address_materialization_failure_replays_without_callback_error
pending_address_materialization_does_not_allocate_a_request
```

For width one, assert pending memory selects at head admitted writeback and the
head-dependent scalar selects one tick later. For width two, assert both select
at the head writeback tick. In both cases the fan-in row remains unresolved
until the dependent load later publishes.

- [ ] **Step 2: Run tests to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_address_scheduler -- --nocapture
```

Expected: pending row is not yet a scheduler candidate.

- [ ] **Step 3: Add a typed memory issue candidate**

Extend `O3LiveSpeculativeIssueKind`:

```rust
PendingDataAddress {
    destination: O3RenameMapEntry,
},
```

When a live-staged entry matches the pending owner, construct an
`O3LiveIssueSchedulingCandidate` with:

```rust
kind: O3LiveSpeculativeIssueKind::PendingDataAddress { destination },
op_class: O3IssueOpClass::Memory,
control_dependency: None,
data_producers: self.live_issue_source_producers(index, &[producer_register]),
```

Validate that the derived producer sequence equals the pending owner's bound
head sequence.

- [ ] **Step 4: Add memory capacity and same-tick reservation ownership**

Extend `O3LiveIssueReservations` with `memory: usize`. Reserve
`O3IssueOpClass::Memory` and add one memory slot to
`live_issue_capacities_after_reservations`:

```rust
(
    O3IssueOpClass::Memory,
    1_usize.saturating_sub(reservations.memory),
),
```

`live_issue_reservations_at` must include a pending row whose selected issue
tick equals the current tick. This is what forces the ready scalar row to the
next tick at width one after the memory row is selected.

- [ ] **Step 5: Materialize into the pending owner, not fixed-FU state**

Change prepared issue recording to dispatch by kind. For the pending branch:

```rust
staged.record_pending_data_address_materialization(
    row.candidate,
    &row.consumed_requests,
    row.issue_tick,
    row.execution,
)?;
```

`record_pending_data_address_materialization` must:

1. require exact sequence, instruction, consumed requests, and producer;
2. require a sequential, trap-free doubleword `MemoryAccessKind::Load` with
   the expected nonzero destination;
3. create one `RiscvCpuExecutionEvent` from the stored fetch event;
4. set `selected_issue_tick` and `materialized`; and
5. allocate no request, outstanding access, writeback reservation, ROB, LSQ,
   or physical register.

Make pending materialization failure a replay outcome, not
`SelectedIssueCandidateNotExecutable`. Have batch preparation evaluate an
issued pending row before any same-tick scalar row and return either the normal
prepared batch or a typed `ReplayPending(sequence)` outcome. On replay, apply
`discard_pending_data_address_from(sequence)` to the cloned runtime, publish
that cleaned clone, record no co-issued suffix row, and return `Ok(())` without
setting `pending_callback_error`. This covers stale fetch identity, changed
producer lineage, canonical execution failure, trap/system side effects, and
shape mismatch.

`live_issue_request_is_recorded` must treat a materialized pending row as
recorded. Existing scalar/control recording remains unchanged.

- [ ] **Step 6: Fence pending selection to the callback's actual tick**

`schedule_live_speculative_issues` currently advances through known future
dependency ticks in one call. Preserve that behavior for existing scalar and
control-only batches, but stop the whole batch before future planning whenever
an unrecorded pending-address candidate remains and its next eligible tick is
greater than the callback's `earliest_tick`. This prevents the ready
head-dependent scalar from being pre-recorded ahead of the older memory row.

At the head-response callback, the existing unpublished-memory-result
writeback calendar owns the wake at the head's admitted writeback tick. At
that actual wake, rerun the complete dependent-plus-suffix request batch so
width one selects the older memory row first and width two may select memory
and scalar together.

- [ ] **Step 7: Wake resource-blocked pending rows at a real later tick**

Add:

```rust
pub(crate) fn pending_data_address_wake_tick(&self) -> Option<u64>;
```

It returns the next tick only when the head scope is already resolved at the
actual callback tick but the pending memory row was resource blocked. Include
that tick in both `requested_o3_writeback_wake_tick` and
`refresh_o3_writeback_wake`; combine it with the existing memory-result,
live-gate, and forwarded-control desired ticks by minimum. Clear it on
materialization or any pending cleanup. Never materialize a future request
during an earlier callback.

- [ ] **Step 8: Verify scheduler and existing issue behavior**

Add a 550-line exclusive cap for `scheduling.rs`, require its exact module
declaration and leaf ownership, and keep the aggregate pending-address test
family below 1,050 lines at this stage.

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_pending_address_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_o3_writeback_wake -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
```

Expected: memory/ALU width behavior passes and existing scalar/control issue
tests retain their exact counts.

- [ ] **Step 9: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/scheduling.rs \
  crates/rem6-cpu/src/o3_runtime_control_window.rs \
  crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue/dependency.rs \
  crates/rem6-cpu/src/riscv_o3_writeback_wake.rs \
  crates/rem6-cpu/src/riscv_live_retire_window.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: schedule dependent o3 data addresses"
git push origin main
```

### Task 5: Bind Materialized Addresses Through the Normal Data Path

**Files:**
- Create: `crates/rem6-cpu/src/riscv_translation/unissued_data.rs`
- Create: `crates/rem6-cpu/src/riscv_data_issue/dependent_result_address.rs`
- Create: `crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/prepared.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Extract unissued-data selection before adding behavior**

Move the complete existing `RiscvCoreState::next_unissued_data_access` method
from `riscv_translation.rs` into new `riscv_translation/unissued_data.rs` with
the imports it needs. Add only this declaration to the root:

```rust
mod unissued_data;
```

Run existing translation and data-issue tests before changing semantics:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_translation -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_data_issue_tests -- --nocapture
```

Expected: PASS after the mechanical move; `riscv_translation.rs` gains line
headroom below the 1,800-line source ceiling.

- [ ] **Step 2: Add failing bind and replay tests**

Declare the new data-issue test child and add:

```text
dependent_result_address_pre_submit_validation_binds_serial_request
dependent_result_address_parallel_transaction_binds_after_submit
dependent_result_address_forwarded_load_binds_without_transport
dependent_result_address_pmp_denial_replays_without_request
dependent_result_address_pma_uncacheable_replays_without_request
dependent_result_address_cross_line_replays_without_request
dependent_result_address_mmio_route_replays_without_mmio_request
dependent_result_address_unknown_memory_route_replays_without_request
dependent_result_address_atomic_overlap_replays_without_request
dependent_result_address_dropped_parallel_prepare_discards_pending_suffix
dependent_result_address_submit_failure_discards_pending_suffix
pending_address_bind_reuses_sequence_and_resolves_lsq_address
pending_address_bind_publishes_one_execution_event
```

The success test must snapshot sequence, physical destination, ROB count, and
LSQ count before preparation; after bind it must assert all four are unchanged
except the exact LSQ address changes from `None` to `Some(expected)` and one
`O3LiveDataAccess` now owns the same sequence.

- [ ] **Step 3: Run tests to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address_pre_submit -- --nocapture
```

Expected: materialized pending execution is not discoverable or bindable.

- [ ] **Step 4: Surface materialized pending execution without early publication**

In `unissued_data.rs`, include the materialized pending event beside public
events and ready terminal results:

```rust
let pending_address = self
    .o3_runtime
    .pending_data_address_execution()
    .filter(|_| !crate::riscv_fetch_ahead::hart_has_enabled_pending_interrupt(&self.hart))
    .filter(|event| !self.issued_data_for_fetches.contains(&event.fetch().request_id()));

let candidate = self
    .events
    .iter()
    .chain(pending_terminal)
    .chain(pending_address)
    .find_map(...);
```

When outstanding O3 data exists, allow the exact candidate through
`pending_data_address_can_issue(fetch_request, &access)` rather than weakening
the general result-window overlap gate.

Extend `RiscvCoreState::data_access_execution` and `_mut` to find the
materialized pending event until bind.

- [ ] **Step 5: Convert speculative preparation failures into replay**

After `next_unissued_data_access` identifies the exact pending fetch, keep the
existing normal preparation logic authoritative but run it through a local
`Result<OutstandingDataAccess, RiscvCpuError>` closure. Add a focused child
helper that classifies only these pending-address preparation failures as
speculative replay:

```text
Transport(UnknownRoute)
DataRoutePartitionMismatch
DataRouteEndpointMismatch
DataPmpAccess
DataPmaAccess
DataAccessCrossesLine
```

For one of those errors on the exact pending fetch, discard the pending row
and suffix and return `Ok(None)` before submission. Propagate the same errors
unchanged for every ordinary access. Do not convert missing data configuration,
translation setup errors, scheduler errors, or unrelated failures.

- [ ] **Step 6: Add focused pre-submit validation**

Create `riscv_data_issue/dependent_result_address.rs` with:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PendingAddressPreSubmit {
    NotPending,
    Ready,
    Replay,
}

impl RiscvCore {
    pub(super) fn validate_pending_address_pre_submit(
        &self,
        issue: &OutstandingDataAccess,
    ) -> PendingAddressPreSubmit;

    pub(super) fn replay_pending_address_before_submit(
        &self,
        fetch_request: MemoryRequestId,
    );
}
```

Validation must require exact fetch, decoded load, destination, selected issue
tick, memory target, cacheable PMA, doubleword span, supported single-line
shape, and atomic-head disjointness. A MMIO route, uncacheable range,
unsupported cross-line span, stale identity, or overlap returns `Replay`.

Call this validator in `prepare_data_access` after the complete
`OutstandingDataAccess` is built but before returning it. On `Replay`, discard
the pending row and suffix and return `Ok(None)` before transport submission.
`prepare_mmio_data_access` must replay a pending address instead of creating an
MMIO request.

- [ ] **Step 7: Bind after successful submission without duplicate allocation**

Add to the pending runtime owner:

```rust
pub(crate) fn bind_pending_data_address_issue(
    &mut self,
    execution: &RiscvCpuExecutionEvent,
    data_request: MemoryRequestId,
    physical_address: Address,
    request_tick: u64,
) -> Option<Vec<MemoryRequestId>>;
```

`None` is permitted only when no pending owner claims the execution fetch. If
the pending owner claims it, every identity checked by pre-submit validation
must be asserted again under the core lock; a mismatch is an invariant failure,
not a fallback to ordinary staging.

The method must resolve the existing LSQ address, push one `O3LiveDataAccess`
with the pending sequence and selected O3 issue tick, remove that sequence from
`live_data_access_younger_sequences`, clear the pending owner, and return the
consumed fetch requests. It must not call `allocate_sequence`,
`allocate_physical_register`, or `stage_live_data_access_issue`.

Construct the live access with `lsq_sequence_span = 1`, the already-observed
ROB/LSQ occupancies, `younger_window_policy =
O3DataAccessWindowPolicy::MemoryResultWindow`, empty response/completion data,
and `Resident` outcome. This preserves the existing wake seed for the fan-in
suffix after the dependent load publishes without restaging any row.

In `try_record_data_issue_state`, branch before ordinary O3 staging:

```rust
let pending_consumed = state.o3_runtime.bind_pending_data_address_issue(
    execution,
    issue.request_id,
    issue.physical_address,
    issue.tick,
);
if pending_consumed.is_none() {
    // existing stage_live_data_access_issue path
}
```

On successful bind, append exactly one cloned execution event to
`state.events`, extend `executed_fetches` with the returned consumed requests,
and skip `stage_o3_data_access_younger_window` because the suffix is already
resident. The O3 event retains the scheduler-selected issue tick; Data/Memory
records retain the actual request tick.

- [ ] **Step 8: Make parallel prepared cleanup pending-aware**

Replace `PreparedDataIssueCleanup`'s direct deferred-abort call with one
focused state helper:

```rust
fn abort_prepared_data_issue(&mut self, fetch_request: MemoryRequestId) {
    if self.o3_runtime.pending_data_address_owns_fetch(fetch_request) {
        self.o3_runtime.discard_pending_data_address();
    }
    self.abort_deferred_o3_live_data_access_execution(fetch_request);
}
```

Use it from `Drop` and from serial `data_issue_attempt` error cleanup. Keep the
existing submit-then-record order: callbacks are guarded by
`owns_outstanding_data_request`, and binding inserts ownership immediately
after a successful submit. Forwarded completion must use the same validator
and bind path before its callback can fire.

Update `clear_deferred_o3_live_data_access_execution` to select either the
ordinary deferred fetch or the sole pending-address fetch and route both
through `abort_prepared_data_issue`. The dropped-parallel and failed-submit
tests must prove empty pending ownership, removed suffix/LSQ rows, restored
rename state, no outstanding request, and no target call.

- [ ] **Step 9: Ratchet focused data-issue ownership**

Add CPU source-policy caps:

```rust
const MAX_RISCV_UNISSUED_DATA_LINES: usize = 110;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_ISSUE_LINES: usize = 350;
const MAX_RISCV_DEPENDENT_RESULT_ADDRESS_ISSUE_TEST_LINES: usize = 550;
```

Require extraction of `next_unissued_data_access`, exact validator/bind helper
ownership, and absence of pending-address struct definitions or broad route
logic in `riscv_translation.rs`, `riscv_data_issue.rs`, and `lib.rs`.

- [ ] **Step 10: Verify serial, parallel, forwarded, and replay behavior**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_data_issue_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu riscv_translation -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
```

Expected: all new bind/replay tests pass and existing serial/parallel data
issue tests remain green.

- [ ] **Step 11: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/riscv_translation.rs \
  crates/rem6-cpu/src/riscv_translation/unissued_data.rs \
  crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_data_issue/dependent_result_address.rs \
  crates/rem6-cpu/src/riscv_data_issue/prepared.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address.rs \
  crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: bind dependent o3 data requests"
git push origin main
```

### Task 6: Close Retry, Redirect, Checkpoint, and Mode Boundaries

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_pending_address_tests/lifecycle.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_authority.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Test: `crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add failing lifecycle tests**

Declare `mod lifecycle;` in the test facade and add tests named in
`lifecycle.rs` with `use super::*;`:

```text
head_retry_discards_pending_address_and_suffix
head_failure_discards_pending_address_and_suffix
redirect_discards_pending_address_and_future_wake
interrupt_discards_pending_address_and_suffix
restart_discards_pending_address_and_suffix
reset_and_restore_clear_pending_address_state
detailed_mode_disable_discards_pending_address_state
pending_address_keeps_live_data_handoff_nonquiescent
pending_address_rejects_live_checkpoint_capture
drained_pending_address_restores_checkpoint_compatibility
```

Every cleanup test must assert all of these:

```rust
assert!(!runtime.has_pending_data_address());
assert!(runtime.snapshot().load_store_queue().iter().all(|entry| entry.address().is_some()));
assert!(runtime.live_data_access_lifecycle_is_quiescent());
assert!(!runtime.has_pending_retirement_authority());
assert!(runtime.pending_data_address_wake_tick().is_none());
```

Also assert the prior architectural rename mapping is restored and no younger
request/target call exists.

- [ ] **Step 2: Run lifecycle tests to verify RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_address_keeps_live_data_handoff -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu head_retry_discards_pending_address -- --nocapture
```

Expected: stale pending state or LSQ rows remain on at least one path.

- [ ] **Step 3: Centralize pending suffix cleanup**

Implement one internal cleanup primitive:

```rust
fn discard_pending_data_address_at_internal(&mut self, now: Option<u64>) {
    let Some(pending) = self.pending_data_address.take() else {
        return;
    };
    self.snapshot
        .load_store_queue
        .retain(|entry| entry.sequence() != pending.sequence);
    match now {
        Some(now) => self.discard_live_staged_window_from_at(pending.sequence, now),
        None => self.discard_live_staged_window_from(pending.sequence),
    }
}
```

All retry/failure, redirect, reset, restore, detailed-disable, and failed
prepared-issue paths call this primitive. Do not duplicate LSQ/rename cleanup
in callbacks.

- [ ] **Step 4: Integrate retry/failure and authority checks**

When `record_o3_data_access_outcome` observes head `Retry` or `Failed`, clear
the dependent authorization and pending suffix before removing younger
outstanding accesses. Extend live-data quiescence and pending-retirement
ownership to include pending state and its selected wake.

Do not add code to the exactly 1,800-line
`riscv_execution_mode_handoff.rs`. Its existing
`live_data_access_lifecycle_is_quiescent` gate must see pending state as
nonquiescent, causing `capture_o3_live_data_handoff_status()` to return
`Rejected`. Checkpoint capture uses the same pending-retirement authority.

- [ ] **Step 5: Verify lifecycle behavior and no cap regression**

Add a 650-line exclusive cap for `lifecycle.rs`, require its exact module
declaration and leaf ownership, and keep the complete pending-address runtime
test family below 1,600 aggregate lines.

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu head_retry_discards_pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu pending_address_keeps_live_data_handoff -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu discards_pending_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_memory_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_checkpoint -- --nocapture
wc -l crates/rem6-cpu/src/riscv_execution_mode_handoff.rs
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
```

Expected: lifecycle tests pass and `riscv_execution_mode_handoff.rs` remains
exactly 1,800 lines.

- [ ] **Step 6: Run the per-task review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/lifecycle.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/o3_runtime_live_window.rs \
  crates/rem6-cpu/src/o3_runtime_authority.rs \
  crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs \
  crates/rem6-cpu/src/riscv_fetch.rs \
  crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address.rs \
  crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "fix: clean dependent o3 address lifecycle"
git push origin main
```

### Task 7: Add the Top-Level Dependent-Address Matrix

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`

- [ ] **Step 1: Add the focused modules and failing CLI tests**

Add to `writeback_port.rs`:

```rust
#[path = "writeback_port/dependent_result_address.rs"]
mod dependent_result_address;
```

Create the parent with `use super::*;`, declare its boundary child, and add
these exact anchors:

```text
rem6_run_o3_dependent_result_address_matrix_direct
rem6_run_o3_dependent_result_address_matrix_cache_fabric_dram
rem6_run_timing_suppresses_o3_dependent_result_address
```

Create the boundary child with:

```text
rem6_run_o3_dependent_result_address_boundaries_and_live_actions
```

Run the focused filter and verify RED:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run dependent_result_address -- --nocapture
```

Expected: positive rows do not yet expose four-row addressless residency or
dependent request timing.

- [ ] **Step 2: Build the exact four-row fixtures**

Use a table-driven fixture:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DependentAddressHead {
    ScalarLoad,
    AtomicSwap,
}

struct DependentAddressRow {
    head: DependentAddressHead,
    memory_system: &'static str,
    issue_width: usize,
    offset: i32,
    route_delay: u64,
    max_tick: u64,
}

const DEPENDENT_ADDRESS_ROWS: [DependentAddressRow; 4] = [
    DependentAddressRow {
        head: DependentAddressHead::ScalarLoad,
        memory_system: "direct",
        issue_width: 1,
        offset: 0,
        route_delay: 9,
        max_tick: 800,
    },
    DependentAddressRow {
        head: DependentAddressHead::ScalarLoad,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        offset: 8,
        route_delay: 9,
        max_tick: 2_000,
    },
    DependentAddressRow {
        head: DependentAddressHead::AtomicSwap,
        memory_system: "direct",
        issue_width: 1,
        offset: 0,
        route_delay: 9,
        max_tick: 800,
    },
    DependentAddressRow {
        head: DependentAddressHead::AtomicSwap,
        memory_system: "cache-fabric-dram",
        issue_width: 2,
        offset: 8,
        route_delay: 9,
        max_tick: 2_000,
    },
];
```

The guest shape at stable PCs is:

```text
HEAD_PC:      LD or unordered AMOSWAP.D -> x5 pointer
DEPENDENT_PC: LD x6, offset(x5)
SCALAR_PC:    ADDI x7, x5, 8
FAN_IN_PC:    ADD x8, x6, x7
              store x8 witness
              m5_exit
```

Data layout must keep the returned pointer disjoint from the AMO head range
and provide exact values at both offset 0 and offset 8. Run with:

```text
--riscv-o3-scalar-memory-depth 4
--riscv-o3-issue-width 1 or 2
--riscv-o3-writeback-width 2
--memory-system row.memory_system
--memory-route-delay row.route_delay
--max-tick row.max_tick
--debug-flags O3,Data,Memory,Fetch,HostAction
```

- [ ] **Step 3: Assert pre-response addressless residency**

Use a completed run to locate the head's `lsq_data_response_tick`, then rerun
at `response_tick - 1`. For each row assert:

```rust
assert_eq!(json_u64(&resident, "/cores/0/o3_runtime/snapshot/rob/count"), 4);
assert_eq!(
    json_u64(&resident, "/cores/0/o3_runtime/snapshot/lsq/count"),
    if row.head == DependentAddressHead::AtomicSwap { 3 } else { 2 },
);
let dependent_sequence = rob_entry_at_pc(&resident, DEPENDENT_PC)
    .pointer("/sequence")
    .and_then(Value::as_u64)
    .unwrap();
let pending_lsq = resident
    .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
    .and_then(Value::as_array)
    .unwrap()
    .iter()
    .find(|entry| entry.pointer("/sequence").and_then(Value::as_u64) == Some(dependent_sequence))
    .unwrap();
assert!(pending_lsq.pointer("/address").is_some_and(Value::is_null));
assert_eq!(data_requests_sent(&resident), 1);
```

Also assert distinct live rename destinations for `x5` and `x6` and that the
architectural `x5` through `x8` values still equal the fixture's explicit old
values.

- [ ] **Step 4: Assert issue, request, response, and commit timing**

For width one:

```rust
assert_eq!(event_u64(dependent, "issue_tick"), event_u64(head, "writeback_tick"));
assert_eq!(event_u64(scalar, "issue_tick"), event_u64(dependent, "issue_tick") + 1);
```

For width two:

```rust
assert_eq!(event_u64(dependent, "issue_tick"), event_u64(head, "writeback_tick"));
assert_eq!(event_u64(scalar, "issue_tick"), event_u64(dependent, "issue_tick"));
```

For every row assert the dependent Data/Memory request-sent tick is at or
after its O3 issue tick, the event's `lsq_load_address` equals the exact
pointer plus offset, the fan-in issue is at or after the dependent load
writeback, O3 sequences are strictly ordered, commit ticks are nondecreasing
to permit same-cycle bounded-prefix retirement, and final register/memory
witnesses match.

Direct rows must show transport activity and zero cache/fabric/DRAM activity.
Hierarchy rows must show nonzero cache, transport, fabric, and DRAM activity.
Every row must expose at least one dependency-blocked issue decision; width-one
rows must additionally expose the resource-blocked scalar decision created
when the older memory candidate wins the single issue slot.

- [ ] **Step 5: Add dynamic, live-action, and timing boundaries**

The boundary test must cover:

- ordered atomic head serializes the dependent load;
- atomic returned pointer overlapping the head range replays with zero early
  dependent request;
- dependent store/atomic and second dependent load do not open the lane;
- dynamically uncacheable or MMIO pointer replays through the normal path;
- live host checkpoint and detailed-to-timing switch reject while the pending
  row is resident; and
- drained execution allows existing checkpoint behavior.

The timing test runs the scalar-load fixture in detailed and timing modes,
asserts identical registers/memory, requires no `/cores/0/o3_runtime`, and
requires an empty `/debug/o3_trace` array and no `sim.cpu0.o3.*` or gem5 O3
aliases.

- [ ] **Step 6: Enforce focused CLI ownership**

Change `WRITEBACK_ROOT_MODULES` from length 6 to length 7, require the new root
module declaration, and extend `writeback_ownership.rs` with:

```rust
const DEPENDENT_RESULT_ADDRESS: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs";
const DEPENDENT_RESULT_ADDRESS_BOUNDARIES: &str =
    "tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/boundaries.rs";
const DEPENDENT_RESULT_ADDRESS_MAX_LINES: usize = 650;
const DEPENDENT_RESULT_ADDRESS_BOUNDARIES_MAX_LINES: usize = 450;
const DEPENDENT_RESULT_ADDRESS_AGGREGATE_MAX_LINES: usize = 1000;
```

Require the root and child module declarations, exact parent/boundary anchor
inventories, leaf ownership, no `include!`, and no duplicate anchor occurrence
in peer writeback modules.

- [ ] **Step 7: Verify the complete CLI slice**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run dependent_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Expected: four positive rows, dynamic boundaries, live-action rejection, and
timing suppression all pass within the new caps.

- [ ] **Step 8: Run the per-task review gate, then commit and push**

```bash
git add docs/superpowers/plans/2026-07-21-riscv-o3-dependent-result-address.md \
  crates/rem6-cpu/src/o3_runtime_handoff.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address.rs \
  crates/rem6-cpu/src/o3_runtime_pending_address_tests/lifecycle.rs \
  crates/rem6-cpu/src/riscv_cluster.rs \
  crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/src/riscv_data_issue/dependent_result_address.rs \
  crates/rem6-cpu/src/riscv_data_issue_tests/dependent_result_address.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/tests/dependent_result_address.rs \
  crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/dependent_result_address/boundaries.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "test: cover dependent o3 result addresses"
git push origin main
```

### Task 8: Lock Ownership, Update the Ledger, and Verify the Workspace

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add final production ownership scans**

In CPU source policy, scan all production Rust files and require:

- exactly one `struct O3PendingDataAddress`;
- exactly one `Option<O3PendingDataAddress>` field;
- pending authorization only through `YoungerDependentRead` and
  `DependentSource`;
- staging/materialization/binding/discard helpers only in the focused pending
  owner;
- pre-submit validation only in the focused data-issue child;
- no pending-address map/set or compatibility alias;
- `o3_runtime.rs < 1200`, `data_access_result.rs <= 450`,
  `riscv_translation.rs < 1800`, `riscv_data_issue.rs < 1800`, and
  `riscv_execution_mode_handoff.rs == 1800`.

Do not duplicate CLI anchor parsing here. Task 7's
`writeback_result_class_cli_evidence_has_focused_ownership` test already reads
the actual parent and boundary module files and requires the exact four-name
inventory; Task 8 only registers those proven names in the central anchor
ledger.

- [ ] **Step 2: Run source policy before ledger edits**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: all ownership and cap tests pass.

- [ ] **Step 3: Register final CLI anchors**

Append to `core_test_anchors.txt`:

```text
rem6_run_o3_dependent_result_address_matrix_direct
rem6_run_o3_dependent_result_address_matrix_cache_fabric_dram
rem6_run_o3_dependent_result_address_boundaries_and_live_actions
rem6_run_timing_suppresses_o3_dependent_result_address
```

- [ ] **Step 4: Update the CPU migration evidence without changing score**

Keep exactly:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw,
capped at the 74% representative bucket cap.
```

Update CPU `Migrated`, `Not migrated`, and `Next evidence` plus the
`tests/gem5/cpu_tests` row to record:

- one untranslated integer-result-to-scalar-`LD` address-generation lane;
- scalar-load and unordered-atomic producer heads;
- addressless LSQ residency before producer response;
- width-one memory priority and width-two memory/ALU co-issue;
- normal data-path PMP/PMA/route/request binding;
- direct and cache/fabric/DRAM evidence;
- atomic-overlap, dynamic-route, retry/failure, checkpoint, mode, and timing
  boundaries; and
- the four exact CLI anchors above.

Remove only the exact covered phrase `dependent result address generation`.
Keep dependent stores/atomics, FP/vector dependent addresses, multiple
unresolved addresses, translated/MMIO pairs, broader result depth, general IQ,
restorable transport ownership, and a general O3 engine open.

- [ ] **Step 5: Preserve mechanical ledger invariants**

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
rg -n "dependent result address generation|dependent stores|multiple unresolved addresses|general O3 engine" docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: ledger remains exactly 1,200 lines, the stale exact open item is
absent, broader boundaries remain present, and all four anchors are found.

- [ ] **Step 6: Run focused and full verification**

Run in this order:

```bash
cargo fmt --all -- --check
git diff --check
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependent_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run dependent_result_address -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --all-targets
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
TMPDIR=$PWD/target/tmp cargo test --workspace
```

Expected: every command exits 0.

- [ ] **Step 7: Dispatch final independent review**

Send the complete implementation diff, design spec, plan, migration ledger,
and test output to two fresh `gpt-5.5:xhigh` read-only reviewers. One reviews
runtime correctness and cleanup; the other reviews CLI evidence, source
ownership, and ledger honesty. Fix every actionable finding and rerun Step 6.

- [ ] **Step 8: Commit and push the final policy/ledger increment**

```bash
git add crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp git commit -m "docs: record dependent o3 result addresses"
git push origin main
git status --short --branch
```

Expected: `main` is clean and matches `origin/main`.
