# RISC-V O3 Live-Window Depth Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an independently configurable eight-row untranslated scalar O3 live window while preserving the four-row scalar-memory limit, existing defaults, ordered retirement, and top-level timing/lifecycle behavior.

**Architecture:** Resolve scalar-memory and scalar-live depths as one validated configuration pair, then apply them atomically to the CPU runtime. Give untranslated scalar-memory-prefix windows a typed deep policy, keep translated/result/control/FU paths bounded to four rows, and make the scoped scheduler the sole dependency/resource classifier through typed ephemeral scopes and reservation-aware planning.

**Tech Stack:** Rust 2021 workspace, `rem6` CLI integration tests, `rem6-cpu` runtime and scheduler tests, Cargo, serde/TOML configuration, structured JSON/text/m5 stats.

---

## Per-Task Push Gate

Before every `git commit`/`git push` block below, dispatch a fresh high-intensity read-only reviewer (`gpt-5.5`, `xhigh`) over that task's diff, the design spec, and the active contract. Fix every actionable finding, rerun the task's listed verification, run `git diff --check`, close the reviewer, then execute the commit and push commands. A task is not complete merely because its focused tests pass.

**Execution prerequisite:** Commit and push this plan document before starting Task 1. During implementation it is a tracked baseline artifact, so the final clean-worktree gate does not need to add it again.

## File Map

- `crates/rem6-cpu/src/riscv_defaults.rs`: public scalar-memory/live depth bounds.
- `crates/rem6/src/config/riscv_timing.rs`: parsing, pair validation, and `RiscvO3WindowDepths` resolution.
- `crates/rem6/src/config.rs`, `config/accessors.rs`, `cli_error.rs`, `run_validation.rs`, `riscv_core_runtime.rs`: thin CLI/TOML/startup wiring.
- `crates/rem6-cpu/src/o3_runtime.rs`, `o3_runtime_memory.rs`, `o3_runtime_memory_window.rs`, `lib.rs`: atomic runtime depth state and typed data-access policy.
- `crates/rem6-cpu/src/riscv_o3_window_policy.rs`, `riscv_fetch_ahead/detailed_o3.rs`, `riscv_live_retire_window.rs`, `riscv_data_issue.rs`: untranslated eight-row admission while all other window families remain four-row bounded.
- `crates/rem6-cpu/src/o3_pipeline.rs`: reservation-aware scoped issue planning.
- `crates/rem6-cpu/src/o3_runtime_control_window.rs`, `o3_runtime_issue.rs`, new `o3_runtime_issue/dependency.rs`, `o3_runtime_error.rs`: metadata/executable split, typed scopes, atomic selected batches.
- `crates/rem6/tests/cli_run/validation/o3_depths.rs`: focused scalar-depth validation.
- `crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth.rs` plus `fixture.rs`, `lifecycle.rs`, `memory_boundary.rs`: top-level matrix and lifecycle evidence.
- `crates/rem6-cpu/src/o3_runtime_issue_tests/dependency_scopes.rs`: focused dependency ownership tests.
- `crates/rem6/tests/source_policy/o3_live_window_ownership.rs`, `crates/rem6-cpu/tests/source_policy.rs`: file ownership and cap enforcement.
- `crates/rem6/tests/source_policy/core_test_anchors.txt`: mechanically register the new top-level evidence anchors.
- `docs/architecture/gem5-to-rem6-migration.md`: honest executable-evidence update at exactly 1,200 lines and unchanged CPU score.

### Task 1: Extract Existing O3 Depth Validation Ownership

**Files:**
- Create: `crates/rem6/tests/cli_run/validation/o3_depths.rs`
- Modify: `crates/rem6/tests/cli_run/validation.rs`

- [ ] **Step 1: Add the focused child declaration**

Insert before the imports in `validation.rs`:

```rust
mod o3_depths;
```

- [ ] **Step 2: Move the existing tests without semantic edits**

Create `validation/o3_depths.rs` with `use super::*;`, then move the complete bodies of exactly these tests from the root:

```text
rem6_run_accepts_max_riscv_o3_scalar_memory_depth
rem6_run_accepts_riscv_o3_scalar_memory_depth_from_config
rem6_run_validates_toml_riscv_o3_scalar_memory_depth_requirements
rem6_run_rejects_riscv_o3_scalar_memory_depth_without_execution
rem6_run_rejects_riscv_o3_scalar_memory_depth_without_riscv_isa
rem6_run_rejects_invalid_riscv_o3_scalar_memory_depth_values
rem6_run_rejects_invalid_riscv_o3_scalar_memory_depth_from_config
rem6_run_config_scan_treats_riscv_o3_scalar_memory_depth_as_value_taking
```

Do not rename tests or change assertions in this commit.

- [ ] **Step 3: Verify behavior and ownership**

Run:

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run validation::o3_depths -- --nocapture
cargo fmt --all -- --check
rg -n "fn rem6_run_.*riscv_o3_scalar_memory_depth" crates/rem6/tests/cli_run/validation.rs crates/rem6/tests/cli_run/validation/o3_depths.rs
```

Expected: all eight tests pass and each test definition appears only in the child.

- [ ] **Step 4: Run the per-task read-only review gate, then commit and push**

```bash
git add crates/rem6/tests/cli_run/validation.rs crates/rem6/tests/cli_run/validation/o3_depths.rs
TMPDIR=$PWD/target/tmp git commit -m "test: extract o3 depth validation"
git push origin main
```

### Task 2: Resolve and Apply the Scalar Depth Pair

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_defaults.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6/src/config/riscv_timing.rs`
- Modify: `crates/rem6/src/config.rs`
- Modify: `crates/rem6/src/config/accessors.rs`
- Modify: `crates/rem6/src/cli_error.rs`
- Modify: `crates/rem6/src/run_validation.rs`
- Modify: `crates/rem6/src/riscv_core_runtime.rs`
- Test: `crates/rem6/tests/cli_run/validation/o3_depths.rs`
- Test: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`

- [ ] **Step 1: Add failing CLI/TOML tests for the complete pair contract**

Add these helpers to `validation/o3_depths.rs`:

```rust
use std::path::{Path, PathBuf};

fn noop_riscv_binary(name: &str) -> PathBuf {
    let program = riscv64_program(&[0x0000_0013; 64]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn depth_command(path: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "80",
        "--stats-format",
        "json",
        "--execute",
    ]);
    command
}

fn assert_rejected(mut command: Command, expected: &str) {
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains(expected), "stderr: {stderr}");
}
```

Add these positive tests for CLI/TOML selection and precedence:

```rust
#[test]
fn rem6_run_accepts_riscv_o3_scalar_live_window_depth_from_cli_and_toml() {
    let cli_path = noop_riscv_binary("riscv-o3-live-depth-cli");
    let mut cli = depth_command(&cli_path);
    cli.args([
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--riscv-o3-scalar-live-window-depth",
        "8",
    ]);
    let output = cli.output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let toml_path = noop_riscv_binary("riscv-o3-live-depth-toml");
    let config = temp_output("riscv-o3-live-depth.toml");
    std::fs::write(
        &config,
        "[run]\nriscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 8\n",
    )
    .unwrap();
    let mut toml = depth_command(&toml_path);
    toml.args(["--config", config.to_str().unwrap()]);
    let output = toml.output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn rem6_run_cli_scalar_live_depth_overrides_toml() {
    let path = noop_riscv_binary("riscv-o3-live-depth-precedence");
    let config = temp_output("riscv-o3-live-depth-precedence.toml");
    std::fs::write(
        &config,
        "[run]\nriscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 6\n",
    )
    .unwrap();
    let mut command = depth_command(&path);
    command.args([
        "--config",
        config.to_str().unwrap(),
        "--riscv-o3-scalar-live-window-depth",
        "8",
    ]);
    let output = command.output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn rem6_run_accepts_live_only_with_implicit_memory_default() {
    let cli_path = noop_riscv_binary("riscv-o3-live-only-cli");
    let mut cli = depth_command(&cli_path);
    cli.args(["--riscv-o3-scalar-live-window-depth", "6"]);
    let output = cli.output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let toml_path = noop_riscv_binary("riscv-o3-live-only-toml");
    let config = temp_output("riscv-o3-live-only.toml");
    std::fs::write(&config, "[run]\nriscv_o3_scalar_live_window_depth = 6\n").unwrap();
    let mut toml = depth_command(&toml_path);
    toml.args(["--config", config.to_str().unwrap()]);
    let output = toml.output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn rem6_run_depth_pair_override_is_validated_after_cli_precedence() {
    let path = noop_riscv_binary("riscv-o3-depth-pair-post-precedence");
    let config = temp_output("riscv-o3-depth-pair-post-precedence.toml");
    std::fs::write(
        &config,
        "[run]\nriscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 3\n",
    )
    .unwrap();
    let mut repaired = depth_command(&path);
    repaired.args([
        "--config",
        config.to_str().unwrap(),
        "--riscv-o3-scalar-live-window-depth",
        "8",
    ]);
    let output = repaired.output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));

    let valid_config = temp_output("riscv-o3-depth-pair-invalid-cli-override.toml");
    std::fs::write(
        &valid_config,
        "[run]\nriscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 8\n",
    )
    .unwrap();
    let mut invalid = depth_command(&path);
    invalid.args([
        "--config",
        valid_config.to_str().unwrap(),
        "--riscv-o3-scalar-live-window-depth",
        "3",
    ]);
    assert_rejected(
        invalid,
        "RISC-V O3 scalar live-window depth 3 is below scalar memory depth 4",
    );
}
```

Add a table-driven negative test with these exact rows:

```rust
#[test]
fn rem6_run_rejects_invalid_scalar_live_depths_from_cli() {
let cases = [
    (
        "live-zero",
        vec!["--riscv-o3-scalar-live-window-depth", "0"],
        "invalid RISC-V O3 scalar live-window depth 0",
    ),
    (
        "live-nine",
        vec!["--riscv-o3-scalar-live-window-depth", "9"],
        "invalid RISC-V O3 scalar live-window depth 9",
    ),
    (
        "live-below-explicit-memory",
        vec![
            "--riscv-o3-scalar-memory-depth",
            "4",
            "--riscv-o3-scalar-live-window-depth",
            "3",
        ],
        "RISC-V O3 scalar live-window depth 3 is below scalar memory depth 4",
    ),
    (
        "live-below-default-memory",
        vec!["--riscv-o3-scalar-live-window-depth", "1"],
        "RISC-V O3 scalar live-window depth 1 is below scalar memory depth 2",
    ),
];

for (name, args, expected) in cases {
    let path = noop_riscv_binary(name);
    let mut command = depth_command(&path);
    command.args(args);
    assert_rejected(command, expected);
}
}
```

Add these named table-driven tests:

```text
rem6_run_rejects_invalid_scalar_live_depths_from_toml
rem6_run_rejects_toml_live_depth_below_memory_depth
rem6_run_rejects_scalar_live_depth_without_execution_or_riscv
rem6_run_validates_toml_scalar_live_depth_requirements
```

The TOML invalid-value rows write live depth `0` and `9`; the ordering row writes memory `4`/live `3`; the requirements rows cover no `--execute` and `--isa x86` for both CLI and TOML, with the exact diagnostics already listed. Extend the pre-scan test so the new flag consumes its following value.

Use these complete bodies:

```rust
#[test]
fn rem6_run_rejects_invalid_scalar_live_depths_from_toml() {
    for value in [0, 9] {
        let path = noop_riscv_binary(&format!("riscv-o3-live-toml-{value}"));
        let config = temp_output(&format!("riscv-o3-live-toml-{value}.toml"));
        std::fs::write(
            &config,
            format!("[run]\nriscv_o3_scalar_live_window_depth = {value}\n"),
        )
        .unwrap();
        let mut command = depth_command(&path);
        command.args(["--config", config.to_str().unwrap()]);
        assert_rejected(
            command,
            &format!("invalid RISC-V O3 scalar live-window depth {value}"),
        );
    }
}

#[test]
fn rem6_run_rejects_toml_live_depth_below_memory_depth() {
    let path = noop_riscv_binary("riscv-o3-live-toml-order");
    let config = temp_output("riscv-o3-live-toml-order.toml");
    std::fs::write(
        &config,
        "[run]\nriscv_o3_scalar_memory_depth = 4\nriscv_o3_scalar_live_window_depth = 3\n",
    )
    .unwrap();
    let mut command = depth_command(&path);
    command.args(["--config", config.to_str().unwrap()]);
    assert_rejected(
        command,
        "RISC-V O3 scalar live-window depth 3 is below scalar memory depth 4",
    );
}

#[test]
fn rem6_run_rejects_scalar_live_depth_without_execution_or_riscv() {
    let riscv = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let x86 = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);
    for (name, isa, elf, execute, expected) in [
        (
            "riscv-o3-live-without-execute",
            "riscv",
            riscv.as_slice(),
            false,
            "--riscv-o3-scalar-live-window-depth requires --execute",
        ),
        (
            "riscv-o3-live-without-riscv",
            "x86",
            x86.as_slice(),
            true,
            "--riscv-o3-scalar-live-window-depth requires --isa riscv",
        ),
    ] {
        let path = temp_binary(name, elf);
        let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
        command.args([
            "run",
            "--isa",
            isa,
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--riscv-o3-scalar-live-window-depth",
            "8",
        ]);
        if execute {
            command.arg("--execute");
        }
        assert_rejected(command, expected);
    }
}

#[test]
fn rem6_run_validates_toml_scalar_live_depth_requirements() {
    let riscv = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let x86 = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);
    for (name, isa, elf, execute, expected) in [
        (
            "riscv-o3-live-toml-without-execute",
            "riscv",
            riscv.as_slice(),
            false,
            "--riscv-o3-scalar-live-window-depth requires --execute",
        ),
        (
            "riscv-o3-live-toml-without-riscv",
            "x86",
            x86.as_slice(),
            true,
            "--riscv-o3-scalar-live-window-depth requires --isa riscv",
        ),
    ] {
        let path = temp_binary(name, elf);
        let config = temp_output(&format!("{name}.toml"));
        std::fs::write(
            &config,
            "[run]\nriscv_o3_scalar_live_window_depth = 8\n",
        )
        .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
        command.args([
            "run",
            "--config",
            config.to_str().unwrap(),
            "--isa",
            isa,
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
        ]);
        if execute {
            command.arg("--execute");
        }
        assert_rejected(command, expected);
    }
}
```

- [ ] **Step 2: Run RED verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run validation::o3_depths -- --nocapture
```

Expected: FAIL because the new flag and TOML key are unknown.

- [ ] **Step 3: Export CPU-owned bounds**

Add to `riscv_defaults.rs`:

```rust
pub const MIN_RISCV_O3_SCALAR_MEMORY_DEPTH: usize = 1;
pub const MAX_RISCV_O3_SCALAR_MEMORY_DEPTH: usize = 4;
pub const MIN_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH: usize = 1;
pub const MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH: usize = 8;
```

Delete the duplicate private memory-depth maxima from `config/riscv_timing.rs` and `o3_runtime_memory_window.rs`.

- [ ] **Step 4: Implement the focused pair resolver**

In `config/riscv_timing.rs`, add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvO3WindowDepths {
    scalar_memory: usize,
    scalar_live: usize,
}

impl RiscvO3WindowDepths {
    pub(crate) const fn scalar_memory(self) -> usize {
        self.scalar_memory
    }

    pub(crate) const fn scalar_live(self) -> usize {
        self.scalar_live
    }
}

pub(crate) fn resolve_riscv_o3_window_depths(
    branch_lookahead: usize,
    scalar_memory: Option<usize>,
    scalar_live: Option<usize>,
) -> Result<RiscvO3WindowDepths, Rem6CliError> {
    let scalar_memory = scalar_memory
        .unwrap_or_else(|| branch_lookahead.saturating_add(1));
    let scalar_live = scalar_live.unwrap_or(scalar_memory);
    validate_riscv_o3_scalar_memory_depth(scalar_memory)?;
    validate_riscv_o3_scalar_live_window_depth(scalar_live)?;
    if scalar_live < scalar_memory {
        return Err(Rem6CliError::RiscvO3ScalarLiveWindowDepthBelowMemoryDepth {
            scalar_memory_depth: scalar_memory,
            scalar_live_window_depth: scalar_live,
        });
    }
    Ok(RiscvO3WindowDepths {
        scalar_memory,
        scalar_live,
    })
}
```

Add this test in the same focused module:

```rust
#[test]
fn riscv_o3_window_depth_resolution_covers_all_omission_combinations() {
    for (memory, live, expected) in [
        (None, None, (2, 2)),
        (Some(4), None, (4, 4)),
        (None, Some(6), (2, 6)),
        (Some(4), Some(8), (4, 8)),
    ] {
        let depths = resolve_riscv_o3_window_depths(1, memory, live).unwrap();
        assert_eq!(
            (depths.scalar_memory(), depths.scalar_live()),
            expected
        );
    }
}
```

Add `parse_riscv_o3_scalar_live_window_depth`, optional validation, and the 1-through-8 range check. Add these error variants and display messages:

```rust
InvalidRiscvO3ScalarLiveWindowDepth { value: String },
RiscvO3ScalarLiveWindowDepthBelowMemoryDepth {
    scalar_memory_depth: usize,
    scalar_live_window_depth: usize,
},
RiscvO3ScalarLiveWindowDepthRequiresExecution,
RiscvO3ScalarLiveWindowDepthRequiresRiscv,
```

```text
invalid RISC-V O3 scalar live-window depth {value}
RISC-V O3 scalar live-window depth {scalar_live_window_depth} is below scalar memory depth {scalar_memory_depth}
--riscv-o3-scalar-live-window-depth requires --execute
--riscv-o3-scalar-live-window-depth requires --isa riscv
```

- [ ] **Step 5: Thread raw fields through the thin config facade**

Add `riscv_o3_scalar_live_window_depth: Option<usize>` beside the existing memory-depth field in `Rem6RunConfig` and `Rem6RunFileConfig`. Parse the TOML value in `riscv_timing.rs`, add this CLI arm, retain both raw options, and invoke the resolver after all CLI overrides and before constructing the config:

```rust
"--riscv-o3-scalar-live-window-depth" => {
    riscv_o3_scalar_live_window_depth = Some(
        parse_riscv_o3_scalar_live_window_depth(&required_value(&flag, args.next())?)?,
    );
}
```

```rust
resolve_riscv_o3_window_depths(
    riscv_branch_lookahead,
    riscv_o3_scalar_memory_depth,
    riscv_o3_scalar_live_window_depth,
)?;
```

Keep every pair/range rule outside `config.rs`; that file must remain below 1,700 lines.

- [ ] **Step 6: Expose resolved accessors and top-level requirements**

In `config/accessors.rs`, add:

```rust
pub(crate) fn riscv_o3_window_depths(&self) -> RiscvO3WindowDepths {
    resolve_riscv_o3_window_depths(
        self.riscv_branch_lookahead,
        self.riscv_o3_scalar_memory_depth,
        self.riscv_o3_scalar_live_window_depth,
    )
    .expect("RISC-V O3 window depths were validated during configuration parsing")
}

pub const fn riscv_o3_scalar_live_window_depth_is_explicit(&self) -> bool {
    self.riscv_o3_scalar_live_window_depth.is_some()
}
```

Make the existing memory-depth accessor delegate to the resolved pair and add a live-depth accessor. In `run_validation.rs`, mirror the existing O3 execute/RISC-V checks for an explicit live-depth selection.

- [ ] **Step 7: Add one atomic runtime pair**

Replace the runtime fields with:

```rust
scalar_memory_window_limit: usize,
scalar_live_window_limit: usize,
window_depths_explicit: bool,
```

Initialize both limits to `2`. Implement in `o3_runtime_memory_window.rs`:

```rust
pub(crate) fn set_window_depths(
    &mut self,
    scalar_memory: usize,
    scalar_live: usize,
) -> bool {
    if !(MIN_RISCV_O3_SCALAR_MEMORY_DEPTH..=MAX_RISCV_O3_SCALAR_MEMORY_DEPTH)
        .contains(&scalar_memory)
        || !(MIN_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH..=MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH)
            .contains(&scalar_live)
        || scalar_live < scalar_memory
    {
        return false;
    }
    self.scalar_memory_window_limit = scalar_memory;
    self.scalar_live_window_limit = scalar_live;
    self.window_depths_explicit = true;
    true
}

pub(crate) fn set_scalar_memory_window_limit(&mut self, limit: usize) {
    let limit = limit.clamp(
        MIN_RISCV_O3_SCALAR_MEMORY_DEPTH,
        MAX_RISCV_O3_SCALAR_MEMORY_DEPTH,
    );
    assert!(self.set_window_depths(limit, limit));
}

pub(crate) fn set_branch_derived_window_depths(&mut self, limit: usize) {
    if self.window_depths_explicit {
        return;
    }
    let limit = limit.clamp(
        MIN_RISCV_O3_SCALAR_MEMORY_DEPTH,
        MAX_RISCV_O3_SCALAR_MEMORY_DEPTH,
    );
    self.scalar_memory_window_limit = limit;
    self.scalar_live_window_limit = limit;
}

pub(crate) const fn scalar_live_window_limit(&self) -> usize {
    self.scalar_live_window_limit
}
```

Update `RiscvCore::set_branch_lookahead` to call `set_branch_derived_window_depths`. Add public `set_o3_window_depths(memory, live)` with an assertion on `set_window_depths`. Preserve the existing public compatibility setter's clamp semantics:

```rust
pub fn set_o3_scalar_memory_depth(&self, depth: usize) {
    let depth = depth.clamp(
        MIN_RISCV_O3_SCALAR_MEMORY_DEPTH,
        MAX_RISCV_O3_SCALAR_MEMORY_DEPTH,
    );
    self.set_o3_window_depths(depth, depth);
}
```

In `riscv_core_runtime.rs`, preserve startup order:

```rust
core.set_branch_lookahead(config.riscv_branch_lookahead());
let depths = config.riscv_o3_window_depths();
core.set_o3_window_depths(depths.scalar_memory(), depths.scalar_live());
```

- [ ] **Step 8: Add focused runtime pair tests**

Add exactly these tests:

```text
window_depth_pair_defaults_and_branch_derivation_move_together
explicit_window_depth_pair_is_atomic_and_stable
scalar_memory_compatibility_setter_sets_both_limits
```

The first proves default `(2,2)` and branch-derived `(4,4)`; the second proves explicit `(4,8)` stability and invalid `(4,3)` rejection without mutation; the third proves compatibility `(3,3)`.

```rust
#[test]
fn window_depth_pair_defaults_and_branch_derivation_move_together() {
    let mut runtime = O3RuntimeState::default();
    assert_eq!(runtime.scalar_memory_window_limit(), 2);
    assert_eq!(runtime.scalar_live_window_limit(), 2);
    runtime.set_branch_derived_window_depths(4);
    assert_eq!(runtime.scalar_memory_window_limit(), 4);
    assert_eq!(runtime.scalar_live_window_limit(), 4);
}

#[test]
fn explicit_window_depth_pair_is_atomic_and_stable() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_window_depths(4, 8));
    runtime.set_branch_derived_window_depths(2);
    assert_eq!(runtime.scalar_memory_window_limit(), 4);
    assert_eq!(runtime.scalar_live_window_limit(), 8);
    assert!(!runtime.set_window_depths(4, 3));
    assert_eq!(runtime.scalar_memory_window_limit(), 4);
    assert_eq!(runtime.scalar_live_window_limit(), 8);
}

#[test]
fn scalar_memory_compatibility_setter_sets_both_limits() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(3);
    assert_eq!(runtime.scalar_memory_window_limit(), 3);
    assert_eq!(runtime.scalar_live_window_limit(), 3);
}
```

- [ ] **Step 9: Run GREEN verification and enforce the facade cap**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run validation::o3_depths -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 riscv_o3_window_depth_resolution_covers_all_omission_combinations -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu window_depth_pair_ -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu scalar_memory_compatibility_setter_sets_both_limits -- --nocapture
test "$(wc -l < crates/rem6/src/config.rs)" -lt 1700
cargo fmt --all -- --check
```

Expected: focused tests pass and `config.rs` remains below 1,700 lines.

- [ ] **Step 10: Run the per-task read-only review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/riscv_defaults.rs crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_memory_window.rs crates/rem6-cpu/src/lib.rs crates/rem6/src/config/riscv_timing.rs crates/rem6/src/config.rs crates/rem6/src/config/accessors.rs crates/rem6/src/cli_error.rs crates/rem6/src/run_validation.rs crates/rem6/src/riscv_core_runtime.rs crates/rem6/tests/cli_run/validation/o3_depths.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: configure scalar o3 live window depth"
git push origin main
```

### Task 3: Separate Scalar-Memory Capacity from Deep Scalar Admission

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- Modify: `crates/rem6-cpu/src/riscv_data_issue.rs`
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Test: `crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_window.rs`
- Test: `crates/rem6-cpu/src/o3_runtime_memory_window.rs`
- Test: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth/fixture.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`

- [ ] **Step 1: Build the exact eight-row fixture**

Declare `live_window_depth` in `o3.rs`; in its root use `super::*`, declare `fixture`, and import it. Define these helpers locally because sibling O3 modules do not export them:

```rust
fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

fn assert_final_witness<const N: usize>(
    json: &Value,
    expected_memory: &str,
    expected_registers: [(&str, &str); N],
) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(expected_memory)
    );
    for (register, value) in expected_registers {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value)
        );
    }
}
```

Create `fixture.rs` with:

```rust
use super::*;

pub(super) const LOAD_PC: &str = "0x8000001c";
pub(super) const ROW_PCS: [&str; 7] = [
    "0x80000020",
    "0x80000024",
    "0x80000028",
    "0x8000002c",
    "0x80000030",
    "0x80000034",
    "0x80000038",
];
pub(super) const FINAL_MEMORY: &str = "09000000000000002a00000000000000";

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x33
}

pub(super) fn scalar_live_window_binary(
    name: &str,
    dump_stats: bool,
) -> std::path::PathBuf {
    let mut words = vec![
        i_type(2, 0, 0x0, 1, 0x13),
        i_type(3, 0, 0x0, 2, 0x13),
        i_type(4, 0, 0x0, 3, 0x13),
        i_type(5, 0, 0x0, 4, 0x13),
        i_type(7, 0, 0x0, 13, 0x13),
        u_type(0, 10, 0x17),
        i_type(76, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b011, 5, 0x03),
        r_type(0x01, 2, 1, 0x0, 6),
        r_type(0x01, 4, 3, 0x0, 7),
        i_type(1, 6, 0x0, 8, 0x13),
        r_type(0x00, 7, 6, 0x0, 9),
        i_type(1, 13, 0x0, 14, 0x13),
        r_type(0x00, 9, 8, 0x0, 16),
        r_type(0x00, 5, 16, 0x0, 17),
        s_type(8, 17, 10, 0b011),
    ];
    if dump_stats {
        words.push(m5op(M5_DUMP_STATS));
    }
    words.extend([m5op(M5_EXIT), m5op(M5_FAIL)]);
    while words.len() * 4 < 96 {
        words.push(0);
    }
    words.extend([9, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
```

Add these command/JSON helpers:

```rust
pub(super) fn scalar_live_window_command(
    path: &Path,
    memory_system: &str,
    live_depth: usize,
    issue_width: usize,
    max_tick: u64,
    mode: &str,
    stats_format: &str,
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
        stats_format,
        "--execute",
        "--riscv-execution-mode",
        mode,
        "--riscv-o3-scalar-memory-depth",
        "1",
        "--riscv-o3-scalar-live-window-depth",
        &live_depth.to_string(),
        "--riscv-o3-issue-width",
        &issue_width.to_string(),
        "--riscv-o3-writeback-width",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "80",
        "--dump-memory",
        "0x80000060:16",
    ]);
    if stats_format == "json" {
        command.args(["--debug-flags", "O3,Data,Fetch,Memory,HostAction"]);
    }
    command
}

pub(super) fn scalar_live_window_json(
    path: &Path,
    memory_system: &str,
    live_depth: usize,
    issue_width: usize,
    max_tick: u64,
) -> Value {
    let output = scalar_live_window_command(
        path,
        memory_system,
        live_depth,
        issue_width,
        max_tick,
        "detailed",
        "json",
    )
    .output()
    .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
```

- [ ] **Step 2: Add the failing depth-eight resident test**

```rust
#[test]
fn rem6_run_o3_scalar_live_depth_eight_resides_with_one_lsq_row() {
    let path = scalar_live_window_binary("o3-scalar-live-depth-eight-resident", false);
    let completed = scalar_live_window_json(&path, "direct", 8, 4, 2_000);
    let response_tick = event_u64(event_at_pc(&completed, LOAD_PC), "lsq_data_response_tick");
    let resident = scalar_live_window_json(&path, "direct", 8, 4, response_tick - 1);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(rob.len(), 8, "resident deep window: {resident}");
    assert_eq!(
        rob.iter()
            .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        std::iter::once(LOAD_PC).chain(ROW_PCS).collect::<Vec<_>>()
    );
    assert!(rob.iter().all(|entry| {
        entry.pointer("/live_staged").and_then(Value::as_bool) == Some(true)
    }));
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_json_stat(&resident, "sim.cpu0.o3.max_rob_occupancy", "Count", 8, "monotonic");
    assert_json_stat(&resident, "sim.cpu0.o3.max_lsq_occupancy", "Count", 1, "monotonic");
}
```

- [ ] **Step 3: Run RED verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run live_window_depth::rem6_run_o3_scalar_live_depth_eight_resides_with_one_lsq_row -- --nocapture
```

Expected: FAIL because the current classifier clamps the live ROB to four rows.

- [ ] **Step 4: Add the typed untranslated policy**

Extend `O3DataAccessWindowPolicy`:

```rust
pub(crate) enum O3DataAccessWindowPolicy {
    None,
    ScalarMemoryPrefix,
    UntranslatedScalarMemoryPrefix,
    MemoryResultWindow,
}

impl O3DataAccessWindowPolicy {
    pub(crate) const fn is_scalar_memory_prefix(self) -> bool {
        matches!(
            self,
            Self::ScalarMemoryPrefix | Self::UntranslatedScalarMemoryPrefix
        )
    }
}
```

Use the predicate for shared memory-ordering rules. In `riscv_data_issue.rs`, choose `UntranslatedScalarMemoryPrefix` only when `eligible_scalar_load && state.data_translation.is_none()`; translated eligible loads retain `ScalarMemoryPrefix`. Update focused data-issue tests for both rows.

- [ ] **Step 5: Give the window classifier distinct bounds**

Add `O3_UNTRANSLATED_SCALAR_LIVE_WINDOW_ROWS = 8` and a `control_row_limit` field. Keep FU, translated scalar-memory, and memory-result constructors capped at four. Add:

```rust
pub(crate) fn from_untranslated_scalar_memory_prefix(
    load_destinations: impl IntoIterator<Item = Register>,
    occupied_rows: usize,
    row_limit: usize,
) -> Option<Self> {
    Self::from_scalar_memory_prefix_with_bounds(
        load_destinations,
        occupied_rows,
        row_limit.clamp(1, O3_UNTRANSLATED_SCALAR_LIVE_WINDOW_ROWS),
        O3_SCALAR_INTEGER_FU_LIVE_WINDOW_ROWS,
    )
}

pub(crate) const fn remaining_rows(&self) -> usize {
    self.row_limit.saturating_sub(self.rows)
}
```

The common constructor accepts validated row/control limits and does not reclamp them. Before classifying a control row, reject when `rows >= control_row_limit`; scalar rows may continue to total depth eight.

- [ ] **Step 6: Use separate memory and total-row authorities**

In `scalar_memory_window_candidate`, read both limits. Inspect the next fetch before applying the memory cap. If it is another load/store, reject before advancing its fetch identity when memory rows reached `scalar_memory_window_limit` or total rows reached `scalar_live_window_limit`. If it is scalar integer, transition with `from_untranslated_scalar_memory_prefix(..., scalar_live_window_limit)`.

In `data_access_integer_window`, map:

```rust
O3DataAccessWindowPolicy::ScalarMemoryPrefix =>
    RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
        destinations,
        occupied_rows,
        self.scalar_memory_window_limit,
    ),
O3DataAccessWindowPolicy::UntranslatedScalarMemoryPrefix =>
    RiscvScalarIntegerLiveWindow::from_untranslated_scalar_memory_prefix(
        destinations,
        occupied_rows,
        self.scalar_live_window_limit,
    ),
```

Translated loads and memory-result windows continue using their existing memory-depth-derived maximum of four. In `stage_o3_data_access_younger_window`, pass `window.remaining_rows()` to `completed_scalar_integer_younger_window` instead of recomputing `scalar_memory_window_limit().min(4) - 1`.

Keep `has_scalar_memory_window_capacity` based only on live memory operations. Add a separate live-ROB capacity predicate and require both when admitting another untranslated memory row.

- [ ] **Step 7: Add concrete focused policy tests**

Add exactly these classifier tests:

```text
untranslated_scalar_memory_window_accepts_eight_total_rows
translated_result_and_fu_windows_remain_capped_at_four
deep_scalar_row_cannot_open_a_fifth_row_control_chain
```

Use the file's existing concrete `addi(rd, rs1)` helper, which constructs `RiscvInstruction::Addi`, and `jal_with_destination(0)`. The first test admits seven scalar successors behind one untranslated load. The second proves translated and memory-result constructors expose only three remaining rows when passed `8`, and `from_fu_head(mul(...))` also exposes only three. The third builds a deep window at four rows, rejects `jal_with_destination(0)`, and accepts another `addi`. Preserve existing producer-forwarded/control tests unchanged.

```rust
#[test]
fn untranslated_scalar_memory_window_accepts_eight_total_rows() {
    let mut window = RiscvScalarIntegerLiveWindow::from_untranslated_scalar_memory_prefix(
        [Register::new(4).unwrap()],
        1,
        8,
    )
    .unwrap();
    for rd in 5..=11 {
        assert_eq!(
            window.classify_younger(addi(rd, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
    }
    assert_eq!(window.remaining_rows(), 0);
    assert!(window.is_full());
}

#[test]
fn translated_result_and_fu_windows_remain_capped_at_four() {
    let translated = RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
        [Register::new(4).unwrap()],
        1,
        8,
    )
    .unwrap();
    let result = RiscvScalarIntegerLiveWindow::from_memory_result(
        Some(Register::new(4).unwrap()),
        8,
    );
    let fu = RiscvScalarIntegerLiveWindow::from_fu_head(mul(4, 1, 2)).unwrap();
    assert_eq!(translated.remaining_rows(), 3);
    assert_eq!(result.remaining_rows(), 3);
    assert_eq!(fu.remaining_rows(), 3);
}

#[test]
fn deep_scalar_row_cannot_open_a_fifth_row_control_chain() {
    let mut window = RiscvScalarIntegerLiveWindow::from_untranslated_scalar_memory_prefix(
        [Register::new(4).unwrap()],
        1,
        8,
    )
    .unwrap();
    for rd in 5..=7 {
        assert_eq!(
            window.classify_younger(addi(rd, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
    }
    assert_eq!(
        window.classify_younger(jal_with_destination(0)),
        RiscvScalarIntegerYoungerDecision::Reject
    );
    assert_eq!(
        window.classify_younger(addi(8, 0)),
        RiscvScalarIntegerYoungerDecision::AdmitContinue
    );
}
```

In `o3_runtime_memory_window.rs`, add `scalar_memory_four_live_eight_admits_four_memory_plus_four_scalar_rows`: configure `(4,8)` and call the production `stage_live_data_access_issue(..., O3DataAccessWindowPolicy::UntranslatedScalarMemoryPrefix)` directly for four independent loads. Do not use `stage_live_data_access_issue_for_test`, because that compatibility helper intentionally selects the translated/four-row `ScalarMemoryPrefix` policy. Assert a fifth memory row is rejected before its fetch identity is consumed, then stage four supported scalar successors and assert exact eight-row ROB/four-row LSQ occupancy. Assert the typed scalar window reports four remaining rows before the scalar suffix.

```rust
#[test]
fn scalar_memory_four_live_eight_admits_four_memory_plus_four_scalar_rows() {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_window_depths(4, 8));
    let loads = [
        scalar_load_event(0x8000, 10, 12, 10, 0x9000),
        scalar_load_event(0x8004, 11, 13, 10, 0x9040),
        scalar_load_event(0x8008, 12, 14, 10, 0x9080),
        scalar_load_event(0x800c, 13, 15, 10, 0x90c0),
        scalar_load_event(0x8010, 14, 16, 10, 0x9100),
    ];
    for (index, load) in loads[..4].iter().enumerate() {
        assert!(runtime.stage_live_data_access_issue(
            load,
            memory_request(20 + index as u64),
            31 + index as u64,
            O3DataAccessWindowPolicy::UntranslatedScalarMemoryPrefix,
        ));
    }
    assert!(!runtime.stage_live_data_access_issue(
        &loads[4],
        memory_request(24),
        35,
        O3DataAccessWindowPolicy::UntranslatedScalarMemoryPrefix,
    ));
    let window = runtime
        .data_access_integer_window(loads[3].fetch().request_id())
        .unwrap();
    assert_eq!(window.remaining_rows(), 4);
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            loads[3].fetch().request_id(),
            [
                (Address::new(0x8020), addi(17, 0)),
                (Address::new(0x8024), addi(18, 0)),
                (Address::new(0x8028), addi(19, 0)),
                (Address::new(0x802c), addi(20, 0)),
            ],
        ),
        4
    );
    let snapshot = runtime.snapshot();
    assert_eq!(snapshot.reorder_buffer().len(), 8);
    assert_eq!(snapshot.load_store_queue().len(), 4);
    assert!(!snapshot
        .reorder_buffer()
        .iter()
        .any(|entry| entry.pc() == Address::new(0x8010)));
}
```

- [ ] **Step 8: Run GREEN verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu untranslated_scalar_memory_window -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu translated_result_and_fu_windows_remain_capped_at_four -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu deep_scalar_row_cannot_open_a_fifth_row_control_chain -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu scalar_memory_four_live_eight_admits_four_memory_plus_four_scalar_rows -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu result_younger_window -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run live_window_depth::rem6_run_o3_scalar_live_depth_eight_resides_with_one_lsq_row -- --nocapture
cargo fmt --all -- --check
```

Expected: focused tests pass; the CLI resident artifact has exactly eight ROB and one LSQ row.

- [ ] **Step 9: Run the per-task read-only review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/o3_runtime_memory.rs crates/rem6-cpu/src/o3_runtime_memory_window.rs crates/rem6-cpu/src/riscv_data_issue.rs crates/rem6-cpu/src/riscv_o3_window_policy.rs crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs crates/rem6-cpu/src/riscv_live_retire_window.rs crates/rem6-cpu/src/riscv_data_issue_tests/result_younger_window.rs crates/rem6/tests/cli_run/m5_host_actions/o3.rs crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth.rs crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth/fixture.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: add deep scalar o3 live windows"
git push origin main
```

### Task 4: Make Scoped Scheduling Reservation-Aware

**Files:**
- Modify: `crates/rem6-cpu/src/o3_pipeline.rs`
- Test: `crates/rem6-cpu/tests/o3_pipeline.rs`

- [ ] **Step 1: Add failing reservation tests**

Add three tests using the existing queue/capacity constructors:

```rust
#[test]
fn scoped_issue_full_reservation_still_classifies_dependency_state() {
    let queue = O3IssueQueueId::new(1);
    let scope = O3DependencyScopeId::new(10);
    let scheduler = O3ScopedIssueScheduler::new(
        2,
        [O3IssueQueueCapacity::new(queue, O3IssueOpClass::IntAlu, 2).unwrap()],
    )
    .unwrap();
    let resolved = O3ScopedReadyInstruction::new(1, queue, O3IssueOpClass::IntAlu);
    let unresolved = O3ScopedReadyInstruction::new(2, queue, O3IssueOpClass::IntAlu)
        .with_waits_on([scope]);
    let plan = scheduler
        .try_plan_with_reserved_width(2, [], [resolved.clone(), unresolved.clone()])
        .unwrap();
    assert_eq!(plan.issue_width(), 2);
    assert_eq!(plan.reserved_width(), 2);
    assert_eq!(plan.available_width(), 0);
    assert!(plan.issued().is_empty());
    assert_eq!(plan.resource_blocked(), &[resolved]);
    assert_eq!(plan.dependency_blocked(), &[unresolved]);
}

#[test]
fn scoped_issue_partial_reservation_limits_selected_rows() {
    let queue = O3IssueQueueId::new(1);
    let scheduler = O3ScopedIssueScheduler::new(
        4,
        [O3IssueQueueCapacity::new(queue, O3IssueOpClass::IntAlu, 4).unwrap()],
    )
    .unwrap();
    let ready = (1..=3)
        .map(|sequence| O3ScopedReadyInstruction::new(sequence, queue, O3IssueOpClass::IntAlu));
    let plan = scheduler.plan_with_reserved_width(2, [], ready);
    assert_eq!(plan.available_width(), 2);
    assert_eq!(plan.issued_sequences().collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(
        plan.resource_blocked().iter().map(|row| row.sequence()).collect::<Vec<_>>(),
        vec![3]
    );
}

#[test]
fn scoped_issue_rejects_reservation_above_configured_width() {
    let scheduler = O3ScopedIssueScheduler::new(
        2,
        std::iter::empty::<O3IssueQueueCapacity>(),
    )
    .unwrap();
    assert_eq!(
        scheduler.try_plan_with_reserved_width(
            3,
            std::iter::empty::<O3DependencyScopeId>(),
            std::iter::empty::<O3ScopedReadyInstruction>(),
        ),
        Err(O3PipelineError::ReservedIssueWidthExceedsConfigured {
            reserved_width: 3,
            issue_width: 2,
        })
    );
}
```

- [ ] **Step 2: Run RED verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test o3_pipeline scoped_issue_full_reservation -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test o3_pipeline scoped_issue_partial_reservation -- --nocapture
```

Expected: FAIL because reservation-aware methods and fields are absent.

- [ ] **Step 3: Add plan fields and compatibility delegation**

Add `reserved_width` to `O3ScopedIssuePlan`, plus:

```rust
pub const fn reserved_width(&self) -> usize {
    self.reserved_width
}

pub const fn available_width(&self) -> usize {
    self.issue_width - self.reserved_width
}
```

Keep `plan` and `try_plan` by delegating to `plan_with_reserved_width(0, ...)` and `try_plan_with_reserved_width(0, ...)`.

- [ ] **Step 4: Implement the complete reservation-aware planner**

Add `ReservedIssueWidthExceedsConfigured { reserved_width, issue_width }` to `O3PipelineError` with display text `O3 reserved issue width {reserved_width} exceeds configured width {issue_width}`. Implement the new fallible method with the current oldest-first behavior:

```rust
pub fn try_plan_with_reserved_width<R, I>(
    &self,
    reserved_width: usize,
    resolved_scopes: R,
    ready: I,
) -> Result<O3ScopedIssuePlan, O3PipelineError>
where
    R: IntoIterator<Item = O3DependencyScopeId>,
    I: IntoIterator<Item = O3ScopedReadyInstruction>,
{
    if reserved_width > self.issue_width {
        return Err(O3PipelineError::ReservedIssueWidthExceedsConfigured {
            reserved_width,
            issue_width: self.issue_width,
        });
    }
    let available_width = self.issue_width - reserved_width;
    let resolved_scopes = resolved_scopes.into_iter().collect::<BTreeSet<_>>();
    let mut pending = ready.into_iter().collect::<Vec<_>>();
    pending.sort_by_key(|instruction| instruction.sequence());
    validate_unique_dependency_producers(&pending)?;
    let mut remaining_capacity = self.capacities.clone();
    let mut issued = Vec::new();
    while issued.len() < available_width {
        let Some(index) = pending.iter().position(|instruction| {
            dependency_ready(&resolved_scopes, instruction)
                && scoped_issue_slots(&remaining_capacity, instruction) != 0
        }) else {
            break;
        };
        let instruction = pending.remove(index);
        if let Some(slots) =
            remaining_capacity.get_mut(&(instruction.queue(), instruction.op_class()))
        {
            *slots -= 1;
        }
        issued.push(instruction);
    }
    let mut resource_blocked = Vec::new();
    let mut dependency_blocked = Vec::new();
    for instruction in pending {
        if dependency_ready(&resolved_scopes, &instruction) {
            resource_blocked.push(instruction);
        } else {
            dependency_blocked.push(instruction);
        }
    }
    Ok(O3ScopedIssuePlan {
        issue_width: self.issue_width,
        reserved_width,
        issued,
        resource_blocked,
        dependency_blocked,
    })
}
```

Add the infallible wrapper calling this method and expecting valid reservations/producers.

- [ ] **Step 5: Run all scheduler tests**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test o3_pipeline -- --nocapture
cargo fmt --all -- --check
```

Expected: every existing zero-reservation test and all three new tests pass.

- [ ] **Step 6: Run the per-task read-only review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/o3_pipeline.rs crates/rem6-cpu/tests/o3_pipeline.rs
TMPDIR=$PWD/target/tmp git commit -m "feat: plan scoped issue reservations"
git push origin main
```

### Task 5: Centralize Live Dependency Scheduling

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue/dependency.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_error.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue_tests/dependency_scopes.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`
- Test: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`
- Test: `crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth.rs`

- [ ] **Step 1: Declare focused production and test children**

```rust
// o3_runtime_issue.rs
#[path = "o3_runtime_issue/dependency.rs"]
mod dependency;
pub(crate) use dependency::O3LiveIssueDependencyTable;

// o3_runtime_issue_tests.rs
#[path = "o3_runtime_issue_tests/dependency_scopes.rs"]
mod dependency_scopes;
```

Update the `o3_runtime.rs` imports so descendants can use `O3LiveIssueSchedulingCandidate` and `O3LiveIssueSourceProducer`, and extend the existing `pub(crate) use o3_runtime_issue::{...}` with `O3LiveIssueDependencyTable` and `O3PreparedLiveIssue`.

- [ ] **Step 2: Add failing tests for the ownership change**

In `dependency_scopes.rs`, add exactly these tests using the existing staged ROB/request helpers:

```text
scheduling_metadata_exists_before_forwarded_values
dependency_table_keeps_data_and_control_release_ticks_distinct
dependency_table_encodes_two_source_fan_in
selected_issue_batch_failure_records_no_partial_state
```

They prove four behaviors:

1. A dependent row has a scheduling candidate before its producer value is forwardable, while executable-candidate construction returns `None`.
2. A candidate waiting on both data and control from one producer receives two distinct scope IDs; data resolves at admitted writeback and control at admitted writeback plus one.
3. The row-five `ADD x9,x6,x7` candidate contains two source producers and two wait scopes.
4. Recording a two-row selected batch where the second row has a mismatched consumed-request identity returns `SelectedIssueCandidateNotExecutable` and leaves the complete `O3RuntimeState` equal to its pre-call clone.

Use these assertion shapes:

```rust
assert_eq!(candidate.data_producers().len(), 2);
assert_ne!(scoped.waits_on()[0], scoped.waits_on()[1]);
assert_eq!(table.resolved_scopes_at(admitted).len(), 1);
assert_eq!(table.resolved_scopes_at(admitted + 1).len(), 2);
assert_eq!(runtime, before);
```

Extend `ScalarIssueCase` with `FanIn` using `[mul(14, 2, 3), mul(15, 4, 5), add(16, 14, 15)]`. Add:

```rust
fn add(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
    RiscvInstruction::Add {
        rd: reg(rd),
        rs1: reg(rs1),
        rs2: reg(rs2),
    }
}
```

and encode `RiscvInstruction::Add` with `r_type(0, rs2.index(), rs1.index(), 0, rd.index(), 0x33)`. Add `O3LiveIssueRequest` accessors for `pc`, `decoded`, and `consumed_requests`. Use these complete child helpers/tests:

```rust
pub(crate) const fn pc(&self) -> Address {
    self.pc
}

pub(crate) const fn decoded(&self) -> RiscvDecodedInstruction {
    self.decoded
}

pub(crate) const fn instruction(&self) -> RiscvInstruction {
    self.decoded.instruction()
}

pub(crate) fn consumed_requests(&self) -> &[MemoryRequestId] {
    &self.consumed_requests
}
```

```rust
use super::*;

fn scheduling_candidate(
    fixture: &ScalarIssueFixture,
    request_index: usize,
) -> O3LiveIssueSchedulingCandidate {
    let request = &fixture.requests[request_index];
    fixture
        .runtime
        .live_issue_scheduling_candidate(
            request_index,
            request.pc(),
            request.instruction(),
            request.consumed_requests(),
        )
        .unwrap()
}

fn sequence_at(fixture: &ScalarIssueFixture, pc: u64) -> u64 {
    fixture
        .runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(pc))
        .unwrap()
        .sequence()
}

fn prepared_issue(
    fixture: &ScalarIssueFixture,
    request_index: usize,
    issue_tick: u64,
) -> O3PreparedLiveIssue {
    let request = &fixture.requests[request_index];
    let scheduling = scheduling_candidate(fixture, request_index);
    let candidate = fixture
        .runtime
        .materialize_live_speculative_issue_candidate(&scheduling)
        .unwrap();
    let mut hart = fixture.hart.clone();
    for write in candidate.forwarded_register_writes() {
        hart.write(write.register(), write.value());
    }
    hart.set_pc(request.pc().get());
    let execution = hart.execute_decoded(request.decoded()).unwrap();
    O3PreparedLiveIssue {
        candidate,
        consumed_requests: request.consumed_requests().to_vec(),
        issue_tick,
        execution,
    }
}

#[test]
fn scheduling_metadata_exists_before_forwarded_values() {
    let fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);
    let candidate = scheduling_candidate(&fixture, 2);
    assert_eq!(candidate.data_producers().len(), 1);
    assert_eq!(
        candidate.data_producers()[0].sequence(),
        sequence_at(&fixture, SECOND_PC)
    );
    assert!(fixture
        .runtime
        .materialize_live_speculative_issue_candidate(&candidate)
        .is_none());
}

#[test]
fn dependency_table_keeps_data_and_control_release_ticks_distinct() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::SameWindowLinkReturn);
    fixture
        .runtime
        .schedule_live_speculative_issues(
            &fixture.hart,
            fixture.head,
            20,
            &fixture.requests[..1],
        )
        .unwrap();
    let candidate = scheduling_candidate(&fixture, 1);
    let table = O3LiveIssueDependencyTable::new(
        &fixture.runtime,
        std::slice::from_ref(&candidate),
    )
    .unwrap();
    let scoped = table.scoped_instruction(&candidate);
    let admitted = fixture.execution_at(BRANCH_PC).admitted_writeback_tick;
    assert_eq!(scoped.waits_on().len(), 2);
    assert_eq!(table.resolved_scopes_at(admitted).len(), 1);
    assert_eq!(table.resolved_scopes_at(admitted + 1).len(), 2);
}

#[test]
fn dependency_table_encodes_two_source_fan_in() {
    let fixture = ScalarIssueFixture::new(4, ScalarIssueCase::FanIn);
    let candidate = scheduling_candidate(&fixture, 2);
    let table = O3LiveIssueDependencyTable::new(
        &fixture.runtime,
        std::slice::from_ref(&candidate),
    )
    .unwrap();
    let scoped = table.scoped_instruction(&candidate);
    assert_eq!(
        candidate
            .data_producers()
            .iter()
            .map(|producer| producer.sequence())
            .collect::<Vec<_>>(),
        vec![sequence_at(&fixture, BRANCH_PC), sequence_at(&fixture, SECOND_PC)]
    );
    assert_eq!(scoped.waits_on().len(), 2);
    assert_ne!(scoped.waits_on()[0], scoped.waits_on()[1]);
}

#[test]
fn selected_issue_batch_failure_records_no_partial_state() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::CrossResource);
    let mut prepared = vec![prepared_issue(&fixture, 0, 20), prepared_issue(&fixture, 1, 20)];
    prepared[1].consumed_requests.push(request(999));
    let before = fixture.runtime.clone();
    assert!(matches!(
        fixture.runtime.record_live_issue_batch(prepared),
        Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { .. })
    ));
    assert_eq!(fixture.runtime, before);
}
```

- [ ] **Step 3: Run RED verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependency_scopes -- --nocapture
```

Expected: FAIL because metadata/executable candidates and the dependency table are not split.

- [ ] **Step 4: Split metadata from executable forwarding**

Replace the combined candidate in `o3_runtime_control_window.rs` with:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueSourceProducer {
    sequence: u64,
    source: Register,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueSchedulingCandidate {
    request_index: usize,
    sequence: u64,
    pc: Address,
    instruction: RiscvInstruction,
    consumed_requests: Vec<MemoryRequestId>,
    kind: O3LiveSpeculativeIssueKind,
    op_class: O3IssueOpClass,
    control_dependency: Option<u64>,
    data_producers: Vec<O3LiveIssueSourceProducer>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveSpeculativeIssueCandidate {
    scheduling: O3LiveIssueSchedulingCandidate,
    producer_sequences: Vec<u64>,
    forwarded_register_writes: Vec<RegisterWrite>,
}
```

Add these accessors:

```rust
impl O3LiveIssueSourceProducer {
    pub(crate) const fn sequence(self) -> u64 { self.sequence }
    pub(crate) const fn source(self) -> Register { self.source }
}

impl O3LiveIssueSchedulingCandidate {
    pub(crate) const fn request_index(&self) -> usize { self.request_index }
    pub(crate) const fn sequence(&self) -> u64 { self.sequence }
    pub(crate) const fn instruction(&self) -> RiscvInstruction { self.instruction }
    pub(crate) const fn op_class(&self) -> O3IssueOpClass { self.op_class }
    pub(crate) const fn control_dependency(&self) -> Option<u64> {
        self.control_dependency
    }
    pub(crate) fn data_producers(&self) -> &[O3LiveIssueSourceProducer] {
        &self.data_producers
    }
    pub(crate) fn consumed_requests(&self) -> &[MemoryRequestId] {
        &self.consumed_requests
    }
}

impl O3LiveSpeculativeIssueCandidate {
    pub(crate) const fn sequence(&self) -> u64 { self.scheduling.sequence }
    pub(crate) const fn request_index(&self) -> usize {
        self.scheduling.request_index
    }
    pub(crate) const fn instruction(&self) -> RiscvInstruction {
        self.scheduling.instruction
    }
    pub(crate) fn forwarded_register_writes(&self) -> &[RegisterWrite] {
        &self.forwarded_register_writes
    }
}
```

Rename the metadata constructor to:

```rust
pub(crate) fn live_issue_scheduling_candidate(
    &self,
    request_index: usize,
    pc: Address,
    instruction: RiscvInstruction,
    consumed_requests: &[MemoryRequestId],
) -> Option<O3LiveIssueSchedulingCandidate>
```

Keep current ROB/fetch/destination validation and copy the exact consumed-request identity into the candidate. Replace value-dependent source forwarding with a producer scan over older live-staged ROB rename destinations. Add:

```rust
pub(crate) fn materialize_live_speculative_issue_candidate(
    &self,
    scheduling: &O3LiveIssueSchedulingCandidate,
) -> Option<O3LiveSpeculativeIssueCandidate>
```

This second method resolves each exact `(producer sequence, source register)` from an issued speculative execution or completed live load, builds forwarded writes, and returns `None` until every source is available. Remove `ready_tick` from the candidate and make `record_live_speculative_execution` use the scheduler-selected issue tick directly.

Keep the existing two-argument API for current focused tests and non-scheduler callers:

```rust
pub(crate) fn live_speculative_issue_candidate(
    &self,
    pc: Address,
    instruction: RiscvInstruction,
) -> Option<O3LiveSpeculativeIssueCandidate> {
    let scheduling = self.live_issue_scheduling_candidate_from_metadata(
        usize::MAX,
        pc,
        instruction,
        Vec::new(),
    )?;
    self.materialize_live_speculative_issue_candidate(&scheduling)
}
```

`live_issue_scheduling_candidate` validates and copies the production request's exact consumed identity, then delegates to the same private `live_issue_scheduling_candidate_from_metadata`. The compatibility wrapper intentionally carries an empty request vector because its existing callers only inspect candidate shape/forwarding and continue passing exact consumed requests to recording separately. Do not migrate or break existing calls in `o3_runtime_live_window_tests.rs`, `o3_runtime_control_window_tests.rs`, or `riscv_fetch_ahead/tests/detailed_o3_control.rs`.

- [ ] **Step 5: Implement typed ephemeral dependency scopes**

Create `o3_runtime_issue/dependency.rs`:

```rust
use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum O3LiveIssueDependencyKey {
    Data(u64),
    Control(u64),
}

pub(crate) struct O3LiveIssueDependencyTable {
    scopes: BTreeMap<O3LiveIssueDependencyKey, O3DependencyScopeId>,
    ready_ticks: BTreeMap<O3DependencyScopeId, u64>,
}
```

`new(runtime, candidates)` collects own Data/Control keys and all waited-on keys into a `BTreeSet`, assigns IDs `1..` in sorted-key order, and fills ready ticks as follows:

```rust
fn data_ready_tick(runtime: &O3RuntimeState, sequence: u64) -> Option<u64> {
    runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == sequence)
        .map(|row| row.admitted_writeback_tick)
        .or_else(|| runtime.completed_live_data_access_ready_tick(sequence))
}

fn control_ready_tick(
    runtime: &O3RuntimeState,
    sequence: u64,
) -> Result<Option<u64>, O3RuntimeError> {
    runtime
        .live_speculative_executions
        .iter()
        .find(|row| row.sequence == sequence)
        .map(|row| {
            row.admitted_writeback_tick
                .checked_add(1)
                .ok_or(O3RuntimeError::WritebackTickOverflow {
                    tick: row.admitted_writeback_tick,
                })
        })
        .transpose()
}
```

Add `completed_live_data_access_ready_tick(sequence)` beside the existing completed-source forwarding code, deriving the admitted memory-result writeback tick without duplicating response parsing.

Expose:

```rust
pub(crate) fn scoped_instruction(
    &self,
    candidate: &O3LiveIssueSchedulingCandidate,
) -> O3ScopedReadyInstruction;

pub(crate) fn resolved_scopes_at(&self, tick: u64) -> BTreeSet<O3DependencyScopeId>;

pub(crate) fn earliest_resolution_after<'a>(
    &self,
    tick: u64,
    blocked: impl IntoIterator<Item = &'a O3ScopedReadyInstruction>,
) -> Option<u64>;
```

`scoped_instruction` puts the candidate's own Data and Control IDs in `produces`; source Data IDs and optional lineage Control ID go in `waits_on`. The earliest-tick method considers only blocked rows' unresolved wait scopes and returns the minimum known tick greater than `tick`.

- [ ] **Step 6: Make scheduler output the sole classifier**

Change `live_issue_candidates` to return every stable scheduling candidate, including unresolved rows. Per loop, build one dependency table and call:

```rust
let scheduler = O3ScopedIssueScheduler::new(
    self.issue_width,
    live_issue_capacities_after_reservations(self.issue_width, reservations),
)
.expect("configured live O3 issue width is nonzero");
let plan = scheduler.try_plan_with_reserved_width(
    reservations.width,
    dependency_table.resolved_scopes_at(tick),
    candidates
        .iter()
        .map(|candidate| dependency_table.scoped_instruction(candidate)),
)
.map_err(|error| O3RuntimeError::InvalidLiveIssuePlan { error })?;
```

Add `InvalidLiveIssuePlan { error: O3PipelineError }` to `O3RuntimeError` with display text `O3 live issue plan is invalid: {error}`. Keep `InvalidPendingState` checkpoint-specific.

The focused capacity helper returns nonzero capacities for IntAlu (`issue_width - reservations.int_alu`), IntMult (`1 - reservations.int_mult`), and Branch (`1 - reservations.branch`). Delete the manual ready/blocked partition, `O3LiveIssueDependencyReadiness`, `earliest_dependency_tick`, `live_issue_dependencies_ready_at`, and `live_issue_dependency_readiness`.

Record stats directly:

```rust
tick_decision.observe(
    plan.issued().len(),
    plan.resource_blocked().len(),
    plan.dependency_blocked().len(),
    plan.reserved_width().saturating_add(plan.issued().len()),
);
```

Call `tick_decision.observe` only after `record_live_issue_batch` succeeds when `plan.issued()` is nonempty. An executable-batch error returns before stats mutation; an empty issued set may record the scheduler's blocked classifications immediately.

Advance one tick when resource-blocked rows exist. If all remaining rows are dependency-blocked, jump to `earliest_resolution_after`; stop when no known tick exists. Never resolve a just-issued producer in the same cycle.

- [ ] **Step 7: Record the selected batch atomically**

Add:

```rust
SelectedIssueCandidateNotExecutable { sequence: u64 },
```

with display text `O3 selected issue candidate {sequence} could not be materialized`.

Prepare all selected candidates and cloned-hart execution records before touching runtime state. Record them through a clone transaction:

```rust
#[derive(Clone)]
pub(crate) struct O3PreparedLiveIssue {
    pub(crate) candidate: O3LiveSpeculativeIssueCandidate,
    pub(crate) consumed_requests: Vec<MemoryRequestId>,
    pub(crate) issue_tick: u64,
    pub(crate) execution: RiscvExecutionRecord,
}

pub(crate) fn record_live_issue_batch(
    &mut self,
    prepared: Vec<O3PreparedLiveIssue>,
) -> Result<(), O3RuntimeError> {
    let mut staged = self.clone();
    for row in prepared {
        let sequence = row.candidate.sequence();
        if !staged.record_live_speculative_execution(
            row.candidate,
            &row.consumed_requests,
            row.issue_tick,
            row.execution,
        )? {
            return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence });
        }
    }
    *self = staged;
    Ok(())
}
```

This must be the only mutation path for a scheduler-selected batch.

- [ ] **Step 8: Run focused and top-level verification**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu dependency_scopes -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_issue_tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test o3_pipeline -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run scoped_issue -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run live_window_depth -- --nocapture
test "$(wc -l < crates/rem6-cpu/src/o3_runtime_issue.rs)" -le 800
cargo fmt --all -- --check
```

Expected: existing scoped-issue timing/stats remain green, dependency tests pass, and the issue owner remains within 800 lines.

- [ ] **Step 9: Run the per-task read-only review gate, then commit and push**

```bash
git add crates/rem6-cpu/src/o3_runtime.rs crates/rem6-cpu/src/o3_runtime_issue.rs crates/rem6-cpu/src/o3_runtime_issue/dependency.rs crates/rem6-cpu/src/o3_runtime_control_window.rs crates/rem6-cpu/src/o3_runtime_error.rs crates/rem6-cpu/src/o3_runtime_issue_tests.rs crates/rem6-cpu/src/o3_runtime_issue_tests/dependency_scopes.rs
TMPDIR=$PWD/target/tmp git commit -m "refactor: centralize live o3 dependency scheduling"
git push origin main
```

### Task 6: Complete the CLI Matrix and Lifecycle Boundaries

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth/fixture.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth/lifecycle.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth/memory_boundary.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_writeback_tests.rs`

- [ ] **Step 1: Add the representative matrix table**

Declare `lifecycle` and `memory_boundary` children from `live_window_depth.rs`. Add:

```rust
struct LiveWindowMatrixRow {
    name: &'static str,
    live_depth: usize,
    issue_width: usize,
    memory_system: &'static str,
    expected_resident_rows: usize,
    expected_issued_rows: u64,
    expected_max_rows_per_cycle: u64,
}

const LIVE_WINDOW_MATRIX: [LiveWindowMatrixRow; 5] = [
    LiveWindowMatrixRow {
        name: "depth-four-width-four-direct",
        live_depth: 4,
        issue_width: 4,
        memory_system: "direct",
        expected_resident_rows: 4,
        expected_issued_rows: 3,
        expected_max_rows_per_cycle: 2,
    },
    LiveWindowMatrixRow {
        name: "depth-six-width-two-direct",
        live_depth: 6,
        issue_width: 2,
        memory_system: "direct",
        expected_resident_rows: 6,
        expected_issued_rows: 5,
        expected_max_rows_per_cycle: 2,
    },
    LiveWindowMatrixRow {
        name: "depth-six-width-two-hierarchy",
        live_depth: 6,
        issue_width: 2,
        memory_system: "cache-fabric-dram",
        expected_resident_rows: 6,
        expected_issued_rows: 5,
        expected_max_rows_per_cycle: 2,
    },
    LiveWindowMatrixRow {
        name: "depth-eight-width-one-direct",
        live_depth: 8,
        issue_width: 1,
        memory_system: "direct",
        expected_resident_rows: 8,
        expected_issued_rows: 7,
        expected_max_rows_per_cycle: 1,
    },
    LiveWindowMatrixRow {
        name: "depth-eight-width-four-hierarchy",
        live_depth: 8,
        issue_width: 4,
        memory_system: "cache-fabric-dram",
        expected_resident_rows: 8,
        expected_issued_rows: 7,
        expected_max_rows_per_cycle: 3,
    },
];
```

The paired depth-six direct/hierarchy rows prove route behavior is orthogonal to live-depth scaling. The width-four maximum is exactly three: the load head reserves one global slot, one MUL row uses the sole IntMult slot, and the independent row-six ADDI uses an IntAlu slot while the second MUL and dependency chain remain blocked.

Add these local helpers:

```rust
const ISSUE_STATS: [(&str, &str, &str); 5] = [
    ("cycles", "issue_cycles", "Cycle"),
    ("issued_rows", "issued_rows", "Count"),
    ("resource_blocked_row_cycles", "resource_blocked_row_cycles", "Cycle"),
    ("dependency_blocked_row_cycles", "dependency_blocked_row_cycles", "Cycle"),
    ("max_rows_per_cycle", "max_rows_per_cycle", "Count"),
];

fn issue_artifact(json: &Value) -> &Value {
    json.pointer("/cores/0/o3_runtime/issue")
        .unwrap_or_else(|| panic!("missing deep scalar issue artifact: {json}"))
}

fn issue_u64(issue: &Value, field: &str) -> u64 {
    issue
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("deep scalar issue artifact lacks {field}: {issue}"))
}

fn assert_issue_native_stats(json: &Value, issue: &Value) {
    for (json_field, stat_field, unit) in ISSUE_STATS {
        assert_json_stat(
            json,
            &format!("sim.cpu0.o3.{stat_field}"),
            unit,
            issue_u64(issue, json_field),
            "monotonic",
        );
    }
}

fn assert_route_activity(json: &Value, memory_system: &str) {
    assert!(
        json.pointer("/memory_resources/transport/data/activity")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0)
    );
    assert_json_stat_at_least(
        json,
        "sim.memory.resources.transport.data.activity",
        "Count",
        1,
        "monotonic",
    );
    for (pointer, path) in [
        ("/memory_resources/cache/data/activity", "sim.memory.resources.cache.data.activity"),
        ("/memory_resources/fabric/activity", "sim.memory.resources.fabric.activity"),
        ("/memory_resources/dram/activity", "sim.memory.resources.dram.activity"),
    ] {
        if memory_system == "direct" {
            assert_eq!(json.pointer(pointer).and_then(Value::as_u64).unwrap_or(0), 0);
            assert_json_stat(json, path, "Count", 0, "monotonic");
        } else {
            assert!(json.pointer(pointer).and_then(Value::as_u64).is_some_and(|value| value > 0));
            assert_json_stat_at_least(json, path, "Count", 1, "monotonic");
        }
    }
}
```

- [ ] **Step 2: Add resident and completed matrix assertions**

Add `rem6_run_o3_deep_scalar_window_matrix`. For each row, run to completion, obtain the delayed load response tick, then rerun at `response_tick - 1`. Assert the resident ROB contains exactly the prefix of `[LOAD_PC, ROW_PCS...]` of `expected_resident_rows`, LSQ count is one, and `max_rob_occupancy`/`max_lsq_occupancy` match.

On the completed artifact assert:

```rust
assert_final_witness(
    &completed,
    FINAL_MEMORY,
    [
        ("x5", "0x9"),
        ("x6", "0x6"),
        ("x7", "0x14"),
        ("x8", "0x7"),
        ("x9", "0x1a"),
        ("x14", "0x8"),
        ("x16", "0x21"),
        ("x17", "0x2a"),
    ],
);
```

Assert row-two and row-three MUL issue ticks differ, row-four waits for row-two writeback, row-five waits for both MUL writebacks, row-seven waits for rows four/five, and row-eight waits for both row-seven and the load writeback. For depth four, assert row five is absent from the resident ROB and its eventual issue tick is not before the load response; for depth six, make the same assertion for row seven.

Read `/cores/0/o3_runtime/issue` and assert exact `issued_rows`, positive resource- and dependency-blocked row cycles, and exact `max_rows_per_cycle` from the table. Also assert the flat native stats mirror the structured fields.

Use this complete body:

```rust
#[test]
fn rem6_run_o3_deep_scalar_window_matrix() {
    for row in LIVE_WINDOW_MATRIX {
        let path = scalar_live_window_binary(row.name, false);
        let completed = scalar_live_window_json(
            &path,
            row.memory_system,
            row.live_depth,
            row.issue_width,
            2_000,
        );
        assert_final_witness(
            &completed,
            FINAL_MEMORY,
            [
                ("x5", "0x9"),
                ("x6", "0x6"),
                ("x7", "0x14"),
                ("x8", "0x7"),
                ("x9", "0x1a"),
                ("x14", "0x8"),
                ("x16", "0x21"),
                ("x17", "0x2a"),
            ],
        );
        let load = event_at_pc(&completed, LOAD_PC);
        let response_tick = event_u64(load, "lsq_data_response_tick");
        let resident = scalar_live_window_json(
            &path,
            row.memory_system,
            row.live_depth,
            row.issue_width,
            response_tick - 1,
        );
        let rob = resident
            .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
            .and_then(Value::as_array)
            .unwrap();
        let expected_pcs = std::iter::once(LOAD_PC)
            .chain(ROW_PCS)
            .take(row.expected_resident_rows)
            .collect::<Vec<_>>();
        assert_eq!(
            rob.iter()
                .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
                .collect::<Vec<_>>(),
            expected_pcs
        );
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_json_stat(
            &resident,
            "sim.cpu0.o3.max_rob_occupancy",
            "Count",
            row.expected_resident_rows as u64,
            "monotonic",
        );
        assert_json_stat(&resident, "sim.cpu0.o3.max_lsq_occupancy", "Count", 1, "monotonic");

        let row2 = event_at_pc(&completed, ROW_PCS[0]);
        let row3 = event_at_pc(&completed, ROW_PCS[1]);
        let row4 = event_at_pc(&completed, ROW_PCS[2]);
        let row5 = event_at_pc(&completed, ROW_PCS[3]);
        let row7 = event_at_pc(&completed, ROW_PCS[5]);
        let row8 = event_at_pc(&completed, ROW_PCS[6]);
        assert_ne!(event_u64(row2, "issue_tick"), event_u64(row3, "issue_tick"));
        assert!(event_u64(row4, "issue_tick") >= event_u64(row2, "writeback_tick"));
        assert!(event_u64(row5, "issue_tick") >= event_u64(row2, "writeback_tick"));
        assert!(event_u64(row5, "issue_tick") >= event_u64(row3, "writeback_tick"));
        assert!(event_u64(row7, "issue_tick") >= event_u64(row4, "writeback_tick"));
        assert!(event_u64(row7, "issue_tick") >= event_u64(row5, "writeback_tick"));
        assert!(event_u64(row8, "issue_tick") >= event_u64(row7, "writeback_tick"));
        assert!(event_u64(row8, "issue_tick") >= event_u64(load, "writeback_tick"));
        if row.live_depth == 4 {
            assert!(event_u64(row5, "issue_tick") >= response_tick);
        }
        if row.live_depth == 6 {
            assert!(event_u64(row7, "issue_tick") >= response_tick);
        }
        let issue = issue_artifact(&completed);
        assert_eq!(issue_u64(issue, "issued_rows"), row.expected_issued_rows);
        assert!(issue_u64(issue, "resource_blocked_row_cycles") > 0);
        assert!(issue_u64(issue, "dependency_blocked_row_cycles") > 0);
        assert_eq!(
            issue_u64(issue, "max_rows_per_cycle"),
            row.expected_max_rows_per_cycle
        );
        assert_issue_native_stats(&completed, issue);
        assert_route_activity(&completed, row.memory_system);
    }
}
```

- [ ] **Step 3: Lock route activity**

For `direct`, assert transport data activity is positive and cache/fabric/DRAM activity is zero in both structured resources and `sim.memory.resources.*` stats. For `cache-fabric-dram`, assert cache data, transport data, fabric, and DRAM activity are all positive. Every row must retain exactly one LSQ entry at peak.

- [ ] **Step 4: Add text and m5 dump stats evidence**

Add `rem6_run_o3_deep_scalar_window_text_stats`. Run the depth-eight/width-one direct row with `stats-format text` and assert exact values for:

```text
sim.cpu0.o3.issue_cycles
sim.cpu0.o3.issued_rows
sim.cpu0.o3.resource_blocked_row_cycles
sim.cpu0.o3.dependency_blocked_row_cycles
sim.cpu0.o3.max_rows_per_cycle
```

Add `rem6_run_o3_deep_scalar_window_dump_stats`. Run the dump-enabled fixture at depth eight/width four and assert one real `m5_dump_stats` action whose `sim.host_actions.stats_dump.cpu0.o3.*` issue counters equal the final pre-dump runtime counters.

```rust
#[test]
fn rem6_run_o3_deep_scalar_window_text_stats() {
    let path = scalar_live_window_binary("o3-deep-scalar-text", false);
    let json = scalar_live_window_json(&path, "direct", 8, 1, 2_000);
    let issue = issue_artifact(&json);
    let output = scalar_live_window_command(
        &path,
        "direct",
        8,
        1,
        2_000,
        "detailed",
        "text",
    )
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    for (json_field, stat_field, unit) in ISSUE_STATS {
        let path = format!("sim.cpu0.o3.{stat_field}");
        let value = issue_u64(issue, json_field);
        match unit {
            "Cycle" => assert_text_cycle_stat(&stdout, &path, value),
            "Count" => assert_text_count_stat(&stdout, &path, value),
            _ => unreachable!(),
        }
        assert_text_stat_occurs_once(&stdout, &path);
    }
}

#[test]
fn rem6_run_o3_deep_scalar_window_dump_stats() {
    let path = scalar_live_window_binary("o3-deep-scalar-dump", true);
    let json = scalar_live_window_json(&path, "direct", 8, 4, 2_000);
    assert_eq!(
        json.pointer("/host_actions/stats_dump_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let dump = json
        .pointer("/host_actions/stats_dumps/0")
        .unwrap_or_else(|| panic!("missing deep scalar stats dump: {json}"));
    let issue = issue_artifact(&json);
    for (json_field, stat_field, unit) in ISSUE_STATS {
        assert_stats_dump_sample(
            dump,
            &format!("sim.host_actions.stats_dump.cpu0.o3.{stat_field}"),
            "counter",
            unit,
            issue_u64(issue, json_field),
            "resettable",
        );
    }
}
```

- [ ] **Step 5: Add the five-memory-row negative boundary**

In `memory_boundary.rs`, add `rem6_run_o3_scalar_live_depth_eight_rejects_fifth_memory_row`. Build a dedicated program with one pointer setup followed by five independent `LD` instructions from offsets `0, 8, 16, 24, 32`, then a scalar `ADDI`, store the five loaded values, and exit. Run with memory depth `4`, live depth `8`, detailed mode, and route delay `80`.

At one tick before the first load response assert exactly four resident ROB rows and four LSQ rows, with the fifth load PC absent. On completion assert all five loaded values are correct and the fifth load's issue tick is at or after the oldest response. This test proves live depth eight does not authorize memory row five.

Use this complete test module body, reusing `r_type` only if the fixture later needs a register-register instruction:

```rust
use super::*;

const LOAD_PCS: [&str; 5] = [
    "0x80000008",
    "0x8000000c",
    "0x80000010",
    "0x80000014",
    "0x80000018",
];
const FIVE_LOAD_MEMORY: &str = concat!(
    "0100000000000000020000000000000003000000000000000400000000000000",
    "0500000000000000010000000000000002000000000000000300000000000000",
    "04000000000000000500000000000000"
);

fn five_load_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![
        u_type(0, 10, 0x17),
        i_type(128, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b011, 5, 0x03),
        i_type(8, 10, 0b011, 6, 0x03),
        i_type(16, 10, 0b011, 7, 0x03),
        i_type(24, 10, 0b011, 8, 0x03),
        i_type(32, 10, 0b011, 9, 0x03),
        i_type(1, 9, 0x0, 11, 0x13),
        s_type(40, 5, 10, 0b011),
        s_type(48, 6, 10, 0b011),
        s_type(56, 7, 10, 0b011),
        s_type(64, 8, 10, 0b011),
        s_type(72, 9, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    while words.len() * 4 < 128 {
        words.push(0);
    }
    words.extend([1, 0, 2, 0, 3, 0, 4, 0, 5, 0]);
    words.extend([0; 10]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn five_load_json(path: &Path, max_tick: u64) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
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
            "O3,Data,Fetch,Memory",
            "--riscv-execution-mode",
            "detailed",
            "--riscv-o3-scalar-memory-depth",
            "4",
            "--riscv-o3-scalar-live-window-depth",
            "8",
            "--memory-system",
            "direct",
            "--memory-route-delay",
            "80",
            "--dump-memory",
            "0x80000080:80",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn rem6_run_o3_scalar_live_depth_eight_rejects_fifth_memory_row() {
    let path = five_load_binary("o3-live-eight-five-load-boundary");
    let completed = five_load_json(&path, 3_000);
    assert_final_witness(
        &completed,
        FIVE_LOAD_MEMORY,
        [
            ("x5", "0x1"),
            ("x6", "0x2"),
            ("x7", "0x3"),
            ("x8", "0x4"),
            ("x9", "0x5"),
            ("x11", "0x6"),
        ],
    );
    let first_response = LOAD_PCS[..4]
        .iter()
        .map(|pc| event_u64(event_at_pc(&completed, pc), "lsq_data_response_tick"))
        .min()
        .unwrap();
    let resident = five_load_json(&path, first_response - 1);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(rob.len(), 4);
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert!(rob.iter().all(|entry| {
        entry.pointer("/pc").and_then(Value::as_str) != Some(LOAD_PCS[4])
    }));
    assert!(event_u64(event_at_pc(&completed, LOAD_PCS[4]), "issue_tick") >= first_response);
}
```

- [ ] **Step 6: Add top-level checkpoint and mode-switch boundaries**

In `lifecycle.rs`, use a depth-eight/width-two direct baseline. Add:

```text
rem6_run_o3_deep_scalar_window_rejects_live_checkpoint_and_restores_drained
rem6_run_host_switch_preserves_deep_scalar_window_timing
rem6_run_timing_suppresses_deep_scalar_window_surfaces
```

First extend the fixture command helper with a `mode: &str` parameter used as the value of `--riscv-execution-mode`; keep the existing detailed helper as a wrapper passing `"detailed"`, and add a timing wrapper passing `"timing"`.

The live checkpoint at `load_response_tick - 1` must fail with `checkpoint component is not quiescent: cpu0`. A checkpoint after load commit and restore one tick later must succeed, preserve issue stats, and expose zero restored ROB/LSQ entries.

Request detailed-to-timing switch after the first younger issue and before load response. Assert the transfer runtime chunk decodes, captures eight ROB/one LSQ rows, the live-data handoff reports seven younger rows, and all issue/writeback/commit ticks for load plus seven rows match the unswitched baseline. The switched run must complete with the exact register/memory witness.

Run the same binary from timing mode and assert identical architecture plus absence of `/cores/0/o3_runtime`, `/debug/o3_trace`, `sim.cpu0.o3.*`, and gem5-style O3 aliases.

Use these complete fixture extensions and lifecycle tests:

```rust
// fixture.rs
pub(super) fn scalar_live_window_json_with_mode_and_args(
    path: &Path,
    memory_system: &str,
    live_depth: usize,
    issue_width: usize,
    max_tick: u64,
    mode: &str,
    extra_args: &[&str],
) -> Value {
    let mut command = scalar_live_window_command(
        path,
        memory_system,
        live_depth,
        issue_width,
        max_tick,
        mode,
        "json",
    );
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}
```

The final signatures are `scalar_live_window_command(..., mode, stats_format)` and `scalar_live_window_json_with_mode_and_args(..., mode, extra_args)`, while `scalar_live_window_json` delegates with `"detailed"` and an empty argument slice.

```rust
// lifecycle.rs
use super::*;

fn component_chunk<'a>(
    action: &'a Value,
    component: &str,
    chunk_name: &str,
    payload: &str,
) -> &'a Value {
    action
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .and_then(|component| component.pointer("/chunks"))
        .and_then(Value::as_array)
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some(chunk_name)
            })
        })
        .and_then(|chunk| chunk.get(payload))
        .unwrap_or_else(|| panic!("missing {component}/{chunk_name}/{payload}: {action}"))
}

fn final_registers() -> [(&'static str, &'static str); 8] {
    [
        ("x5", "0x9"),
        ("x6", "0x6"),
        ("x7", "0x14"),
        ("x8", "0x7"),
        ("x9", "0x1a"),
        ("x14", "0x8"),
        ("x16", "0x21"),
        ("x17", "0x2a"),
    ]
}

#[test]
fn rem6_run_o3_deep_scalar_window_rejects_live_checkpoint_and_restores_drained() {
    let path = scalar_live_window_binary("o3-deep-scalar-checkpoint", false);
    let baseline = scalar_live_window_json(&path, "direct", 8, 2, 2_000);
    let load = event_at_pc(&baseline, LOAD_PC);
    let live_arg = format!(
        "{}:deep-scalar-live",
        event_u64(load, "lsq_data_response_tick") - 1
    );
    let mut live = scalar_live_window_command(
        &path,
        "direct",
        8,
        2,
        2_000,
        "detailed",
        "json",
    );
    live.args(["--host-checkpoint", &live_arg]);
    let output = live.output().unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("checkpoint component is not quiescent: cpu0"));

    let checkpoint_tick = event_u64(load, "commit_tick") + 1;
    let restore_tick = checkpoint_tick + 1;
    let checkpoint_arg = format!("{checkpoint_tick}:deep-scalar-drained");
    let restore_arg = format!("{restore_tick}:deep-scalar-drained");
    let restored = scalar_live_window_json_with_mode_and_args(
        &path,
        "direct",
        8,
        2,
        2_000,
        "detailed",
        &[
            "--host-checkpoint",
            &checkpoint_arg,
            "--host-restore-checkpoint",
            &restore_arg,
        ],
    );
    assert_final_witness(&restored, FINAL_MEMORY, final_registers());
    let checkpoint = restored.pointer("/host_actions/checkpoints/0").unwrap();
    let restore = restored.pointer("/host_actions/checkpoint_restores/0").unwrap();
    let captured = component_chunk(checkpoint, "cpu0", "o3-runtime-state", "o3_runtime");
    let replayed = component_chunk(restore, "cpu0", "o3-runtime-state", "o3_runtime");
    for field in ["snapshot_rob_entries", "snapshot_lsq_entries"] {
        assert_eq!(captured.get(field).and_then(Value::as_u64), Some(0));
        assert_eq!(replayed.get(field).and_then(Value::as_u64), Some(0));
    }
    assert_eq!(captured.pointer("/stats_issued_rows").and_then(Value::as_u64), Some(7));
    assert_eq!(replayed.pointer("/stats_issued_rows"), captured.pointer("/stats_issued_rows"));
}

#[test]
fn rem6_run_host_switch_preserves_deep_scalar_window_timing() {
    let path = scalar_live_window_binary("o3-deep-scalar-switch", false);
    let baseline = scalar_live_window_json(&path, "direct", 8, 2, 2_000);
    let requested = event_u64(event_at_pc(&baseline, ROW_PCS[0]), "issue_tick") + 1;
    let switch_arg = format!("{requested}:cpu0:timing");
    let switched = scalar_live_window_json_with_mode_and_args(
        &path,
        "direct",
        8,
        2,
        2_000,
        "detailed",
        &["--host-switch-cpu-mode", &switch_arg],
    );
    assert_final_witness(&switched, FINAL_MEMORY, final_registers());
    for pc in std::iter::once(LOAD_PC).chain(ROW_PCS) {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(event_u64(actual, field), event_u64(expected, field));
        }
    }
    let switch = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
            })
        })
        .unwrap();
    let switch_tick = switch.pointer("/tick").and_then(Value::as_u64).unwrap();
    assert!(switch_tick >= requested);
    assert!(
        switch_tick
            < event_u64(event_at_pc(&baseline, LOAD_PC), "lsq_data_response_tick")
    );
    let transfer = switch.pointer("/state_transfer").unwrap();
    let runtime = component_chunk(transfer, "cpu0", "o3-runtime-state", "o3_runtime");
    let handoff = component_chunk(transfer, "cpu0", "o3-live-data-handoff", "o3_live_data_handoff");
    assert_eq!(runtime.pointer("/decode_error").and_then(Value::as_bool), Some(false));
    assert_eq!(runtime.pointer("/snapshot_rob_entries").and_then(Value::as_u64), Some(8));
    assert_eq!(runtime.pointer("/snapshot_lsq_entries").and_then(Value::as_u64), Some(1));
    assert_eq!(handoff.pointer("/younger_rows").and_then(Value::as_u64), Some(7));
}

#[test]
fn rem6_run_timing_suppresses_deep_scalar_window_surfaces() {
    let path = scalar_live_window_binary("o3-deep-scalar-timing", false);
    let timing = scalar_live_window_json_with_mode_and_args(
        &path,
        "direct",
        8,
        2,
        2_000,
        "timing",
        &[],
    );
    assert_final_witness(&timing, FINAL_MEMORY, final_registers());
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing.pointer("/debug/o3_trace").is_none());
    for (_, stat_field, _) in ISSUE_STATS {
        assert_json_stat_absent(&timing, &format!("sim.cpu0.o3.{stat_field}"));
    }
    for alias in [
        "system.cpu.iq.issuedInstType.IntMult",
        "system.cpu.iew.wbRate",
        "system.cpu.rob.maxOccupancy",
    ] {
        assert_json_stat_absent(&timing, alias);
    }
}
```

- [ ] **Step 7: Add focused retry/failure/redirect cleanup for a deep suffix**

Declare `deep_scalar_cleanup` in `o3_runtime_writeback_tests.rs`. Add `retry_cleanup_discards_deep_scalar_suffix`, `failure_cleanup_discards_deep_scalar_suffix`, and `redirect_cleanup_discards_deep_scalar_suffix`. In the child, extend the existing cleanup fixture to `(memory=1, live=8)` and seven younger staged scalar rows. Stage the load head by calling `stage_live_data_access_issue(..., O3DataAccessWindowPolicy::UntranslatedScalarMemoryPrefix)` directly; do not use the compatibility test helper that selects `ScalarMemoryPrefix`. Capture `let committed_rename_map = runtime.snapshot.rename_map().to_vec();` before staging. For both `Retry` and `Failed`, complete enough independent producers to reserve future writeback slots, apply the head outcome, and assert:

```rust
assert!(runtime.live_speculative_executions.is_empty());
assert!(runtime.live_data_access_younger_sequences.is_empty());
assert!(runtime.live_control_lineages.is_empty());
assert!(runtime.live_staged_fetch_identities.is_empty());
assert!(runtime.writeback_reservations().is_empty());
assert!(runtime.snapshot.reorder_buffer().is_empty());
assert!(runtime.snapshot.load_store_queue().is_empty());
assert!(runtime.snapshot.committed_rename_map.is_none());
assert_eq!(runtime.snapshot.rename_map(), committed_rename_map.as_slice());
```

Add an older redirect row using `discard_live_staged_instructions_at(now)` and assert ROB, rename, speculative execution, lineage, and all future writeback reservations are empty. Keep these cleanup checks in focused CPU tests rather than duplicating fault injection in the CLI matrix.

Use this complete child module:

```rust
use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvHartState};

use super::*;

fn i_type(imm: i64, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32 & 0x0fff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn decoded_addi(instruction: RiscvInstruction) -> RiscvDecodedInstruction {
    let RiscvInstruction::Addi { rd, rs1, imm } = instruction else {
        unreachable!()
    };
    RiscvInstruction::decode_with_length(i_type(
        imm.value(),
        rs1.index(),
        0,
        rd.index(),
        0x13,
    ))
    .unwrap()
}

fn deep_runtime() -> (O3RuntimeState, RiscvCpuExecutionEvent, Vec<O3LiveIssueRequest>) {
    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_window_depths(1, 8));
    assert!(runtime.set_issue_width(4));
    assert!(runtime.set_writeback_width(4));
    let load = scalar_load_event(0x8000, 10, 12, 0x9000);
    assert!(runtime.stage_live_data_access_issue(
        &load,
        memory_request(20),
        31,
        O3DataAccessWindowPolicy::UntranslatedScalarMemoryPrefix,
    ));
    let younger = (0..7)
        .map(|index| {
            (
                Address::new(0x8004 + index * 4),
                addi(13 + index as u8, 0, index as i64 + 1),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            younger.iter().copied(),
        ),
        7
    );
    let requests = younger
        .iter()
        .copied()
        .enumerate()
        .map(|(index, (pc, instruction))| {
            let consumed = vec![memory_request(100 + index as u64)];
            assert!(runtime.bind_live_staged_fetch_identity(pc, instruction, &consumed));
            O3LiveIssueRequest::new(pc, consumed, decoded_addi(instruction))
        })
        .collect::<Vec<_>>();
    let head = runtime
        .live_data_access_head_reservation(load.fetch().request_id())
        .unwrap();
    runtime
        .schedule_live_speculative_issues(
            &RiscvHartState::new(0x8000),
            head,
            31,
            &requests,
        )
        .unwrap();
    assert_eq!(runtime.live_speculative_executions.len(), 7);
    assert!(!runtime.writeback_reservations().is_empty());
    (runtime, load, requests)
}

fn assert_outcome_cleanup(kind: RiscvDataAccessEventKind) {
    let (mut runtime, load, _) = deep_runtime();
    let committed_rename_map = Vec::<O3RenameMapEntry>::new();
    let mut outcome = load;
    outcome.set_data_access_event_kind(kind);
    assert!(runtime
        .complete_live_data_access_response(
            &outcome,
            memory_request(20),
            40,
            9,
            None,
        )
        .unwrap());
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.live_data_access_younger_sequences.is_empty());
    assert!(runtime.live_control_lineages.is_empty());
    assert!(runtime.live_staged_fetch_identities.is_empty());
    assert!(runtime.writeback_reservations().is_empty());
    assert!(runtime.snapshot.reorder_buffer().is_empty());
    assert!(runtime.snapshot.load_store_queue().is_empty());
    assert_eq!(runtime.live_data_accesses.len(), 1);
    assert!(matches!(
        runtime.live_data_accesses[0].outcome,
        O3LiveDataAccessOutcome::Retried | O3LiveDataAccessOutcome::Failed
    ));
    assert!(runtime.snapshot.committed_rename_map.is_none());
    assert_eq!(runtime.snapshot.rename_map(), committed_rename_map.as_slice());
}

#[test]
fn retry_cleanup_discards_deep_scalar_suffix() {
    assert_outcome_cleanup(RiscvDataAccessEventKind::Retry);
}

#[test]
fn failure_cleanup_discards_deep_scalar_suffix() {
    assert_outcome_cleanup(RiscvDataAccessEventKind::Failed);
}

#[test]
fn redirect_cleanup_discards_deep_scalar_suffix() {
    let (mut runtime, _, _) = deep_runtime();
    runtime.discard_live_staged_instructions_at(31);
    assert!(runtime.snapshot.reorder_buffer().is_empty());
    assert!(runtime.snapshot.rename_map().is_empty());
    assert!(runtime.live_speculative_executions.is_empty());
    assert!(runtime.live_control_lineages.is_empty());
    assert!(runtime.live_staged_fetch_identities.is_empty());
    assert!(runtime.writeback_reservations().is_empty());
}
```

- [ ] **Step 8: Run the complete focused matrix**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run live_window_depth -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu deep_scalar_cleanup -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu o3_runtime_writeback_tests -- --nocapture
cargo fmt --all -- --check
```

Expected: all route/depth/width/lifecycle rows pass with exact architectural and structured evidence.

- [ ] **Step 9: Run the per-task read-only review gate, then commit and push**

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth.rs crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth/fixture.rs crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth/lifecycle.rs crates/rem6/tests/cli_run/m5_host_actions/o3/live_window_depth/memory_boundary.rs crates/rem6-cpu/src/o3_runtime_writeback_tests.rs crates/rem6-cpu/src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs
TMPDIR=$PWD/target/tmp git commit -m "test: cover deep scalar o3 matrix"
git push origin main
```

### Task 7: Lock Ownership, Update the Ledger, and Verify the Workspace

**Files:**
- Create: `crates/rem6/tests/source_policy/o3_live_window_ownership.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add the thin rem6 source-policy child declaration**

Add only these two lines to `crates/rem6/tests/source_policy.rs`:

```rust
#[path = "source_policy/o3_live_window_ownership.rs"]
mod o3_live_window_ownership;
```

- [ ] **Step 2: Protect CLI/config/test ownership in the child**

Create `o3_live_window_ownership.rs` with tests that read from `CARGO_MANIFEST_DIR` and assert:

```rust
assert!(line_count("src/config.rs") < 1700);
assert!(line_count("tests/cli_run/validation.rs") < 3800);
assert!(line_count("tests/cli_run/validation/o3_depths.rs") <= 900);
assert!(line_count("tests/cli_run/m5_host_actions/o3/live_window_depth.rs") <= 900);
assert!(line_count("tests/cli_run/m5_host_actions/o3/live_window_depth/fixture.rs") <= 350);
assert!(line_count("tests/cli_run/m5_host_actions/o3/live_window_depth/lifecycle.rs") <= 700);
assert!(line_count("tests/cli_run/m5_host_actions/o3/live_window_depth/memory_boundary.rs") <= 500);
```

Also assert `config/riscv_timing.rs` contains `RiscvO3WindowDepths`, `resolve_riscv_o3_window_depths`, and both exported maximum names; `config.rs` must not define scalar-depth maximum constants or pair-order logic. Assert `validation.rs` contains only `mod o3_depths;` ownership for the moved scalar-depth tests.

Use this child structure:

```rust
use std::fs;
use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn contents(relative: &str) -> String {
    fs::read_to_string(crate_root().join(relative)).unwrap()
}

fn line_count(relative: &str) -> usize {
    contents(relative).lines().count()
}

#[test]
fn o3_live_window_files_keep_focused_ownership_and_caps() {
    for (relative, maximum) in [
        ("src/config.rs", 1699),
        ("tests/cli_run/validation.rs", 3799),
        ("tests/cli_run/validation/o3_depths.rs", 900),
        ("tests/cli_run/m5_host_actions/o3/live_window_depth.rs", 900),
        ("tests/cli_run/m5_host_actions/o3/live_window_depth/fixture.rs", 350),
        ("tests/cli_run/m5_host_actions/o3/live_window_depth/lifecycle.rs", 700),
        ("tests/cli_run/m5_host_actions/o3/live_window_depth/memory_boundary.rs", 500),
    ] {
        let lines = line_count(relative);
        assert!(lines <= maximum, "{relative} has {lines} lines, max {maximum}");
    }
    let timing = contents("src/config/riscv_timing.rs");
    for anchor in [
        "struct RiscvO3WindowDepths",
        "fn resolve_riscv_o3_window_depths",
        "MAX_RISCV_O3_SCALAR_MEMORY_DEPTH",
        "MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH",
    ] {
        assert!(timing.contains(anchor), "missing timing owner {anchor}");
    }
    let root = contents("src/config.rs");
    assert!(!root.contains("const MAX_RISCV_O3_SCALAR"));
    assert!(!root.contains("BelowMemoryDepth {"));
    let validation = contents("tests/cli_run/validation.rs");
    assert!(validation.contains("mod o3_depths;"));
    assert!(!validation.contains("fn rem6_run_accepts_max_riscv_o3_scalar_memory_depth"));
}
```

Use this exact evidence-anchor set in the compiled-source child assertion now; defer appending it to `core_test_anchors.txt` until the ledger is updated in Step 5:

```text
rem6_run_o3_deep_scalar_window_matrix
rem6_run_o3_deep_scalar_window_text_stats
rem6_run_o3_deep_scalar_window_dump_stats
rem6_run_o3_scalar_live_depth_eight_rejects_fifth_memory_row
rem6_run_o3_deep_scalar_window_rejects_live_checkpoint_and_restores_drained
rem6_run_host_switch_preserves_deep_scalar_window_timing
rem6_run_timing_suppresses_deep_scalar_window_surfaces
```

In `o3_live_window_ownership.rs`, concatenate `live_window_depth.rs`, `lifecycle.rs`, and `memory_boundary.rs` and independently assert that every new anchor string appears in those compiled test modules, preventing a docs-only anchor from passing. The existing `gem5_migration_doc_tracks_core_test_anchors` check remains unchanged in this source-policy commit because the new strings are not appended to `core_test_anchors.txt` until Step 5.

```rust
#[test]
fn o3_live_window_ledger_anchors_name_real_cli_tests() {
    let tests = [
        "tests/cli_run/m5_host_actions/o3/live_window_depth.rs",
        "tests/cli_run/m5_host_actions/o3/live_window_depth/lifecycle.rs",
        "tests/cli_run/m5_host_actions/o3/live_window_depth/memory_boundary.rs",
    ]
    .into_iter()
    .map(contents)
    .collect::<Vec<_>>()
    .join("\n");
    for anchor in [
        "rem6_run_o3_deep_scalar_window_matrix",
        "rem6_run_o3_deep_scalar_window_text_stats",
        "rem6_run_o3_deep_scalar_window_dump_stats",
        "rem6_run_o3_scalar_live_depth_eight_rejects_fifth_memory_row",
        "rem6_run_o3_deep_scalar_window_rejects_live_checkpoint_and_restores_drained",
        "rem6_run_host_switch_preserves_deep_scalar_window_timing",
        "rem6_run_timing_suppresses_deep_scalar_window_surfaces",
    ] {
        assert!(tests.contains(anchor), "missing compiled CLI test {anchor}");
    }
}
```

- [ ] **Step 3: Protect CPU dependency ownership and caps**

In `crates/rem6-cpu/tests/source_policy.rs`, add constants:

```rust
const MAX_O3_RUNTIME_ISSUE_DEPENDENCY_LINES: usize = 500;
const MAX_O3_RUNTIME_ISSUE_DEPENDENCY_TEST_LINES: usize = 500;
const MAX_O3_RUNTIME_DEEP_CLEANUP_TEST_LINES: usize = 350;
```

Add a test asserting those caps and that `o3_runtime_issue.rs` contains the dependency child declaration while no source file contains these deleted manual authorities:

```text
enum O3LiveIssueDependencyReadiness
fn earliest_dependency_tick
fn live_issue_dependencies_ready_at
fn live_issue_dependency_readiness
```

Assert `riscv_defaults.rs` is the only CPU/config source defining `MAX_RISCV_O3_SCALAR_MEMORY_DEPTH` and `MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH`, and that the issue root remains at or below 800 lines.

```rust
#[test]
fn deep_scalar_issue_dependency_ownership_is_focused() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let issue = fs::read_to_string(crate_dir.join("src/o3_runtime_issue.rs")).unwrap();
    let dependency =
        fs::read_to_string(crate_dir.join("src/o3_runtime_issue/dependency.rs")).unwrap();
    let dependency_tests = fs::read_to_string(
        crate_dir.join("src/o3_runtime_issue_tests/dependency_scopes.rs"),
    )
    .unwrap();
    let cleanup = fs::read_to_string(
        crate_dir.join("src/o3_runtime_writeback_tests/deep_scalar_cleanup.rs"),
    )
    .unwrap();
    assert!(issue.lines().count() <= MAX_O3_RUNTIME_ISSUE_LINES);
    assert!(dependency.lines().count() <= MAX_O3_RUNTIME_ISSUE_DEPENDENCY_LINES);
    assert!(dependency_tests.lines().count() <= MAX_O3_RUNTIME_ISSUE_DEPENDENCY_TEST_LINES);
    assert!(cleanup.lines().count() <= MAX_O3_RUNTIME_DEEP_CLEANUP_TEST_LINES);
    assert!(issue.contains("mod dependency;"));
    for removed in [
        "enum O3LiveIssueDependencyReadiness",
        "fn earliest_dependency_tick",
        "fn live_issue_dependencies_ready_at",
        "fn live_issue_dependency_readiness",
    ] {
        assert!(!issue.contains(removed), "manual issue authority remains: {removed}");
    }

    let defaults = fs::read_to_string(crate_dir.join("src/riscv_defaults.rs")).unwrap();
    let runtime_memory =
        fs::read_to_string(crate_dir.join("src/o3_runtime_memory_window.rs")).unwrap();
    let rem6_timing = fs::read_to_string(
        crate_dir
            .parent()
            .unwrap()
            .join("rem6/src/config/riscv_timing.rs"),
    )
    .unwrap();
    for constant in [
        "MAX_RISCV_O3_SCALAR_MEMORY_DEPTH",
        "MAX_RISCV_O3_SCALAR_LIVE_WINDOW_DEPTH",
    ] {
        assert!(defaults.contains(&format!("pub const {constant}")));
        assert!(!runtime_memory.contains(&format!("const {constant}")));
        assert!(!rem6_timing.contains(&format!("const {constant}")));
    }
}
```

- [ ] **Step 4: Run source-policy verification and the per-task read-only review gate, then commit separately**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo fmt --all -- --check
git diff --check
```

Expected: both source-policy suites pass with no facade cap increase.

After the per-task review gate:

```bash
git add crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/o3_live_window_ownership.rs crates/rem6-cpu/tests/source_policy.rs
TMPDIR=$PWD/target/tmp git commit -m "test: lock deep scalar o3 ownership"
git push origin main
```

- [ ] **Step 5: Update only the covered CPU ledger boundary**

In the CPU section of `gem5-to-rem6-migration.md`:

- Keep `### CPU Execution Models - 74% representative` unchanged.
- Keep `8 of 10`, `80% raw`, and the representative cap unchanged.
- Replace only the exact claim that scalar-memory-prefix/scalar-ALU windows stop at four rows with evidence for configurable untranslated depth six/eight, one-LSQ/eight-ROB residency, widths 1/2/4, direct/hierarchy routes, transitive/fan-in wakeup, scheduler-owned typed data/control scopes, stats surfaces, live checkpoint rejection, timing-preserving mode transfer, focused retry/failure cleanup, and timing suppression.
- Keep translated loads, result windows, fixed-FU windows, control/producer-forwarded chains, more than four memory rows, broader FP/vector/atomic/device shapes, restorable transport ownership, and a general O3 engine in `Not migrated`.
- Set `Next evidence` to arbitrary broader mixed windows and deeper non-scalar/control/result/device ownership, not another scalar-only depth slice.
- Add the exact new CLI test anchors from Task 6.

Append the seven strings listed in Step 2 to `crates/rem6/tests/source_policy/core_test_anchors.txt` in the same edit as the ledger. This keeps `gem5_migration_doc_tracks_core_test_anchors` green at every commit boundary.

Reflow nearby prose so the file remains exactly 1,200 lines; do not add a second ledger or change checklist marks.

- [ ] **Step 6: Run full verification from a clean task state**

```bash
cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --test source_policy -- --nocapture
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
git diff --check
timeout 2h env TMPDIR=$PWD/target/tmp cargo test -p rem6-cpu --all-targets
timeout 2h env TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
timeout 2h env TMPDIR=$PWD/target/tmp cargo test --workspace
```

Expected: every command exits zero; ledger line count is exactly 1,200.

- [ ] **Step 7: Run two independent high-intensity read-only reviews**

Dispatch two fresh `gpt-5.5` reviewers at `xhigh` reasoning. Give each the design spec, this plan, `git diff`/new commits, and the active contract. Reviewer A focuses on runtime/config ownership, dead code, typed dependency semantics, rollback, and hidden widening. Reviewer B focuses on top-level executable evidence, lifecycle/stat assertions, source-policy caps, and ledger honesty.

Fix every actionable finding, rerun the affected focused tests, then rerun the complete Step 6 verification. Close both reviewers after their findings are resolved.

- [ ] **Step 8: After the two-review gate passes, commit and push the ledger closeout**

```bash
git add docs/architecture/gem5-to-rem6-migration.md crates/rem6/tests/source_policy/core_test_anchors.txt
TMPDIR=$PWD/target/tmp git commit -m "docs: record deep scalar o3 windows"
git push origin main
git status --short --branch
```

Expected: `main` matches `origin/main` and the worktree is clean.
