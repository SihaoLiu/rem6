# RISC-V O3 Reverse Coroutine Matrix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the CPU ledger's reverse `x5 -> x1` coroutine suppression, repair, and lifecycle evidence gap without widening the bounded O3 capability or raising its migration score.

**Architecture:** Keep the approved distinct-link coroutine implementation unchanged unless a real reverse-direction behavior test proves a production defect. First split the 1,062-line CLI test module into same-namespace concern includes, then add reverse suppression and repair rows plus a two-direction lifecycle case table. Preserve exact test names for existing rows, real `rem6 run --execute` coverage, the four-ROB/one-LSQ boundary, and the current debug-stat contract.

**Tech Stack:** Rust workspace, Cargo integration tests, RISC-V instruction encoders, JSON artifacts from `rem6 run --execute`, source-policy anchors, migration ledger.

---

## Scope And Invariants

- Approved design: `docs/superpowers/specs/2026-07-15-riscv-o3-same-window-coroutine-design.md`.
- Ledger target: `### CPU Execution Models - 74% representative` in `docs/architecture/gem5-to-rem6-migration.md`.
- Current score remains `74% representative`, raw `8/10`; general O3 remains unchecked.
- `temp/` and `temp/reference_designs/gem5` remain read-only and uncommitted.
- The new matrix covers reverse lookahead, overwrite, older repair, wrong-target repair, live mode switch, live/drained checkpoint, and timing suppression.
- A later ordinary return may consume the coroutine replacement push only after repair and ordered drain. Another same-window linked consumer remains open.
- No live producer may forward an indirect target into `x11`; it remains a committed non-link target source.
- Preserve the intentional timing-mode `sim.debug.o3_trace.*` zero schema when the O3 debug flag is enabled. Suppression applies to runtime `sim.cpu0.o3.*`, gem5-style O3 aliases, `/cores/0/o3_runtime`, and structured O3 trace records.
- Migration ledger remains exactly 1,200 lines.

## File Ownership

- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`: imports, shared constants, shared case data, positive rows, shared runners, and fixture builders.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/suppression.rs`: forward and reverse lookahead/overwrite rows.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/repair.rs`: forward and reverse older-branch/wrong-target rows.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/lifecycle.rs`: two-direction switch/checkpoint/timing rows.
- Create `crates/rem6/tests/source_policy/coroutine_ownership.rs`: include ownership and line-count ratchets.
- Modify `crates/rem6/tests/source_policy.rs`: include the focused ownership policy.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: add four reverse suppression/repair test anchors.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record the reverse matrix honestly without changing score/checklist state.

### Task 1: Split Coroutine CLI Evidence By Concern

**Files:**
- Create: `crates/rem6/tests/source_policy/coroutine_ownership.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/suppression.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/repair.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/lifecycle.rs`

- [ ] **Step 1: Write the failing ownership policy**

Add the module declaration near the existing focused source-policy modules:

```rust
#[path = "source_policy/coroutine_ownership.rs"]
mod coroutine_ownership;
```

Create the policy file:

```rust
use super::*;

const COROUTINE_ROOT: &str =
    "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs";

#[test]
fn coroutine_cli_evidence_uses_focused_same_namespace_includes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(root.join(COROUTINE_ROOT)).unwrap();
    for include in [
        "include!(\"coroutine/suppression.rs\");",
        "include!(\"coroutine/repair.rs\");",
        "include!(\"coroutine/lifecycle.rs\");",
    ] {
        assert!(source.contains(include), "missing {include} in {COROUTINE_ROOT}");
    }
    assert!(line_count(&root.join(COROUTINE_ROOT)) <= 500);
    for relative in [
        "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/suppression.rs",
        "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/repair.rs",
        "tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/lifecycle.rs",
    ] {
        assert!(line_count(&root.join(relative)) <= 700, "{relative} is oversized");
    }
}
```

- [ ] **Step 2: Run the policy red**

Run:

```text
cargo test -p rem6 --test source_policy coroutine_cli_evidence_uses_focused_same_namespace_includes -- --nocapture
```

Expected: FAIL because the three includes do not exist and the root is above 500 lines.

- [ ] **Step 3: Move existing tests without changing behavior**

Move these complete existing item groups unchanged:

- suppression: `rem6_run_o3_same_window_coroutine_requires_branch_lookahead_two` and `rem6_run_o3_same_window_overwritten_coroutine_source_stays_terminal`;
- repair: `rem6_run_o3_older_branch_discards_same_window_coroutine_chain` and `rem6_run_o3_same_window_coroutine_wrong_target_repairs_descendants`;
- lifecycle: `rem6_run_host_switch_transfers_o3_same_window_coroutine`, `rem6_run_o3_same_window_coroutine_checkpoint_boundary`, and `rem6_run_timing_suppresses_o3_same_window_coroutine`.

Replace the moved blocks in `coroutine.rs` with:

```rust
include!("coroutine/suppression.rs");
include!("coroutine/repair.rs");
include!("coroutine/lifecycle.rs");
```

Keep the two positive tests, shared helpers, and all fixture builders in `coroutine.rs`. `include!` is required so existing fully qualified test names remain under `...::predicted_control::coroutine::...`.

- [ ] **Step 4: Verify the mechanical split**

Run:

```text
cargo test -p rem6 --test cli_run coroutine -- --nocapture
cargo test -p rem6 --test source_policy coroutine_cli_evidence_uses_focused_same_namespace_includes -- --nocapture
```

Expected: 9 coroutine CLI tests pass; ownership policy passes.

- [ ] **Step 5: Commit the mechanical refactor**

```text
git add crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/coroutine_ownership.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine
git commit -m "test: split coroutine evidence ownership"
```

### Task 2: Add Shared Direction And Fixture Data

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`

- [ ] **Step 1: Give the reverse positive fixture checkpoint padding**

Change:

```rust
fn indirect_coroutine_binary(name: &str) -> std::path::PathBuf
```

to:

```rust
fn indirect_coroutine_binary(name: &str, exit_padding_words: usize) -> std::path::PathBuf
```

After the success store at `0x80000030`, insert:

```rust
words.extend(std::iter::repeat_n(
    i_type(0, 0, 0x0, 0, 0x13),
    exit_padding_words,
));
words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
```

Update the existing positive call to use padding `0`.

- [ ] **Step 2: Add the lifecycle case descriptor**

Add:

```rust
#[derive(Clone, Copy)]
struct CoroutineLifecycleCase {
    label: &'static str,
    binary: fn(&str, usize) -> std::path::PathBuf,
    memory_system: &'static str,
    max_tick: u64,
    load_pc: &'static str,
    call_pc: &'static str,
    coroutine_pc: &'static str,
    descendant_pc: &'static str,
    success_store_pc: &'static str,
    call_kind: &'static str,
    call_destination: u8,
    coroutine_destination: u8,
    final_x1: u64,
    final_x5: u64,
    final_x13: u64,
    memory_hex: &'static str,
    provider_no_target: u64,
    provider_indirect: u64,
}

const COROUTINE_LIFECYCLE_CASES: [CoroutineLifecycleCase; 2] = [
    CoroutineLifecycleCase {
        label: "forward-direct",
        binary: direct_coroutine_binary,
        memory_system: "direct",
        max_tick: 2_500,
        load_pc: "0x8000000c",
        call_pc: "0x80000010",
        coroutine_pc: "0x8000001c",
        descendant_pc: "0x80000014",
        success_store_pc: "0x80000028",
        call_kind: "call_direct",
        call_destination: 1,
        coroutine_destination: 5,
        final_x1: 0x8000_0014,
        final_x5: 0x8000_0020,
        final_x13: 0x8000_0020,
        memory_hex: "2a000000200000800000000000000000",
        provider_no_target: 1,
        provider_indirect: 0,
    },
    CoroutineLifecycleCase {
        label: "reverse-indirect",
        binary: indirect_coroutine_binary,
        memory_system: "cache-fabric-dram",
        max_tick: 3_000,
        load_pc: "0x80000014",
        call_pc: "0x80000018",
        coroutine_pc: "0x80000024",
        descendant_pc: "0x8000001c",
        success_store_pc: "0x80000030",
        call_kind: "call_indirect",
        call_destination: 5,
        coroutine_destination: 1,
        final_x1: 0x8000_0028,
        final_x5: 0x8000_001c,
        final_x13: 0x8000_0028,
        memory_hex: "2a000000280000800000000000000000",
        provider_no_target: 0,
        provider_indirect: 1,
    },
];
```

- [ ] **Step 3: Verify unchanged positives**

Run:

```text
cargo test -p rem6 --test cli_run same_window_coroutine_commits -- --nocapture
```

Expected: both positive rows pass with unchanged values.

- [ ] **Step 4: Commit fixture support**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs
git commit -m "test: describe coroutine direction cases"
```

### Task 3: Add Reverse Suppression Rows

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/suppression.rs`

- [ ] **Step 1: Generalize the terminal frontend assertion**

Change the helper to accept the exact call kind and target provider:

```rust
fn assert_terminal_coroutine_frontend(
    resident: &Value,
    call_kind: &str,
    call_provider: &str,
    fetched_pc: &str,
    suppressed_pcs: &[&str],
) {
    for kind in ["call_direct", "call_indirect"] {
        let expected = u64::from(kind == call_kind);
        assert_eq!(
            resident
                .pointer(&format!("/cores/0/branch_predictor/lookups/{kind}"))
                .and_then(Value::as_u64),
            Some(expected)
        );
    }
    assert_eq!(
        resident
            .pointer(&format!("/cores/0/branch_predictor/target_provider/{call_provider}"))
            .and_then(Value::as_u64),
        Some(1)
    );
    for (pointer, expected) in [
        ("/cores/0/branch_predictor/ras/pushes", 1),
        ("/cores/0/branch_predictor/lookups/return", 0),
        ("/cores/0/branch_predictor/ras/pops", 0),
        ("/cores/0/branch_predictor/ras/used", 0),
        ("/cores/0/branch_predictor/target_provider/ras", 0),
    ] {
        assert_eq!(resident.pointer(pointer).and_then(Value::as_u64), Some(expected));
    }
    assert!(resident
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .is_some_and(|records| records.iter().any(|record| {
            record.pointer("/pc").and_then(Value::as_str) == Some(fetched_pc)
        })));
    for pc in suppressed_pcs {
        assert_no_fetch_pc(resident, pc);
    }
}
```

Update forward calls with `"call_direct", "no_target"`.

- [ ] **Step 2: Add reverse lookahead-one evidence**

Add `rem6_run_o3_same_window_indirect_coroutine_requires_branch_lookahead_two` using `indirect_coroutine_binary(name, 0)`, hierarchy memory, max tick `3_000`, and lookahead `1`.

Assert:

```text
final x5=0x8000001c, x1=x13=0x80000028
memory=2a000000280000800000000000000000
load/call/coroutine/descendant PCs=14/18/24/1c
call issue < response; descendant issue > response
at response-1: ROB=[14,18], LSQ=1, x5 rename belongs to call row
fetch contains 24 and excludes 1c
call kind=call_indirect; provider=indirect
```

- [ ] **Step 3: Add the reverse overwritten-source fixture**

Create `overwritten_indirect_coroutine_binary` with this exact suffix after the shared reverse prefix through the indirect call at `0x18`:

```rust
words.extend([
    s_type(8, 7, 18, 0b010),
    m5op(M5_FAIL),
    i_type(24, 5, 0x0, 5, 0x13),
    i_type(0, 5, 0x0, 1, 0x67),
    s_type(12, 7, 18, 0b010),
    m5op(M5_FAIL),
    i_type(0, 1, 0x0, 13, 0x13),
    s_type(4, 13, 18, 0b010),
    m5op(M5_EXIT),
    m5op(M5_FAIL),
]);
```

Key PCs: load `0x14`, call `0x18`, overwrite `0x24`, coroutine `0x28`, final target `0x34`.

- [ ] **Step 4: Add reverse overwrite evidence**

Add `rem6_run_o3_same_window_indirect_overwritten_coroutine_source_stays_terminal` with hierarchy memory, lookahead `2`, and exact assertions:

```text
final x5=0x80000034, x1=x13=0x8000002c
memory=2a0000002c0000800000000000000000
no data at +8 or +12
at response-1: ROB=[14,18,24,28], LSQ=1
rename x5 -> overwrite row 24; x1 -> coroutine row 28
fetch contains 24; excludes 1c and 34
only the indirect call owns predictor/RAS activity before response
```

- [ ] **Step 5: Run suppression rows**

```text
cargo test -p rem6 --test cli_run coroutine_requires_branch_lookahead_two -- --nocapture
cargo test -p rem6 --test cli_run overwritten_coroutine_source_stays_terminal -- --nocapture
```

Expected: four rows pass. If a reverse row fails, preserve the assertion and use systematic debugging before touching production code.

- [ ] **Step 6: Commit reverse suppression evidence**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/suppression.rs
git commit -m "test: cover reverse coroutine suppression"
```

### Task 4: Add Reverse Repair Rows

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/repair.rs`

- [ ] **Step 1: Add the reverse older-repair fixture**

Create `older_branch_indirect_coroutine_binary` with exact PCs:

```text
20 load
24 taken BEQ -> 40
28 indirect call x5 -> 34, link 2c
2c speculative scalar descendant
30 wrong store +8
34 coroutine x1 <- jalr 0(x5), target 2c, link 38
38 wrong store +12
40 x15=0x33
44 success store +4
48 exit
```

The prelude sets committed `x1=0x11`, `x5=0x55`, `x7=1`, and committed `x11=0x80000034`.

- [ ] **Step 2: Add reverse older-repair evidence**

Add `rem6_run_o3_older_branch_discards_same_window_indirect_coroutine_chain` with hierarchy memory and lookahead `3`. Assert:

```text
final x1=0x11, x5=0x55, x13 absent/zero, x15=0x33
memory=2a000000330000000000000000000000
no +8/+12 stores
no retired rows 28/2c/34
pre-repair ROB=[20,24,28,34], LSQ=1
live x5 rename -> 28; live x1 rename -> 34
lookups conditional/call_indirect/return = 1/1/1
commits call_indirect/return = 0/0
squashes call_indirect/return = 1/1
provider no_target/ras/indirect/total = 1/1/1/3
RAS pushes/pops/squashes/used/correct/incorrect = 3/3/2/0/0/0
```

- [ ] **Step 3: Add the reverse wrong-target fixture**

Create `wrong_target_indirect_coroutine_binary` with:

```text
14 delayed load
18 indirect call writing x5, target 24, link 1c
1c wrong-target addi x14,99
20 wrong store +8
24 coroutine jalr x1,20(x5), predicted 1c, resolved 30, link 28
28 success store +4
2c exit
30 repaired addi x13,x1
34 ordinary jalr x0,0(x1), target 28
38 fail
```

- [ ] **Step 4: Add reverse wrong-target evidence**

Add `rem6_run_o3_same_window_indirect_coroutine_wrong_target_repairs_descendants`. Assert:

```text
final x5=0x8000001c, x1=x13=0x80000028, x14 absent/zero
memory=2a000000280000800000000000000000
wrong target 1c fetched exactly once before coroutine issue and never retires
coroutine predicted/resolved/squashed targets=1c/30/1c, repair=wrong_target
later ordinary return has link=false, predicted target null, resolved target 28
repaired descendant issue > coroutine commit
later return issue > repaired descendant writeback
lookups/commits call_indirect=1, return=2
providers indirect=1, ras=2, no_target=0, total=3
RAS pushes/pops/used/correct/incorrect=2/2/2/1/1
branch-kind squashes remain zero
```

Do not add a same-window return consumer. The later ordinary return executes only after repair and ordered drain.

- [ ] **Step 5: Run repair rows**

```text
cargo test -p rem6 --test cli_run older_branch_discards_same_window -- --nocapture
cargo test -p rem6 --test cli_run coroutine_wrong_target_repairs_descendants -- --nocapture
```

Expected: four repair rows pass with exact RAS counters.

- [ ] **Step 6: Commit reverse repair evidence**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/repair.rs
git commit -m "test: prove reverse coroutine repair"
```

### Task 5: Extend Lifecycle Tests Across Both Directions

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/lifecycle.rs`

- [ ] **Step 1: Parameterize mode-switch coverage**

Keep the existing test name and loop over `COROUTINE_LIFECYCLE_CASES`. For each case:

```rust
let path = (case.binary)(&format!("o3-same-window-coroutine-switch-{}", case.label), 0);
let baseline = run_coroutine_json(
    &path,
    case.memory_system,
    case.max_tick,
    "detailed",
    2,
    &DIRECT_WIDTH_ARGS,
);
let switch_tick = event_u64(event_at_pc(&baseline, case.descendant_pc), "issue_tick") + 1;
```

Assert exact per-case final registers/memory, four-row/one-LSQ transfer, schema v7 load handoff, three younger rows, row-relative link rename ownership, preserved issue/writeback/commit ticks for all four PCs, common RAS totals, and case-specific call/provider counters.

- [ ] **Step 2: Parameterize checkpoint coverage**

For each case, use padding `8`. Reject a live checkpoint at `descendant.issue_tick + 1`. For drained restore, schedule checkpoint at `success_store.commit_tick + 1` and restore one tick later. Assert final architecture equals baseline, one checkpoint/restore, no live-data handoff chunk, and zero checkpoint/runtime ROB/LSQ rows.

Do not assert an inferred reverse rename-count constant until the generated artifact proves it; use relational link-row assertions in the live path and zero ROB/LSQ for the drained path.

- [ ] **Step 3: Parameterize timing suppression**

For each case, run the same binary with timing mode and no O3 width arguments. Assert exact final registers/memory, no wrong store, absent `/cores/0/o3_runtime`, empty `/debug/o3_trace`, and `assert_no_o3_stats` success. Preserve zero-valued `sim.debug.o3_trace.*` schema stats.

- [ ] **Step 4: Run lifecycle rows**

```text
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_o3_same_window_coroutine -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_same_window_coroutine_checkpoint_boundary -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_same_window_coroutine -- --nocapture
```

Expected: all three tests pass both labeled case iterations.

- [ ] **Step 5: Commit lifecycle matrix**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine/lifecycle.rs crates/rem6/tests/cli_run/m5_host_actions/o3/predicted_control/coroutine.rs
git commit -m "test: cover reverse coroutine lifecycle"
```

### Task 6: Update Anchors And Migration Ledger

**Files:**
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add exact reverse test anchors**

Insert after the existing nine coroutine anchors:

```text
rem6_run_o3_same_window_indirect_coroutine_requires_branch_lookahead_two
rem6_run_o3_same_window_indirect_overwritten_coroutine_source_stays_terminal
rem6_run_o3_older_branch_discards_same_window_indirect_coroutine_chain
rem6_run_o3_same_window_indirect_coroutine_wrong_target_repairs_descendants
```

The three lifecycle test names remain unchanged because each now owns both directions.

- [ ] **Step 2: Update the CPU narrative without score inflation**

Change only the bounded coroutine prose and test-map row. State that both directions now cover positive execution, lookahead/overwrite suppression, older/wrong-target repair, switch, live/drained checkpoint, and timing suppression. Preserve as open:

- another same-window linked control consuming the replacement push;
- same-link coroutine forms;
- other link-sourced indirect controls outside exact adjacent distinct-link lineage;
- producer-forwarded target for a further control descendant;
- fourth/deeper chains and general O3.

Keep heading `74% representative`, raw `8/10`, and general O3 unchecked.

- [ ] **Step 3: Run policy and ledger gates**

```text
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: 50/50 rem6 source-policy tests, 25/25 CPU source-policy tests, ledger exactly 1,200 lines.

- [ ] **Step 4: Commit documentation evidence**

```text
git add crates/rem6/tests/source_policy/core_test_anchors.txt docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record reverse coroutine matrix"
```

### Task 7: Final Verification, Review, And Push

**Files:**
- Verify all files in this plan.

- [ ] **Step 1: Run focused and full verification**

```text
cargo test -p rem6-cpu coroutine -- --nocapture
cargo test -p rem6 --test cli_run coroutine -- --nocapture
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run --quiet
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
cargo fmt --all -- --check
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
cargo test --workspace --all-targets --quiet
```

- [ ] **Step 2: Audit the worktree**

```text
git status --short --branch
git diff -- temp temp/reference_designs/gem5
git diff --stat origin/main...HEAD
git log --oneline origin/main..HEAD
```

Expected: clean worktree, no temp diff, only scoped commits.

- [ ] **Step 3: Run independent read-only review**

Use separate reviewers for:

1. reverse fixture PC/register/memory arithmetic;
2. RAS producer/consumer/replacement lineage and exact counters;
3. lifecycle switch/checkpoint/timing assertion strength;
4. same-namespace module ownership, dead-code/slop, anchors, and ledger honesty.

Fix valid findings with a failing focused test and a separate commit.

- [ ] **Step 4: Push and prove remote equality**

```text
git push origin main
git fetch origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

Expected: local and remote hashes match; branch is clean and synchronized.

## Completion Criteria

1. Existing qualified coroutine test names remain stable after the split.
2. Reverse lookahead and overwrite suppression have exact pre-response ROB/LSQ/rename/fetch evidence.
3. Reverse older and wrong-target repairs have exact architecture, memory, wrong-path, predictor, and RAS evidence.
4. Existing switch/checkpoint/timing tests execute and label both directions.
5. No production code changes unless a reverse behavior test first fails for a proven implementation defect.
6. No general indirect-target forwarding or same-window replacement-push consumer is introduced.
7. Source policy passes, ledger stays exactly 1,200 lines, CPU remains 74% and raw 8/10, general O3 stays unchecked.
8. Full workspace verification and independent review pass.
9. Commits are pushed to `origin/main` and local/remote HEADs match.
