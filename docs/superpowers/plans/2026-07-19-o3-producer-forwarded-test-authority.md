# O3 Producer-Forwarded Test Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move four producer-forwarded O3 test hooks out of production-owned source while preserving every caller and assertion unchanged.

**Architecture:** A raw-source policy rejects the four helper definitions in non-test paths. Three `O3RuntimeState` methods move to a test-only sibling of `o3_runtime`; the scalar-chain helper moves to a test-only child of its private value owner.

**Tech Stack:** Rust workspace, `rem6-cpu`, source-policy path classification, private module test support, and real `rem6` CLI tests.

---

### Task 1: Add the RED source-placement policy

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs:2145-2180`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add the path-aware test helper policy**

Add this focused test beside `producer_forwarded_chain_authority_stays_focused`:

```rust
#[test]
fn producer_forwarded_test_helpers_live_only_in_test_modules() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let forbidden = [
        "retire_producer_forwarded_data_head_for_test",
        "producer_forwarded_scalar_return_issue_tick_for_test",
        "replace_producer_forwarded_chain_fetch_identity_for_test",
        "repeated_last_for_test",
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
        "producer-forwarded test helpers must live in test-only modules, not production source files: {}",
        offenders.join(", ")
    );
}
```

Do not use `production_rust_source`; it intentionally removes the current
`#[cfg(test)]` definitions before inspection.

- [ ] **Step 2: Run the exact policy and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --test source_policy producer_forwarded_test_helpers_live_only_in_test_modules -- --exact --nocapture
```

Expected: FAIL and report three definitions in
`src/o3_runtime_producer_forwarded_chain.rs` plus one definition in
`src/o3_runtime_producer_forwarded_chain/value.rs`.

### Task 2: Relocate the runtime-level hooks

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime.rs:50-66`
- Modify: `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs:556-600`
- Create: `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain_tests.rs`

- [ ] **Step 1: Declare the test-only runtime sibling**

Immediately after the production producer-forwarded owner declaration in
`o3_runtime.rs`, add:

```rust
#[cfg(test)]
#[path = "o3_runtime_producer_forwarded_chain_tests.rs"]
mod o3_runtime_producer_forwarded_chain_tests;
```

- [ ] **Step 2: Move the three methods unchanged**

Create `o3_runtime_producer_forwarded_chain_tests.rs`:

```rust
use super::*;

impl O3RuntimeState {
    pub(crate) fn retire_producer_forwarded_data_head_for_test(
        &mut self,
        retire_tick: u64,
    ) -> bool {
        if self.live_data_accesses.len() != 1
            || self.producer_forwarded_scalar_chain().is_none()
            || self
                .snapshot
                .reorder_buffer
                .first()
                .map(|entry| entry.sequence())
                != self.live_data_accesses.first().map(|head| head.sequence)
        {
            return false;
        }
        self.live_data_accesses.clear();
        self.snapshot.reorder_buffer.remove(0);
        self.last_live_commit_tick = Some(retire_tick);
        true
    }

    pub(crate) fn producer_forwarded_scalar_return_issue_tick_for_test(&self) -> Option<u64> {
        let sequence = self.producer_forwarded_return_descendant()?.sequence();
        self.live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == sequence)
            .map(|issued| issued.issue_tick)
    }

    pub(crate) fn replace_producer_forwarded_chain_fetch_identity_for_test(
        &mut self,
        sequence: u64,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        let Some(issued) = self
            .live_speculative_executions
            .iter_mut()
            .find(|issued| issued.sequence == sequence)
        else {
            return false;
        };
        issued.consumed_requests = consumed_requests.to_vec();
        true
    }
}
```

Delete the identical three `#[cfg(test)]` methods from
`o3_runtime_producer_forwarded_chain.rs`. Do not change callers or method
visibility.

- [ ] **Step 3: Verify cross-module callers still pass**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::producer_forwarded_chain_validation::direct_return_apply_fails_closed_after_fetch_identity_changes -- --exact --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::producer_forwarded_scalar_return::scalar_return_issue_waits_for_data_head_retirement_tick -- --exact --nocapture
```

Expected: both PASS. The source-placement policy remains RED only because
`repeated_last_for_test` still lives in `value.rs`.

### Task 3: Relocate the scalar-chain helper beside its private owner

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value.rs:1-5,260-273`
- Create: `crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value_tests.rs`

- [ ] **Step 1: Declare the test-only value child**

After `use super::*;` in `value.rs`, add:

```rust
#[cfg(test)]
#[path = "value_tests.rs"]
mod tests;
```

- [ ] **Step 2: Move the repeated-chain method unchanged**

Create `value_tests.rs`:

```rust
use super::*;

impl O3ProducerForwardedScalarChain {
    pub(crate) fn repeated_last_for_test(&self) -> Self {
        let mut repeated = self.clone();
        if let Some(descendant) = repeated.last() {
            assert!(repeated.push(descendant));
        }
        repeated
    }
}
```

Delete the identical `#[cfg(test)]` method from `value.rs`. Do not expose
`push`, descendants, or scalar-chain fields.

- [ ] **Step 3: Verify retained-chain rejection and line budgets**

Run:

```bash
cargo test -p rem6-cpu --lib o3_runtime::o3_runtime_control_window_tests::producer_forwarded_chain_validation::retained_scalar_chain_rejects_longer_candidate -- --exact --nocapture
wc -l crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value.rs crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value_tests.rs
```

Expected: the test PASSes; the production root remains at or below 650 lines,
the value owner remains at or below 400 lines, and the aggregate module family
remains at or below 1,000 lines.

### Task 4: Turn the source policy GREEN

**Files:**
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Run the new and existing focused policies**

Run:

```bash
cargo test -p rem6-cpu --test source_policy producer_forwarded_test_helpers_live_only_in_test_modules -- --exact --nocapture
cargo test -p rem6-cpu --test source_policy producer_forwarded_chain_authority_stays_focused -- --exact --nocapture
```

Expected: both PASS. The new helper definitions are ignored only because their
paths are classified as test-only.

- [ ] **Step 2: Verify helper placement**

Run:

```bash
rg -n "fn (retire_producer_forwarded_data_head_for_test|producer_forwarded_scalar_return_issue_tick_for_test|replace_producer_forwarded_chain_fetch_identity_for_test|repeated_last_for_test)" crates/rem6-cpu/src
```

Expected: the first three definitions appear only in
`src/o3_runtime_producer_forwarded_chain_tests.rs`; the repeated-chain method
appears only in `src/o3_runtime_producer_forwarded_chain/value_tests.rs`.

### Task 5: Verify runtime behavior, review, commit, and push

**Files:**
- Verify only: `docs/architecture/gem5-to-rem6-migration.md`
- Verify only: `temp/improve-rem6-0.md`

- [ ] **Step 1: Run all affected CPU tests**

Run:

```bash
cargo test -p rem6-cpu --all-targets
```

Expected: PASS with unchanged fetch-ahead locking, identity-failure,
retirement-tick, and retained-chain assertions.

- [ ] **Step 2: Run representative real CLI rows**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::producer_forwarded_return::rem6_run_o3_producer_forwarded_return_descendants_cover_link_shape_route_matrix -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::producer_forwarded_scalar_return::rem6_run_o3_producer_forwarded_scalar_returns_cover_link_shape_route_matrix -- --exact --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::producer_forwarded_scalar_return::rem6_run_o3_producer_forwarded_scalar_return_rejects_non_link_scalar -- --exact --nocapture
```

Expected: all PASS with unchanged direct/cache-fabric-DRAM link-shape coverage
and non-link scalar rejection.

- [ ] **Step 3: Run final workspace and hygiene verification**

Run:

```bash
cargo fmt --all -- --check
cargo test --workspace --all-targets -q
git diff --check
wc -l crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value.rs docs/architecture/gem5-to-rem6-migration.md
git status --short -- temp docs/architecture/gem5-to-rem6-migration.md
```

Expected: every command exits successfully; the production owner files remain
within source-policy caps; the ledger remains exactly 1,200 lines; protected
paths are untouched.

- [ ] **Step 4: Request independent read-only review**

Reviewers must check definition-only relocation, private-field access,
cross-module caller availability, source-policy path classification, line
budgets, focused and CLI behavior, dead code, and ledger honesty. Address
actionable findings and rerun affected tests.

- [ ] **Step 5: Commit and push**

```bash
git add \
  docs/superpowers/specs/2026-07-19-o3-producer-forwarded-test-authority-design.md \
  docs/superpowers/plans/2026-07-19-o3-producer-forwarded-test-authority.md \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain.rs \
  crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain_tests.rs \
  crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value.rs \
  crates/rem6-cpu/src/o3_runtime_producer_forwarded_chain/value_tests.rs \
  crates/rem6-cpu/tests/source_policy.rs
git commit -m "refactor: isolate O3 producer-forwarded test authority"
git push origin main
```

Verify `git status --short --branch` is clean and `git rev-parse HEAD` equals
`git rev-parse origin/main`.
