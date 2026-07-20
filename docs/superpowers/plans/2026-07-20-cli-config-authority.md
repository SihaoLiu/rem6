# CLI Config Authority Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace duplicated CLI config prescanning, TOML read/parse mapping, required-value handling, and relative-path resolution with one private top-level authority while preserving every command's current semantics.

**Architecture:** `cli_config.rs` owns the generic scanner and seven command profiles, generic TOML loading, required-value errors, and relative path resolution. Command modules retain private TOML types, root-table unwrapping, `config_dir` assignment, validation, and CLI-over-file merge behavior. The core wildcard scanners and auxiliary explicit scanners remain behaviorally distinct.

**Tech Stack:** Rust 2021, serde/TOML, Cargo unit/source-policy/integration tests, real CLI subprocess tests.

---

## File Map

- `crates/rem6/src/cli_config.rs`: shared config mechanics, command profiles, and focused unit tests.
- `crates/rem6/src/lib.rs`: private module declaration.
- `crates/rem6/src/config/file_scan.rs`: delete after moving core profiles.
- `crates/rem6/src/config.rs`: core scanner imports, generic TOML load, shared path resolution.
- `crates/rem6/src/config/trace_replay.rs`: trace prescan import.
- `crates/rem6/src/config/parse.rs`: config-subtree re-export of shared `required_value`.
- `crates/rem6/src/gpu_cli.rs`: shared GPU profile, load, value, and path helpers.
- `crates/rem6/src/accelerator_cli.rs`: shared accelerator profile, load, value, and path helpers.
- `crates/rem6/src/multi_run_cli.rs`: shared multi-run profile, load, value, and path helpers.
- `crates/rem6/src/resource_acquire_config.rs`: shared resource profile, load, value, and path helpers.
- `crates/rem6/src/power_import_cli.rs`: shared value helper only.
- `crates/rem6/tests/source_policy.rs`: focused source-policy module registration.
- `crates/rem6/tests/source_policy/cli_config_authority.rs`: ownership and duplicate-retirement policy.

### Task 1: Create the authority and migrate run, GUPS, and trace replay

**Files:**
- Create: `crates/rem6/src/cli_config.rs`
- Modify: `crates/rem6/src/lib.rs`
- Delete: `crates/rem6/src/config/file_scan.rs`
- Modify: `crates/rem6/src/config.rs`
- Modify: `crates/rem6/src/config/trace_replay.rs`
- Modify: `crates/rem6/src/config/parse.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Create: `crates/rem6/tests/source_policy/cli_config_authority.rs`

- [x] **Step 1: Add the failing core source-policy row**

Register the focused module in `crates/rem6/tests/source_policy.rs`:

```rust
#[path = "source_policy/cli_config_authority.rs"]
mod cli_config_authority;
```

Create `cli_config_authority.rs` with reusable file and production-section
helpers, then add:

```rust
#[test]
fn core_cli_config_mechanics_have_one_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = read(crate_dir.join("src/lib.rs"));
    let authority_path = crate_dir.join("src/cli_config.rs");
    let config = read(crate_dir.join("src/config.rs"));
    let parse = read(crate_dir.join("src/config/parse.rs"));

    assert!(lib.contains("mod cli_config;"));
    assert!(authority_path.is_file());
    assert!(!crate_dir.join("src/config/file_scan.rs").exists());

    let authority = read(authority_path);
    for anchor in [
        "fn config_path_from_args(",
        "pub(crate) fn run_file_config_from_args(",
        "pub(crate) fn gups_file_config_from_args(",
        "pub(crate) fn trace_replay_file_config_from_args(",
        "pub(crate) fn read_toml_config<",
        "pub(crate) fn required_value(",
        "pub(crate) fn resolve_config_path(",
    ] {
        assert!(authority.contains(anchor), "missing authority `{anchor}`");
    }

    assert!(!config.contains("fn resolve_config_path("));
    assert!(!config.contains("Rem6CliError::ReadConfig"));
    assert!(!config.contains("Rem6CliError::ParseConfig"));
    assert!(!parse.contains("fn required_value("));
    assert!(parse.contains("pub(super) use crate::cli_config::required_value;"));
    assert!(line_count(&crate_dir.join("src/config.rs")) < 1700);
}
```

Set a focused `cli_config.rs` source budget of 500 lines in the policy module.

- [x] **Step 2: Run the policy test and observe RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy cli_config_authority::core_cli_config_mechanics_have_one_authority -- --exact
```

Expected: FAIL because `cli_config.rs` does not exist and `config/file_scan.rs`
still owns the scanner.

- [x] **Step 3: Add failing core scanner and helper unit tests**

Create `cli_config.rs` with imports, type declarations, and a `#[cfg(test)]`
module first. Add tests for:

```rust
#[test]
fn wildcard_profile_suppresses_literal_config_values_and_keeps_last_config() {
    let args = strings(&[
        "--future-value-flag",
        "--config",
        "--config",
        "first.toml",
        "--config",
        "last.toml",
    ]);

    assert_eq!(
        config_path_from_args(&args, ConfigPrescanProfile::wildcard(&[])).unwrap(),
        Some(PathBuf::from("last.toml"))
    );
}

#[test]
fn boolean_flag_does_not_consume_real_config_selector() {
    let args = strings(&["--execute", "--config", "run.toml"]);

    assert_eq!(
        run_file_config_from_args(&args).unwrap(),
        Some(PathBuf::from("run.toml"))
    );
}

#[test]
fn missing_config_value_reports_the_original_flag() {
    assert_eq!(
        gups_file_config_from_args(&strings(&["--config"])),
        Err(Rem6CliError::MissingFlagValue {
            flag: "--config".to_string(),
        })
    );
}

#[test]
fn required_value_and_path_resolution_preserve_existing_contracts() {
    assert_eq!(required_value("--output", Some("out".to_string())).unwrap(), "out");
    assert!(matches!(
        required_value("--output", None),
        Err(Rem6CliError::MissingFlagValue { flag }) if flag == "--output"
    ));
    assert_eq!(
        resolve_config_path(Some(Path::new("configs")), Path::new("out.json")),
        PathBuf::from("configs/out.json")
    );
    assert_eq!(
        resolve_config_path(None, Path::new("out.json")),
        PathBuf::from("out.json")
    );
    assert_eq!(
        resolve_config_path(Some(Path::new("configs")), Path::new("/tmp/out.json")),
        PathBuf::from("/tmp/out.json")
    );
}
```

Also add one temporary-directory test using a local `Deserialize` fixture that
proves `read_toml_config` success plus `ReadConfig` and `ParseConfig` variants.

- [x] **Step 4: Run the focused unit tests and observe RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 cli_config::tests -- --nocapture
```

Expected: compile failure or failing tests because the shared implementations
are not present yet.

- [x] **Step 5: Implement the shared primitives and core profiles**

Implement private scanner machinery:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UnknownLongFlagMode {
    ConsumeFollowingValue,
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ConfigPrescanProfile {
    value_flags: &'static [&'static str],
    bool_flags: &'static [&'static str],
    unknown_long_flags: UnknownLongFlagMode,
}

impl ConfigPrescanProfile {
    const fn wildcard(bool_flags: &'static [&'static str]) -> Self {
        Self {
            value_flags: &[],
            bool_flags,
            unknown_long_flags: UnknownLongFlagMode::ConsumeFollowingValue,
        }
    }

    const fn explicit(
        value_flags: &'static [&'static str],
        bool_flags: &'static [&'static str],
    ) -> Self {
        Self {
            value_flags,
            bool_flags,
            unknown_long_flags: UnknownLongFlagMode::Ignore,
        }
    }
}
```

Implement `config_path_from_args` with the exact ordering:

1. recognized `--config` reads through `required_value`, stores the path, and
   advances two tokens;
2. known booleans advance one;
3. known value flags advance two;
4. wildcard unknown long flags advance two when a following token exists,
   otherwise one;
5. all other tokens advance one.

Add these core wrappers:

```rust
const RUN_BOOL_FLAGS: &[&str] = &[
    "--execute",
    "--checker-cpu",
    "--dram-memory",
    "--riscv-se",
    "--riscv-sbi",
];

pub(crate) fn run_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, ConfigPrescanProfile::wildcard(RUN_BOOL_FLAGS))
}

pub(crate) fn gups_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, ConfigPrescanProfile::wildcard(&[]))
}

pub(crate) fn trace_replay_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(args, ConfigPrescanProfile::wildcard(&[]))
}
```

Implement `read_toml_config`, `required_value`, and `resolve_config_path`
exactly as specified in the design.

- [x] **Step 6: Migrate the core config family**

1. Add `mod cli_config;` in `lib.rs`.
2. Delete `config/file_scan.rs` and its module declaration/import.
3. Import core scanner wrappers, `read_toml_config`, and
   `resolve_config_path` from `crate::cli_config`.
4. Make all three `Rem6RunFileConfig`/`Rem6GupsFileConfig`/
   `Rem6TraceReplayFileConfig::resolve_path` methods call the shared resolver.
5. Delete local `resolve_config_path`.
6. Replace `load_file_config(path)?` in the three typed loaders with
   `read_toml_config::<Rem6FileConfig>(path)?`, then delete local
   `load_file_config`.
7. In `config/parse.rs`, replace the local function with:

```rust
pub(super) use crate::cli_config::required_value;
```

8. Import `trace_replay_file_config_from_args` directly in
   `config/trace_replay.rs`.

- [x] **Step 7: Run core GREEN verification**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6 cli_config::tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy cli_config_authority::core_cli_config_mechanics_have_one_authority -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run execution::rem6_run_resolves_toml_relative_binary_from_config_directory -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run validation::rem6_run_config_scan_treats_riscv_se_stdin_as_value_taking -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run trace_replay::rem6_trace_replay_loads_toml_config_relative_trace_and_cli_route_override -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --lib
git diff --check
```

Expected: all tests PASS, `config.rs` has meaningful line headroom, and no
auxiliary command behavior has changed yet.

- [x] **Step 8: Commit the core authority slice**

```bash
git add crates/rem6/src/cli_config.rs crates/rem6/src/lib.rs crates/rem6/src/config.rs crates/rem6/src/config/trace_replay.rs crates/rem6/src/config/parse.rs crates/rem6/src/config/file_scan.rs crates/rem6/tests/source_policy.rs crates/rem6/tests/source_policy/cli_config_authority.rs
git diff --cached --check
TMPDIR=$PWD/target/tmp git commit -m "refactor: centralize core cli config mechanics"
```

### Task 2: Migrate auxiliary command config mechanics

**Files:**
- Modify: `crates/rem6/src/cli_config.rs`
- Modify: `crates/rem6/src/gpu_cli.rs`
- Modify: `crates/rem6/src/accelerator_cli.rs`
- Modify: `crates/rem6/src/multi_run_cli.rs`
- Modify: `crates/rem6/src/resource_acquire_config.rs`
- Modify: `crates/rem6/src/power_import_cli.rs`
- Modify: `crates/rem6/tests/source_policy/cli_config_authority.rs`

- [x] **Step 1: Add the failing auxiliary source-policy row**

Add:

```rust
#[test]
fn auxiliary_commands_consume_cli_config_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let authority = read(crate_dir.join("src/cli_config.rs"));

    for wrapper in [
        "gpu_run_file_config_from_args",
        "accelerator_run_file_config_from_args",
        "multi_run_file_config_from_args",
        "resource_acquire_file_config_from_args",
    ] {
        assert!(authority.contains(&format!("pub(crate) fn {wrapper}(")));
    }

    for relative in [
        "src/gpu_cli.rs",
        "src/accelerator_cli.rs",
        "src/multi_run_cli.rs",
        "src/resource_acquire_config.rs",
    ] {
        let source = read(crate_dir.join(relative));
        assert!(source.contains("crate::cli_config"));
        assert!(!source.contains("Rem6CliError::ReadConfig"));
        assert!(!source.contains("Rem6CliError::ParseConfig"));
        assert!(!source.contains("fn required_value("));
        assert!(!source.contains("if path.is_relative()"));
    }

    assert!(!read(crate_dir.join("src/power_import_cli.rs"))
        .contains("fn required_value("));
}
```

Also scan every production Rust file and assert that only `cli_config.rs`
defines `fn config_path_from_args`, `fn required_value`, or
`fn resolve_config_path`.

- [x] **Step 2: Run the auxiliary policy and observe RED**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy cli_config_authority::auxiliary_commands_consume_cli_config_authority -- --exact
```

Expected: FAIL on the existing auxiliary scanner/load/value/path copies.

- [x] **Step 3: Add failing explicit-profile unit tests**

Extend `cli_config::tests` with:

```rust
#[test]
fn explicit_profile_suppresses_known_values_but_not_unknown_flags() {
    let known = strings(&["--output", "--config", "--config", "gpu.toml"]);
    assert_eq!(
        gpu_run_file_config_from_args(&known).unwrap(),
        Some(PathBuf::from("gpu.toml"))
    );

    let unknown = strings(&["--future-flag", "--config", "gpu.toml"]);
    assert_eq!(
        gpu_run_file_config_from_args(&unknown).unwrap(),
        Some(PathBuf::from("gpu.toml"))
    );
}

#[test]
fn auxiliary_profiles_preserve_value_and_boolean_vocabularies() {
    assert_eq!(
        accelerator_run_file_config_from_args(&strings(&[
            "--gpu-kernel", "--config", "--config", "accelerator.toml"
        ])).unwrap(),
        Some(PathBuf::from("accelerator.toml"))
    );
    assert_eq!(
        multi_run_file_config_from_args(&strings(&[
            "--continue-on-failure", "--config", "multi.toml"
        ])).unwrap(),
        Some(PathBuf::from("multi.toml"))
    );
    assert_eq!(
        resource_acquire_file_config_from_args(&strings(&[
            "--output", "--config", "--config", "resources.toml"
        ])).unwrap(),
        Some(PathBuf::from("resources.toml"))
    );
}
```

Add missing-config assertions for each auxiliary wrapper.

- [x] **Step 4: Add auxiliary profiles to `cli_config.rs`**

Move the exact current GPU value and boolean lists, accelerator value list,
multi-run value/boolean lists, and resource-acquire value list into constants.
Add command-named wrappers that call
`ConfigPrescanProfile::explicit(value_flags, bool_flags)`.

Do not add power-import config reading or a power-import profile.

- [x] **Step 5: Migrate GPU, accelerator, multi-run, and resource-acquire**

For each command module:

1. import its shared scanner wrapper, `read_toml_config`, `required_value`, and
   `resolve_config_path`;
2. delete the local scanner loop;
3. replace local file read/TOML parse blocks with `read_toml_config::<Root>`;
4. delete local `required_value`;
5. make the private file-config `resolve_path` method delegate to
   `resolve_config_path`; and
6. preserve root-table unwrapping, defaults, `config_dir`, parser loops,
   validation, and list semantics byte-for-byte.

In `power_import_cli.rs`, import only `required_value` and delete its local
copy. Keep power artifact reading and `PowerAnalysis` error mapping unchanged.

- [x] **Step 6: Run auxiliary GREEN verification**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6 cli_config::tests -- --nocapture
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy cli_config_authority::auxiliary_commands_consume_cli_config_authority -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run gpu::rem6_gpu_run_config_scan_skips_value_that_matches_config_flag -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run accelerator::rem6_accelerator_run_loads_toml_config_and_writes_artifacts -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run multi_run::rem6_multi_run_executes_run_configs_and_writes_aggregate_artifacts -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run resource_acquire::rem6_resource_acquire_loads_config_manifest_and_local_artifact -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --lib
git diff --check
```

Expected: all tests PASS with unchanged config paths, output paths, child
commands, and error surfaces.

- [x] **Step 7: Commit the auxiliary migration**

```bash
git add crates/rem6/src/cli_config.rs crates/rem6/src/gpu_cli.rs crates/rem6/src/accelerator_cli.rs crates/rem6/src/multi_run_cli.rs crates/rem6/src/resource_acquire_config.rs crates/rem6/src/power_import_cli.rs crates/rem6/tests/source_policy/cli_config_authority.rs
git diff --cached --check
TMPDIR=$PWD/target/tmp git commit -m "refactor: unify auxiliary cli config mechanics"
```

### Task 3: Prove command compatibility and workspace health

**Files:**
- Test only unless a regression is found.

- [x] **Step 1: Run wildcard suppression and negative parser rows**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run validation::rem6_run_config_scan_treats_riscv_se_stdin_as_value_taking -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run validation::rem6_trace_replay_config_scan_treats_resource_and_qos_flags_as_value_taking -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run execution::rem6_run_config_scan_preserves_non_config_flag_errors -- --exact
```

- [x] **Step 2: Run relative-path and command-load rows**

```bash
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run execution::rem6_run_resolves_toml_relative_binary_from_config_directory -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run trace_replay::rem6_trace_replay_loads_toml_config_relative_trace_and_cli_route_override -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run gpu::rem6_gpu_run_accepts_toml_config_for_top_level_execution -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run accelerator::rem6_accelerator_run_loads_toml_config_and_writes_artifacts -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run multi_run::rem6_multi_run_executes_run_configs_and_writes_aggregate_artifacts -- --exact
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test cli_run resource_acquire::rem6_resource_acquire_loads_config_manifest_and_local_artifact -- --exact
```

- [x] **Step 3: Run broad verification**

```bash
TMPDIR=$PWD/target/tmp cargo fmt --all -- --check
TMPDIR=$PWD/target/tmp cargo test -p rem6 --test source_policy
TMPDIR=$PWD/target/tmp cargo test -p rem6 --all-targets
TMPDIR=$PWD/target/tmp cargo test --workspace --all-targets
git diff --check
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
test "$(wc -l < crates/rem6/src/config.rs)" -lt 1700
test "$(wc -l < crates/rem6/src/lib.rs)" -le 1250
test "$(wc -l < crates/rem6/tests/source_policy.rs)" -le 1400
git status --short
```

Expected: all tests PASS; the ledger stays 1,200 lines; only the live plan is
dirty before closeout; no protected `temp/` path is staged or tracked.

### Task 4: Independent review, closeout, and push

**Files:**
- Modify: `docs/superpowers/plans/2026-07-20-cli-config-authority.md`

- [x] **Step 1: Dispatch the mandatory high-intensity read-only review**

```text
Review only; do not edit. Audit the full increment for prescan semantic drift,
literal --config value handling, wildcard/explicit mode confusion, last-config
precedence, missing-value errors, TOML ReadConfig/ParseConfig drift, relative
path changes, CLI-over-file/list behavior changes, power-import error changes,
source-policy blind spots, and missing real CLI coverage. Report findings by
severity with file and line references; explicitly say when there are none.
```

- [x] **Step 2: Resolve every finding and rerun affected plus workspace tests**

Reproduce each finding, add a failing regression when appropriate, make the
smallest fix, rerun the focused command, then rerun Task 3 Step 3. Commit fixes
with a scoped message.

- [x] **Step 3: Mark all plan tasks complete and commit closeout**

```bash
git add docs/superpowers/plans/2026-07-20-cli-config-authority.md
git diff --cached --check
TMPDIR=$PWD/target/tmp git commit -m "docs: close cli config authority plan"
```

- [x] **Step 4: Push and verify synchronization**

```bash
git status --short --branch
git log --oneline --decorate -8
TMPDIR=$PWD/target/tmp git push origin main
git status --short --branch
git rev-parse HEAD origin/main
```

Expected: push succeeds, final status is `main...origin/main`, and both commit
IDs are identical.
