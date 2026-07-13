# Execution Mode Lane Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace duplicated `rem6` CLI execution-mode names, indexes, and static trace suffixes with one compile-time exhaustive crate-private authority while preserving every current output and unknown-mode fallback.

**Architecture:** A focused `execution_mode_lanes.rs` macro declaration will generate the lane count, descriptor table, static suffixes, and exhaustive enum-to-name match from three variant/name rows. Configuration, summaries, debug output, and stats output retain their existing mechanics but consume the generated descriptors and lookup helpers. A crate-local source-policy test prevents standalone lane names, generated suffixes, local indexes, and fixed three-lane counter dimensions from returning.

**Tech Stack:** Rust 2021, `rem6_system::ExecutionMode`, `rem6_stats::StatsRegistry`, Cargo integration tests, repository source-policy tests.

---

### Task 1: Add the red representation-authority policy

**Files:**
- Create: `crates/rem6/tests/source_policy/execution_mode_lanes.rs`
- Modify: `crates/rem6/tests/source_policy.rs:3-7`

- [ ] **Step 1: Register the focused policy module**

Add this beside the existing O3 alias policy declaration:

```rust
#[path = "source_policy/execution_mode_lanes.rs"]
mod execution_mode_lanes;
```

- [ ] **Step 2: Write the failing policy test**

Create `crates/rem6/tests/source_policy/execution_mode_lanes.rs` with this structure:

```rust
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

use super::rust_source_files;

const CONSUMERS: &[(&str, &str)] = &[
    ("CLI config", "src/config.rs"),
    ("host-event config", "src/config/host_event.rs"),
    ("host-action summaries", "src/host_actions.rs"),
    ("run execution summaries", "src/run_execution_summary.rs"),
    (
        "O3 execution-mode debug stats",
        "src/debug_output/o3_execution_mode_stats.rs",
    ),
    (
        "O3 checkpoint-restore debug JSON",
        "src/debug_output/o3_checkpoint_restore_json.rs",
    ),
    ("host-action debug JSON", "src/debug_output/host_action.rs"),
    ("O3 runtime stats", "src/stats_output/o3_runtime.rs"),
    (
        "O3 snapshot/restore stats",
        "src/stats_output/o3_runtime_snapshot_restore.rs",
    ),
    ("host-action stats", "src/stats_output/host_actions.rs"),
    ("CPU checker stats", "src/stats_output/cpu.rs"),
];

const FORBIDDEN_LOCAL_AUTHORITIES: &[&str] = &[
    "EXECUTION_MODE_STAT_LANES",
    "EXECUTION_MODE_AUTHORITY_JSON_LANES",
    "EXECUTION_MODE_STATS",
    "O3_CHECKPOINT_RESTORE_AUTHORITY_STAT_LANES",
    "fn execution_mode_authority_lane_index(",
    "fn execution_mode_index(",
    "fn parse_execution_mode(",
    "fn execution_mode_name(",
    "[u64; 3]",
    "[0_u64; 3]",
];

const FORBIDDEN_PRODUCTION_LITERALS: &[&str] = &[
    r#""functional""#,
    r#""timing""#,
    r#""detailed""#,
    "execution_mode.functional",
    "execution_mode.timing",
    "execution_mode.detailed",
    "checkpoint_restore.execution_mode_authority.mode.functional",
    "checkpoint_restore.execution_mode_authority.mode.timing",
    "checkpoint_restore.execution_mode_authority.mode.detailed",
];

#[test]
fn execution_mode_cli_lanes_have_one_representation_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let authority_path = crate_dir.join("src/execution_mode_lanes.rs");

    assert!(
        lib.contains("mod execution_mode_lanes;"),
        "src/lib.rs must declare the shared execution-mode lane authority"
    );
    assert!(
        authority_path.exists(),
        "CLI execution-mode lane mappings belong in src/execution_mode_lanes.rs"
    );

    let authority = fs::read_to_string(authority_path).unwrap();
    for anchor in [
        "macro_rules! define_execution_mode_lanes",
        "pub(crate) const EXECUTION_MODE_LANE_COUNT",
        "pub(crate) const EXECUTION_MODE_LANES",
        "pub(crate) fn execution_mode_from_name(",
        "pub(crate) const fn execution_mode_name(",
        "pub(crate) fn execution_mode_lane_index(",
        "ExecutionMode::$variant => $name",
    ] {
        assert!(
            authority.contains(anchor),
            "execution-mode lane authority is missing `{anchor}`"
        );
    }
    assert_eq!(
        authority.matches("define_execution_mode_lanes! {").count(),
        1,
        "execution-mode lane rows must have one declaration"
    );

    for (name, relative) in CONSUMERS {
        let source = fs::read_to_string(crate_dir.join(relative)).unwrap();
        let production = production_source(&source);
        assert!(
            production.contains("crate::execution_mode_lanes"),
            "{name} must consume the shared execution-mode lane authority"
        );
        for forbidden in FORBIDDEN_LOCAL_AUTHORITIES {
            assert!(
                !production.contains(forbidden),
                "{name} must not retain local execution-mode authority `{forbidden}`"
            );
        }
    }

    for path in rust_source_files(&crate_dir.join("src")) {
        let relative = path.strip_prefix(crate_dir.join("src")).unwrap();
        if relative == Path::new("execution_mode_lanes.rs") || is_test_only_source(relative) {
            continue;
        }
        let source = fs::read_to_string(&path).unwrap();
        let production = production_source(&source);
        for forbidden in FORBIDDEN_PRODUCTION_LITERALS {
            assert!(
                !production.contains(forbidden),
                "{} must consume the shared execution-mode representation instead of `{forbidden}`",
                relative.display()
            );
        }
    }
}

fn production_source(source: &str) -> &str {
    source
        .split_once("#[cfg(test)]\nmod tests {")
        .map_or(source, |(production, _tests)| production)
}

fn is_test_only_source(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == OsStr::new("tests"))
        || path.file_name().and_then(OsStr::to_str).is_some_and(|name| {
            name == "tests.rs" || name.ends_with("_tests.rs")
        })
}
```

- [ ] **Step 3: Run the policy test and record the red state**

Run:

```bash
cargo test -p rem6 --test source_policy execution_mode_lanes::execution_mode_cli_lanes_have_one_representation_authority -- --exact
```

Expected: FAIL at `src/lib.rs must declare the shared execution-mode lane authority`. Do not weaken or reorder the assertion to manufacture a different failure.

### Task 2: Implement the compile-time exhaustive authority

**Files:**
- Create: `crates/rem6/src/execution_mode_lanes.rs`
- Modify: `crates/rem6/src/lib.rs:45-55`

- [ ] **Step 1: Declare the focused module**

Add `mod execution_mode_lanes;` in alphabetical order near the other crate-private modules in `src/lib.rs`.

- [ ] **Step 2: Implement the generated lane table and lookups**

Create `crates/rem6/src/execution_mode_lanes.rs`:

```rust
use rem6_system::ExecutionMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ExecutionModeLane {
    mode: ExecutionMode,
    name: &'static str,
    o3_trace_stat_suffix: &'static str,
    o3_checkpoint_restore_trace_stat_suffix: &'static str,
}

impl ExecutionModeLane {
    const fn new(
        mode: ExecutionMode,
        name: &'static str,
        o3_trace_stat_suffix: &'static str,
        o3_checkpoint_restore_trace_stat_suffix: &'static str,
    ) -> Self {
        Self {
            mode,
            name,
            o3_trace_stat_suffix,
            o3_checkpoint_restore_trace_stat_suffix,
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        self.name
    }

    pub(crate) const fn o3_trace_stat_suffix(self) -> &'static str {
        self.o3_trace_stat_suffix
    }

    pub(crate) const fn o3_checkpoint_restore_trace_stat_suffix(self) -> &'static str {
        self.o3_checkpoint_restore_trace_stat_suffix
    }
}

macro_rules! define_execution_mode_lanes {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        pub(crate) const EXECUTION_MODE_LANE_COUNT: usize =
            [$(ExecutionMode::$variant),+].len();

        pub(crate) const EXECUTION_MODE_LANES:
            [ExecutionModeLane; EXECUTION_MODE_LANE_COUNT] = [
                $(ExecutionModeLane::new(
                    ExecutionMode::$variant,
                    $name,
                    concat!("execution_mode.", $name),
                    concat!("checkpoint_restore.execution_mode_authority.mode.", $name),
                )),+
            ];

        pub(crate) const fn execution_mode_name(mode: ExecutionMode) -> &'static str {
            match mode {
                $(ExecutionMode::$variant => $name,)+
            }
        }
    };
}

define_execution_mode_lanes! {
    Functional => "functional",
    Timing => "timing",
    Detailed => "detailed",
}

pub(crate) fn execution_mode_from_name(name: &str) -> Option<ExecutionMode> {
    EXECUTION_MODE_LANES
        .iter()
        .find(|lane| lane.name == name)
        .map(|lane| lane.mode)
}

pub(crate) fn execution_mode_lane_index(name: &str) -> Option<usize> {
    EXECUTION_MODE_LANES
        .iter()
        .position(|lane| lane.name == name)
}
```

- [ ] **Step 3: Add exact authority unit tests**

Append a `#[cfg(test)] mod tests` that asserts:

```rust
#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use rem6_system::ExecutionMode;

    use super::{
        execution_mode_from_name, execution_mode_lane_index, execution_mode_name,
        EXECUTION_MODE_LANES, EXECUTION_MODE_LANE_COUNT,
    };

    #[test]
    fn execution_mode_lane_vocabulary_and_order_are_stable() {
        assert_eq!(EXECUTION_MODE_LANE_COUNT, EXECUTION_MODE_LANES.len());
        assert_eq!(
            EXECUTION_MODE_LANES.map(|lane| lane.name()),
            ["functional", "timing", "detailed"]
        );
        assert_eq!(
            EXECUTION_MODE_LANES.map(|lane| lane.o3_trace_stat_suffix()),
            [
                "execution_mode.functional",
                "execution_mode.timing",
                "execution_mode.detailed",
            ]
        );
        assert_eq!(
            EXECUTION_MODE_LANES
                .map(|lane| lane.o3_checkpoint_restore_trace_stat_suffix()),
            [
                "checkpoint_restore.execution_mode_authority.mode.functional",
                "checkpoint_restore.execution_mode_authority.mode.timing",
                "checkpoint_restore.execution_mode_authority.mode.detailed",
            ]
        );
    }

    #[test]
    fn execution_mode_lane_names_and_suffixes_are_unique() {
        for values in [
            EXECUTION_MODE_LANES.map(|lane| lane.name()),
            EXECUTION_MODE_LANES.map(|lane| lane.o3_trace_stat_suffix()),
            EXECUTION_MODE_LANES
                .map(|lane| lane.o3_checkpoint_restore_trace_stat_suffix()),
        ] {
            let expected_len = values.len();
            assert_eq!(
                values.into_iter().collect::<BTreeSet<_>>().len(),
                expected_len
            );
        }
    }

    #[test]
    fn execution_mode_names_round_trip_and_index_in_descriptor_order() {
        for (index, mode) in [
            ExecutionMode::Functional,
            ExecutionMode::Timing,
            ExecutionMode::Detailed,
        ]
        .into_iter()
        .enumerate()
        {
            let name = execution_mode_name(mode);
            assert_eq!(execution_mode_from_name(name), Some(mode));
            assert_eq!(execution_mode_lane_index(name), Some(index));
        }
        assert_eq!(execution_mode_from_name("unknown"), None);
        assert_eq!(execution_mode_lane_index("unknown"), None);
    }
}
```

- [ ] **Step 4: Run the authority unit tests**

Run:

```bash
cargo test -p rem6 --lib execution_mode_lanes::tests
```

Expected: all three authority tests PASS. The source-policy test remains red because consumers still contain local mappings.

### Task 3: Migrate parsing and execution summaries

**Files:**
- Modify: `crates/rem6/src/config.rs`
- Modify: `crates/rem6/src/config/host_event.rs`
- Modify: `crates/rem6/src/config/riscv_timing.rs`
- Modify: `crates/rem6/src/host_actions.rs`
- Modify: `crates/rem6/src/run_execution_summary.rs`

- [ ] **Step 1: Route both config parsers through the authority**

In `config.rs`, add:

```rust
use crate::execution_mode_lanes::execution_mode_from_name as parse_execution_mode;
```

Remove `parse_execution_mode` from the `riscv_timing::{...}` import. In `config/host_event.rs`, replace the `super::riscv_timing` import with the same crate authority import. In `config/riscv_timing.rs`, remove `use rem6_system::ExecutionMode;` and delete the local `parse_execution_mode` function.

- [ ] **Step 2: Route host-action enum serialization through the authority**

In `host_actions.rs`, import:

```rust
use crate::execution_mode_lanes::execution_mode_name;
```

Remove `ExecutionMode` from the `rem6_system` import and delete the local exhaustive `execution_mode_name` function. Keep all existing call sites unchanged.

- [ ] **Step 3: Remove run-summary string literals**

In `run_execution_summary.rs`, import `ExecutionMode` from `rem6_system` and `execution_mode_name` from the new crate module. Replace the functional default with:

```rust
execution_mode_for_cpu(host_actions, cpu)
    .or(Some(execution_mode_name(ExecutionMode::Functional)))
```

Replace the detailed restore comparison with:

```rust
mode.mode == execution_mode_name(ExecutionMode::Detailed)
```

- [ ] **Step 4: Check the migrated production literals**

Run:

```bash
rg -n '"functional"|"timing"|"detailed"' crates/rem6/src/config.rs crates/rem6/src/config crates/rem6/src/host_actions.rs crates/rem6/src/run_execution_summary.rs
```

Expected: no production mapping literals outside tests; only test-oracle strings are allowed.

### Task 4: Migrate debug counters, JSON, and static suffixes

**Files:**
- Modify: `crates/rem6/src/debug_output/o3_execution_mode_stats.rs`
- Modify: `crates/rem6/src/debug_output/o3_checkpoint_restore_json.rs`
- Modify: `crates/rem6/src/debug_output/host_action.rs`

- [ ] **Step 1: Replace the O3 execution-mode tuple table**

Import:

```rust
use crate::execution_mode_lanes::{
    execution_mode_lane_index, EXECUTION_MODE_LANES, EXECUTION_MODE_LANE_COUNT,
};
```

Delete `EXECUTION_MODE_STATS` and `execution_mode_index`. Change every execution-mode counter type and initializer to `EXECUTION_MODE_LANE_COUNT`. Iterate descriptors and use `lane.name()` or `lane.o3_trace_stat_suffix()` instead of tuple fields.

- [ ] **Step 2: Remove the checkpoint suffix/index table**

In `o3_checkpoint_restore_json.rs`, import the same three authority items, delete both local lane constants, and change every counter dimension from `3` or a local `.len()` to `EXECUTION_MODE_LANE_COUNT`. In `push_stats`, replace the local `(suffix, index)` loop with:

```rust
for (index, lane) in EXECUTION_MODE_LANES.iter().enumerate() {
    stats.push(Rem6O3TraceStat {
        suffix: lane.o3_checkpoint_restore_trace_stat_suffix(),
        unit: "Count",
        value: self.modes[index],
    });
}
```

Use `execution_mode_lane_index` for all counting and use `lane.name()` for generated JSON/stat paths. Delete `execution_mode_authority_lane_index`.

- [ ] **Step 3: Migrate host-action debug JSON**

In `debug_output/host_action.rs`, import the three authority items, delete `EXECUTION_MODE_AUTHORITY_JSON_LANES`, and replace all array dimensions and initializers with `EXECUTION_MODE_LANE_COUNT`. Iterate `EXECUTION_MODE_LANES` for JSON fields and stat paths, use the shared index helper, and delete the local index helper.

- [ ] **Step 4: Confirm no fixed execution-mode dimensions remain**

Run:

```bash
rg -n '\[u64; 3\]|\[0_u64; 3\]|EXECUTION_MODE_STATS|O3_CHECKPOINT_RESTORE_AUTHORITY_STAT_LANES|execution_mode_authority_lane_index|fn execution_mode_index' crates/rem6/src/debug_output
```

Expected: no matches.

### Task 5: Migrate stats consumers and preserve unknown lanes

**Files:**
- Modify: `crates/rem6/src/stats_output/o3_runtime.rs`
- Modify: `crates/rem6/src/stats_output/o3_runtime_snapshot_restore.rs`
- Modify: `crates/rem6/src/stats_output/host_actions.rs`
- Modify: `crates/rem6/src/stats_output/cpu.rs`

- [ ] **Step 1: Replace local stats lane arrays**

In each file, import the required authority items directly from `crate::execution_mode_lanes`. Replace every loop over `EXECUTION_MODE_STAT_LANES` with a loop over descriptors:

```rust
for lane in EXECUTION_MODE_LANES {
    let mode = lane.name();
}
```

This is a loop-header substitution: place each existing loop body after the new
`let mode` binding without changing its increments, paths, values, units, reset
policies, or error propagation.

Replace every `.contains(&mode)` known-lane check with:

```rust
execution_mode_lane_index(mode).is_some()
```

Use `execution_mode_lane_index(mode).is_none()` at the existing unknown-lane
branches. Delete all three local `EXECUTION_MODE_STAT_LANES` constants and
remove the snapshot/restore module's import of its parent's local constant.

- [ ] **Step 2: Extract the O3 mode emission block for focused testing**

Move only the current execution-mode block from `emit_o3_runtime_stats` into:

```rust
fn emit_o3_execution_mode_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    execution_mode: Option<&str>,
) -> Result<(), Rem6CliError> {
    for lane in EXECUTION_MODE_LANES {
        let mode = lane.name();
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.execution_mode.{mode}"),
            u64::from(execution_mode == Some(mode)),
        )?;
    }
    if let Some(mode) = execution_mode {
        if execution_mode_lane_index(mode).is_none() {
            increment_count_stat(
                stats,
                format!("sim.cpu{cpu}.o3.execution_mode.{}", stat_path_segment(mode)),
                1,
            )?;
        }
    }
    Ok(())
}
```

Call it at the original location so stat ordering remains unchanged.

- [ ] **Step 3: Add focused unknown-mode regression tests**

Add a test module to `o3_runtime.rs` that calls `emit_o3_execution_mode_stats` with `Some("future-mode")`, snapshots the registry, asserts the three known lanes are zero, and asserts `sim.cpu2.o3.execution_mode.future_mode == 1`.

Add a test module to `cpu.rs` that calls `emit_checker_execution_mode_stats` with:

```rust
Rem6CheckerSummary {
    checked_instructions: 7,
    mismatches: 2,
    execution_mode: Some("future-mode"),
}
```

Assert the three known checked-instruction and mismatch lanes are zero, then assert the sanitized `future_mode` lanes retain values `7` and `2`. Use `StatsRegistry::snapshot(0)` and `crate::stats_output::snapshot_sample_value` for exact values.

- [ ] **Step 4: Run the focused stats tests**

Run:

```bash
cargo test -p rem6 --lib unknown_dynamic_lane
```

Expected: both O3-runtime and checker unknown-lane tests PASS.

### Task 6: Turn the policy green and verify behavior

**Files:**
- Verify all files modified in Tasks 1-5
- Do not modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Format the workspace**

Run:

```bash
cargo fmt --all
```

- [ ] **Step 2: Run the authority and policy tests**

Run:

```bash
cargo test -p rem6 --lib execution_mode_lanes::tests
cargo test -p rem6 --test source_policy execution_mode_lanes::execution_mode_cli_lanes_have_one_representation_authority -- --exact
cargo test -p rem6 --test source_policy
```

Expected: all commands PASS, including the formerly red policy test.

- [ ] **Step 3: Run focused executable regressions**

Run:

```bash
cargo test -p rem6 --test cli_run o3_start_mode
cargo test -p rem6 --test cli_run checker_cpu
cargo test -p rem6 --test cli_run m5_host_actions
```

Expected: all selected CLI tests PASS with unchanged config parsing, summary strings, JSON fields, trace suffixes, stat paths, checkpoint restore selection, and unknown-mode behavior.

- [ ] **Step 4: Run full verification**

Run:

```bash
cargo fmt --all -- --check
cargo test -p rem6
cargo test --workspace
git diff --check
wc -l docs/architecture/gem5-to-rem6-migration.md
```

Expected: formatting and all tests PASS, `git diff --check` is silent, and the migration ledger reports exactly `1200` lines.

- [ ] **Step 5: Audit the final diff with an xhigh read-only reviewer**

Request a whole-diff review focused on behavior changes, missed execution-mode literals or fixed dimensions, dead helpers, crate-boundary violations, insufficient tests, source-policy loopholes, and migration-ledger honesty. Resolve every actionable finding and rerun the affected verification commands.

- [ ] **Step 6: Commit and push the green implementation**

Run:

```bash
git add crates/rem6/src crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/execution_mode_lanes.rs
git commit -m "refactor: centralize execution mode lanes"
git push origin main
```

Confirm local `HEAD` equals `origin/main` after the push. The design and plan documentation commits must be included; the migration ledger must remain unchanged.
