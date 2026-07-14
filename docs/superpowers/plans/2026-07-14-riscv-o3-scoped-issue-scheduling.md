# RISC-V O3 Scoped Issue Scheduling Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the existing `O3ScopedIssueScheduler` control cycle-visible issue width, operation-class capacity, and register-dependency blocking for the bounded detailed RISC-V O3 live window through the real `rem6` CLI.

**Architecture:** Add one shared issue-width configuration, then place a focused `o3_runtime_issue.rs` bridge between staged live ROB rows and `O3ScopedIssueScheduler`. The bridge reserves the current window head and already-issued rows, schedules only newly available younger candidates, records plan-derived arbitration stats, and reuses the existing ROB/LSQ/rename/control/retirement authorities. Preserve checkpoint compatibility by appending version-22 stats only; do not serialize a second issue queue or scheduler calendar.

**Tech Stack:** Rust workspace, `rem6-cpu`, `rem6-system`, handwritten `rem6` CLI/TOML config, scoped O3 scheduler, JSON/text stats, checkpoint codec, Cargo tests, source-policy tests.

---

## File Structure

- Modify `crates/rem6-cpu/src/riscv_defaults.rs`: own issue-width minimum, default, and maximum constants.
- Modify `crates/rem6-cpu/src/public_api.rs`: export the shared issue-width constants.
- Modify `crates/rem6-cpu/src/lib.rs`: expose `RiscvCore::set_o3_issue_width`.
- Modify `crates/rem6-cpu/src/o3_runtime.rs`: declare the focused issue module and retain configuration/transient issue-stat state.
- Create `crates/rem6-cpu/src/o3_runtime_issue.rs`: own live issue requests, reservations, instruction classification, scoped planning, execution, and plan-derived stat recording.
- Create `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`: focused width, resource, dependency, and partial re-entry tests.
- Modify `crates/rem6-cpu/src/o3_runtime_control_window.rs`: expose candidate sequence/class/dependency metadata without moving rollback ownership.
- Modify `crates/rem6-cpu/src/o3_runtime_memory.rs`: expose the matching scalar-memory head issue reservation.
- Modify `crates/rem6-cpu/src/riscv_live_retire_window.rs`: convert completed fetch rows into issue requests and delegate to the runtime issue owner.
- Modify `crates/rem6-cpu/src/o3_runtime_stats.rs`: add five arbitration counters and accessors.
- Modify `crates/rem6-cpu/src/o3_runtime_checkpoint.rs`: add version-22 arbitration-stat compatibility.
- Modify `crates/rem6-cpu/src/o3_runtime_checkpoint_tests.rs`: prove v22 round trip and v21 zero defaults.
- Modify `crates/rem6/src/config.rs`: parse/store CLI and TOML issue width.
- Modify `crates/rem6/src/config/riscv_timing.rs`: validate issue width from shared CPU constants.
- Modify `crates/rem6/src/config/accessors.rs`: expose effective and explicit issue-width accessors.
- Modify `crates/rem6/src/config/file_scan.rs`: classify the new CLI flag as value-taking.
- Modify `crates/rem6/src/cli_error.rs`: add typed invalid/routing errors.
- Modify `crates/rem6/src/run_validation.rs`: enforce `--execute` and RISC-V requirements.
- Modify `crates/rem6/src/riscv_core_runtime.rs`: configure every constructed core.
- Modify `crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs`: register and update host-action arbitration stats.
- Modify `crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs`: snapshot the new stat IDs.
- Create `crates/rem6/src/stats_output/o3_runtime_issue.rs`: emit native arbitration text stats without growing the O3 root past policy.
- Modify `crates/rem6/src/stats_output/o3_runtime.rs`: delegate issue-stat emission.
- Modify `crates/rem6/src/core_summary_json.rs`: add the structured `o3_runtime.issue` object.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`: top-level width/resource/dependency/transfer/checkpoint/timing matrix.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`: register the focused CLI module.
- Modify `crates/rem6/tests/cli_run/validation.rs`: prove CLI/TOML acceptance, rejection, and routing.
- Modify `crates/rem6-cpu/tests/source_policy.rs`: protect the runtime issue owner and consumer boundary.
- Modify `crates/rem6/tests/source_policy.rs`: protect the focused stats-output and CLI module boundaries.
- Modify `crates/rem6/tests/source_policy/core_test_anchors.txt`: anchor config, artifacts, and every new top-level row.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record executable evidence while preserving 74%, 8/10, unchecked items, and exactly 1,200 lines.

### Task 0: Verify the Clean Baseline

**Files:**
- Read only: current worktree and existing tests.

- [ ] **Step 1: Confirm the execution workspace**

Run:

```bash
git status --short --branch
git rev-parse HEAD
```

Expected: `main` is clean after the design and plan commits, with no unrelated
changes. Work in this checkout because the user explicitly requested commits
and pushes on the active branch.

- [ ] **Step 2: Run baseline suites before behavior edits**

Run:

```bash
cargo test -p rem6-cpu --lib
cargo test -p rem6 --test cli_run m5_host_actions::o3::lsq_fu_window -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control -- --nocapture
cargo test -p rem6 --test cli_run riscv_o3_scalar_memory_depth -- --nocapture
```

Expected: all commands exit zero. If a baseline command fails, investigate and
separate the pre-existing failure before starting Task 1.

### Task 1: Configure RISC-V O3 Issue Width

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_defaults.rs`
- Modify: `crates/rem6-cpu/src/public_api.rs`
- Modify: `crates/rem6-cpu/src/lib.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6/src/config.rs`
- Modify: `crates/rem6/src/config/riscv_timing.rs`
- Modify: `crates/rem6/src/config/accessors.rs`
- Modify: `crates/rem6/src/config/file_scan.rs`
- Modify: `crates/rem6/src/cli_error.rs`
- Modify: `crates/rem6/src/run_validation.rs`
- Modify: `crates/rem6/src/riscv_core_runtime.rs`
- Test: `crates/rem6/tests/cli_run/validation.rs`

- [ ] **Step 1: Write failing table-driven CLI and TOML tests**

Add focused tests beside the scalar-memory-depth rows. Use one table for value
validation and one table for routing requirements:

```rust
#[test]
fn rem6_run_accepts_riscv_o3_issue_width_cli_and_toml() {
    let program = riscv64_program(&[0x0000_0013; 64]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-o3-issue-width-accepted", &elf);

    for (name, extra_args) in [
        ("cli-one", vec!["--riscv-o3-issue-width", "1"]),
        ("cli-four", vec!["--riscv-o3-issue-width", "4"]),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run", "--isa", "riscv", "--binary", path.to_str().unwrap(),
                "--max-tick", "40", "--stats-format", "json", "--execute",
            ])
            .args(extra_args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{name}: {}", String::from_utf8_lossy(&output.stderr));
    }

    let config = temp_output("riscv-o3-issue-width.toml");
    std::fs::write(&config, "[run]\nriscv_o3_issue_width = 4\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run", "--config", config.to_str().unwrap(), "--isa", "riscv",
            "--binary", path.to_str().unwrap(), "--max-tick", "40",
            "--stats-format", "json", "--execute",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn rem6_run_rejects_invalid_riscv_o3_issue_width_cli_and_toml() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-o3-issue-width-invalid", &elf);

    for value in ["0", "5", "invalid"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run", "--isa", "riscv", "--binary", path.to_str().unwrap(),
                "--max-tick", "40", "--stats-format", "json", "--execute",
                "--riscv-o3-issue-width", value,
            ])
            .output()
            .unwrap();
        assert!(!output.status.success(), "value {value} unexpectedly succeeded");
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains(&format!("invalid RISC-V O3 issue width {value}")));
    }

    for value in [0, 5] {
        let config = temp_output(&format!("riscv-o3-issue-width-invalid-{value}.toml"));
        std::fs::write(&config, format!("[run]\nriscv_o3_issue_width = {value}\n")).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run", "--config", config.to_str().unwrap(), "--isa", "riscv",
                "--binary", path.to_str().unwrap(), "--max-tick", "40",
                "--stats-format", "json", "--execute",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains(&format!("invalid RISC-V O3 issue width {value}")));
    }
}

#[test]
fn rem6_run_validates_riscv_o3_issue_width_requirements() {
    let riscv = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let x86 = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);

    for (name, isa, image, execute, expected) in [
        ("without-execute", "riscv", riscv.as_slice(), false,
         "--riscv-o3-issue-width requires --execute"),
        ("without-riscv", "x86", x86.as_slice(), true,
         "--riscv-o3-issue-width requires --isa riscv"),
    ] {
        let path = temp_binary(name, image);
        let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
        command.args([
            "run", "--isa", isa, "--binary", path.to_str().unwrap(),
            "--max-tick", "40", "--stats-format", "json",
            "--riscv-o3-issue-width", "2",
        ]);
        if execute { command.arg("--execute"); }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains(expected));
    }
}
```

Extend the existing config-file argument scanner test with
`--riscv-o3-issue-width 2` and assert the following flag is not consumed as its
value.

- [ ] **Step 2: Run the new tests and verify RED**

Run:

```bash
cargo test -p rem6 --test cli_run riscv_o3_issue_width -- --nocapture
```

Expected: FAIL because the flag/TOML field is unknown and the typed errors do
not exist. Confirm failures are about the missing configuration surface, not
fixture construction.

- [ ] **Step 3: Add shared CPU constants and runtime configuration**

Add to `riscv_defaults.rs` and export through `public_api.rs`:

```rust
pub const MIN_RISCV_O3_ISSUE_WIDTH: usize = 1;
pub const DEFAULT_RISCV_O3_ISSUE_WIDTH: usize = 4;
pub const MAX_RISCV_O3_ISSUE_WIDTH: usize = 4;
```

Add `issue_width: usize` to `O3RuntimeState`, initialize it to the shared
default, preserve it across `restore`, and add:

```rust
pub(crate) fn set_issue_width(&mut self, width: usize) {
    assert!(
        (MIN_RISCV_O3_ISSUE_WIDTH..=MAX_RISCV_O3_ISSUE_WIDTH).contains(&width),
        "RISC-V O3 issue width must be between {MIN_RISCV_O3_ISSUE_WIDTH} and {MAX_RISCV_O3_ISSUE_WIDTH}"
    );
    self.issue_width = width;
}
```

Expose the core setter beside `set_o3_scalar_memory_depth`:

```rust
pub fn set_o3_issue_width(&self, width: usize) {
    self.state
        .lock()
        .expect("riscv core lock")
        .o3_runtime
        .set_issue_width(width);
}
```

- [ ] **Step 4: Add handwritten CLI/TOML plumbing**

In `config/riscv_timing.rs`, import the shared constants and add:

```rust
pub(crate) fn parse_riscv_o3_issue_width(value: &str) -> Result<usize, Rem6CliError> {
    let width = value
        .parse()
        .map_err(|_| Rem6CliError::InvalidRiscvO3IssueWidth { value: value.to_string() })?;
    validate_riscv_o3_issue_width(width)
}

pub(crate) fn validate_optional_riscv_o3_issue_width(
    width: Option<usize>,
) -> Result<Option<usize>, Rem6CliError> {
    width.map(validate_riscv_o3_issue_width).transpose()
}

fn validate_riscv_o3_issue_width(width: usize) -> Result<usize, Rem6CliError> {
    if !(MIN_RISCV_O3_ISSUE_WIDTH..=MAX_RISCV_O3_ISSUE_WIDTH).contains(&width) {
        return Err(Rem6CliError::InvalidRiscvO3IssueWidth { value: width.to_string() });
    }
    Ok(width)
}
```

Add `riscv_o3_issue_width: Option<usize>` to both run config structs, parse it
from CLI and TOML, add effective/explicit accessors using
`DEFAULT_RISCV_O3_ISSUE_WIDTH`, and include the flag in `config/file_scan.rs`.

Add these error variants and exact display strings:

```rust
InvalidRiscvO3IssueWidth { value: String },
RiscvO3IssueWidthRequiresExecution,
RiscvO3IssueWidthRequiresRiscv,
```

```text
invalid RISC-V O3 issue width {value}
--riscv-o3-issue-width requires --execute
--riscv-o3-issue-width requires --isa riscv
```

Mirror scalar-memory-depth routing in `run_validation.rs`. Configure each core
in `riscv_core_runtime.rs`:

```rust
core.set_o3_issue_width(config.riscv_o3_issue_width());
```

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6 --test cli_run riscv_o3_issue_width -- --nocapture
cargo test -p rem6 --test cli_run config_scan_treats_riscv_o3_issue_width -- --nocapture
```

Expected: all selected tests pass.

```bash
git add crates/rem6-cpu/src/riscv_defaults.rs \
  crates/rem6-cpu/src/public_api.rs \
  crates/rem6-cpu/src/lib.rs \
  crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6/src/config.rs \
  crates/rem6/src/config/riscv_timing.rs \
  crates/rem6/src/config/accessors.rs \
  crates/rem6/src/config/file_scan.rs \
  crates/rem6/src/cli_error.rs \
  crates/rem6/src/run_validation.rs \
  crates/rem6/src/riscv_core_runtime.rs \
  crates/rem6/tests/cli_run/validation.rs
git commit -m "cli: configure RISC-V O3 issue width"
```

### Task 2: Make Scoped Scheduling Own Live Younger Issue

**Files:**
- Create: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Create: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_control_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_memory.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`

- [ ] **Step 1: Create the focused CLI fixture and failing behavior rows**

Register:

```rust
#[path = "o3/scoped_issue.rs"]
mod scoped_issue;
```

In the new module, define:

```rust
#[derive(Clone, Copy)]
enum ScopedIssueVariant {
    CrossResource,
    SameMultiply,
    Dependent,
}
```

Build one delayed-load program whose three younger rows are selected by the
variant. Initialize independent source registers before the load and store the
three results after the live window:

```rust
let younger = match variant {
    ScopedIssueVariant::CrossResource => [
        i_type(1, 5, 0x0, 13, 0x13),
        r_type(0x01, 7, 6, 0x0, 14, 0x33),
        i_type(1, 8, 0x0, 15, 0x13),
    ],
    ScopedIssueVariant::SameMultiply => [
        i_type(1, 5, 0x0, 13, 0x13),
        r_type(0x01, 7, 6, 0x0, 14, 0x33),
        r_type(0x01, 9, 8, 0x0, 15, 0x33),
    ],
    ScopedIssueVariant::Dependent => [
        r_type(0x01, 6, 5, 0x0, 13, 0x33),
        i_type(1, 13, 0x0, 14, 0x13),
        i_type(1, 7, 0x0, 15, 0x13),
    ],
};
```

The command must use `--debug-flags O3,Data,Fetch,Memory,HostAction`, scalar
memory depth four, route delay sixteen, and the selected issue width.

Add these initial tests:

```rust
#[test]
fn rem6_run_o3_scoped_issue_width_one_serializes_direct_window() {
    let json = run_scoped_issue_json(ScopedIssueVariant::CrossResource, "direct", 1, "detailed", &[]);
    let load_tick = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(event_u64(event_at_pc(&json, FIRST_PC), "issue_tick"), load_tick + 1);
    assert_eq!(event_u64(event_at_pc(&json, SECOND_PC), "issue_tick"), load_tick + 2);
    assert_eq!(event_u64(event_at_pc(&json, THIRD_PC), "issue_tick"), load_tick + 3);
    assert_final_scoped_issue_architecture(&json, ScopedIssueVariant::CrossResource);
}

#[test]
fn rem6_run_o3_scoped_issue_width_two_coissues_cross_resource_rows() {
    let json = run_scoped_issue_json(ScopedIssueVariant::CrossResource, "direct", 2, "detailed", &[]);
    let load_tick = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(event_u64(event_at_pc(&json, FIRST_PC), "issue_tick"), load_tick);
    let second_tick = event_u64(event_at_pc(&json, SECOND_PC), "issue_tick");
    assert_eq!(second_tick, load_tick + 1);
    assert_eq!(event_u64(event_at_pc(&json, THIRD_PC), "issue_tick"), second_tick);
}

#[test]
fn rem6_run_o3_scoped_issue_serializes_same_multiply_resource() {
    let json = run_scoped_issue_json(ScopedIssueVariant::SameMultiply, "cache-fabric-dram", 2, "detailed", &[]);
    let load_tick = event_u64(event_at_pc(&json, LOAD_PC), "issue_tick");
    assert_eq!(event_u64(event_at_pc(&json, FIRST_PC), "issue_tick"), load_tick);
    assert_eq!(event_u64(event_at_pc(&json, SECOND_PC), "issue_tick"), load_tick + 1);
    assert_eq!(event_u64(event_at_pc(&json, THIRD_PC), "issue_tick"), load_tick + 2);
    assert_hierarchy_activity(&json);
}

#[test]
fn rem6_run_o3_scoped_issue_dependency_waits_for_multiply() {
    let json = run_scoped_issue_json(ScopedIssueVariant::Dependent, "direct", 2, "detailed", &[]);
    let multiply = event_at_pc(&json, FIRST_PC);
    let dependent = event_at_pc(&json, SECOND_PC);
    let independent = event_at_pc(&json, THIRD_PC);
    assert!(event_u64(dependent, "issue_tick") >= event_u64(multiply, "writeback_tick"));
    assert!(event_u64(independent, "issue_tick") < event_u64(dependent, "issue_tick"));
}
```

- [ ] **Step 2: Run the matrix and verify RED**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::scoped_issue -- --nocapture
```

Expected: width-one, same-resource, and dependency tick assertions fail because
the live path still records every eligible younger row at the common base tick
or applies only post-hoc dependency arithmetic.

- [ ] **Step 3: Define the focused runtime issue API**

Declare the module and tests from `o3_runtime.rs`:

```rust
#[path = "o3_runtime_issue.rs"]
mod o3_runtime_issue;
#[cfg(test)]
#[path = "o3_runtime_issue_tests.rs"]
mod o3_runtime_issue_tests;
```

Create these owned request/reservation types in `o3_runtime_issue.rs`:

```rust
pub(crate) struct O3LiveIssueRequest {
    pc: Address,
    consumed_requests: Vec<MemoryRequestId>,
    decoded: rem6_isa_riscv::RiscvDecodedInstruction,
}

pub(crate) struct O3LiveIssueHeadReservation {
    sequence: u64,
    issue_tick: u64,
    op_class: O3IssueOpClass,
}
```

Provide constructors used only by `riscv_live_retire_window.rs`. Add one entry
point:

```rust
pub(crate) fn schedule_live_speculative_issues(
    &mut self,
    hart: &rem6_isa_riscv::RiscvHartState,
    head: O3LiveIssueHeadReservation,
    earliest_tick: u64,
    requests: &[O3LiveIssueRequest],
)
```

Do not add either type to `public_api.rs`.

- [ ] **Step 4: Expose candidate data without moving ownership**

In `o3_runtime_control_window.rs`, keep `producer_sequences` for validation and
rollback. Add a separate data-dependency representation:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3LiveIssueDependency {
    pub(super) sequence: u64,
    pub(super) ready_tick: u64,
}
```

Store `data_dependencies: Vec<O3LiveIssueDependency>` on the candidate. Change
`live_speculative_source_forwarding` to return the per-producer ready ticks
instead of only a maximum; `issue_tick` remains the maximum of those ticks.

Add `pub(super)` accessors for sequence, instruction, data dependencies, and
forwarded writes. Keep the existing control sequence in `producer_sequences`,
but remove the requirement that the controlling branch already has an issue
record. The staged `live_control_dependencies` edge remains rollback authority,
not a scheduler data wait.

- [ ] **Step 5: Implement one scoped arbitration loop**

Use one private queue ID. Classify direct conditionals as `Branch`, scalar
integer multiply/divide latency classes as `IntMult`, and other admitted scalar
integer rows as `IntAlu`.

For each candidate tick:

1. Count the head if its issue tick matches.
2. Count already-issued requests from this same request slice at that tick.
3. Subtract those reservations from configured width and the centralized class
   capacities.
4. If width is exhausted, classify dependency-ready candidates as resource
   blocked and unresolved candidates as dependency blocked.
5. Otherwise construct `O3ScopedReadyInstruction` values with sequence scopes,
   call `O3ScopedIssueScheduler::try_plan`, and execute only newly issued rows.
6. Execute selected rows in sequence order by cloning the supplied hart,
   applying forwarded writes, setting PC, and calling `execute_decoded`.
7. Record with the exact selected tick through
   `record_live_speculative_execution`.
8. Advance one tick for resource-blocked work or jump to the earliest unresolved
   producer-ready tick when only dependency-blocked work remains.

Use this capacity construction after subtracting reservations:

```rust
let scheduler = O3ScopedIssueScheduler::new(
    remaining_width,
    [
        (O3IssueOpClass::IntAlu, remaining_int_alu),
        (O3IssueOpClass::IntMult, remaining_int_mult),
        (O3IssueOpClass::Branch, remaining_branch),
    ]
    .into_iter()
    .filter(|(_, slots)| *slots != 0)
    .map(|(class, slots)| O3IssueQueueCapacity::new(LIVE_ISSUE_QUEUE, class, slots).unwrap()),
)
.expect("validated live O3 issue width");
```

- [ ] **Step 6: Delegate both live-window call paths**

Replace `record_o3_live_speculative_younger_executions` with conversion to
`O3LiveIssueRequest` plus one runtime call. For scalar memory, obtain the head
reservation by matching `fetch_request` in `live_scalar_memories`. For the FU
live-retire window, return the head sequence from `stage_live_retire_window` and
classify its instruction through the same issue-profile helper.

The caller must clone `state.hart` before mutably borrowing `state.o3_runtime`.
Keep split-fetch `consumed_requests` exactly as captured by
`RiscvCompletedFetchInstruction`.

- [ ] **Step 7: Add focused runtime tests**

In `o3_runtime_issue_tests.rs`, add:

```text
scoped_issue_reserves_head_width
scoped_issue_allows_cross_resource_peer
scoped_issue_serializes_same_multiply_class
scoped_issue_waits_for_register_producer_ready_tick
scoped_issue_partial_reentry_does_not_overbook_prior_tick
scoped_issue_rollback_uses_existing_producer_chain
```

Use real `O3RuntimeState` staging and `RiscvHartState`; do not test a duplicated
fake scheduler. Assert exact candidate issue ticks and final
`live_speculative_executions` sequence order.

- [ ] **Step 8: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-cpu scoped_issue_ --lib -- --nocapture
cargo test -p rem6-cpu scheduled_live_younger_issue --lib -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::scoped_issue -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::lsq_fu_window -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control -- --nocapture
```

Expected: all selected tests pass and existing predicted-control rows retain
architecture, rollback, and transfer behavior under the default width.

```bash
git add crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_control_window.rs \
  crates/rem6-cpu/src/o3_runtime_memory.rs \
  crates/rem6-cpu/src/riscv_live_retire_window.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs
git commit -m "cpu: schedule live O3 issue width"
```

### Task 3: Record and Checkpoint Arbitration Stats

**Files:**
- Modify: `crates/rem6-cpu/src/o3_runtime.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_stats.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_checkpoint.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_checkpoint_tests.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_issue_tests.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`

- [ ] **Step 1: Add failing stat and checkpoint tests**

Add a runtime test that executes the width-one cross-resource window, then
asserts:

```rust
let stats = runtime.stats();
assert_eq!(stats.issued_rows(), 3);
assert!(stats.issue_cycles() >= 3);
assert!(stats.resource_blocked_row_cycles() > 0);
assert_eq!(stats.dependency_blocked_row_cycles(), 0);
assert_eq!(stats.max_rows_per_cycle(), 1);
```

Add a dependency variant asserting a positive dependency-blocked count. Reset
stats and assert all five fields return zero.

Add codec tests:

```rust
fn issue_stats_checkpoint_payload() -> O3RuntimeCheckpointPayload {
    O3RuntimeCheckpointPayload::from_snapshot_with_stats(
        super::super::default_o3_runtime_snapshot(),
        O3RuntimeStats {
            issue_cycles: 3,
            issued_rows: 3,
            resource_blocked_row_cycles: 6,
            dependency_blocked_row_cycles: 1,
            max_rows_per_cycle: 2,
            ..O3RuntimeStats::default()
        },
    )
    .unwrap()
}

fn encoded_without_issue_arbitration_stats(encoded: &[u8]) -> Vec<u8> {
    let trailer_offset = encoded
        .len()
        .checked_sub(1)
        .unwrap();
    assert_eq!(encoded[trailer_offset], 0, "test payload has no live retire gate");
    let issue_offset = trailer_offset
        .checked_sub(ISSUE_ARBITRATION_STATS_BYTES)
        .unwrap();
    [
        &encoded[..issue_offset],
        &encoded[trailer_offset..],
    ]
    .concat()
}

#[test]
fn checkpoint_v22_payloads_round_trip_issue_arbitration_stats() {
    let payload = issue_stats_checkpoint_payload();
    let decoded = O3RuntimeCheckpointPayload::decode(&payload.encode()).unwrap();
    assert_eq!(decoded.stats().issue_cycles(), 3);
    assert_eq!(decoded.stats().issued_rows(), 3);
    assert_eq!(decoded.stats().resource_blocked_row_cycles(), 6);
    assert_eq!(decoded.stats().dependency_blocked_row_cycles(), 1);
    assert_eq!(decoded.stats().max_rows_per_cycle(), 2);
}

#[test]
fn checkpoint_v21_payloads_decode_without_issue_arbitration_stats() {
    let mut encoded = encoded_without_issue_arbitration_stats(
        &issue_stats_checkpoint_payload().encode(),
    );
    encoded[4] = 21;
    let stats = O3RuntimeCheckpointPayload::decode(&encoded).unwrap().stats();
    assert_eq!(stats.issue_cycles(), 0);
    assert_eq!(stats.issued_rows(), 0);
    assert_eq!(stats.resource_blocked_row_cycles(), 0);
    assert_eq!(stats.dependency_blocked_row_cycles(), 0);
    assert_eq!(stats.max_rows_per_cycle(), 0);
}
```

Add `ISSUE_ARBITRATION_STATS_BYTES = 5 * U64_BYTES`, include it in
`CURRENT_STATS_BYTES`, and update every legacy downgrade helper to remove the
new tail before calculating a version-21-or-earlier payload. The live-retire-
gate payload remains the final trailer.

Add `rem6_run_o3_scoped_issue_checkpoint_boundary` before changing the codec.
The live capture must reject as before. The drained capture/restore must assert
version 22, zero ROB/LSQ, and preserved issue-arbitration stats from the decoded
runtime chunk.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6-cpu issue_arbitration --lib -- --nocapture
cargo test -p rem6-cpu checkpoint_v2 --lib -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_scoped_issue_checkpoint_boundary -- --exact --nocapture
```

Expected: compile/test failures because accessors, counters, and version 22 do
not exist.

- [ ] **Step 3: Add runtime counters and distinct-cycle tracking**

Add these `u64` fields and same-name public accessors to `O3RuntimeStats`:

```rust
issue_cycles,
issued_rows,
resource_blocked_row_cycles,
dependency_blocked_row_cycles,
max_rows_per_cycle,
```

Add:

```rust
pub(super) fn record_issue_cycle(
    &mut self,
    new_cycle: bool,
    issued_rows: usize,
    resource_blocked_rows: usize,
    dependency_blocked_rows: usize,
    total_rows_at_tick: usize,
) {
    self.issue_cycles = self
        .issue_cycles
        .saturating_add(if new_cycle { 1 } else { 0 });
    self.issued_rows = self.issued_rows.saturating_add(issued_rows as u64);
    self.resource_blocked_row_cycles = self
        .resource_blocked_row_cycles
        .saturating_add(resource_blocked_rows as u64);
    self.dependency_blocked_row_cycles = self
        .dependency_blocked_row_cycles
        .saturating_add(dependency_blocked_rows as u64);
    self.max_rows_per_cycle = self.max_rows_per_cycle.max(total_rows_at_tick as u64);
}
```

Add `live_issue_cycle_ticks: BTreeSet<u64>` to `O3RuntimeState`. Clear it in
`restore` and `reset_stats`. In the issue module, call
`insert(current_tick)` to decide `new_cycle`; count only newly scheduled younger
rows in `issued_rows`, and pass head/prior reservations plus new rows to
`total_rows_at_tick`.

Update `O3RuntimeStats::has_activity` for all five counters.

- [ ] **Step 4: Append checkpoint version 22**

Add:

```rust
const O3_RUNTIME_CHECKPOINT_VERSION_WITH_ISSUE_ARBITRATION_STATS: u8 = 22;
const O3_RUNTIME_CHECKPOINT_VERSION: u8 =
    O3_RUNTIME_CHECKPOINT_VERSION_WITH_ISSUE_ARBITRATION_STATS;
```

Tail-append the five counters in `write_o3_runtime_stats`. Add
`has_issue_arbitration_stats` to `read_o3_runtime_stats`; initialize all five to
zero for versions below 22. Add version 22 to the decoder's supported-version
match and preserve decode support for every version 1 through 21.

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-cpu issue_arbitration --lib -- --nocapture
cargo test -p rem6-cpu checkpoint_v22 --lib -- --nocapture
cargo test -p rem6-cpu checkpoint_v21 --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_checkpoint --lib -- --nocapture
```

Expected: all selected tests pass.

```bash
git add crates/rem6-cpu/src/o3_runtime.rs \
  crates/rem6-cpu/src/o3_runtime_issue.rs \
  crates/rem6-cpu/src/o3_runtime_issue_tests.rs \
  crates/rem6-cpu/src/o3_runtime_stats.rs \
  crates/rem6-cpu/src/o3_runtime_checkpoint.rs \
  crates/rem6-cpu/src/o3_runtime_checkpoint_tests.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs
git commit -m "cpu: record scoped issue arbitration"
```

### Task 4: Expose JSON, Text, and Stats-Dump Evidence

**Files:**
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs`
- Modify: `crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs`
- Create: `crates/rem6/src/stats_output/o3_runtime_issue.rs`
- Modify: `crates/rem6/src/stats_output/o3_runtime.rs`
- Modify: `crates/rem6/src/core_summary_json.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`

- [ ] **Step 1: Add failing top-level artifact assertions**

Extend the width/resource tests to assert this structured object:

```rust
let issue = json.pointer("/cores/0/o3_runtime/issue")
    .unwrap_or_else(|| panic!("missing scoped issue summary: {json}"));
assert!(issue.pointer("/cycles").and_then(Value::as_u64).is_some_and(|value| value > 0));
assert_eq!(issue.pointer("/issued_rows").and_then(Value::as_u64), Some(3));
assert!(issue.pointer("/resource_blocked_row_cycles")
    .and_then(Value::as_u64).is_some_and(|value| value > 0));
assert!(issue.pointer("/max_rows_per_cycle").and_then(Value::as_u64)
    .is_some_and(|value| value <= 2));
```

Add exact native stat assertions:

```text
sim.cpu0.o3.issue_cycles
sim.cpu0.o3.issued_rows
sim.cpu0.o3.resource_blocked_row_cycles
sim.cpu0.o3.dependency_blocked_row_cycles
sim.cpu0.o3.max_rows_per_cycle
```

Add one `m5_dump_stats` fixture row and assert matching
`sim.host_actions.stats_dump.cpu0.o3.*` paths.

Add `rem6_run_timing_suppresses_o3_scoped_issue_surface`. It must preserve the
fixture's final registers and memory while asserting the structured issue
object and all five native arbitration stats are absent in timing mode.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6 --test cli_run scoped_issue_width_one -- --nocapture
cargo test -p rem6 --test cli_run scoped_issue_resource_contention -- --nocapture
cargo test -p rem6 --test cli_run scoped_issue_stats_dump -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_scoped_issue_surface -- --exact --nocapture
```

Expected: FAIL because the JSON object and stat paths are absent.

- [ ] **Step 3: Register host-action stats**

Add five `StatId` fields to `RiscvO3RuntimeCpuStats`. Register native paths with
units:

```text
issue_cycles                       Cycle
issued_rows                        Count
resource_blocked_row_cycles       Cycle
dependency_blocked_row_cycles     Cycle
max_rows_per_cycle                Count
```

Update both `increment_delta` and `cpu/snapshot.rs::set_snapshot` using the new
`O3RuntimeStats` accessors.

- [ ] **Step 4: Add focused run-output emitters**

Create `stats_output/o3_runtime_issue.rs`:

```rust
use rem6_cpu::O3RuntimeStats;
use rem6_stats::StatsRegistry;

use super::increment_count_stat;
use crate::Rem6CliError;

pub(super) fn emit_o3_runtime_issue_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    o3: O3RuntimeStats,
) -> Result<(), Rem6CliError> {
    for (name, value) in [
        ("issue_cycles", o3.issue_cycles()),
        ("issued_rows", o3.issued_rows()),
        ("resource_blocked_row_cycles", o3.resource_blocked_row_cycles()),
        ("dependency_blocked_row_cycles", o3.dependency_blocked_row_cycles()),
        ("max_rows_per_cycle", o3.max_rows_per_cycle()),
    ] {
        increment_count_stat(stats, format!("sim.cpu{cpu}.o3.{name}"), value)?;
    }
    Ok(())
}
```

Declare/delegate from `stats_output/o3_runtime.rs`. Keep the root below 1,700
lines.

Add this helper in `core_summary_json.rs` and interpolate it under
`o3_runtime`:

```rust
fn o3_runtime_issue_json(stats: O3RuntimeStats) -> String {
    format!(
        "{{\"cycles\":{},\"issued_rows\":{},\"resource_blocked_row_cycles\":{},\"dependency_blocked_row_cycles\":{},\"max_rows_per_cycle\":{}}}",
        stats.issue_cycles(),
        stats.issued_rows(),
        stats.resource_blocked_row_cycles(),
        stats.dependency_blocked_row_cycles(),
        stats.max_rows_per_cycle(),
    )
}
```

- [ ] **Step 5: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6 --test cli_run scoped_issue_ -- --nocapture
cargo test -p rem6-system riscv_o3_runtime_stats -- --nocapture
cargo test -p rem6 --test cli_run o3_runtime_text_stats -- --nocapture
```

Expected: structured JSON, normal stats, and host-action dump stats all pass.

```bash
git add crates/rem6-system/src/riscv_o3_runtime_stats/cpu.rs \
  crates/rem6-system/src/riscv_o3_runtime_stats/cpu/snapshot.rs \
  crates/rem6/src/stats_output/o3_runtime.rs \
  crates/rem6/src/stats_output/o3_runtime_issue.rs \
  crates/rem6/src/core_summary_json.rs \
  crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs
git commit -m "stats: expose scoped O3 issue arbitration"
```

### Task 5: Expose Scoped-Issue State in Mode-Transfer Evidence

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs`
- Modify: `crates/rem6/src/host_actions.rs`

- [ ] **Step 1: Add a failing transfer-summary row**

Add:

```text
rem6_run_host_switch_preserves_o3_scoped_issue_ticks
```

The transfer row must run a detailed baseline, schedule a detailed-to-timing
switch after the first younger issue and before the delayed load response, then
assert all four rows' issue/writeback/commit ticks equal the baseline. Assert
the transfer chunk has four ROB rows and one LSQ row. Require these new decoded
summary fields:

```text
stats_issue_cycles
stats_issued_rows
stats_resource_blocked_row_cycles
stats_dependency_blocked_row_cycles
stats_max_rows_per_cycle
```

Assert they match the source core's `O3RuntimeStats` at transfer time.

- [ ] **Step 2: Run and verify RED**

Run:

```bash
cargo test -p rem6 --test cli_run rem6_run_host_switch_preserves_o3_scoped_issue_ticks -- --exact --nocapture
```

Expected: FAIL because `Rem6HostO3RuntimeCheckpointChunkSummary` does not expose
the five issue-arbitration fields.

- [ ] **Step 3: Extend the existing decoded transfer summary**

Add five `Option<u64>` fields to `Rem6HostO3RuntimeCheckpointChunkSummary`, set
them to `None` on decode failure, populate them from decoded
`O3RuntimeStats`, include them in JSON, and register the same selected transfer
stats paths used by existing `stats_max_rob_occupancy` and
`stats_rename_map_entries` fields.

Keep schema-v7 live-data handoff unchanged; these are decoded O3 runtime chunk
summary fields, not a new handoff payload.

- [ ] **Step 4: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6 --test cli_run m5_host_actions::o3::scoped_issue -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::switch -- --nocapture
cargo test -p rem6 host_o3_runtime_checkpoint_chunk -- --nocapture
```

Expected: all selected lifecycle matrices pass.

```bash
git add crates/rem6/tests/cli_run/m5_host_actions/o3/scoped_issue.rs \
  crates/rem6/src/host_actions.rs
git commit -m "debug: expose scoped issue transfer state"
```

### Task 6: Protect Boundaries and Update the Migration Ledger

**Files:**
- Modify: `crates/rem6-cpu/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `crates/rem6/tests/source_policy/core_test_anchors.txt`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Add failing source-policy ownership tests**

Require `o3_runtime.rs` to declare `mod o3_runtime_issue;`, require the focused
module to contain `O3ScopedIssueScheduler`, `schedule_live_speculative_issues`,
and `record_issue_cycle`, and require `riscv_live_retire_window.rs` to call the
focused entry point without directly constructing `O3ScopedIssueScheduler`.

Require `stats_output/o3_runtime.rs` to declare/delegate to
`o3_runtime_issue.rs`, and keep both files under existing line limits.

Add exact anchors for:

```text
--riscv-o3-issue-width
riscv_o3_issue_width
/cores/0/o3_runtime/issue
sim.cpu0.o3.issue_cycles
sim.cpu0.o3.issued_rows
sim.cpu0.o3.resource_blocked_row_cycles
sim.cpu0.o3.dependency_blocked_row_cycles
sim.cpu0.o3.max_rows_per_cycle
sim.host_actions.stats_dump.cpu0.o3.issue_cycles
rem6_run_o3_scoped_issue_width_one_serializes_direct_window
rem6_run_o3_scoped_issue_width_two_coissues_cross_resource_rows
rem6_run_o3_scoped_issue_serializes_same_multiply_resource
rem6_run_o3_scoped_issue_dependency_waits_for_multiply
rem6_run_host_switch_preserves_o3_scoped_issue_ticks
rem6_run_o3_scoped_issue_checkpoint_boundary
rem6_run_timing_suppresses_o3_scoped_issue_surface
```

- [ ] **Step 2: Run source policy and verify RED**

Run:

```bash
cargo test -p rem6-cpu --test source_policy o3_runtime_issue -- --nocapture
cargo test -p rem6 --test source_policy o3_runtime_issue -- --nocapture
cargo test -p rem6 --test source_policy core_test_anchors -- --nocapture
```

Expected: FAIL until anchors and ownership assertions match the implementation.

- [ ] **Step 3: Update the 1,200-line ledger honestly**

Keep:

```text
### CPU Execution Models - 74% representative
**Score calculation:** 8 of 10 items have executable evidence, or 80% raw,
```

Keep both unchecked CPU checklist items. Add one bounded migrated-evidence
sentence covering:

- configured widths one, two, and four;
- load-head width reservation;
- cross-resource ALU/MUL co-issue;
- same-MUL resource serialization;
- producer-ready dependency blocking;
- direct and cache/fabric/DRAM rows;
- plan-derived JSON/text/stats-dump counters;
- version-22 stats checkpoint compatibility;
- mode-switch timing continuity;
- live rejection, drained cleanup, and timing suppression.

Remove only `scoped issue-width/resource contention` from `Next evidence`.
Retain fourth/deeper controls, indirect/unconditional controls, arbitrary mixed
memory/control windows, writeback-port contention, general IQ/wakeup/select,
restorable transport, and a general O3 engine.

Correct stale `Not migrated` wording that says only one- and two-branch windows;
it must acknowledge the existing three-deep direct-conditional evidence.

Preserve exactly 1,200 lines by consolidating prose rather than weakening the
source-policy requirement.

- [ ] **Step 4: Verify GREEN and commit**

Run:

```bash
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
```

Expected: all source-policy tests pass and the ledger is exactly 1,200 lines.

```bash
git add crates/rem6-cpu/tests/source_policy.rs \
  crates/rem6/tests/source_policy.rs \
  crates/rem6/tests/source_policy/core_test_anchors.txt \
  docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record scoped O3 issue evidence"
```

### Task 7: Full Verification, Review, and Push

**Files:**
- Review all files changed since design commit `877a8713`.

- [ ] **Step 1: Format and run focused suites**

```bash
cargo fmt --all -- --check
cargo test -p rem6-cpu scoped_issue_ --lib -- --nocapture
cargo test -p rem6-cpu o3_runtime_checkpoint --lib -- --nocapture
cargo test -p rem6 --test cli_run riscv_o3_issue_width -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::scoped_issue -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::lsq_fu_window -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::predicted_control -- --nocapture
cargo test -p rem6-cpu --test source_policy -- --nocapture
cargo test -p rem6 --test source_policy -- --nocapture
```

Expected: all commands exit zero.

- [ ] **Step 2: Run broad suites**

```bash
cargo test -p rem6-cpu --all-targets
cargo test -p rem6 --test cli_run
cargo test --workspace --all-targets
```

Expected: all commands exit zero within the goal's timeout limits.

- [ ] **Step 3: Run mechanical closeout checks**

```bash
cargo fmt --all -- --check
git diff --check
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
git status --short --branch
```

Expected: formatting/diff checks pass; only intended committed state remains.

- [ ] **Step 4: Dispatch high-intensity read-only review**

Ask an xhigh reviewer to inspect the complete range from `877a8713` through
`HEAD` for:

- scheduler authority versus timestamp arithmetic;
- width/resource/dependency correctness;
- partial re-entry overbooking;
- architectural publication before retirement;
- rollback, transfer, checkpoint, and timing-mode regressions;
- checkpoint version compatibility;
- dead/orphan APIs or duplicate policy;
- source-policy and ledger honesty.

Fix every concrete finding, rerun affected tests, and commit fixes with a
behavior-specific English message.

- [ ] **Step 5: Push and verify the remote**

```bash
git push origin main
git rev-parse HEAD
git rev-parse origin/main
git status --short --branch
```

Expected: local and remote hashes match and the worktree is clean.
