# RISC-V O3 Memory-Result Writeback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make representative integer, floating-point, LR/AMO, one-register vector, and readfile-MMIO memory results share the detailed-O3 writeback-port calendar without widening scalar-memory concurrency or claiming a general O3 engine.

**Architecture:** First extract one typed data-completion authority from the near-cap issue root, then remove scalar-only names from the generic live data-access lifecycle without changing behavior. Extend that single lifecycle with an exact terminal result policy, an AMO two-sequence LSQ span, and deferred typed completion publication through the existing writeback calendar. Real CLI evidence covers direct, hierarchy, MMIO, suppression, and failure boundaries while preserving the 74% CPU score and the 1,200-line ledger.

**Tech Stack:** Rust workspace, Cargo tests, RISC-V instruction encoders, detailed O3 ROB/LSQ/writeback snapshots, structured `rem6 run --execute` JSON, source-policy ownership checks, migration ledger.

---

## Scope And Invariants

- Authoritative design: `docs/superpowers/specs/2026-07-16-riscv-o3-memory-result-writeback-design.md`.
- `temp/` and `temp/reference_designs/gem5/` remain read-only and uncommitted.
- Existing scalar `Load`/`Store` remains the only multi-row, forwarding, and younger-scalar-window lane.
- New FP, LR, AMO, vector, and MMIO result rows are terminal and single-row.
- AMO owns one ROB row and a two-sequence LSQ span; both LSQ rows complete and disappear together.
- Valid all-inactive unit-stride vector instructions remain suppressed in `rem6-isa-riscv` before data issue.
- `StoreConditional`, multi-register/segment/strided/indexed/fault-only-first vectors, and zero-destination result rows do not consume a result writeback slot.
- Non-scalar live checkpoint and detailed-to-timing switch remain rejected at `RiscvCoreCheckpointPort::validate_capture` with cpu0 nonquiescence.
- No checkpoint schema, writeback-stat schema, checklist state, raw score, or CPU percentage changes.

## File Ownership Map

- `crates/rem6-cpu/src/riscv_data_completion.rs`: typed response completion payload and the sole architectural response-application owner.
- `crates/rem6-cpu/src/riscv_data_issue.rs`: transport/MMIO callback orchestration and construction of typed completions; no response-application detail.
- `crates/rem6-cpu/src/o3_runtime_memory.rs`: generic live data-access lifecycle and exact result admission/publication policy.
- `crates/rem6-cpu/src/o3_runtime_writeback.rs`: generic fixed-FU versus memory-result writeback calendar inputs.
- `crates/rem6-cpu/src/o3_runtime_retire.rs`: sequence-span allocation and full-span live retirement cleanup.
- `crates/rem6-cpu/src/o3_runtime.rs`: focused module wiring and admitted completion publication order.
- `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`: focused CPU behavior matrix.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs`: positive direct, hierarchy, width, and MMIO matrix.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs`: suppression, permission, checkpoint, switch, and timing boundaries.
- `crates/rem6/tests/source_policy/writeback_ownership.rs`: exact CLI include/anchor and line ownership.
- `docs/architecture/gem5-to-rem6-migration.md`: honest executable-evidence record, exactly 1,200 lines.

### Task 1: Extract Typed Data Completion Authority

**Files:**
- Create: `crates/rem6-cpu/src/riscv_data_completion.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add a failing source-policy ownership test**

Add beside `riscv_data_issue_lives_in_focused_module`:

```rust
#[test]
fn riscv_data_completion_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let issue = fs::read_to_string(crate_dir.join("src/riscv_data_issue.rs")).unwrap();
    let completion_path = crate_dir.join("src/riscv_data_completion.rs");

    assert!(lib.contains("mod riscv_data_completion;"));
    assert!(completion_path.exists());
    let completion = fs::read_to_string(completion_path).unwrap();
    for anchor in [
        "pub(crate) struct RiscvDataCompletion",
        "pub(crate) fn apply_completed_data_access(",
        "fn scatter_segment_load(",
        "fn write_vector_register_group(",
    ] {
        assert!(completion.contains(anchor), "missing completion owner {anchor}");
        assert!(!issue.contains(anchor), "data issue still owns {anchor}");
    }
    assert!(!issue.contains("fn record_load_completion("));
}
```

- [ ] **Step 2: Run the policy test and record the red result**

Run:

```bash
cargo test -p rem6-cpu --test source_policy riscv_data_completion_lives_in_focused_module -- --nocapture
```

Expected: FAIL because the module, payload, and delegated owner do not exist.

- [ ] **Step 3: Add the focused completion module without changing behavior**

Add `mod riscv_data_completion;` in `lib.rs` next to `mod riscv_data_access;`.

Move these functions byte-for-byte from `riscv_data_issue.rs` into the new module, renaming only `record_load_completion` to `apply_completed_data_access`:

```text
record_load_completion -> apply_completed_data_access
scatter_segment_load
scatter_strided_load
scatter_indexed_load
read_vector_register_group
write_vector_register_group
vector_register_at
```

Define the payload before the moved functions:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvDataCompletion {
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    physical_address: Address,
    size: AccessSize,
    request_byte_offset: usize,
    data: Option<Vec<u8>>,
}

impl RiscvDataCompletion {
    pub(crate) fn new(
        fetch_request: MemoryRequestId,
        access: MemoryAccessKind,
        physical_address: Address,
        size: AccessSize,
        request_byte_offset: usize,
        data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            fetch_request,
            access,
            physical_address,
            size,
            request_byte_offset,
            data,
        }
    }

    pub(crate) const fn access(&self) -> &MemoryAccessKind {
        &self.access
    }

    pub(crate) fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}
```

Give `apply_completed_data_access` this signature and replace its former `IssuedDataAccess` field reads with payload accessors:

```rust
pub(crate) fn apply_completed_data_access(
    state: &mut RiscvCoreState,
    cpu: CpuId,
    completion: &RiscvDataCompletion,
    missing_data: &'static str,
)
```

Use `completion.physical_address` and `completion.size` for LR reservation state, and `completion.request_byte_offset` for vector normalization. Keep `record_completed_load_data` in this owner, using `completion.fetch_request`.

In both transport and MMIO callbacks, construct `RiscvDataCompletion` from the exact `IssuedDataAccess`, then call `apply_completed_data_access` where `record_load_completion` was called. Do not change the existing deferral predicate yet.

- [ ] **Step 4: Verify the extraction is behavior-preserving**

Run:

```bash
cargo test -p rem6-cpu --test source_policy riscv_data_completion_lives_in_focused_module -- --nocapture
cargo test -p rem6-cpu riscv_data_issue --quiet
cargo test -p rem6-cpu --test riscv_cluster_data --quiet
cargo test -p rem6-system riscv_o3_runtime_stats --quiet
cargo fmt --all -- --check
git diff --check
```

Expected: all PASS; `wc -l crates/rem6-cpu/src/riscv_data_issue.rs` is materially below 1,793 lines.

- [ ] **Step 5: Commit the extraction**

```bash
git add crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/riscv_data_completion.rs \
  crates/rem6-cpu/src/riscv_data_issue.rs \
  crates/rem6-cpu/tests/source_policy.rs
git commit -m "refactor: isolate RISC-V data completion"
git push origin main
```

### Task 2: Remove Scalar-Only Names From The Generic Live Owner

**Files:**
- Modify: every Rust file returned by the exact `rg -l` command below
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add a failing stale-name policy test**

Add:

```rust
#[test]
fn generic_o3_live_data_owner_uses_data_access_names() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stale = [
        "O3LiveScalarMemory",
        "O3LiveScalarMemoryOutcome",
        "live_scalar_memories",
        "deferred_scalar_memory_execution",
        "is_deferred_o3_scalar_memory_access",
        "is_deferred_o3_scalar_memory_instruction",
        "is_terminal_o3_scalar_memory_event",
        "stage_live_scalar_memory_issue",
        "complete_live_scalar_memory_response",
        "take_ready_live_scalar_memory_event",
        "consume_live_scalar_memory_retirement",
        "record_ready_o3_scalar_memory_event_with_trace",
        "scalar_memory_lifecycle_is_quiescent",
        "has_pending_scalar_memory_retirement",
        "pending_scalar_memory_retirement_count",
        "owns_pending_scalar_memory_retirement",
        "defer_scalar_memory_execution",
        "defer_scalar_memory_if_detailed",
        "abort_deferred_scalar_memory_execution",
        "clear_deferred_scalar_memory_execution",
        "has_live_scalar_memory",
        "has_live_scalar_memory_window",
        "earliest_unpublished_scalar_load_writeback_tick",
        "ready_live_scalar_memory_event_kind",
        "ready_live_scalar_memory_completion_timing",
        "replace_ready_live_scalar_memory_execution",
        "live_scalar_memory_publication_is_admitted",
        "ready_live_scalar_load_writeback",
        "discard_live_scalar_memory_lifecycle",
        "remove_live_scalar_memory_rows",
        "o3_scalar_memory_lifecycle_is_quiescent",
        "has_pending_o3_scalar_memory_retirement",
        "pending_o3_scalar_memory_retirement_count",
        "owns_pending_o3_scalar_memory_retirement",
        "ready_o3_scalar_memory_event_kind",
        "clear_deferred_o3_scalar_memory_execution",
    ];
    let offenders = rust_source_files(&crate_dir.join("src"))
        .into_iter()
        .filter_map(|path| {
            let source = fs::read_to_string(&path).unwrap();
            let names = stale
                .iter()
                .filter(|name| source.contains(**name))
                .copied()
                .collect::<Vec<_>>();
            (!names.is_empty()).then_some((path, names))
        })
        .collect::<Vec<_>>();
    assert!(offenders.is_empty(), "stale generic live-data names: {offenders:?}");
}
```

- [ ] **Step 2: Run the red policy test**

```bash
cargo test -p rem6-cpu --test source_policy generic_o3_live_data_owner_uses_data_access_names -- --nocapture
```

Expected: FAIL with the current generic scalar-only symbols.

- [ ] **Step 3: Perform the behavior-preserving semantic rename**

Use this exact mapping across the files returned by:

```bash
rg -l "O3LiveScalarMemory|live_scalar_memories|deferred_scalar_memory_execution|is_deferred_o3_scalar_memory|is_terminal_o3_scalar_memory|stage_live_scalar_memory_issue|complete_live_scalar_memory_response|take_ready_live_scalar_memory_event|consume_live_scalar_memory_retirement|record_ready_o3_scalar_memory_event_with_trace|scalar_memory_lifecycle_is_quiescent|pending_scalar_memory_retirement|defer_scalar_memory|has_live_scalar_memory|earliest_unpublished_scalar_load_writeback_tick|ready_live_scalar_memory|replace_ready_live_scalar_memory|live_scalar_memory_publication_is_admitted|discard_live_scalar_memory_lifecycle|remove_live_scalar_memory_rows|o3_scalar_memory_lifecycle_is_quiescent|pending_o3_scalar_memory_retirement|ready_o3_scalar_memory_event_kind|clear_deferred_o3_scalar_memory_execution" crates --glob '*.rs'
```

```text
O3LiveScalarMemory                         -> O3LiveDataAccess
O3LiveScalarMemoryOutcome                  -> O3LiveDataAccessOutcome
live_scalar_memories                       -> live_data_accesses
deferred_scalar_memory_execution           -> deferred_live_data_access_execution
is_deferred_o3_scalar_memory_access         -> is_deferred_o3_data_access
is_deferred_o3_scalar_memory_instruction    -> is_deferred_o3_data_instruction
is_terminal_o3_scalar_memory_event          -> is_terminal_o3_data_access_event
scalar_memory_lifecycle_is_quiescent       -> live_data_access_lifecycle_is_quiescent
has_pending_scalar_memory_retirement       -> has_pending_live_data_access_retirement
pending_scalar_memory_retirement_count     -> pending_live_data_access_retirement_count
owns_pending_scalar_memory_retirement      -> owns_pending_live_data_access_retirement
defer_scalar_memory_execution              -> defer_live_data_access_execution
defer_scalar_memory_if_detailed             -> defer_live_data_access_if_detailed
abort_deferred_scalar_memory_execution     -> abort_deferred_live_data_access_execution
clear_deferred_scalar_memory_execution     -> clear_deferred_live_data_access_execution
has_live_scalar_memory                     -> has_live_data_access
has_live_scalar_memory_window              -> has_live_data_access_window
has_ready_live_scalar_memory_event         -> has_ready_live_data_access_event
earliest_unpublished_scalar_load_writeback_tick -> earliest_unpublished_memory_result_writeback_tick
ready_live_scalar_memory_event_kind         -> ready_live_data_access_event_kind
ready_live_scalar_memory_completion_timing  -> ready_live_data_access_completion_timing
replace_ready_live_scalar_memory_execution  -> replace_ready_live_data_access_execution
stage_live_scalar_memory_issue             -> stage_live_data_access_issue
complete_live_scalar_memory_response       -> complete_live_data_access_response
take_ready_live_scalar_memory_event        -> take_ready_live_data_access_event
consume_live_scalar_memory_retirement      -> consume_live_data_access_retirement
live_scalar_memory_publication_is_admitted  -> live_data_access_publication_is_admitted
ready_live_scalar_load_writeback            -> ready_live_memory_result_completion
record_ready_o3_scalar_memory_event_with_trace -> record_ready_o3_data_access_event_with_trace
discard_live_scalar_memory_lifecycle       -> discard_live_data_access_lifecycle
remove_live_scalar_memory_rows             -> remove_live_data_access_rows
o3_scalar_memory_lifecycle_is_quiescent    -> o3_live_data_access_lifecycle_is_quiescent
has_pending_o3_scalar_memory_retirement    -> has_pending_o3_live_data_access_retirement
pending_o3_scalar_memory_retirement_count  -> pending_o3_live_data_access_retirement_count
owns_pending_o3_scalar_memory_retirement   -> owns_pending_o3_live_data_access_retirement
ready_o3_scalar_memory_event_kind          -> ready_o3_live_data_access_event_kind
clear_deferred_o3_scalar_memory_execution  -> clear_deferred_o3_live_data_access_execution
```

Also rename generic completion, publication, discard, and row-removal methods to `live_data_access` terminology. Retain names containing `scalar_memory_window`, `scalar_load_younger_window`, forwarding, scalar depth, and scalar overlap when they still describe the bounded scalar-only policy.

Do not add compatibility aliases or deprecated wrappers. Update all workspace callers and tests in the same commit.

- [ ] **Step 4: Verify no behavior changed**

```bash
cargo test -p rem6-cpu --test source_policy generic_o3_live_data_owner_uses_data_access_names -- --nocapture
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo fmt --all -- --check
git diff --check
```

Expected: all PASS, and the stale-name `rg` returns no production matches.

- [ ] **Step 5: Commit the semantic cleanup**

```bash
git add crates/rem6-cpu crates/rem6-system
git commit -m "refactor: generalize O3 live data naming"
git push origin main
```

### Task 3: Add The CPU Result Policy And Runtime Behavior

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_retire.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/forwarding.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_execution_event.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_completion.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add the focused test module and red behavior tests**

Wire it in `o3_runtime.rs`:

```rust
#[cfg(test)]
#[path = "o3_runtime_memory_result_tests.rs"]
mod o3_runtime_memory_result_tests;
```

The new module must define and run these tests:

```rust
#[test]
fn memory_result_policy_accepts_exact_one_destination_matrix() { /* table: Load, FloatLoad, LR, AMO, e64/m1 unit-stride */ }

#[test]
fn memory_result_policy_rejects_zero_destination_and_unsupported_shapes() { /* x0 Load/LR/AMO, SC, group>1, segment, strided, indexed, FOF */ }

#[test]
fn non_scalar_result_is_terminal_while_scalar_overlap_remains_available() { /* second row rejected after FLD; existing Load/Store overlap remains */ }

#[test]
fn live_atomic_reserves_and_retires_two_lsq_sequences() { /* ROB seq N, LSQ seq N/N+1, next row N+2 */ }

#[test]
fn memory_result_response_waits_for_admitted_writeback_tick() { /* FLD payload absent at raw-ready-1 and present at admitted */ }

#[test]
fn load_reserved_installs_physical_reservation_only_at_admission() { /* virtual address differs from completion physical address */ }

#[test]
fn masked_vector_result_applies_at_preserved_nonzero_request_offset() { /* e64/m1 vl=2, inactive leading element */ }

#[test]
fn memory_result_collision_uses_older_sequence_and_width_two_exact_fit() { /* fixed FU plus memory result */ }

#[test]
fn memory_result_retry_and_failure_discard_reservation_and_full_lsq_span() { /* LR plus AMO retry/failure */ }

#[test]
fn live_atomic_squash_redirect_and_rollback_remove_both_lsq_rows() { /* all three cleanup entry points */ }
```

Add `denied_atomic_write_never_stages_live_result_authority` to `riscv_data_issue_tests.rs`, using the real PMP/PMA and issue path rather than a pure runtime helper.

Use real `RiscvCpuExecutionEvent` records and inspect ROB, LSQ, rename destination, reservation calendar, completion payload, and retirement. Do not test only a classifier helper.

- [ ] **Step 2: Run the tests and preserve the red evidence**

```bash
cargo test -p rem6-cpu o3_runtime_memory_result_tests -- --nocapture
```

Expected: FAIL because FP/LR/AMO/vector rows are not deferred, AMO does not reserve its second live sequence, and memory-result publication is scalar-load-only.

- [ ] **Step 3: Implement exact access and destination policy**

In `o3_runtime_memory.rs`, replace scalar-only deferral classification with two explicit helpers:

```rust
fn is_scalar_window_access(access: &MemoryAccessKind) -> bool {
    matches!(access, MemoryAccessKind::Load { .. } | MemoryAccessKind::Store { .. })
}

fn o3_memory_result_destination(
    access: &MemoryAccessKind,
) -> Option<(O3RegisterClass, u32)> {
    let supported = match access {
        MemoryAccessKind::Load { rd, .. }
        | MemoryAccessKind::LoadReserved { rd, .. }
        | MemoryAccessKind::AtomicMemory { rd, .. } => !rd.is_zero(),
        MemoryAccessKind::FloatLoad { .. } => true,
        MemoryAccessKind::VectorLoadUnitStride {
            width,
            byte_len,
            byte_mask,
            group_registers,
            fault_only_first,
            ..
        } => {
            *width == MemoryWidth::Doubleword
                && *byte_len > 0
                && *byte_len <= RISCV_VECTOR_REGISTER_BYTES
                && *group_registers == 1
                && !*fault_only_first
                && byte_mask
                    .as_ref()
                    .is_none_or(|mask| mask.iter().any(|active| *active))
        }
        _ => false,
    };
    supported
        .then(|| o3_memory_destination_registers(access))
        .and_then(|destinations| (destinations.len() == 1).then_some(destinations[0]))
}
```

`is_deferred_o3_data_access` accepts existing scalar Load/Store plus accesses for which `o3_memory_result_destination` returns `Some`. Non-scalar result rows may stage only when `live_data_accesses` is empty; scalar overlap continues through the existing `can_stage_scalar_memory` policy.

- [ ] **Step 4: Unify sequence-span allocation**

Add:

```rust
fn o3_instruction_sequence_span(access: Option<&MemoryAccessKind>) -> u64 {
    u64::from(matches!(access, Some(MemoryAccessKind::AtomicMemory { .. }))) + 1
}

fn allocate_sequence_span(&mut self, span: u64) -> u64 {
    let first = self.next_sequence;
    self.next_sequence = self.next_sequence.saturating_add(span);
    first
}
```

Use it in both live staging and normal retirement. Remove the separate post-retire `AtomicMemory` increment. Store `lsq_sequence_span` in `O3LiveDataAccess`. Completion marks every LSQ entry in `[sequence, sequence + span)` complete; retry, failure, explicit squash, redirect, checkpoint rollback, and retirement remove the full span.

- [ ] **Step 5: Generalize the writeback-ready source and typed completion flow**

In `o3_runtime_writeback.rs`:

```text
O3LiveWritebackReady::scalar_load -> O3LiveWritebackReady::memory_result
O3LiveWritebackReadySource::ScalarLoad -> O3LiveWritebackReadySource::MemoryResult
```

At response time, construct one `RiscvDataCompletion` from the issued access. Store it in the live row only when `o3_memory_result_destination` is present. Reserve at `response_tick + 1`; resultless scalar stores and zero-destination rows receive no reservation.

The normal transport callback, MMIO callback, and `riscv_data_issue/forwarding.rs` local forwarded-load callback must all construct the same typed completion before removing `outstanding_data`. Forwarded scalar loads continue to use their overlaid bytes and forwarding plan.

At admitted publication, use this order:

```rust
let mut wake_tick = current_tick;
if let Some((fetch_request, issue_tick, publication_tick)) = state
    .o3_runtime
    .ready_live_data_access_completion_timing()
{
    wake_tick = publication_tick;
    let execution = crate::riscv_data_issue::record_deferred_o3_data_retire_cycle(
        &mut state,
        fetch_request,
        issue_tick,
        publication_tick,
    )
    .expect("completed O3 data access has a matching execution event");
    assert!(
        state
            .o3_runtime
            .replace_ready_live_data_access_execution(&execution),
        "completed O3 data access accepts its ordered pipeline retirement"
    );
}
let completion = state.o3_runtime.ready_live_memory_result_completion();
let execution = state.o3_runtime.take_ready_live_data_access_event(current_tick)?;
if let Some(completion) = completion.as_ref() {
    crate::riscv_data_completion::apply_completed_data_access(
        &mut state,
        self.id(),
        completion,
        "deferred O3 data response",
    );
    crate::riscv_checker::sync_checker_hart(&mut state);
}
state.wake_ready_o3_scalar_memory_younger_window(wake_tick, &fetch_events);
state
    .o3_runtime
    .record_retired_instruction_with_trace(&execution, trace_enabled);
```

For LR, install the reservation from the payload's physical address and size at admission. For vectors, normalize from the payload's request byte offset. Ensure `record_completed_load_data` runs once while the live row still exists.

- [ ] **Step 6: Make access permission and mode-transfer behavior fail closed**

Do not change PMP/PMA classification: AMO remains `Write`. Do not add non-scalar handoff encoding. A live non-scalar result must make `capture_o3_live_data_handoff_status` return `Rejected`; normal checkpoint capture then returns cpu0 `ComponentNotQuiescent`.

- [ ] **Step 7: Run focused CPU/system verification**

```bash
cargo test -p rem6-cpu o3_runtime_memory_result_tests -- --nocapture
cargo test -p rem6-cpu o3_runtime_memory_tests --quiet
cargo test -p rem6-cpu o3_runtime_writeback_tests --quiet
cargo test -p rem6-cpu --test riscv_translation_frontend --quiet
cargo test -p rem6-system --test riscv_checkpoint --quiet
cargo test -p rem6-cpu --test source_policy --quiet
cargo fmt --all -- --check
git diff --check
```

Expected: all PASS.

- [ ] **Step 8: Commit CPU behavior**

```bash
git add crates/rem6-cpu crates/rem6-system
git commit -m "feat: arbitrate O3 memory-result writeback"
git push origin main
```

### Task 4: Add Real Direct, Hierarchy, And MMIO Matrix Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs`
- Create: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`

- [ ] **Step 1: Add exact include and ownership policy first**

Add to `writeback_port.rs`:

```rust
include!("writeback_port/result_classes.rs");
```

Add `#[path = "source_policy/writeback_ownership.rs"] mod writeback_ownership;` to the source-policy root. The child policy must require exactly this include and exactly these anchors in the child file:

```text
rem6_run_o3_memory_result_writeback_matrix_direct
rem6_run_o3_memory_result_writeback_matrix_cache_fabric_dram
rem6_run_o3_memory_result_writeback_width_two_exact_fit
rem6_run_o3_memory_result_writeback_readfile_mmio
```

Ratchets: `writeback_port.rs < 1,300` lines and `result_classes.rs <= 700` lines.

- [ ] **Step 2: Write the positive tests before relying on implementation**

Create table-driven cases for:

```text
float_load: FLD f1, then FSD witness
load_reserved: LR.D x7, integer witness
atomic: AMOSWAP.D x11, integer plus final-memory witness
vector: masked VLE64.V v1 with vl=2/m1 and an inactive leading element, then VSE64.V witness
mmio: LD x12 from readfile device
```

Each memory-backed fixture also contains a fixed-latency DIV row. Use a bounded route-delay calibration set and require exactly one raw-ready collision. Width one must preserve older-sequence priority and one deferred row; width two must admit both at the shared raw-ready tick.

The direct test asserts transport activity and zero cache/fabric/DRAM activity. The hierarchy test covers FLD, AMO, and VLE64 through `cache-fabric-dram` and asserts all four resource layers. The MMIO test asserts device activity and zero ordinary memory transport/cache/fabric/DRAM activity.

For every case assert exact `lsq_data_response_tick + 1 == raw_ready_tick`, admitted/writeback/commit ordering, unready live ROB state before admission, no final witness before admission, final witness at or after admission, aggregate writeback-port counters, and ordered architectural completion. The vector case must additionally assert that the response request starts eight bytes after the architectural base and updates only the second 64-bit element.

- [ ] **Step 3: Run the CLI matrix**

```bash
cargo test -p rem6 --test cli_run o3_memory_result_writeback_matrix -- --nocapture
cargo test -p rem6 --test cli_run o3_memory_result_writeback_width_two -- --nocapture
cargo test -p rem6 --test cli_run o3_memory_result_writeback_readfile_mmio -- --nocapture
cargo test -p rem6 --test source_policy writeback -- --nocapture
```

Expected: PASS on the implemented CPU path. If a route-delay calibration has multiple or zero matches, reduce the candidate set and lock the unique deterministic value in the test.

- [ ] **Step 4: Commit positive executable evidence**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs \
  crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/writeback_ownership.rs \
  crates/rem6/tests/source_policy/core_test_anchors.txt
git commit -m "test: cover O3 memory-result writeback matrix"
git push origin main
```

### Task 5: Lock Suppression, Permission, Lifecycle, And Timing Boundaries

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_boundaries.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Test only if a real defect appears: focused CPU/system owner identified by the failing assertion

- [ ] **Step 1: Add representative negative real-binary tests**

Add `include!("writeback_port/result_boundaries.rs");` after the positive child include. Extend the ownership policy to require both includes in order and ratchet `result_boundaries.rs` to `<= 700` lines.

Add these anchors:

```text
rem6_run_o3_memory_result_writeback_rejects_resultless_and_unsupported_shapes
rem6_run_o3_memory_result_writeback_all_inactive_vector_issues_no_request
rem6_run_o3_memory_result_writeback_denied_amo_traps_before_transport
rem6_run_o3_memory_result_writeback_live_checkpoint_rejects
rem6_run_o3_memory_result_writeback_live_mode_switch_rejects
rem6_run_timing_suppresses_o3_memory_result_writeback_surface
```

Use table-driven resultless/unsupported representatives: integer load to x0, LR to x0, AMO to x0, SC, one LMUL2 unit-stride vector load, and one fault-only-first vector load. Assert no result reservation and no claimed terminal-lane row while preserving each instruction's existing architectural behavior.

The all-inactive masked unit-stride case must assert no Data request, no Memory request, no live ROB/LSQ row for the vector instruction, no writeback reservation, and preserved destination bytes.

The denied AMO case must assert the existing write-side protection trap, no transport request, no memory mutation, and no live O3 result state.

For live FLD or AMO, schedule host checkpoint and detailed-to-timing switch strictly after issue and before response. Both commands must fail with `checkpoint component is not quiescent: cpu0`, empty stdout, and no artifact. A drained checkpoint remains restorable through the existing test.

Timing mode must match final direct architecture while omitting `/cores/0/o3_runtime/writeback_port`, writeback calendar entries, O3 trace result rows, and all `sim.cpu0.o3.writeback_port.*` stats.

- [ ] **Step 2: Run the boundary tests**

```bash
cargo test -p rem6 --test cli_run o3_memory_result_writeback_rejects -- --nocapture
cargo test -p rem6 --test cli_run o3_memory_result_writeback_all_inactive -- --nocapture
cargo test -p rem6 --test cli_run o3_memory_result_writeback_denied_amo -- --nocapture
cargo test -p rem6 --test cli_run o3_memory_result_writeback_live -- --nocapture
cargo test -p rem6 --test cli_run timing_suppresses_o3_memory_result -- --nocapture
cargo test -p rem6 --test source_policy writeback -- --nocapture
```

Expected: all PASS. If a negative exposes a production bug, modify only the owning policy/callback/checkpoint module and add a focused unit regression before rerunning.

- [ ] **Step 3: Commit boundary evidence**

```bash
git add crates/rem6-cpu crates/rem6-system crates/rem6/tests
git commit -m "test: lock O3 memory-result boundaries"
git push origin main
```

### Task 6: Record Honest Migration Evidence

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update only the CPU component and test-anchor row**

Record:

- direct FP/LR/AMO/vector/MMIO result-writeback evidence;
- cache/fabric/DRAM FP/AMO/vector evidence;
- width-one and width-two collision behavior;
- typed completion payload preserving physical address, size, offset, and bytes;
- AMO two-sequence LSQ ownership and write-side permission behavior;
- all-inactive/no-destination/SC/vector-shape/timing suppression;
- live checkpoint and mode-switch nonquiescent rejection.

Keep these exact score statements unchanged:

```text
CPU Execution Models - 74% representative
8 of 10 items have executable evidence, or 80% raw
```

Keep open: general IQ/wakeup/select, broader multi-row FP/vector/atomic/MMIO windows, SC result arbitration, broad vector shapes, restorable transport ownership, and a general O3 engine.

- [ ] **Step 2: Add the representative test-anchor row without inflating score**

Add the new CLI child and its positive/negative anchors to the existing CPU O3 migration row. Remove equal obsolete prose so the ledger remains exactly 1,200 lines.

- [ ] **Step 3: Verify documentation policy**

```bash
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
cargo test -p rem6 --test source_policy --quiet
git diff --check
```

Expected: PASS.

- [ ] **Step 4: Commit the ledger update**

```bash
git add docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record O3 memory-result writeback"
git push origin main
```

### Task 7: Final Review, Verification, And Remote Equality

**Files:**
- Review all files changed since `1ca65d1d`

- [ ] **Step 1: Dispatch fresh read-only reviewers**

Use separate high-intensity reviewers for:

```text
1. Spec compliance and unsupported-boundary honesty.
2. Typed completion publication, LR physical reservation, vector offset, and exactly-once application.
3. AMO sequence-span, LSQ completion/removal, retry/squash, and permission semantics.
4. CLI route/timing fixtures, source-policy ownership, and migration-ledger accuracy.
```

Fix every concrete finding and re-run the affected reviewer until approved.

- [ ] **Step 2: Run focused verification on the final tree**

```bash
cargo test -p rem6-cpu o3_runtime_memory_result_tests --quiet
cargo test -p rem6-cpu o3_runtime_memory_tests --quiet
cargo test -p rem6-cpu o3_runtime_writeback_tests --quiet
cargo test -p rem6 --test cli_run o3_memory_result_writeback --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
```

- [ ] **Step 3: Run full verification**

```bash
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test --workspace --all-targets --quiet
cargo fmt --all -- --check
git diff --check
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
git diff --quiet -- temp temp/reference_designs/gem5
```

Expected: every command exits 0.

- [ ] **Step 4: Audit commits and push any review fix**

```bash
git status --short --branch
git log --oneline 1ca65d1d..HEAD
git diff --stat 1ca65d1d..HEAD
git push origin main
git rev-parse HEAD origin/main
```

Expected: clean `main`, local and remote SHA equal, and no `temp` changes.
