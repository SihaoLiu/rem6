# RISC-V Cold Translated O3 Younger Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Admit a bounded younger scalar-ALU O3 window after a cold cacheable data-translation miss completes, with direct/hierarchy handoff, checkpoint, restore/stats, and timing-suppression evidence.

**Architecture:** Translation advancement is split from issue consumption so a validated cold cacheable scalar load can drive the existing bounded translated fetch-ahead before its physical request issues. The translated-window marker remains issue-time authority. Existing schema-v7 handoff and checkpoint codecs remain unchanged; focused CPU and top-level CLI tests prove the new runtime shape and its lifecycle boundaries.

**Tech Stack:** Rust workspace, `rem6-cpu` translated data issue and O3 runtime, parallel top-level `rem6 run`, TOML host actions, JSON/debug/stats/checkpoint artifacts, and migration source policy.

---

### Task 1: Add the focused RED behavior test

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_data_issue_tests/translated.rs:7-27`
- Test: `crates/rem6-cpu/src/riscv_data_issue_tests/translated.rs`

- [x] **Step 1: Flip the cold translated test to the required shape**

Rename the test to:

```rust
detailed_translated_cold_cacheable_scalar_load_stages_younger_after_translation_completion
```

Keep the existing empty-TLB, nonzero-latency translation setup and one
completed younger fetch. After `issue_translated_data_without_response`,
replace the one-row assertion with:

```rust
let snapshot = core.o3_runtime_snapshot();
assert_eq!(
    snapshot
        .reorder_buffer()
        .iter()
        .map(|row| row.pc())
        .collect::<Vec<_>>(),
    vec![Address::new(0x8000), Address::new(0x8004)]
);
assert_eq!(snapshot.load_store_queue().len(), 1);
```

- [x] **Step 2: Run the exact test and confirm RED**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_data_issue::tests::translated::detailed_translated_cold_cacheable_scalar_load_stages_younger_after_translation_completion -- --exact --nocapture
```

Expected: FAIL because the actual ROB contains only `0x8000`.

### Task 2: Add RED direct and hierarchy CLI handoff rows

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/translated_scalar_load.rs:1-210,629-671`
- Test: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/translated_scalar_load.rs`

- [x] **Step 1: Give the cold fixture architectural byte witnesses**

After the three existing scalar ALUs, append stores of `x13`, `x14`, and
`x15` to `DATA_OFFSET + 4`, `+8`, and `+12`. Update the expected memory image
to:

```rust
const TRANSLATED_RESULTS: &str = "2a00000005000000100000003a000000";
```

Keep the cold load and three ALU PCs unchanged. Add a cold-store PC array so
the tests can prove the fifth and later rows are not inherited O3 authority.

- [x] **Step 2: Replace the old cold terminal test with a route helper**

Create these tests:

```rust
#[test]
fn rem6_run_host_switch_transfers_cold_translated_scalar_load_younger_window_direct() {
    assert_cold_translated_scalar_load_younger_window_handoff("direct");
}

#[test]
fn rem6_run_host_switch_transfers_cold_translated_scalar_load_younger_window_cache_fabric_dram() {
    assert_cold_translated_scalar_load_younger_window_handoff("cache-fabric-dram");
}
```

The shared helper must:

- Run a no-switch baseline and derive a switch tick strictly between load
  issue and response.
- Run the switched case through the selected memory system.
- Require final registers `x12=0x2a`, `x13=0x5`, `x14=0x10`, `x15=0x3a` and
  `TRANSLATED_RESULTS` memory.
- Call `assert_translated_handoff(transfer, 4, 3, issue_tick)`.
- Preserve load plus three ALU issue/writeback/commit ticks against baseline.
- Require the load response tick to match baseline.
- Require all cold store PCs to remain absent from O3 events.
- Require one translated load plus three later stores in Data trace.
- Call `super::scalar_load::assert_memory_resources` for direct/hierarchy
  route assertions.

- [x] **Step 3: Run both exact rows and confirm RED**

Run both exact CLI tests before changing production code. Expected: FAIL at
the four-ROB/three-younger assertion because the old runtime reports one/zero.

### Task 3: Open a validated ready-translation fetch phase

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_translation.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/driver.rs`
- Modify: `crates/rem6-cpu/src/riscv_cluster.rs`
- Test: `crates/rem6-cpu/src/riscv_data_issue_tests/translated.rs`
- Test: `crates/rem6-cpu/tests/riscv_translation_frontend.rs`
- Test: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/translated_scalar_load.rs`

- [x] **Step 1: Split translation advancement from issue consumption**

Complete or enqueue the next data translation without immediately removing a
ready translated access. Reuse ordinary request preparation to validate PMP,
PMA, route, line layout, target, destination, and terminal-result boundaries.

- [x] **Step 2: Drive the bounded suffix before issue**

For a validated cacheable scalar integer load targeting memory, establish the
existing translated-window marker and classify younger fetches from the
post-load architectural PC. Retain the ready translated access while an
authorized fetch is pending. Keep the marker requirement in
`record_data_issue_state` unchanged.

- [x] **Step 3: Apply the shared phase to every translated driver**

Invoke the phase from the serial translated driver, parallel translated memory
driver, and parallel MMIO-aware translated driver. Probe mapped MMIO before
opening a memory window. Add a nonzero-latency serial row proving the younger
fetch precedes data issue and blocks issue while that fetch is pending.

- [x] **Step 4: Turn the focused positive GREEN**

Run the exact test from Task 1.

Expected: PASS with ROB PCs `0x8000`, `0x8004` and one LSQ row.

- [x] **Step 5: Turn both CLI rows GREEN**

Expected: both PASS with exact four-row transfer and route activity.

- [x] **Step 6: Preserve focused suppression boundaries**

Run:

```bash
cargo test -p rem6-cpu --lib riscv_data_issue::tests::translated::detailed_translated_uncacheable_scalar_load_is_terminal_before_retirement -- --exact --nocapture
cargo test -p rem6-cpu --lib riscv_fetch_ahead::tests::data_access_result::cached_translated_mmio_scalar_load_uses_result_only_driver_fetch_window -- --exact --nocapture
```

Expected: both PASS. The uncacheable snapshot remains one ROB/one LSQ row;
translated MMIO remains terminal.

### Task 4: Add checkpoint, restore/stats, and timing boundaries

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/translated_scalar_load.rs`

- [x] **Step 1: Add live checkpoint rejection**

Add:

```rust
rem6_run_rejects_live_cold_translated_scalar_load_younger_window_checkpoint
```

Derive a live tick between the baseline cold load issue and response. First
assert the baseline contains four O3 rows. Run the same direct fixture with:

```toml
host_checkpoints = ["<live_tick>:cold-translated-live"]
```

Require non-success, empty stdout, and stderr containing:

```text
checkpoint component is not quiescent: cpu0
```

- [x] **Step 2: Add a drained checkpoint fixture**

Build a dedicated cold translated binary from the same load/ALU/store body,
clear the m5 delay/period argument registers, then append
`m5op(M5_CHECKPOINT)`, `m5op(M5_DUMP_STATS)`, one non-idempotent `x16`
increment, four scalar NOPs, and host stop before the data image.

- [x] **Step 3: Add drained restore and stats evidence**

Add:

```rust
rem6_run_restores_drained_cold_translated_scalar_load_younger_window_and_stats
```

Run the checkpoint fixture once to obtain the post-checkpoint `x16` increment
commit tick, then run it again with:

```toml
host_checkpoint_restores = ["<mutation_commit_tick + 1>:gem5-m5-checkpoint"]
```

Require:

- Final registers and `TRANSLATED_RESULTS` memory.
- Final `x16=1`, proving restore resets the first increment before replay.
- `checkpoint_count=1`, `checkpoint_restored_count=1`, and
  `stats_dump_count=2`.
- Captured and restored `cpu0/o3-runtime-state` decoded payloads are equal.
- `snapshot_rob_entries=0`, `snapshot_lsq_entries=0`,
  `stats_max_rob_occupancy=4`, `stats_max_lsq_occupancy=1`,
  `stats_lsq_operation_load=1`, and `stats_lsq_operation_store=3`.
- First and restored dumps have equal values for
  `sim.host_actions.stats_dump.cpu0.o3.lsq_operation.load`,
  `sim.host_actions.stats_dump.cpu0.o3.lsq_operation.store`,
  `system.cpu.lsq0.operation.load`, `system.cpu.lsq0.operation.store`, and
  `system.cpu.lsq0.operation.total`.

- [x] **Step 4: Add timing suppression**

Add:

```rust
rem6_run_timing_suppresses_cold_translated_scalar_load_younger_window_o3_artifacts
```

Run the regular cold fixture with `m5_switch_cpu_mode = "timing"`. Require
the same final registers/memory, no `/cores/0/o3_runtime`, an empty
`/debug/o3_trace`, no `sim.cpu0.o3.*` or gem5 O3 alias prefixes, and zero
debug O3 aggregate counters.

- [x] **Step 5: Run the three exact lifecycle rows**

Expected: all PASS.

### Task 5: Ratchet source policy and update the ledger

**Files:**
- Modify: `crates/rem6/tests/source_policy.rs:32-55,793-830`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt:1022-1028`
- Modify: `docs/architecture/gem5-to-rem6-migration.md:169-221`

- [x] **Step 1: Add a focused CLI line budget**

Add:

```rust
const MAX_TRANSLATED_SCALAR_LOAD_LINES: usize = 1400;
```

and a source-policy test requiring
`tests/cli_run/m5_host_actions/o3/switch/translated_scalar_load.rs` to stay at
or below that limit.

- [x] **Step 2: Update core test anchors**

Replace the old terminal cold handoff anchor with the two new route anchors,
then add the live checkpoint, drained restore/stats, and timing suppression
anchors. Keep existing cached, multicore, and MMIO translated anchors.

- [x] **Step 3: Update CPU evidence without changing score**

In the CPU section:

- Replace the cold one-row/no-younger statement with the direct/hierarchy
  four-ROB/one-LSQ/three-younger evidence and lifecycle rows.
- Remove only `cold-miss translated younger windows` from `Not migrated` and
  `Next evidence` text.
- Add the five exact CLI anchors to the evidence list.
- Keep `8 of 10`, `80% raw`, `74% representative`, and both unchecked items.
- Preserve exactly 1,200 ledger lines.

- [x] **Step 4: Run source-policy and ledger gates**

Run:

```bash
cargo test -p rem6 --test source_policy
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: all source-policy tests PASS and the ledger remains 1,200 lines.

### Task 6: Verify, review, commit, and push

**Files:**
- Verify all changed files.

- [x] **Step 1: Run affected targets**

Run:

```bash
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch::translated_scalar_load -- --nocapture
```

Expected: all PASS.

- [x] **Step 2: Run the full workspace and mechanical gates**

Run:

```bash
cargo test --workspace --all-targets -q
cargo fmt --all -- --check
git diff --check
git status --short -- temp
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: all PASS; no `temp` changes; the ledger is exactly 1,200 lines.

- [x] **Step 3: Obtain independent read-only review**

Dispatch high-intensity reviewers for issue-time authorization safety,
serial/parallel consistency, CLI artifact honesty, checkpoint/stats behavior,
source-policy quality, and ledger scoring. Resolve every actionable finding.

- [x] **Step 4: Commit and push**

Stage only the design/plan, CPU issue/test files, translated CLI module,
source-policy files, and migration ledger. Commit with:

```bash
git commit -m "feat: admit cold translated O3 younger windows"
git push origin main
```

Verify a clean branch and exact `HEAD`/`origin/main` parity.
