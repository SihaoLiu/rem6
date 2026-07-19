# RISC-V O3 Memory-Result Pair Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Admit exactly two independent result-producing data accesses followed by up to two scalar integer successors in the detailed RISC-V O3 window.

**Architecture:** Generalize the current scalar-suffix-specific authorization and policy into a typed per-fetch memory-result-window authority. Keep both data accesses as real live ROB/LSQ rows, reuse sequence-owned writeback and oldest-first retirement, and allow only an untranslated cacheable read as the younger result. Extract result fetch admission into a focused child module before adding the pair scanner.

**Tech Stack:** Rust workspace, `rem6-cpu`, top-level `rem6 run --execute` CLI tests, JSON/debug O3 artifacts, source-policy tests, Cargo.

---

### Task 1: Extract and Generalize Result Authorization Ownership

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch.rs`
- Modify: `crates/rem6-cpu/src/riscv_execute.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_gate.rs`
- Test: `crates/rem6-cpu/tests/source_policy.rs`

- [ ] **Step 1: Add a failing source-policy ownership test**

Require `detailed_o3.rs` to declare the focused child, require result probe and
authorization functions to live in the child, cap the child at 450 lines, and
reject stale production identifiers:

```rust
for stale in [
    "O3MemoryResultScalarSuffixAuthorization",
    "O3MemoryResultScalarSuffixRoute",
    "memory_result_scalar_suffix_authorizations",
    "MemoryResultScalarSuffix",
] {
    assert!(!production.contains(stale), "stale result-window name: {stale}");
}
```

- [ ] **Step 2: Run the source-policy test and verify RED**

Run: `cargo test -p rem6-cpu --test source_policy riscv_data_access_result_fetch_authority_is_focused -- --exact`

Expected: FAIL because the child module and generalized names do not exist.

- [ ] **Step 3: Extract the existing probe code without behavior changes**

Move `DataAccessResultHeadPhysicalProbe`, head/translation probes, result shape
classification, and authorization construction into the child. Re-export the
driver-facing items from `detailed_o3.rs`.

- [ ] **Step 4: Rename the typed owner everywhere**

Use these production names:

```rust
enum O3MemoryResultWindowRoute { Memory, Mmio }
enum O3MemoryResultWindowRole { Head, YoungerRead }
struct O3MemoryResultWindowAuthorization {
    integer_destination: Option<Register>,
    route: O3MemoryResultWindowRoute,
    physical_range: AddressRange,
    role: O3MemoryResultWindowRole,
}
```

Rename the core map to `memory_result_window_authorizations` and the runtime
policy variant to `O3DataAccessWindowPolicy::MemoryResultWindow`. Preserve all
existing clear/consume/quiescence paths.

- [ ] **Step 5: Run focused regression tests**

Run:

```bash
cargo test -p rem6-cpu --test source_policy
cargo test -p rem6-cpu data_access_result --lib
cargo test -p rem6-cpu result_younger_window --lib
```

Expected: all existing one-result scalar-suffix behavior remains green.

### Task 2: Add the Two-Result Runtime Window Model

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_memory_result_tests/pair_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_result_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`

- [ ] **Step 1: Write failing runtime tests**

Add tests that explicitly call `stage_live_data_access_issue` with
`MemoryResultWindow` and assert:

```rust
assert!(runtime.stage_live_data_access_issue(&first, request(20), 31, policy));
assert!(runtime.stage_live_data_access_issue(&second_read, request(21), 32, policy));
assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
assert_eq!(runtime.data_access_integer_window(second.fetch().request_id()).unwrap().rows(), 2);
assert!(!runtime.stage_live_data_access_issue(&third, request(22), 33, policy));
assert!(!runtime.stage_live_data_access_issue(&younger_atomic, request(23), 34, policy));
```

Cover FP plus vector, vector plus integer load, atomic-head plus FP load,
duplicate integer destinations, and scalar consumers of the first and second
integer results.

- [ ] **Step 2: Run the new module and verify RED**

Run: `cargo test -p rem6-cpu pair_window --lib`

Expected: FAIL at the current second-data-access rejection.

- [ ] **Step 3: Implement the bounded state**

Add `O3MemoryResultWindowState` with result-row count and ordered integer
destinations. Add:

```rust
RiscvScalarIntegerLiveWindow::from_memory_results(
    integer_destinations,
    occupied_rows,
    row_limit,
)
```

Permit one younger read-only result only when all existing result rows are
resident, no scalar younger sequence exists, and the total row limit allows it.
Keep atomic/LR/SC/MMIO/translated younger rows out of this method.

- [ ] **Step 4: Run the runtime tests and verify GREEN**

Run: `cargo test -p rem6-cpu pair_window --lib`

Expected: all pair-window admission and scalar dependency tests pass.

### Task 3: Authorize the Second Result During Fetch Ahead

**Files:**
- Create: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/data_access_result_pair.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3/data_access_result.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`

- [ ] **Step 1: Write failing fetch tests**

Build completed fetch streams for head plus second result and assert the
candidate returns the next PC with two per-fetch authorizations. Add negative
tests for dependent base/data operands, third result, translated second result,
MMIO second result, PMA-uncacheable range, unsupported vector shape, and split
fetch identity mismatch.

- [ ] **Step 2: Run and verify RED**

Run: `cargo test -p rem6-cpu data_access_result_pair --lib`

Expected: FAIL because the current scalar classifier stops at the second data
instruction and records only the head authorization.

- [ ] **Step 3: Implement the pair scanner**

Replace the single authorization candidate payload with:

```rust
ReadyDataAccessResultWindow {
    pc: Address,
    authorizations: Vec<(MemoryRequestId, O3MemoryResultWindowAuthorization)>,
}
```

Walk continuity using each instruction's last consumed request, key each token
by its first consumed request, admit one `YoungerRead` before scalar rows, and
record all returned authorizations before requesting the next fetch.

- [ ] **Step 4: Run and verify GREEN**

Run:

```bash
cargo test -p rem6-cpu data_access_result_pair --lib
cargo test -p rem6-cpu data_access_result --lib
```

Expected: pair tests pass and all head-only authorization tests remain green.

### Task 4: Wire Execute, Translation Selection, and Data Issue

**Files:**
- Create: `crates/rem6-cpu/src/riscv_data_issue_tests/result_pair_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/riscv_scalar_memory_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`

- [ ] **Step 1: Write failing issue-path tests**

Prove an authorized second integer/FP/vector read is selected while the first
request is outstanding, receives `MemoryResultWindow`, and consumes exactly its
token. Prove unauthorized, dependent, stale-range, route-changed, PMA-changed,
translated, MMIO, side-effecting, and third-result rows remain blocked.

- [ ] **Step 2: Run and verify RED**

Run: `cargo test -p rem6-cpu result_pair_window --lib`

Expected: FAIL because `next_unissued_data_access`, execution blocking, and
`stage_live_data_access_issue` still accept only scalar-memory overlap.

- [ ] **Step 3: Implement one shared overlap decision**

Add a state helper that returns true for either the established scalar-memory
window or an exact authorized younger result read. Use it from live-retire
execution blocking and `next_unissued_data_access`. In `record_data_issue_state`,
prefer an exact `YoungerRead` result token over ordinary scalar-load-prefix
classification when a result row is already live.

- [ ] **Step 4: Run and verify GREEN**

Run:

```bash
cargo test -p rem6-cpu result_pair_window --lib
cargo test -p rem6-cpu result_younger_window --lib
cargo test -p rem6-cpu --lib
```

Expected: focused tests and the complete CPU library pass.

### Task 5: Prove Completion, Publication, and Failure Ordering

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_result_tests/pair_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue/o3_callback.rs`

- [ ] **Step 1: Add failing ordering tests**

Complete the younger result first and assert `take_ready_live_data_access_event`
returns `None` until the older result is ready. Then retire the older row and
assert the younger result publishes as the new head. Add older retry/failure and
younger retry/failure cases that verify exact ROB/LSQ, pending-request, and
writeback cleanup.

- [ ] **Step 2: Run and verify RED or existing GREEN behavior**

Run: `cargo test -p rem6-cpu pair_window --lib`

Expected: oldest-first completion may already pass; any cleanup gap must fail on
an exact retained row/request/reservation assertion.

- [ ] **Step 3: Fix only demonstrated lifecycle gaps**

Preserve head-only publication through `live_data_accesses.first()`. Generalize
younger-request cleanup naming and behavior from scalar-memory-only to all live
data-window reads without adding younger-first publication.

- [ ] **Step 4: Run and verify GREEN**

Run: `cargo test -p rem6-cpu pair_window --lib`

Expected: all completion and rollback cases pass.

### Task 6: Add Real CLI Pair Evidence

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes/pairs.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/writeback_port/result_classes.rs`
- Modify: `crates/rem6/tests/source_policy/writeback_ownership.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`

- [ ] **Step 1: Add source-policy ownership for the new child**

Register `result_classes/pairs.rs`, cap it independently, update the family
aggregate for three focused files, and require these anchors:

```text
rem6_run_o3_memory_result_pair_matrix_direct
rem6_run_o3_memory_result_pair_matrix_cache_fabric_dram
rem6_run_o3_memory_result_pair_width_two_exact_fit_direct
rem6_run_o3_memory_result_pair_boundaries
rem6_run_timing_suppresses_o3_memory_result_pairs
```

- [ ] **Step 2: Write the failing CLI tests**

Build fixed-PC ELF fixtures for atomic-head/FP-read, vector-head/integer-read,
and FP-head/vector-read. Assert two requests issue before the first response,
four ROB rows, exact LSQ occupancy, scalar dependency timing, final witnesses,
and route resources. Add dependent-second, denied-head, third-result, and timing
boundaries.

- [ ] **Step 3: Run and verify RED**

Run: `cargo test -p rem6 --test cli_run memory_result_pair`

Expected: positive pair rows fail before the production path is complete.

- [ ] **Step 4: Calibrate writeback collision evidence**

Scan the existing finite route-delay candidates, require exactly one delay that
aligns the selected result raw-ready tick with the DIV raw-ready tick, and assert
width-one older priority versus width-two exact fit. Do not hardcode an
uncalibrated delay.

- [ ] **Step 5: Run and verify GREEN**

Run:

```bash
cargo test -p rem6 --test source_policy writeback_result_class_cli_evidence_has_focused_ownership -- --exact
cargo test -p rem6 --test cli_run memory_result_pair
cargo test -p rem6 --test cli_run o3_memory_result_writeback
cargo test -p rem6 --test cli_run scalar_suffix
```

Expected: new pair evidence and all previous result evidence pass.

### Task 7: Update the Ledger Without Moving the Score

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update executable-evidence prose**

Describe the exact two-result matrix, real route/resource evidence, scalar
wakeup, oldest-first publication, and negative boundaries. Narrow only the
covered one-result gap and keep translated/MMIO pairs, dependent address
generation, broader depth, general IQ, restorable transport, and general O3
open.

- [ ] **Step 2: Preserve mechanical invariants**

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
cargo test -p rem6 --test source_policy
```

Expected: exactly 1200 lines and 76 passing source-policy tests or the updated
expected total.

### Task 8: Review, Verify, Commit, and Push

**Files:**
- Review all modified `crates/**` and `docs/**` files.

- [ ] **Step 1: Run formatting and focused suites**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6-cpu --lib
cargo test -p rem6-cpu --test source_policy
cargo test -p rem6 --test source_policy
cargo test -p rem6 --test cli_run memory_result_pair
```

- [ ] **Step 2: Dispatch high-intensity read-only review**

Review runtime correctness, fetch authorization lifecycle, CLI evidence quality,
dead/stale names, and ledger honesty. Fix every high- or medium-severity finding
and rerun the affected tests.

- [ ] **Step 3: Run full verification**

Run:

```bash
git diff --check
cargo test --workspace --all-targets -q
```

Expected: exit code 0.

- [ ] **Step 4: Stage only repository evidence**

Run:

```bash
git add crates docs
git diff --cached --check
git diff --cached --name-only
```

Expected: no `temp/**` path is staged.

- [ ] **Step 5: Commit and push**

Run:

```bash
git commit -m "feat: admit paired O3 memory results"
git push origin main
```

Expected: `HEAD` and `origin/main` resolve to the same new commit and the
worktree is clean.
