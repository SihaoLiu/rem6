# RISC-V O3 Younger Atomic Result Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Admit one independent unordered AMO as the buffered second result in the bounded four-row detailed-O3 memory-result window.

**Architecture:** Add a typed younger buffered-effect authorization, reuse the existing memory-result ROB/LSQ/writeback path, and generalize predecessor-gated store buffering into effect buffering so the AMO owns O3 state before the older read completes without reaching transport early. Keep SC, translation, MMIO, dependent addresses, atomic/LR heads, and broader side-effect chains closed.

**Tech Stack:** Rust workspace, rem6 RISC-V CPU/O3 runtime, rem6 transport and memory hierarchy, top-level `rem6 run --execute` CLI tests, TOML/JSON artifacts, source-policy ratchets.

---

### Task 1: Extract And Type Memory-Result Authorization

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/memory_result_authorization.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [x] **Step 1: Add a failing source-policy test for focused ownership**

Add `riscv_memory_result_authorization_has_focused_ownership` that requires:

```rust
#[path = "riscv_fetch_ahead/memory_result_authorization.rs"]
mod memory_result_authorization;
```

and rejects production occurrences of:

```rust
BufferedO3Store
buffered_o3_stores
```

outside the policy file while ratcheting the new authorization owner to at
most 150 lines.

- [x] **Step 2: Run the policy test and verify RED**

Run:

```bash
cargo test -p rem6-cpu --test source_policy riscv_memory_result_authorization_has_focused_ownership -- --exact
```

Expected: FAIL because the focused module and generalized buffered-effect names
do not exist.

- [x] **Step 3: Move the authorization types into the focused module**

Move `O3MemoryResultWindowRoute`, `O3MemoryResultWindowRole`, and
`O3MemoryResultWindowAuthorization` out of `riscv_fetch_ahead.rs`. Extend the
role with:

```rust
pub(crate) enum O3MemoryResultWindowRole {
    Head,
    YoungerRead,
    YoungerBufferedEffect,
}
```

Add focused predicates:

```rust
pub(crate) const fn is_younger(self) -> bool;
pub(crate) const fn is_buffered_effect(self) -> bool;
```

Re-export the types from `riscv_fetch_ahead.rs` so existing users retain one
public crate path.

- [x] **Step 4: Run focused authorization and existing pair tests**

Run:

```bash
cargo test -p rem6-cpu --test source_policy riscv_memory_result_authorization_has_focused_ownership -- --exact
cargo test -p rem6-cpu --lib data_access_result_pair -- --nocapture
```

Expected: PASS with no behavior change.

### Task 2: Authorize Exact Younger Atomic Effects

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_effect_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result_pair_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result_effect.rs`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`

- [x] **Step 1: Write failing fetch-policy tests**

Cover these exact cases:

```rust
#[test]
fn float_head_authorizes_a_disjoint_unordered_younger_amo_effect();

#[test]
fn masked_vector_head_authorizes_a_disjoint_unordered_younger_amo_effect();

#[test]
fn younger_amo_effect_rejects_ordering_overlap_dependencies_and_zero_destination();

#[test]
fn scalar_load_head_selects_the_effect_lane_only_for_an_adjacent_atomic();

#[test]
fn atomic_lr_translation_and_mmio_heads_do_not_open_the_effect_lane();

#[test]
fn a_second_effect_third_result_or_scalar_before_effect_stays_outside_the_window();
```

The positive tests must assert two exact authorizations, with the second role
equal to `YoungerBufferedEffect`. Negative tests must assert no stale younger
authorization.

- [x] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test -p rem6-cpu --lib data_access_result_effect -- --nocapture
```

Expected: FAIL because the effect role is never emitted.

- [x] **Step 3: Add focused younger-effect policy**

Implement a focused predicate with this contract:

```rust
fn result_head_allows_younger_effect(
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    younger_authorization: O3MemoryResultWindowAuthorization,
) -> bool;
```

It must accept only scalar `Load`, `FloatLoad`, or the existing supported
vector-load head and an unordered atomic younger row with independent integer
sources and disjoint physical range. Keep the existing read/read and
atomic/read policy in `data_access_result_pair_policy.rs`.

For scalar-load heads, probe this effect candidate only after the completed
adjacent younger AMO is available. Fall back to the existing
`ScalarMemoryPrefix` candidate for every non-effect shape so load-plus-ALU and
load-prefix behavior remain unchanged.

- [x] **Step 4: Emit `YoungerBufferedEffect` from result scanning**

Generalize the younger authorization helper so it selects:

```rust
MemoryAccessKind::Load | FloatLoad | VectorLoadUnitStride => YoungerRead
MemoryAccessKind::AtomicMemory { acquire: false, release: false, rd, .. }
    if !rd.is_zero() => YoungerBufferedEffect
```

Keep translation, MMIO, unsupported vector, SC, LR, store, zero-destination,
and PMA exclusions unchanged.

- [x] **Step 5: Run fetch tests and source policy**

Run:

```bash
cargo test -p rem6-cpu --lib data_access_result_effect -- --nocapture
cargo test -p rem6-cpu --lib data_access_result_pair -- --nocapture
cargo test -p rem6-cpu --test source_policy riscv_data_access_result_fetch_authority_is_focused -- --exact
```

Expected: PASS.

### Task 3: Admit The Atomic As Result Row Two

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_memory_result_tests/younger_effect.rs`
- Modify: `crates/rem6-cpu/src/riscv_memory_result_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Create: `crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_effect.rs`

- [x] **Step 1: Write failing runtime admission tests**

Add tests proving:

```rust
#[test]
fn result_window_accepts_a_disjoint_unordered_atomic_as_row_two();

#[test]
fn younger_atomic_owns_one_rob_row_and_two_lsq_entries();

#[test]
fn younger_atomic_result_stages_two_scalar_rows_and_wakes_its_consumer();

#[test]
fn younger_atomic_rejects_nonread_heads_ordering_overlap_and_dependencies();

#[test]
fn older_retry_discards_the_atomic_span_and_scalar_suffix();
```

- [x] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test -p rem6-cpu --lib younger_effect -- --nocapture
cargo test -p rem6-cpu --lib result_younger_effect -- --nocapture
```

Expected: FAIL because runtime admission accepts only younger reads.

- [x] **Step 3: Generalize younger result classification**

Replace the read-only runtime helper with role-neutral supported-result helpers.
`can_stage_memory_result_window_access` must use this matrix:

```rust
(pure_read_head, unordered_atomic_younger) => disjoint
(unordered_atomic_head, pure_read_younger) => disjoint
(pure_read_head, pure_read_younger) => true
_ => false
```

Do not admit atomic/atomic, LR/effect, SC, store, translated/MMIO-specific
shapes, or ordered atomics.

- [x] **Step 4: Revalidate role and exact range before issue**

Update `riscv_memory_result_window.rs` and `riscv_data_issue.rs` so the expected
younger role is derived from the actual access. A younger AMO must match
`YoungerBufferedEffect`, exact memory route/range, cacheable PMA, nonzero result
destination, and runtime capacity.

- [x] **Step 5: Run runtime and issue tests**

Run:

```bash
cargo test -p rem6-cpu --lib younger_effect -- --nocapture
cargo test -p rem6-cpu --lib result_younger_effect -- --nocapture
cargo test -p rem6-cpu --lib pair_window -- --nocapture
cargo test -p rem6-cpu --lib result_pair_window -- --nocapture
```

Expected: PASS.

### Task 4: Generalize Store Buffering Into Effect Buffering

**Files:**
- Rename: `crates/rem6-cpu/src/riscv_data_issue/buffered_store.rs` to `crates/rem6-cpu/src/riscv_data_issue/buffered_effect.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/prepared.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster_drive.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- Modify: store-prefix tests under `crates/rem6-cpu/src/riscv_data_issue_tests/`

- [x] **Step 1: Add failing serial and parallel buffering tests**

Add exact tests:

```rust
#[test]
fn younger_atomic_issue_is_recorded_but_transport_waits_for_the_head();

#[test]
fn completed_head_releases_exactly_one_buffered_atomic_serially();

#[test]
fn completed_head_releases_exactly_one_buffered_atomic_in_parallel();

#[test]
fn buffered_scalar_store_handoff_behavior_is_unchanged();
```

The pre-release test must assert the AMO request is present in O3 and
`outstanding_data`, present in the buffered-effect map, and absent from target
calls.

- [x] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test -p rem6-cpu --lib buffered_atomic -- --nocapture
```

Expected: FAIL because only scalar stores receive predecessors.

- [x] **Step 3: Perform the mechanical effect rename**

Rename these production authorities consistently:

```text
BufferedO3Store -> BufferedO3Effect
buffered_o3_stores -> buffered_o3_effects
BufferedStore -> BufferedEffect
has_ready_buffered_o3_store -> has_ready_buffered_o3_effect
ready_buffered_o3_store -> ready_buffered_o3_effect
submit/prepare/schedule/record_buffered_o3_store_* -> *_buffered_o3_effect_*
```

Keep the handoff enum's serialized `BufferedStore` spelling unchanged because
only scalar stores are projected into that existing schema.

- [x] **Step 4: Add a unified predecessor selector**

Replace `o3_store_predecessor` with:

```rust
fn o3_buffered_effect_predecessor(
    &self,
    issue: &OutstandingDataAccess,
) -> Option<MemoryRequestId>;
```

It first asks the runtime for a scalar-store predecessor, then for a younger
atomic-result predecessor. The runtime result predecessor must return the live
head request only for an admitted `YoungerBufferedEffect` row.

- [x] **Step 5: Release buffered effects through existing guarded submission**

Use the same serial and parallel ownership guard for stores and atomic effects.
`BufferedO3Effect::scalar_memory_handoff` must return `None` for AMO, causing
live handoff capture to reject without a new schema.

- [x] **Step 6: Run buffering and existing store-prefix suites**

Run:

```bash
cargo test -p rem6-cpu --lib buffered_atomic -- --nocapture
cargo test -p rem6-cpu --lib store_store_load -- --nocapture
cargo test -p rem6 --test cli_run disjoint_store_prefix -- --nocapture
```

Expected: PASS.

### Task 5: Prove Cancellation And Lifecycle Cleanup

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_gate.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_effect.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result_effect.rs`

- [x] **Step 1: Add failing cleanup tests**

Cover:

```rust
#[test]
fn older_retry_cancels_buffered_atomic_before_target_delivery();

#[test]
fn older_failure_cancels_buffered_atomic_before_target_delivery();

#[test]
fn redirect_and_mode_disable_remove_buffered_effect_authority();

#[test]
fn younger_retry_preserves_only_the_completed_older_result();

#[test]
fn aborting_the_head_removes_both_younger_roles();
```

- [x] **Step 2: Run the tests and verify RED**

Run:

```bash
cargo test -p rem6-cpu --lib result_younger_effect -- --nocapture
cargo test -p rem6-cpu --lib data_access_result_effect -- --nocapture
```

Expected: at least one stale authorization or buffered-effect assertion fails.

- [x] **Step 3: Centralize younger-role cleanup**

Replace equality checks against `YoungerRead` with `role().is_younger()` in
abort, retry/failure, mode-disable, reset, translation-fault, and fetch cleanup.
Remove both the outstanding request and buffered-effect entry for every
squashed younger live row.

- [x] **Step 4: Run lifecycle tests**

Run:

```bash
cargo test -p rem6-cpu --lib result_younger_effect -- --nocapture
cargo test -p rem6-cpu --lib data_access_result_effect -- --nocapture
cargo test -p rem6-cpu --lib result_pair_window -- --nocapture
cargo test -p rem6-cpu --lib result_younger_window -- --nocapture
```

Expected: PASS.

### Task 6: Add Top-Level Younger Atomic CLI Evidence

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/younger_atomic_result.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`

- [x] **Step 1: Add a failing CLI matrix**

Add these exact anchors:

```rust
rem6_run_o3_younger_atomic_result_matrix_direct
rem6_run_o3_younger_atomic_result_matrix_cache_fabric_dram
rem6_run_o3_younger_atomic_result_boundaries_and_live_actions
rem6_run_timing_suppresses_o3_younger_atomic_result
```

Use three fixtures:

```text
LD -> AMOSWAP.D -> DIV -> ADDI(AMO result)
FLD -> AMOSWAP.D -> DIV -> ADDI(AMO result)
masked e64/m1 VLE64.V -> AMOADD.D -> DIV -> ADDI(AMO result)
```

All three fixtures must store final FP/vector and scalar witnesses and dump the
atomic target bytes.

- [x] **Step 2: Prove the CLI test is RED**

Run:

```bash
cargo test -p rem6 --test cli_run younger_atomic_result -- --nocapture
```

Expected: FAIL because the younger AMO remains serialized outside the result
window.

- [x] **Step 3: Assert the positive matrix**

For direct and hierarchy rows assert:

```text
ROB count before head response = 4
LSQ count = head load rows + AMO load/store rows
AMO issue_tick < head response tick
pre-response data request_sent count = 1
AMO request_sent tick >= head response tick
dependent scalar issue_tick >= AMO writeback_tick
commit ticks are sequence ordered
final registers and memory bytes match
```

Direct rows must show transport activity without cache/fabric/DRAM activity.
Hierarchy rows must show nonzero cache, fabric, and DRAM activity.

- [x] **Step 4: Add boundary and lifecycle rows**

The boundary test must serialize ordered, overlapping, dependent-source, and
zero-destination atomics. It must also attempt a live checkpoint or mode switch
while the AMO is buffered and assert explicit rejection.

The timing test must preserve final architecture and memory while exposing no
O3 runtime or O3 debug trace surface.

- [x] **Step 5: Ratchet focused CLI ownership**

Ratchet `younger_atomic_result.rs` to 450 lines, its `boundaries.rs` child to
350 lines, and the aggregate family to 750 lines with exact anchor inventory.
Do not increase the result-class family cap. Add the four anchors to
`core_test_anchors.txt`.

- [x] **Step 6: Run CLI and policy tests**

Run:

```bash
cargo test -p rem6 --test cli_run younger_atomic_result -- --nocapture
cargo test -p rem6 --test cli_run memory_result_pair -- --nocapture
cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --exact
cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --exact
```

Expected: PASS.

### Task 7: Update The Migration Ledger Honestly

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [x] **Step 1: Update the CPU evidence prose only after CLI tests pass**

Add the predecessor-gated younger atomic matrix to `Migrated`. Narrow
`Not migrated` and `Next evidence` from all younger side-effecting results to:

```text
SC, ordinary younger stores, broader atomic chains, translated/MMIO effects,
dependent result addresses, and restorable transport ownership
```

Keep 8 of 10, 80% raw, 74% representative.

- [x] **Step 2: Preserve the 1,200-line invariant**

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: exactly `1200`.

- [x] **Step 3: Run ledger policy tests**

Run:

```bash
cargo test -p rem6 --test source_policy architecture_docs_have_clear_boundaries -- --exact
cargo test -p rem6 --test source_policy gem5_migration_sections_are_auditable -- --exact
cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --exact
```

Expected: PASS.

### Task 8: Review, Verify, Commit, And Push

**Files:**
- Review all modified production, test, policy, and documentation files.

- [x] **Step 1: Run focused regression suites**

Run:

```bash
cargo test -p rem6-cpu --lib data_access_result_effect -- --nocapture
cargo test -p rem6-cpu --lib result_younger_effect -- --nocapture
cargo test -p rem6-cpu --lib younger_effect -- --nocapture
cargo test -p rem6 --test cli_run younger_atomic_result -- --nocapture
cargo test -p rem6 --test cli_run memory_result_pair -- --nocapture
cargo test -p rem6 --test cli_run disjoint_store_prefix -- --nocapture
```

Expected: PASS.

- [x] **Step 2: Run source-policy, formatting, and diff checks**

Run:

```bash
cargo test -p rem6-cpu --test source_policy
cargo test -p rem6 --test source_policy
cargo fmt --all -- --check
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: all tests pass, formatting/diff checks are clean, ledger is 1,200
lines.

- [x] **Step 3: Dispatch high-intensity read-only review**

Review for:

- early target mutation;
- rollback gaps;
- stale younger roles or buffered effects;
- serialized-schema drift;
- source-policy cap inflation;
- dead compatibility names; and
- migration claims that exceed executable evidence.

Fix every substantive finding and rerun affected tests.

- [x] **Step 4: Run the full workspace**

Run:

```bash
cargo test --workspace --all-targets -q
```

Expected: exit 0 with zero failed tests.

- [ ] **Step 5: Commit the mechanical generalization**

If the buffered-effect rename is independently clean after review:

```bash
git add crates/rem6-cpu/src crates/rem6-cpu/tests/source_policy.rs
git commit -m "refactor: generalize buffered O3 effects"
```

Otherwise keep it in the behavior commit to avoid a partially useful boundary.

- [ ] **Step 6: Commit the behavior, evidence, and ledger**

```bash
git add crates docs
git diff --cached --check
git commit -m "feat: buffer younger O3 atomic results"
```

- [ ] **Step 7: Push and verify the remote state**

```bash
git push origin main
git rev-parse HEAD origin/main
git status --short
```

Expected: `HEAD` equals `origin/main` and the worktree is clean.
