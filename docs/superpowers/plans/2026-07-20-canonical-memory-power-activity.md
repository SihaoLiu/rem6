# Canonical Memory Power Activity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the normal `rem6 run` power path derive every memory-component record from `Rem6MemoryResourceSummary`, remove duplicate raw cache/DRAM activity authority, and prove representative route/format/suppression behavior.

**Architecture:** `build_run_execution_summary` will construct the existing canonical memory resource summary and pass it, with core summaries, to power assembly. Focused cache, transport, fabric, and DRAM activity projections in `power_output.rs` will select records and calculate deterministic values; GPU and trace-replay retain their current adapters. A focused CLI matrix will reconcile power targets with run-artifact resource activity across direct, DRAM, cache, and cache-fabric-DRAM routes.

**Tech Stack:** Rust, Cargo integration tests, `serde_json`, `rem6-power` McPAT/DSENT importers, source-policy syntax checks, Markdown migration ledger.

---

## File Structure

- Create `crates/rem6/src/power_output/tests.rs` for focused projection and suppression unit tests.
- Modify `crates/rem6/src/power_output.rs` to own canonical activity projections and delegate tests.
- Modify `crates/rem6/src/run_execution_summary.rs` to remove raw cache/DRAM power inputs.
- Create `crates/rem6/tests/cli_run/load/power_activity_matrix.rs` for extracted and new run power evidence.
- Modify `crates/rem6/tests/cli_run/load.rs` to declare the child module and remove moved power component tests/helpers.
- Create `crates/rem6/tests/source_policy/power_activity_ownership.rs` to lock normal-run ownership and test placement.
- Modify `crates/rem6/tests/source_policy.rs` to register the policy module.
- Modify `docs/architecture/gem5-to-rem6-migration.md` without changing its 1,200-line count.

### Task 1: Extract Run Power Tests Behind an Ownership Policy

**Files:**
- Create: `crates/rem6/tests/source_policy/power_activity_ownership.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Create: `crates/rem6/tests/cli_run/load/power_activity_matrix.rs`
- Modify: `crates/rem6/tests/cli_run/load.rs`

- [ ] **Step 1: Register a failing focused policy test**

Add the module declaration beside the other source-policy children:

```rust
#[path = "source_policy/power_activity_ownership.rs"]
mod power_activity_ownership;
```

Create a policy test that requires:

```rust
const LOAD_ROOT: &str = "tests/cli_run/load.rs";
const POWER_MATRIX: &str = "tests/cli_run/load/power_activity_matrix.rs";

#[test]
fn run_power_activity_matrix_lives_in_focused_module() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = fs::read_to_string(crate_dir.join(LOAD_ROOT)).unwrap();
    let matrix = fs::read_to_string(crate_dir.join(POWER_MATRIX)).unwrap();

    assert!(root.contains("#[path = \"load/power_activity_matrix.rs\"]"));
    assert!(root.contains("mod power_activity_matrix;"));
    for test in [
        "rem6_run_power_analysis_includes_dram_activity",
        "rem6_run_power_analysis_includes_cache_activity",
        "rem6_run_power_analysis_includes_shared_cache_activity",
        "rem6_run_power_analysis_includes_fabric_activity",
        "rem6_run_power_analysis_includes_transport_activity",
    ] {
        assert!(matrix.contains(&format!("fn {test}")));
        assert!(!root.contains(&format!("fn {test}")));
    }
}
```

- [ ] **Step 2: Run the policy test and observe RED**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy power_activity_ownership -- --nocapture
```

Expected: FAIL because the focused module does not exist and `load.rs` still owns the component tests.

- [ ] **Step 3: Extract existing component tests and shared helper**

Declare the child at the top of `load.rs`:

```rust
#[path = "load/power_activity_matrix.rs"]
mod power_activity_matrix;
```

Move `assert_power_component_dynamic_watts_positive` and the five existing component-presence tests into the child. Import parent helpers and standard dependencies there:

```rust
use std::{fs, process::Command};

use rem6_power::PowerAnalysisExport;

use super::*;
```

Keep test names and assertions unchanged. Do not move output-path, invalid-format, TOML, or envelope tests; those remain command/config ownership rather than activity calibration.

- [ ] **Step 4: Run the extracted tests and policy GREEN**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run load::power_activity_matrix -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy power_activity_ownership -- --nocapture
```

Expected: the five existing CLI tests and the placement policy PASS.

- [ ] **Step 5: Commit the mechanical extraction**

```bash
git add crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/power_activity_ownership.rs crates/rem6/tests/cli_run/load.rs crates/rem6/tests/cli_run/load/power_activity_matrix.rs
TMPDIR=$PWD/target/tmp git commit -m "test: extract run power activity coverage"
```

### Task 2: Add Failing Canonical Activity Tests

**Files:**
- Create: `crates/rem6/src/power_output/tests.rs`
- Modify: `crates/rem6/src/power_output.rs`
- Modify: `crates/rem6/tests/cli_run/load/power_activity_matrix.rs`

- [ ] **Step 1: Add focused unit tests for missing boundaries**

Declare the child in `power_output.rs`:

```rust
#[cfg(test)]
#[path = "power_output/tests.rs"]
mod tests;
```

In the child, add a `record_for_target` lookup helper and construct focused
resource values directly. Add these exact cases:

```rust
#[test]
fn run_power_emits_refresh_only_dram_resource() {
    let dram = Rem6DramResourceSummary {
        activity: 1,
        active: 1,
        refreshes: 1,
        refresh_ticks: 9,
        ..Rem6DramResourceSummary::default()
    };
    let record = dram_resource_power_record(&dram, 0).expect("refresh is DRAM activity");
    assert_eq!(record.target(), "memory.dram");
    assert_eq!(record.residency_ticks(PowerStateKind::On), 9);
}

#[test]
fn run_power_emits_low_power_only_dram_resource() {
    let dram = Rem6DramResourceSummary {
        activity: 1,
        active: 1,
        low_power_self_refresh_entries: 1,
        low_power_self_refresh_ticks: 11,
        ..Rem6DramResourceSummary::default()
    };
    let record = dram_resource_power_record(&dram, 0).expect("self refresh is DRAM activity");
    assert_eq!(record.residency_ticks(PowerStateKind::On), 11);
}

#[test]
fn run_power_suppresses_zero_memory_resources() {
    assert!(run_memory_power_records(20, &Rem6MemoryResourceSummary::default()).is_empty());
}

#[test]
fn run_dram_power_uses_canonical_byte_total() {
    let dram = Rem6DramResourceSummary {
        activity: 2,
        active: 1,
        active_banks: 1,
        accesses: 2,
        reads: 1,
        writes: 1,
        read_bytes: 8,
        write_bytes: 4,
        commands: 3,
        ..Rem6DramResourceSummary::default()
    };
    let record = dram_resource_power_record(&dram, 20).unwrap();
    let expected = watts_from_activity(2, 3, 12, 0.000_004, 0.000_003, 0.000_000_5);
    assert!((record.dynamic_watts() - expected).abs() < 1e-12);
}
```

The first two tests must assert a `memory.dram` record exists. The byte test must assert exact floating-point agreement within `1e-12`; it must not use the legacy `2 * 64` estimate.

- [ ] **Step 2: Add the representative CLI matrix**

Define a case table:

```rust
struct PowerActivityCase {
    name: &'static str,
    extra_args: &'static [&'static str],
    format: &'static str,
    required: &'static [&'static str],
    suppressed: &'static [&'static str],
}
```

Use four rows: `direct`, `dram`, `cache`, and `hierarchy`. For each row:

1. write the same load/store ELF;
2. request `--output`, `--stats-output`, `--power-output`, and the row's format;
3. parse the run artifact with `serde_json`;
4. import XML or CSV with `PowerAnalysisExport`;
5. assert required/suppressed target sets; and
6. correlate target presence with these canonical JSON paths:

```text
/memory_resources/cache/instruction/l1/active
/memory_resources/cache/data/l1/active
/memory_resources/cache/l2/active
/memory_resources/cache/l3/active
/memory_resources/transport/active
/memory_resources/fabric/active
/memory_resources/dram/active
```

For every active record assert `dynamic_watts() > 0.0`,
`residency_ticks(PowerStateKind::On) > 0`, and a target-specific minimum
temperature. For DRAM, calculate the expected byte contribution from
`/memory_resources/dram/read_bytes` plus `write_bytes` and assert the imported
record matches the canonical formula.

- [ ] **Step 3: Run focused tests and observe RED**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --lib power_output::tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run rem6_run_power_activity_matches_canonical_resource_matrix -- --nocapture
```

Expected: unit tests fail to compile because `dram_resource_power_record` and
`run_memory_power_records` do not exist yet; after temporary test adaptation to
the legacy helper, refresh/low-power and byte assertions also demonstrate the
behavioral mismatch. The CLI matrix should expose the byte mismatch while
existing presence rows remain green.

- [ ] **Step 4: Commit RED tests**

```bash
git add crates/rem6/src/power_output.rs crates/rem6/src/power_output/tests.rs crates/rem6/tests/cli_run/load/power_activity_matrix.rs crates/rem6/tests/source_policy/power_activity_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "test: cover canonical run power activity"
```

### Task 3: Consolidate the Normal Run Power Authority

**Files:**
- Modify: `crates/rem6/src/power_output.rs`
- Modify: `crates/rem6/src/run_execution_summary.rs`
- Modify: `crates/rem6/tests/source_policy/power_activity_ownership.rs`

- [ ] **Step 1: Narrow the normal-run builder signature**

Change:

```rust
pub(crate) fn run_power_analysis_records_from_parts(
    final_tick: u64,
    cores: &[Rem6CoreSummary],
    memory_resources: &Rem6MemoryResourceSummary,
) -> Vec<PowerAnalysisRecord>
```

Update both callers. `run_power_analysis_records` forwards only the execution's
tick, cores, and memory resources. `build_run_execution_summary` no longer passes
raw instruction cache, data cache, or DRAM summaries.

- [ ] **Step 2: Introduce focused activity projections**

Replace the duplicate raw/resource cache predicates with one projection:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CachePowerActivity {
    events: u64,
    operations: u64,
    bytes: u64,
}
```

Add analogous `TransportPowerActivity` and `DramPowerActivity`; retain the
existing `FabricPowerActivity`. Each type owns `is_active`, `operation_count`
where needed, and `residency_ticks`.

Build normal-run activities from:

```rust
memory_resources.cache_instruction.l1
memory_resources.cache_data.l1
memory_resources.cache_l2
memory_resources.cache_l3
memory_resources.transport
memory_resources.fabric
memory_resources.dram
```

Keep target-specific cache constants in a small `CachePowerCalibration` value so
the one cache record constructor handles L1 and shared-cache targets.

- [ ] **Step 3: Make DRAM activity complete and byte-accurate**

The run DRAM projection must include:

```rust
events = max(accesses, refreshes, low_power_entries, low_power_exits)
operations = commands + refreshes + low_power_entries + low_power_exits
bytes = read_bytes + write_bytes
residency = max(final_tick, refresh_ticks, low_power_ticks, exit_latency_ticks, accesses, 1)
```

Use saturating arithmetic. Keep the existing event/operation/byte scales,
temperature base, cap, and static-bank term. Raw `Rem6DramSummary` adapters used
by GPU and trace replay should populate the same projection without changing
their target names or route selection.

- [ ] **Step 4: Harden the ownership policy**

Parse or inspect the `run_power_analysis_records_from_parts` source section and
assert it does not contain `CliDataCacheSummary` or `Rem6DramSummary`, and that
`run_execution_summary.rs` passes `&memory_resources` but not raw cache/DRAM
arguments. Scope the scan to the normal-run builder so GPU and trace-replay
adapters remain legal.

- [ ] **Step 5: Run focused GREEN verification**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --lib power_output::tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run load::power_activity_matrix -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy power_activity_ownership -- --nocapture
```

Expected: all canonical activity, extraction, representative matrix, and policy tests PASS.

- [ ] **Step 6: Commit implementation**

```bash
git add crates/rem6/src/power_output.rs crates/rem6/src/run_execution_summary.rs crates/rem6/tests/source_policy/power_activity_ownership.rs
TMPDIR=$PWD/target/tmp git commit -m "refactor: unify run memory power activity"
```

### Task 4: Record Representative Migration Evidence

**Files:**
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Update the Stats component**

Change the heading to `74% representative`. Record 24 of 26 checked items, 92%
raw, capped at 74%. Check the calibrated power/thermal activity item and describe
the canonical route/format/suppression matrix while retaining hierarchy-counter,
GDB CSR, broad O3, and physical-coefficient gaps.

- [ ] **Step 2: Update the Power component**

Add one checked checklist item for canonical normal-run CPU cache, transport,
fabric, and DRAM activity calibration across McPAT/DSENT exports. Change the
heading to `74% representative` and the score calculation to 6 of 8, 75% raw,
capped at 74%. Preserve full external schema/tool parity and broader GPU,
trace-replay, NoMali, and physical calibration as open.

- [ ] **Step 3: Update the `tests/gem5/stats` crosswalk**

Raise the row to `74% representative` and cite the canonical direct/DRAM/cache/
hierarchy matrix, actual DRAM byte calibration, inactive target suppression, and
refresh/low-power boundary tests. Keep its next-evidence cell explicit.

- [ ] **Step 4: Preserve the exact ledger line count**

Run:

```bash
wc -l docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy architecture_docs_have_clear_boundaries -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy gem5_migration_sections_are_auditable -- --nocapture
```

Expected: exactly 1,200 lines and both policy tests PASS. Reflow existing prose rather than adding filler or a second progress table.

- [ ] **Step 5: Commit the ledger update**

```bash
git add docs/architecture/gem5-to-rem6-migration.md
TMPDIR=$PWD/target/tmp git commit -m "docs: record canonical power activity matrix"
```

### Task 5: Full Verification, Review, and Push

**Files:**
- Review all changed files.

- [ ] **Step 1: Run formatting and focused suites**

```bash
cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6 --lib power_output::tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run load::power_activity_matrix -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run power_import -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: all PASS.

- [ ] **Step 2: Run broad verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
TMPDIR=$PWD/target/tmp cargo test --workspace
```

Expected: all targets and workspace/doc tests PASS.

- [ ] **Step 3: Review output stability and protected paths**

```bash
git diff --check
git status --short --branch
git diff --stat origin/main...HEAD
git diff origin/main...HEAD -- crates/rem6/src/power_output.rs crates/rem6/src/run_execution_summary.rs crates/rem6/tests/cli_run/load.rs crates/rem6/tests/cli_run/load/power_activity_matrix.rs crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/power_activity_ownership.rs docs/architecture/gem5-to-rem6-migration.md
```

Confirm no CLI/config schema changes, no unrelated files, stable target ordering,
and no duplicate raw normal-run activity authority.

- [ ] **Step 4: Request independent review and address findings**

Ask one reviewer to inspect runtime correctness, saturation/units, and GPU/trace
boundaries, and another to inspect matrix claims, score arithmetic, exact line
count, and source-policy ownership. Fix findings with focused commits and rerun
affected verification.

- [ ] **Step 5: Push the completed increment**

```bash
git push origin main
git status --short --branch
git rev-parse HEAD
git rev-parse origin/main
```

Expected: `main` is clean and synchronized with `origin/main`.
