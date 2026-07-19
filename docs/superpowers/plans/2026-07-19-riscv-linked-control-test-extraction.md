# RISC-V Linked-Control Test Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the linked-control and RAS-lineage test family from the nearly full detailed O3 control test owner without changing production behavior or test assertions.

**Architecture:** A path-owned `linked_control.rs` child contains the linked-call, return, coroutine, producer-forwarded, and RAS-lineage fixtures and tests. A focused source-policy ratchet locks the parent/child ownership boundary, forbids hidden includes, and gives both files explicit headroom.

**Tech Stack:** Rust workspace, `rem6-cpu` unit tests, source-policy file ownership checks, and real `rem6` CLI regression rows.

---

### Task 1: Add the RED module-boundary policy

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs:1-45`
- Modify: `crates/rem6-cpu/tests/source_policy.rs:370-395`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add focused line-budget constants**

Add beside the existing source-size constants:

```rust
const MAX_RISCV_DETAILED_O3_CONTROL_TEST_ROOT_LINES: usize = 450;
const MAX_RISCV_DETAILED_O3_LINKED_CONTROL_TEST_LINES: usize = 1500;
```

- [ ] **Step 2: Add the owner policy**

Add this test after `cpu_source_files_stay_within_size_limit`:

```rust
#[test]
fn riscv_detailed_o3_linked_control_tests_have_focused_owner() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root_path = crate_dir.join("src/riscv_fetch_ahead/tests/detailed_o3_control.rs");
    let linked_path =
        crate_dir.join("src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs");
    let root = fs::read_to_string(&root_path).unwrap();

    assert_eq!(
        path_owned_module_declaration_count(
            &root,
            "detailed_o3_control/linked_control.rs",
            "linked_control"
        ),
        1,
        "the detailed O3 control test root must declare its linked-control child"
    );
    assert!(
        linked_path.exists(),
        "linked-control tests belong in {}",
        linked_path.display()
    );

    let linked = fs::read_to_string(&linked_path).unwrap();
    for (path, source) in [(&root_path, &root), (&linked_path, &linked)] {
        let include_lines = include_macro_lines(source);
        assert!(
            include_lines.is_empty(),
            "{} must use path-owned modules instead of include! fragments; found lines {include_lines:?}",
            path.display(),
        );
    }

    let root_code = rust_code_without_comments_and_literals(&root);
    let linked_code = rust_code_without_comments_and_literals(&linked);
    for function in [
        "recorded_same_window_coroutine_core",
        "detailed_live_same_link_control_uses_runtime_forwarded_target",
        "detailed_scalar_window_direct_call_follows_target_and_pushes_ras",
        "detailed_recorded_coroutine_accepts_exact_pop_then_push",
        "detailed_invalid_recorded_return_does_not_retry_as_fresh_prediction",
    ] {
        assert!(
            production_defines_exact_function(&linked_code, function),
            "linked-control owner is missing `{function}`"
        );
        assert!(
            !production_defines_exact_function(&root_code, function),
            "detailed O3 control root still owns `{function}`"
        );
    }

    for function in [
        "live_same_link_core",
        "detailed_scalar_window_returns_existing_branch_prediction_decision",
        "detailed_control_target_authority_rejects_non_predicted_decision",
        "detailed_split_control_keys_prediction_to_prefix_request",
    ] {
        assert!(
            production_defines_exact_function(&root_code, function),
            "detailed O3 control root is missing `{function}`"
        );
        assert!(
            !production_defines_exact_function(&linked_code, function),
            "linked-control child must not own `{function}`"
        );
    }
    assert!(
        production_function_is_visible(&root_code, "live_same_link_core"),
        "live_same_link_core must remain visible to its sibling test caller"
    );

    let root_lines = line_count(&root_path);
    assert!(
        root_lines <= MAX_RISCV_DETAILED_O3_CONTROL_TEST_ROOT_LINES,
        "detailed_o3_control.rs exceeds its focused root budget: {root_lines}"
    );
    let linked_lines = line_count(&linked_path);
    assert!(
        linked_lines <= MAX_RISCV_DETAILED_O3_LINKED_CONTROL_TEST_LINES,
        "linked_control.rs exceeds its focused child budget: {linked_lines}"
    );
}
```

- [ ] **Step 3: Run the exact policy and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --test source_policy riscv_detailed_o3_linked_control_tests_have_focused_owner -- --exact --nocapture
```

Expected: FAIL because the root has no `linked_control` child declaration and
still owns the linked-control anchors.

### Task 2: Extract the linked-control test family

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs:1-1646`
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`

- [ ] **Step 1: Declare the path-owned child**

Immediately after `use super::*;` in the parent, add:

```rust
#[path = "detailed_o3_control/linked_control.rs"]
mod linked_control;
```

- [ ] **Step 2: Move the linked fixture block unchanged**

Create `linked_control.rs` with `use super::*;`, then move the complete item
block beginning at:

```rust
fn recorded_same_window_coroutine_core() -> RiscvCore {
```

and ending after:

```rust
fn recorded_second_linked_coroutine_pc(core: &RiscvCore) -> RecordedPredictedPc {
    // existing body unchanged
}
```

Move `producer_forwarded_target` from the parent into the child before this
block. Keep `detailed_linked_control_core` and `live_same_link_core` in the
parent because the latter has a sibling-module caller.

- [ ] **Step 3: Move the initial linked-call tests unchanged**

Move the complete test block beginning with:

```rust
#[test]
fn detailed_scalar_window_direct_call_follows_target_and_pushes_ras() {
```

and ending after:

```rust
#[test]
fn detailed_scalar_window_forwards_call_ras_to_same_window_coroutine() {
    // existing body unchanged
}
```

Keep `detailed_scalar_window_returns_existing_branch_prediction_decision` in
the parent.

- [ ] **Step 4: Move the recorded coroutine and return tests unchanged**

Move the complete test block beginning with:

```rust
#[test]
fn detailed_recorded_coroutine_accepts_exact_pop_then_push() {
```

and ending after:

```rust
#[test]
fn detailed_invalid_recorded_return_does_not_retry_as_fresh_prediction() {
    // existing body unchanged
}
```

Keep `detailed_control_target_authority_rejects_non_predicted_decision` in the
parent between the two moved test blocks.

- [ ] **Step 5: Format and verify line budgets**

Run:

```bash
cargo fmt --all
wc -l crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs
```

Expected: the parent is at or below 450 lines and the child is at or below
1,500 lines.

### Task 3: Turn the policy GREEN and verify unit boundaries

**Files:**
- Test: `crates/rem6-cpu/tests/source_policy.rs`
- Test: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`
- Test: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control/linked_control.rs`

- [ ] **Step 1: Run the exact source policy**

Run:

```bash
cargo test -p rem6-cpu --test source_policy riscv_detailed_o3_linked_control_tests_have_focused_owner -- --exact --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run representative linked-child tests**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control::linked_control::detailed_live_same_link_control_uses_runtime_forwarded_target -- --exact --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control::linked_control::detailed_recorded_coroutine_round_trip_rejects_stale_producer_stack -- --exact --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control::linked_control::detailed_invalid_recorded_return_does_not_retry_as_fresh_prediction -- --exact --nocapture
```

Expected: all PASS with unchanged assertions.

- [ ] **Step 3: Run retained-parent and sibling-caller tests**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control::detailed_scalar_window_returns_existing_branch_prediction_decision -- --exact --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::detailed_o3_control::detailed_split_control_keys_prediction_to_prefix_request -- --exact --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::producer_forwarded_control_validation::producer_forwarded_control_requires_exact_speculation_kind_and_ras -- --exact --nocapture
```

Expected: all PASS, proving the parent boundary and `live_same_link_core`
sibling caller remain intact.

### Task 4: Verify runtime evidence and close the increment

**Files:**
- Verify only: `docs/architecture/gem5-to-rem6-migration.md`
- Verify only: `temp/improve-rem6-0.md`

- [ ] **Step 1: Run all affected CPU targets**

Run:

```bash
cargo test -p rem6-cpu --all-targets
```

Expected: PASS.

- [ ] **Step 2: Run representative real CLI rows**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::link_return::rem6_run_o3_same_window_link_return_commits_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::coroutine::round_trip::rem6_run_o3_same_window_coroutine_round_trip_commits_direct -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::producer_forwarded_return::rem6_run_o3_producer_forwarded_return_descendants_cover_link_shape_route_matrix -- --exact --nocapture
```

Expected: all PASS.

- [ ] **Step 3: Run workspace and mechanical gates**

Run:

```bash
cargo test --workspace --all-targets -q
cargo fmt --all -- --check
git diff --check
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: all tests and checks PASS; protected status is empty; the ledger is
exactly 1,200 lines.

- [ ] **Step 4: Obtain independent read-only review**

Dispatch high-intensity reviewers for exact move preservation, module/privacy
correctness, source-policy honesty, and scope/ledger integrity. Resolve every
actionable finding before staging.

- [ ] **Step 5: Commit and push the exact increment**

Stage only the two documentation files, the parent, the new child, and the
source-policy file. Commit with:

```bash
git commit -m "refactor: extract RISC-V linked-control tests"
git push origin main
```

Verify `git status --short --branch` is clean and `HEAD` equals `origin/main`.
