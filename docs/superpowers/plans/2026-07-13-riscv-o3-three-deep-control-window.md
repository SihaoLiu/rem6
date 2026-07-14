# RISC-V O3 Three-Deep Control Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support and verify a four-row detailed O3 window containing one delayed scalar load plus three mixed direct conditional controls under branch lookahead three.

**Architecture:** Move branch-lookahead bounds into the existing public RISC-V defaults authority, enforce them in both direct CPU configuration and CLI/TOML parsing, and let the existing immediate control-dependency runtime scale to a third branch. Keep top-level evidence in a new `three_deep.rs` module and extract the duplicated predicted-control command/JSON helpers before adding the matrix.

**Tech Stack:** Rust workspace, `rem6-cpu`, `rem6` CLI, scheduler-backed detailed/timing execution, JSON debug/runtime artifacts, Cargo tests, source-policy tests.

---

## File Structure

- Modify `crates/rem6-cpu/src/riscv_defaults.rs`: own minimum, default, and maximum branch-lookahead constants.
- Modify `crates/rem6-cpu/src/lib.rs`: enforce the shared range in `set_branch_lookahead` and use the default constant in core state.
- Modify `crates/rem6/src/config.rs`: use the shared default for file/CLI configuration.
- Modify `crates/rem6/src/config/riscv_branch.rs`: validate against the shared inclusive range.
- Modify `crates/rem6/tests/cli_run/validation.rs`: prove three is valid and zero/four are invalid.
- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: derive predicted-control depth from the shared maximum.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`: cover three recorded predictions and lookahead-two suppression.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`: cover immediate three-level ownership and middle rollback.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs`: shared command, JSON, resident-ROB, and no-data-address helpers.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`: include shared support and the new matrix module.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs`: consume shared support without changing behavior.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/three_deep.rs`: original three-deep CLI matrix.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: anchor every original CLI row.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record evidence honestly while preserving 74% and exactly 1,200 lines.

### Task 1: Extract Shared Predicted-Control CLI Support

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs`

- [ ] **Step 1: Include the focused support module**

Add beside the existing nested module declaration:

```rust
#[path = "predicted_control/window_support.rs"]
mod window_support;
```

- [ ] **Step 2: Move duplicated command and JSON parsing into shared support**

Create `window_support.rs` with these functions:

```rust
use super::*;

pub(super) fn control_window_command(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
    data_address: &str,
    dump_bytes: u64,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,Fetch,Memory,HostAction",
        "--riscv-branch-lookahead",
        &branch_lookahead.to_string(),
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        execution_mode,
        "--dump-memory",
        &format!("{data_address}:{dump_bytes}"),
    ]);
    command
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_control_window_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    branch_lookahead: usize,
    data_address: &str,
    dump_bytes: u64,
    extra_args: &[&str],
) -> Value {
    let mut command = control_window_command(
        path,
        memory_system,
        max_tick,
        execution_mode,
        branch_lookahead,
        data_address,
        dump_bytes,
    );
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} {execution_mode} lookahead={branch_lookahead}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid control-window JSON: {error}"))
}

pub(super) fn resident_rob_pcs(json: &Value) -> Vec<&str> {
    json.pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing resident control-window ROB: {json}"))
        .iter()
        .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
        .collect()
}

pub(super) fn assert_no_data_address(json: &Value, address: &str) {
    for pointer in ["/debug/data_trace", "/debug/memory_trace"] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_array)
                .is_some_and(|records| records.iter().all(|record| {
                    record.pointer("/address").and_then(Value::as_str) != Some(address)
                })),
            "unexpected data access at {address}: {json}"
        );
    }
}
```

- [ ] **Step 3: Rewire nested control wrappers**

Import shared helpers:

```rust
use super::window_support::{
    assert_no_data_address, control_window_command, resident_rob_pcs, run_control_window_json,
};
use super::*;
```

Make `nested_control_command_with_lookahead` delegate to
`control_window_command(..., DATA_ADDRESS, 16)`. Make both JSON wrappers delegate
to `run_control_window_json`. Delete the local `resident_rob_pcs` and
`assert_no_data_address` implementations.

- [ ] **Step 4: Verify behavior is unchanged**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::nested -- --nocapture
```

Expected: 10 passed, 0 failed.

- [ ] **Step 5: Commit the mechanical cleanup**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/nested.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/window_support.rs
git commit -m "test: share predicted control run support"
```

### Task 2: Centralize and Enforce Branch Lookahead Bounds

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_defaults.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6/src/config.rs`
- Modify: `crates/rem6/src/config/riscv_branch.rs`
- Modify: `crates/rem6/tests/cli_run/validation.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs`

- [ ] **Step 1: Write failing CLI validation coverage**

Change the invalid-value matrix to `0` and `4`, then add a successful run with
lookahead three:

```rust
#[test]
fn rem6_run_accepts_riscv_branch_lookahead_three() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-branch-lookahead-three", &elf);
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-format",
            "json",
            "--riscv-branch-lookahead",
            "3",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
```

Add a direct-API invariant test using the existing completed-fetch core helper:

```rust
#[test]
#[should_panic(expected = "RISC-V branch lookahead must be between 1 and 3")]
fn riscv_core_rejects_branch_lookahead_above_supported_maximum() {
    let core = core_with_completed_fetches([]);
    core.set_branch_lookahead(4);
}
```

- [ ] **Step 2: Run validation tests and verify the new positive row fails**

Run:

```bash
cargo test -p rem6 --test cli_run riscv_branch_lookahead -- --nocapture
```

Expected: the lookahead-three positive row fails with `invalid RISC-V branch lookahead 3`.

- [ ] **Step 3: Add the shared constants**

In `riscv_defaults.rs` add:

```rust
pub const MIN_RISCV_BRANCH_LOOKAHEAD: usize = 1;
pub const DEFAULT_RISCV_BRANCH_LOOKAHEAD: usize = MIN_RISCV_BRANCH_LOOKAHEAD;
pub const MAX_RISCV_BRANCH_LOOKAHEAD: usize = 3;
```

Use `DEFAULT_RISCV_BRANCH_LOOKAHEAD` for the core state and config-file default.
Use the inclusive range in CLI validation:

```rust
if !(MIN_RISCV_BRANCH_LOOKAHEAD..=MAX_RISCV_BRANCH_LOOKAHEAD).contains(&value) {
    return Err(Rem6CliError::InvalidRiscvBranchLookahead {
        value: value.to_string(),
    });
}
```

Enforce direct callers in `RiscvCore::set_branch_lookahead`:

```rust
assert!(
    (MIN_RISCV_BRANCH_LOOKAHEAD..=MAX_RISCV_BRANCH_LOOKAHEAD).contains(&lookahead),
    "RISC-V branch lookahead must be between {MIN_RISCV_BRANCH_LOOKAHEAD} and {MAX_RISCV_BRANCH_LOOKAHEAD}"
);
```

Delete the old `lookahead.max(1)` compatibility clamp.

- [ ] **Step 4: Run focused validation and CPU tests**

Run:

```bash
cargo test -p rem6 --test cli_run riscv_branch_lookahead -- --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead -- --nocapture
```

Expected: all selected tests pass.

- [ ] **Step 5: Commit the shared authority**

```bash
git add crates/rem6-cpu/src/riscv_defaults.rs crates/rem6-cpu/src/lib.rs \
  crates/rem6/src/config.rs crates/rem6/src/config/riscv_branch.rs \
  crates/rem6/tests/cli_run/validation.rs crates/rem6-cpu/src/riscv_fetch_ahead/tests.rs
git commit -m "cpu: centralize branch lookahead limits"
```

### Task 3: Admit Three Predicted Controls in the Four-Row Policy

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window_tests.rs`

- [ ] **Step 1: Replace the third-control rejection test with a failing admission test**

Use mixed branch classes and retain fourth-row rejection:

```rust
#[test]
fn scalar_memory_prefix_opens_three_predicted_control_paths() {
    let mut window = scalar_load_window(4);
    assert_eq!(
        window.classify_younger(bne()),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(blt()),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window.classify_younger(bgeu()),
        RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert!(window.is_full());
    assert_eq!(
        window.classify_younger(beq()),
        RiscvScalarIntegerYoungerDecision::Reject
    );
}
```

Keep a separate unsupported-row test after two controls for memory, JAL, and
ECALL rejection.

- [ ] **Step 2: Add failing detailed fetch tests**

Create a completed-fetch helper with load, BNE, BLT, and BGEU rows at `0x8000`,
`0x8004`, `0x8008`, and `0x800c`. Set branch lookahead three and depth four.

Add:

```rust
#[test]
fn detailed_scalar_window_follows_three_recorded_control_paths() {
    let core = detailed_three_deep_control_core();
    for expected_pc in [0x8004, 0x8008, 0x800c] {
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(
            decision.branch_speculation().unwrap().pc(),
            Address::new(expected_pc)
        );
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&decision).unwrap(),
        );
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_three_deep_control_respects_branch_lookahead_two() {
    let core = detailed_three_deep_control_core();
    core.set_branch_lookahead(2);
    for expected_pc in [0x8004, 0x8008] {
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(
            decision.branch_speculation().unwrap().pc(),
            Address::new(expected_pc)
        );
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&decision).unwrap(),
        );
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
```

Add a split-third-control variant where the BGEU at `0x800c` consumes two
halfword fetch requests. After recording the outer and middle predictions,
assert the third `branch_speculation().sequence()` equals the first halfword
request sequence and the decision PC is the predicted path after the complete
instruction:

```rust
#[test]
fn detailed_split_third_control_keys_prediction_to_prefix_request() {
    let core = detailed_three_deep_split_control_core();
    for _ in 0..2 {
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&decision).unwrap(),
        );
    }
    let inner = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(inner.branch_speculation().unwrap().sequence(), 3);
    assert_eq!(inner.pc(), Address::new(0x8010));
}
```

- [ ] **Step 3: Run the policy and fetch tests to verify failure**

Run:

```bash
cargo test -p rem6-cpu --lib scalar_memory_prefix_opens_three_predicted_control_paths -- --nocapture
cargo test -p rem6-cpu --lib detailed_scalar_window_follows_three_recorded_control_paths -- --nocapture
```

Expected: the third prediction is rejected under the old depth-two constant.

- [ ] **Step 4: Derive policy depth from the shared maximum**

Import and use the shared constant:

```rust
use crate::MAX_RISCV_BRANCH_LOOKAHEAD;

const MAX_PREDICTED_CONTROL_DEPTH: usize = MAX_RISCV_BRANCH_LOOKAHEAD;
```

No runtime implementation change is expected; row count, immediate dependency
assignment, rollback, and timing ownership already iterate over arbitrary
accepted prefixes.

- [ ] **Step 5: Add runtime ownership tests**

Stage load plus BNE, BLT, and BGEU. Assert:

```rust
let rob = runtime.snapshot().reorder_buffer();
let outer = rob[1].sequence();
let middle = rob[2].sequence();
let inner = rob[3].sequence();
assert_eq!(runtime.live_control_dependencies.get(&middle), Some(&outer));
assert_eq!(runtime.live_control_dependencies.get(&inner), Some(&middle));
assert_eq!(runtime.live_control_window_sequences.len(), 3);
```

Then call `discard_live_control_descendants_from(middle)` and assert the ROB PCs
are exactly load, outer, and middle, with no inner speculative execution or
timing-ownership row.

- [ ] **Step 6: Run focused CPU modules**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_o3_window_policy::tests -- --nocapture
cargo test -p rem6-cpu --lib detailed_o3_control -- --nocapture
cargo test -p rem6-cpu --lib o3_runtime_control_window_tests -- --nocapture
```

Expected: all selected tests pass.

- [ ] **Step 7: Commit policy and ownership evidence**

```bash
git add crates/rem6-cpu/src/riscv_o3_window_policy.rs \
  crates/rem6-cpu/src/riscv_fetch_ahead/tests/detailed_o3_control.rs \
  crates/rem6-cpu/src/o3_runtime_control_window_tests.rs
git commit -m "cpu: admit three-deep predicted controls"
```

### Task 4: Add Three-Deep Admission and Rollback CLI Matrix

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/three_deep.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs`

- [ ] **Step 1: Include the new focused module**

```rust
#[path = "predicted_control/three_deep.rs"]
mod three_deep;
```

- [ ] **Step 2: Build the deterministic mixed-control fixture**

Define these PC and address constants:

```rust
const LOAD_PC: &str = "0x80000028";
const OUTER_BRANCH_PC: &str = "0x8000002c";
const MIDDLE_BRANCH_PC: &str = "0x80000030";
const INNER_BRANCH_PC: &str = "0x80000034";
const DESCENDANT_PC: &str = "0x80000038";
const WRONG_STORE_PC: &str = "0x8000003c";
const INNER_TARGET_PC: &str = "0x80000044";
const MIDDLE_TARGET_PC: &str = "0x80000048";
const OUTER_TARGET_PC: &str = "0x8000004c";
const DATA_ADDRESS: &str = "0x80000100";
const WRONG_STORE_ADDRESS: &str = "0x80000108";
```

The fixture must emit:

```rust
b_type(32, 6, 5, 0b001) // BNE x5, x6, outer target
b_type(24, 8, 7, 0b100) // BLT x7, x8, middle target
b_type(16, 11, inner_rs1, 0b111) // BGEU inner_rs1, x11, inner target
```

Use `inner_rs1 = 12` for the load-dependent row and `9` otherwise. Initialize
registers as x5=1, x6=1, x7=3, x8=2, x9=5, and x11=6 so the default path is
not taken. Set x6=2 for outer-taken, x8=4 for middle-taken, and x11=4 for
inner-taken. Use x11=43 for the load-dependent inner row so loaded x12=42 keeps
the unsigned BGEU not taken after the response. Emit MUL/store fall-through
witnesses and marker writes to x14, x15, x16, and x17. Place data at offset 256
and initialize the first word to 42.

- [ ] **Step 3: Add the positive and lookahead-negative tests**

Add original rows:

```text
rem6_run_o3_three_deep_mixed_controls_commit_direct
rem6_run_o3_three_deep_control_requires_branch_lookahead_three
```

The positive row must assert exact four-row residency `[load, outer, middle,
inner]`, one LSQ row, three predictor lookups, all three branch issues before the
load response, nondecreasing branch commit order, final x13=15 and marker values,
the fall-through store, and max ROB/LSQ stats 4/1.

The lookahead-two row must assert a live pre-response snapshot containing only
`[load, outer, middle]` and exactly two direct-conditional predictor lookups,
while the completed run still reaches the correct final state.

- [ ] **Step 4: Run the two tests**

Run:

```bash
cargo test -p rem6 --test cli_run o3_three_deep_mixed_controls_commit_direct -- --nocapture
cargo test -p rem6 --test cli_run o3_three_deep_control_requires_branch_lookahead_three -- --nocapture
```

Expected: both pass after Tasks 2 and 3.

- [ ] **Step 5: Add position-specific rollback tests**

Add original rows:

```text
rem6_run_o3_three_deep_outer_misprediction_discards_younger_controls_cache_fabric_dram
rem6_run_o3_three_deep_middle_misprediction_preserves_outer_control_direct
rem6_run_o3_three_deep_inner_misprediction_preserves_older_controls_direct
```

For each row assert predicted-not-taken/resolved-taken/mispredicted metadata at
the selected branch, absence of every younger branch and fall-through effect,
the exact target-marker register set, no Data or Memory access at
`WRONG_STORE_ADDRESS`, and zero final ROB/LSQ rows. The hierarchy-backed outer
row must also assert nonzero cache/data, transport/data, fabric, and DRAM
activity.

- [ ] **Step 6: Run the rollback matrix**

Run:

```bash
cargo test -p rem6 --test cli_run o3_three_deep_ -- --nocapture
```

Expected: five selected rows pass.

- [ ] **Step 7: Commit admission and rollback evidence**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/three_deep.rs
git commit -m "test: cover three-deep control rollback"
```

### Task 5: Add Three-Deep Dependency and Lifecycle Evidence

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/three_deep.rs`

- [ ] **Step 1: Add the load-dependent terminal row**

Add:

```text
rem6_run_o3_three_deep_load_dependent_inner_control_is_terminal
```

Assert outer and middle issue before the load response, inner and descendant
issue no earlier than the response, the live ROB is `[load, outer, middle,
inner]`, and predictor lookups remain two because the dependent inner control is
terminal rather than predicted.

- [ ] **Step 2: Add detailed-to-timing transfer**

Add:

```text
rem6_run_host_switch_transfers_o3_three_deep_controls
```

Switch one tick after the inner issue and before the load response. Assert the
transfer is non-restorable, runtime rows are 4 ROB/1 LSQ, schema is 7, younger
rows are 3, and load/outer/middle/inner issue, writeback, and commit ticks match
the no-switch baseline. Assert final mode is timing, final ROB/LSQ are zero, and
fall-through state is architecturally correct.

- [ ] **Step 3: Add checkpoint boundaries**

Add:

```text
rem6_run_o3_three_deep_control_checkpoint_boundary
```

At the live inner-issue boundary, require checkpoint failure containing
`checkpoint component is not quiescent: cpu0`. After the outer target commits,
capture and restore at adjacent ticks; assert one capture, one restore, no
`o3-live-data-handoff` chunk, and zero decoded ROB/LSQ entries.

- [ ] **Step 4: Add timing-mode suppression**

Add:

```text
rem6_run_timing_suppresses_o3_three_deep_controls
```

Assert final load/store bytes and marker registers, absence of
`/cores/0/o3_runtime`, an empty O3 trace, and absence of every `sim.cpu0.o3.*`
and `system.cpu.{rob,lsq0,rename,iq,iew,commit,ftq}.*` stat path.

- [ ] **Step 5: Run the complete three-deep module**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control::three_deep -- --nocapture
```

Expected: 9 passed, 0 failed.

- [ ] **Step 6: Commit lifecycle evidence**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/three_deep.rs
git commit -m "test: cover three-deep control lifecycle"
```

### Task 6: Record Anchors and Migration Evidence

**Files:**
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add all nine original CLI test names to the anchor list**

Add the exact names from Tasks 4 and 5 next to the existing nested-control
anchors.

- [ ] **Step 2: Update the CPU evidence without score inflation**

Keep:

```markdown
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw,
capped at the 74% representative bucket cap.
```

Extend the nested-control evidence with the mixed BNE/BLT/BGEU three-deep
matrix, lookahead-two suppression, immediate rollback positions, dependent
terminal inner row, hierarchy activity, schema-v7 timing-preserving transfer,
checkpoint boundary, and timing suppression.

Narrow remaining evidence to fourth/deeper chains, indirect/unconditional nested
control, arbitrary mixed memory/control windows, restorable transport ownership,
issue-width/resource contention, and a general O3 engine.

- [ ] **Step 3: Preserve the ledger line count**

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: exactly 1200 lines.

- [ ] **Step 4: Run source-policy tests**

Run:

```bash
cargo test -p rem6 --test source_policy
cargo test -p rem6-cpu --test source_policy
```

Expected: 39 rem6 tests and 12 rem6-cpu tests pass.

- [ ] **Step 5: Commit evidence**

```bash
git add crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record three-deep control evidence"
```

### Task 7: Full Verification, Review, and Push

**Files:**
- Verify all files changed by Tasks 1-6.

- [ ] **Step 1: Run focused verification**

```bash
cargo test -p rem6-cpu --lib --quiet
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control -- --nocapture
cargo test -p rem6 --test cli_run riscv_branch_lookahead -- --nocapture
```

Expected: all commands exit zero; CPU count increases from 434 and predicted
control count increases from 17.

- [ ] **Step 2: Run broad verification**

```bash
cargo test -p rem6 --test cli_run --quiet
cargo test --workspace --all-targets --quiet
cargo fmt --all -- --check
git diff --check origin/main..HEAD
wc -l docs/architecture/gem5-to-rem6-migration.md
git status --short --branch
```

Expected: all tests pass, formatting and diff checks are clean, ledger is 1200
lines, and the branch is ahead of `origin/main` with no worktree changes.

- [ ] **Step 3: Request xhigh read-only review**

Review the full range from the current pushed `origin/main` to HEAD. Require
findings-first output and explicit checks for shared limit authority, lookahead
budget enforcement, immediate three-level rollback, split/fetch identity,
transfer timing lifetime, checkpoint/timing suppression, source boundaries, and
ledger honesty.

- [ ] **Step 4: Resolve every Critical, High, or Medium finding**

For each finding, reproduce it with a failing test, fix the root cause, rerun the
focused and broad gates, commit the closure, and request a focused closure review.

- [ ] **Step 5: Fetch and push**

```bash
git fetch origin
git rev-list --left-right --count origin/main...HEAD
git push origin main
git rev-parse HEAD origin/main
git status --short --branch
```

Expected: divergence is `0 N` before push, local and remote hashes match after
push, and status is clean.
