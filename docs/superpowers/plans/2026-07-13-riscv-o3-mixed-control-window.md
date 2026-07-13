# RISC-V O3 Mixed Control Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the bounded detailed RISC-V scalar-memory O3 window with one terminal direct conditional branch, preserving ordered retirement, branch squash evidence, wrong-path suppression, mode handoff, and checkpoint quiescence through real CLI execution.

**Architecture:** `riscv_o3_window_policy.rs` classifies a direct conditional branch as a terminal no-rename row, while the existing live-window runtime stages it in the ROB without speculative execution. Normal branch retirement remains the only redirect and statistics authority; existing O3 runtime and live-data handoff payloads carry the generic row through mode transfer, and checkpoint capture remains closed while non-restorable scalar-memory state is live.

**Tech Stack:** Rust 2021 workspace, `rem6-cpu`, `rem6-system`, `rem6` CLI integration tests, RISC-V instruction encoding helpers, O3 runtime snapshots, host execution-mode actions, checkpoint banks, Cargo test and source-policy gates.

---

## File Map

- Modify `crates/rem6-cpu/src/riscv_o3_window_policy.rs`: classify direct conditional branches as terminal live-window rows.
- Modify `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`: stop detailed fetch-ahead after a decoded terminal branch.
- Modify `crates/rem6-cpu/src/riscv_live_retire_window.rs`: retain the terminal branch in the accepted completed-fetch prefix without speculatively executing it.
- Modify `crates/rem6-cpu/src/o3_runtime_live_window.rs`: stage the no-rename branch ROB row and stop younger allocation.
- Modify `crates/rem6-cpu/src/riscv_execute.rs` only if a red integration test proves branch statistics are cleared before redirect observation.
- Modify `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs` only if existing generic younger-row validation rejects the terminal no-rename row.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/lsq_fu_branch.rs`: own the direct, hierarchy, resident, timing, and checkpoint CLI matrix plus shared fixture helpers.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`: register the focused mixed-window module.
- Create `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/scalar_load_branch.rs`: own detailed-to-timing transfer evidence for the mixed window.
- Modify `crates/rem6/tests/cli_run/m5_host_actions/o3/switch.rs`: register the switch test module.
- Modify `crates/rem6/tests/source_policy.rs`: require migration-ledger evidence fields to start on their own lines.
- Modify `docs/architecture/gem5-to-rem6-migration.md`: record the executable matrix without changing 74%, 8/10, or any checklist state.

### Task 1: Admit And Stage A Terminal Direct Conditional Branch

**Files:**
- Modify: `crates/rem6-cpu/src/riscv_o3_window_policy.rs`
- Modify: `crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs`
- Modify: `crates/rem6-cpu/src/riscv_live_retire_window.rs`
- Modify: `crates/rem6-cpu/src/o3_runtime_live_window.rs`
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/lsq_fu_branch.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3.rs`

- [ ] **Step 1: Add the failing policy test**

In `riscv_o3_window_policy.rs`, extend the test import to
`use rem6_isa_riscv::{Immediate, MemoryWidth};`, then add these helpers and
tests inside the existing `tests` module:

```rust
fn conditional_branches() -> [RiscvInstruction; 6] {
    let rs1 = Register::new(4).unwrap();
    let rs2 = Register::new(6).unwrap();
    let offset = Immediate::new(8);
    [
        RiscvInstruction::Beq { rs1, rs2, offset },
        RiscvInstruction::Bne { rs1, rs2, offset },
        RiscvInstruction::Blt { rs1, rs2, offset },
        RiscvInstruction::Bge { rs1, rs2, offset },
        RiscvInstruction::Bltu { rs1, rs2, offset },
        RiscvInstruction::Bgeu { rs1, rs2, offset },
    ]
}

fn load_x4() -> RiscvInstruction {
    RiscvInstruction::Load {
        rd: Register::new(4).unwrap(),
        rs1: Register::new(10).unwrap(),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    }
}

#[test]
fn scalar_memory_prefix_admits_direct_conditional_as_terminal_control() {
    for branch in conditional_branches() {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window.classify_younger(addi(5, 0)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            window.classify_younger(addi(6, 5)),
            RiscvScalarIntegerYoungerDecision::AdmitContinue
        );
        assert_eq!(
            window.classify_younger(branch),
            RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
        );
        assert!(window.is_full());
    }
}

#[test]
fn scalar_memory_prefix_rejects_nonconditional_control_and_system_rows() {
    for instruction in [
        RiscvInstruction::Jal {
            rd: Register::new(0).unwrap(),
            offset: Immediate::new(8),
        },
        RiscvInstruction::Jalr {
            rd: Register::new(0).unwrap(),
            rs1: Register::new(1).unwrap(),
            offset: Immediate::new(0),
        },
        RiscvInstruction::Ecall,
        load_x4(),
    ] {
        let mut window = scalar_load_window(4);
        assert_eq!(
            window.classify_younger(instruction),
            RiscvScalarIntegerYoungerDecision::Reject
        );
    }
}
```

- [ ] **Step 2: Add the failing live-staging test**

In `o3_runtime_live_window.rs`, add:

```rust
#[test]
fn scalar_load_head_stages_terminal_branch_without_rename_or_younger_rows() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let first = addi(5, 0);
    let second = addi(6, 5);
    let branch = RiscvInstruction::Beq {
        rs1: Register::new(4).unwrap(),
        rs2: Register::new(6).unwrap(),
        offset: Immediate::new(8),
    };
    let rejected = addi(7, 0);
    assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));

    runtime.stage_live_scalar_memory_younger_window(
        load.fetch().request_id(),
        [
            (Address::new(0x8004), first),
            (Address::new(0x8008), second),
            (Address::new(0x800c), branch),
            (Address::new(0x8010), rejected),
        ],
    );

    let rob = runtime.snapshot().reorder_buffer();
    assert_eq!(rob.len(), 4);
    assert_eq!(rob[3].pc(), Address::new(0x800c));
    assert_eq!(rob[3].destination(), None);
    assert_eq!(rob[3].rename_destination(), None);
    assert!(rob[3].is_live_staged());
    assert!(!rob[3].is_ready());
    assert!(rob.iter().all(|row| row.pc() != Address::new(0x8010)));
    assert!(runtime
        .live_speculative_issue_candidate(Address::new(0x800c), branch)
        .is_none());
}
```

- [ ] **Step 3: Add the failing top-level resident-row test**

Register a new module in `m5_host_actions/o3.rs`:

```rust
#[path = "o3/lsq_fu_branch.rs"]
mod lsq_fu_branch;
```

Create `lsq_fu_branch.rs` with these constants and the deterministic fixture:

```rust
use super::*;

pub(super) const LOAD_PC: &str = "0x80000014";
pub(super) const FIRST_ALU_PC: &str = "0x80000018";
pub(super) const SECOND_ALU_PC: &str = "0x8000001c";
pub(super) const BRANCH_PC: &str = "0x80000020";
pub(super) const WRONG_STORE_PC: &str = "0x80000024";
pub(super) const TARGET_STORE_PC: &str = "0x80000028";
const DATA_ADDRESS: &str = "0x80000080";
const FINAL_MEMORY: &str = "2a0000001000000088776655";

pub(super) fn mixed_load_alu_branch_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 5, 0x17),
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13),
        i_type(42, 0, 0x0, 11, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 5, 0b010, 12, 0x03),
        i_type(5, 0, 0x0, 13, 0x13),
        i_type(11, 13, 0x0, 14, 0x13),
        b_type(8, 11, 12, 0b000),
        s_type(8, 0, 5, 0b010),
        s_type(4, 14, 5, 0b010),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
        i_type(0, 0, 0x0, 0, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([42, 0, 0x5566_7788]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
```

Add these reusable command and event helpers after the fixture:

```rust
pub(super) fn mixed_branch_command(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
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
        "--riscv-o3-scalar-memory-depth",
        "4",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--m5-switch-cpu-mode",
        execution_mode,
        "--dump-memory",
        "0x80000080:12",
    ]);
    command
}

pub(super) fn run_mixed_branch_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    execution_mode: &str,
    extra_args: &[&str],
) -> Value {
    let mut command = mixed_branch_command(path, memory_system, max_tick, execution_mode);
    command.args(extra_args);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{memory_system} {execution_mode}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid mixed-window JSON: {error}"))
}

pub(super) fn event_at_pc_if_present<'a>(json: &'a Value, pc: &str) -> Option<&'a Value> {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
}

pub(super) fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    event_at_pc_if_present(json, pc)
        .unwrap_or_else(|| panic!("missing O3 event at {pc}: {json}"))
}

pub(super) fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing {field}: {event}"))
}
```

The command helper always passes:

```text
run --isa riscv --execute --stats-format json
--debug-flags O3,Data,Fetch,Memory,HostAction
--riscv-o3-scalar-memory-depth 4
--memory-route-delay 16
--dump-memory 0x80000080:12
```

The helper accepts `memory_system`, `max_tick`, the `--m5-switch-cpu-mode`
target (`detailed` or `timing`), and extra host-action arguments. It must return
the configured `Command` so switch and checkpoint tests can reuse the same
binary and arguments without duplicating the runtime surface.

Add this first integration test:

```rust
#[test]
fn rem6_run_o3_mixed_load_alu_branch_exposes_terminal_resident_row_direct() {
    let path = mixed_load_alu_branch_binary("o3-mixed-load-alu-branch-resident-direct");
    let completed = run_mixed_branch_json(&path, "direct", 1_500, "detailed", &[]);
    let load = event_at_pc(&completed, LOAD_PC);
    let issue = event_u64(load, "issue_tick");
    let response = event_u64(load, "lsq_data_response_tick");
    let stop_tick = issue + (response - issue) / 2;
    assert!(issue < stop_tick && stop_tick < response);

    let resident = run_mixed_branch_json(&path, "direct", stop_tick, "detailed", &[]);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .expect("mixed window ROB rows");
    assert_eq!(
        rob.iter()
            .map(|row| row.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        [LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC, BRANCH_PC]
    );
    assert_eq!(rob[3].pointer("/destination"), Some(&Value::Null));
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
}
```

- [ ] **Step 4: Run the red tests**

Run:

```text
cargo test -p rem6-cpu scalar_memory_prefix_admits_direct_conditional_as_terminal_control -- --exact
cargo test -p rem6-cpu scalar_load_head_stages_terminal_branch_without_rename_or_younger_rows -- --exact
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_load_alu_branch_exposes_terminal_resident_row_direct -- --exact --nocapture
```

Expected: compilation fails for the missing
`AdmitTerminalControl` variant, then the CLI assertion fails because the branch
is not present in the resident ROB.

- [ ] **Step 5: Implement terminal-control classification**

In `riscv_o3_window_policy.rs`, add:

```rust
const fn scalar_integer_terminal_control(instruction: RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::Beq { .. }
            | RiscvInstruction::Bne { .. }
            | RiscvInstruction::Blt { .. }
            | RiscvInstruction::Bge { .. }
            | RiscvInstruction::Bltu { .. }
            | RiscvInstruction::Bgeu { .. }
    )
}
```

Extend the decision enum:

```rust
pub(crate) enum RiscvScalarIntegerYoungerDecision {
    AdmitContinue,
    AdmitStop,
    AdmitTerminalControl,
    Reject,
}
```

Immediately after the full-window check in `classify_younger`, add:

```rust
if scalar_integer_terminal_control(instruction) {
    self.rows += 1;
    return RiscvScalarIntegerYoungerDecision::AdmitTerminalControl;
}
```

- [ ] **Step 6: Apply the terminal decision to fetch and staging consumers**

Update every exhaustive decision match:

1. In both candidate loops in `riscv_fetch_ahead/detailed_o3.rs`, treat
   `AdmitTerminalControl` as `Blocked` after the decoded branch is present.
2. In `accepted_scalar_integer_younger_window` in
   `riscv_live_retire_window.rs`, push the row and break for both
   `AdmitStop` and `AdmitTerminalControl`.
3. In `stage_live_scalar_memory_younger_window` in
   `o3_runtime_live_window.rs`, stage the row, record its sequence, and break
   for both terminal decisions.

Use this match shape in the two staging helpers:

```rust
match window.classify_younger(instruction) {
    RiscvScalarIntegerYoungerDecision::AdmitContinue => {
        accepted.push(row);
    }
    RiscvScalarIntegerYoungerDecision::AdmitStop
    | RiscvScalarIntegerYoungerDecision::AdmitTerminalControl => {
        accepted.push(row);
        break;
    }
    RiscvScalarIntegerYoungerDecision::Reject => break,
}
```

Do not change `live_speculative_issue_candidate`; its scalar-ALU-only filter is
the required branch-execution suppression boundary.

- [ ] **Step 7: Run focused and integration tests**

Run:

```text
cargo test -p rem6-cpu scalar_memory_prefix_admits_direct_conditional_as_terminal_control -- --exact
cargo test -p rem6-cpu scalar_load_head_stages_terminal_branch_without_rename_or_younger_rows -- --exact
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_load_alu_branch_exposes_terminal_resident_row_direct -- --exact --nocapture
```

Expected: all pass; the resident branch row has no rename destination and no
row after it is staged.

- [ ] **Step 8: Commit the behavior boundary**

```text
git add crates/rem6-cpu/src/riscv_o3_window_policy.rs \
        crates/rem6-cpu/src/riscv_fetch_ahead/detailed_o3.rs \
        crates/rem6-cpu/src/riscv_live_retire_window.rs \
        crates/rem6-cpu/src/o3_runtime_live_window.rs \
        crates/rem6/tests/cli_run/m5_host_actions/o3.rs \
        crates/rem6/tests/cli_run/m5_host_actions/o3/lsq_fu_branch.rs
git commit -m "cpu: stage terminal branches in mixed O3 windows"
```

### Task 2: Complete The Direct, Hierarchy, And Timing Matrix

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/lsq_fu_branch.rs`
- Modify: `crates/rem6-cpu/src/riscv_execute.rs` only if event ordering is wrong

- [ ] **Step 1: Add final-state and branch-evidence assertions**

Add this shared completed-run assertion:

```rust
pub(super) fn assert_completed_mixed_branch_window(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/simulation/stop_code").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(FINAL_MEMORY)
    );
    for (register, value) in [("x12", "0x2a"), ("x13", "0x5"), ("x14", "0x10")] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value)
        );
    }

    let load = event_at_pc(json, LOAD_PC);
    let first = event_at_pc(json, FIRST_ALU_PC);
    let second = event_at_pc(json, SECOND_ALU_PC);
    let branch = event_at_pc(json, BRANCH_PC);
    assert_eq!(branch.pointer("/branch_event").and_then(Value::as_bool), Some(true));
    assert_eq!(
        branch.pointer("/branch_kind").and_then(Value::as_str),
        Some("direct_conditional")
    );
    assert_eq!(
        branch.pointer("/branch_predicted_taken").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        branch.pointer("/branch_resolved_taken").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        branch.pointer("/branch_mispredicted").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(branch.pointer("/branch_squash").and_then(Value::as_bool), Some(true));
    assert_eq!(
        branch.pointer("/branch_resolved_target").and_then(Value::as_str),
        Some(TARGET_STORE_PC)
    );
    assert_eq!(
        branch.pointer("/branch_squashed_target").and_then(Value::as_str),
        Some(WRONG_STORE_PC)
    );

    let response_tick = event_u64(load, "lsq_data_response_tick");
    assert!(event_u64(first, "issue_tick") < response_tick);
    assert!(event_u64(second, "issue_tick") < response_tick);
    assert!(event_u64(branch, "issue_tick") >= response_tick);
    let ordered = [load, first, second, branch];
    assert!(ordered.windows(2).all(|events| {
        event_u64(events[0], "commit_tick") <= event_u64(events[1], "commit_tick")
    }));
    assert!(event_at_pc_if_present(json, WRONG_STORE_PC).is_none());

    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .expect("mixed-window Data trace");
    assert_eq!(
        data.iter()
            .filter(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("store")
                    && record.pointer("/address").and_then(Value::as_str)
                        == Some("0x80000084")
            })
            .count(),
        1
    );
    assert!(data.iter().all(|record| {
        record.pointer("/address").and_then(Value::as_str) != Some("0x80000088")
    }));
    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("mixed-window Memory trace");
    assert!(memory.iter().all(|record| {
        !(record.pointer("/channel").and_then(Value::as_str) == Some("data")
            && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
            && record.pointer("/address").and_then(Value::as_str) == Some("0x80000088"))
    }));

    assert_json_stat(json, "sim.cpu0.o3.max_rob_occupancy", "Count", 4, "monotonic");
    assert_json_stat(json, "sim.cpu0.o3.max_lsq_occupancy", "Count", 1, "monotonic");
    assert_json_stat(json, "sim.cpu0.o3.branch_event.squashes", "Count", 1, "monotonic");
    assert_json_stat(
        json,
        "sim.cpu0.o3.branch_event.squash_kind.direct_conditional",
        "Count",
        1,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.ftq.squashes_0::DirectCond",
        "Count",
        1,
        "monotonic",
    );
}
```

The two ALUs must issue before the load response, the branch must issue no
earlier than the load response, and commit ticks must remain nondecreasing.

- [ ] **Step 2: Assert wrong-path suppression**

Require all of these:

1. No O3 event at `WRONG_STORE_PC`.
2. No Data trace row at address `0x80000088`.
3. No data-channel Memory request whose address is `0x80000088`.
4. The final third data word remains `0x55667788`.
5. The target store at `0x80000084` appears exactly once.

Do not accept either branch prediction outcome. The fixture must keep the
single deterministic not-taken prediction followed by a taken resolution.

- [ ] **Step 3: Add direct and cache/fabric/DRAM completed rows**

Add these tests using the shared final-state assertion from Step 1:

```rust
#[test]
fn rem6_run_o3_mixed_load_alu_branch_squash_direct() {
    let path = mixed_load_alu_branch_binary("o3-mixed-load-alu-branch-direct");
    let json = run_mixed_branch_json(&path, "direct", 1_500, "detailed", &[]);
    assert_completed_mixed_branch_window(&json);
    assert!(json
        .pointer("/memory_resources/transport/data/activity")
        .and_then(Value::as_u64)
        .is_some_and(|value| value > 0));
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert_eq!(json.pointer(pointer).and_then(Value::as_u64), Some(0));
    }
}

#[test]
fn rem6_run_o3_mixed_load_alu_branch_squash_cache_fabric_dram() {
    let path = mixed_load_alu_branch_binary("o3-mixed-load-alu-branch-hierarchy");
    let json = run_mixed_branch_json(
        &path,
        "cache-fabric-dram",
        1_500,
        "detailed",
        &[],
    );
    assert_completed_mixed_branch_window(&json);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "missing hierarchy activity at {pointer}: {json}"
        );
    }
}
```

For the direct row, require data transport activity and zero cache, fabric, and
DRAM activity. For the hierarchy row, require all four activity counters to be
nonzero in both structured JSON and `sim.memory.resources.*` stats.

- [ ] **Step 4: Add the timing-mode suppression row**

Run the same binary with `--m5-switch-cpu-mode timing` and assert the same final
registers and memory, then require:

```rust
assert!(json.pointer("/cores/0/o3_runtime").is_none());
assert!(json
    .pointer("/debug/o3_trace")
    .and_then(Value::as_array)
    .is_some_and(Vec::is_empty));
```

Reject every stat beginning with `sim.cpu0.o3.` or the gem5 O3 alias prefixes
`system.cpu.rob.`, `system.cpu.lsq0.`, `system.cpu.rename.`, `system.cpu.iq.`,
`system.cpu.iew.`, `system.cpu.commit.`, or `system.cpu.ftq.`.

- [ ] **Step 5: Run the matrix red, fix only proven retirement ordering gaps, and rerun**

Run:

```text
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_load_alu_branch -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_timing_suppresses_o3_mixed_load_alu_branch_window -- --exact --nocapture
```

If branch statistics are absent because `riscv_execute.rs` discards staged rows
before `record_o3_retired_instruction_with_trace`, reorder the existing calls so
the current branch event is recorded before `discard_live_staged_instructions`.
Do not create a second branch-statistics path.

Expected after the fix: four detailed tests pass, timing emits no O3 surface,
and wrong-path memory remains untouched.

- [ ] **Step 6: Commit the runtime matrix**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/lsq_fu_branch.rs \
        crates/rem6-cpu/src/riscv_execute.rs
git commit -m "test: cover mixed O3 branch windows"
```

Omit `riscv_execute.rs` from the commit if no production correction is needed.

### Task 3: Preserve The Mixed Window Across Mode Handoff

**Files:**
- Create: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch/scalar_load_branch.rs`
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/switch.rs`
- Modify: `crates/rem6-cpu/src/riscv_execution_mode_handoff.rs` only if current validation fails

- [ ] **Step 1: Register the focused switch module**

Add to `switch.rs`:

```rust
#[path = "switch/scalar_load_branch.rs"]
mod scalar_load_branch;
```

Expose only these helpers from `lsq_fu_branch.rs` with `pub(super)` visibility:
the fixture path builder, command builder, JSON decoder, PC constants, event
lookup, and numeric event-field lookup.

- [ ] **Step 2: Add the failing handoff test**

In `scalar_load_branch.rs`:

```rust
use super::*;
use super::super::lsq_fu_branch::{
    assert_completed_mixed_branch_window, event_at_pc, event_at_pc_if_present,
    event_u64, mixed_load_alu_branch_binary, run_mixed_branch_json, BRANCH_PC,
    FIRST_ALU_PC, LOAD_PC, SECOND_ALU_PC, WRONG_STORE_PC,
};

#[test]
fn rem6_run_host_switch_transfers_o3_mixed_load_alu_branch_until_squash() {
    let path = mixed_load_alu_branch_binary("host-switch-o3-mixed-load-alu-branch");
    let baseline = run_mixed_branch_json(&path, "direct", 1_500, "detailed", &[]);
    let load = event_at_pc(&baseline, LOAD_PC);
    let switch_tick = event_u64(load, "issue_tick")
        + (event_u64(load, "lsq_data_response_tick") - event_u64(load, "issue_tick")) / 2;
    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let json = run_mixed_branch_json(
        &path,
        "direct",
        1_500,
        "detailed",
        &["--host-switch-cpu-mode", &switch_arg],
    );

    assert_completed_mixed_branch_window(&json);
    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .expect("execution-mode switches");
    let timing_switch = switches
        .iter()
        .find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                && switch.pointer("/previous_mode").and_then(Value::as_str)
                    == Some("detailed")
        })
        .expect("detailed-to-timing switch");
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("mixed-window state transfer");
    assert_eq!(
        transfer.pointer("/live_data_handoff").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(transfer.pointer("/restorable").and_then(Value::as_bool), Some(false));

    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime.pointer("/snapshot_rob_entries").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        runtime.pointer("/snapshot_lsq_entries").and_then(Value::as_u64),
        Some(1)
    );
    let handoff = super::scalar_load::transfer_handoff_chunk(transfer, "cpu0");
    assert_eq!(handoff.pointer("/resident_rows").and_then(Value::as_u64), Some(1));
    assert_eq!(handoff.pointer("/younger_rows").and_then(Value::as_u64), Some(3));
    assert_eq!(
        handoff.pointer("/outstanding_requests").and_then(Value::as_u64),
        Some(1)
    );

    for pc in [LOAD_PC, FIRST_ALU_PC, SECOND_ALU_PC, BRANCH_PC] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&json, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(event_u64(actual, field), event_u64(expected, field));
        }
    }
    assert!(event_at_pc_if_present(&json, WRONG_STORE_PC).is_none());
}
```

Assert the detailed-to-timing switch occurs between load issue and response.
From its `state_transfer`:

1. `live_data_handoff == true`
2. `restorable == false`
3. decoded `o3-runtime-state.snapshot_rob_entries == 4`
4. decoded `o3-runtime-state.snapshot_lsq_entries == 1`
5. decoded `o3-live-data-handoff.resident_rows == 1`
6. decoded `o3-live-data-handoff.younger_rows == 3`
7. decoded `o3-live-data-handoff.outstanding_requests == 1`

Compare `issue_tick`, `writeback_tick`, and `commit_tick` for `LOAD_PC`, both ALU
PCs, and `BRANCH_PC` against the no-switch baseline. Require the same branch
squash stats and no event at `WRONG_STORE_PC`.

- [ ] **Step 3: Run the handoff test red**

Run:

```text
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_o3_mixed_load_alu_branch_until_squash -- --exact --nocapture
```

Expected: either PASS with the existing generic handoff, or FAIL at a precise
shape/row validation boundary. A pass is acceptable evidence that no production
schema change is needed.

- [ ] **Step 4: Fix only a demonstrated generic-row validation gap**

If the test fails in `riscv_execution_mode_handoff.rs`, preserve schema version
6 and the existing `younger_rows` count. Strengthen validation so a generic
no-rename younger ROB row is legal without adding branch-specific fields. Keep
all current maximum-row, sequence-order, target, ownership, and non-restorable
checks.

Do not add a new payload version unless the current encoded counts cannot
represent the row, which should be disproven by the existing generic ROB
snapshot.

- [ ] **Step 5: Run handoff and existing scalar-load switch coverage**

Run:

```text
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_o3_mixed_load_alu_branch_until_squash -- --exact --nocapture
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_outstanding_o3_scalar_load -- --nocapture
cargo test -p rem6-cpu riscv_execution_mode_handoff -- --nocapture
```

Expected: mixed and existing handoffs pass with schema version 6.

- [ ] **Step 6: Commit the handoff evidence**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/switch.rs \
        crates/rem6/tests/cli_run/m5_host_actions/o3/switch/scalar_load_branch.rs \
        crates/rem6-cpu/src/riscv_execution_mode_handoff.rs
git commit -m "test: cover mixed O3 branch handoff"
```

Omit the production file if the existing handoff already passes.

### Task 4: Lock The Checkpoint Quiescence Boundary

**Files:**
- Modify: `crates/rem6/tests/cli_run/m5_host_actions/o3/lsq_fu_branch.rs`
- Modify: `crates/rem6-system/tests/riscv_checkpoint.rs` only if CLI failure cannot prove preflight ordering

- [ ] **Step 1: Add a live-checkpoint rejection test**

Use the completed baseline to select the same midpoint between load issue and
response where the four-row resident test succeeds. Run the command with:

```text
--host-checkpoint <midpoint>:mixed-branch-live
```

Assert the process fails and stderr contains:

```text
checkpoint component is not quiescent: cpu0
```

The test must first prove the selected tick lies between issue and response and
that the separate tick-limited run contains all four expected ROB PCs. This
prevents a false-positive rejection before the branch becomes resident.

- [ ] **Step 2: Add a post-drain checkpoint success test**

From the completed baseline, choose `checkpoint_tick = branch_commit_tick + 1`.
Run with:

```text
--host-checkpoint <checkpoint_tick>:mixed-branch-drained
```

Assert:

1. process success and `stopped_by_host`
2. one checkpoint with label `mixed-branch-drained`
3. CPU0 checkpoint component exists
4. CPU0 has no `o3-live-data-handoff` chunk
5. decoded O3 runtime snapshot has zero live ROB and LSQ rows
6. final memory and branch squash evidence still match the baseline

- [ ] **Step 3: Run both checkpoint tests**

Run:

```text
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_branch_checkpoint -- --nocapture
```

Expected: live capture fails before checkpoint output, while post-drain capture
succeeds without stale mixed-window authority.

- [ ] **Step 4: Strengthen system preflight evidence only if needed**

If the CLI error does not distinguish preflight from partial registry writes,
extend `riscv_core_checkpoint_rejects_live_scalar_memory_before_any_bank_writes`
in `crates/rem6-system/tests/riscv_checkpoint.rs` so CPU1 stages load, ALU, ALU,
and branch rows before `capture_all_into`. Keep the existing assertions that
both CPU component chunks remain absent after `ComponentNotQuiescent`.

- [ ] **Step 5: Run checkpoint regressions**

Run:

```text
cargo test -p rem6-system riscv_core_checkpoint_rejects_live_scalar_memory_before_any_bank_writes -- --exact --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_branch_checkpoint -- --nocapture
cargo test -p rem6 --test cli_run m5_host_actions::o3::checkpoint -- --nocapture
```

Expected: all pass and no new checkpoint schema is introduced.

- [ ] **Step 6: Commit the checkpoint evidence**

```text
git add crates/rem6/tests/cli_run/m5_host_actions/o3/lsq_fu_branch.rs \
        crates/rem6-system/tests/riscv_checkpoint.rs
git commit -m "test: cover mixed O3 branch checkpoints"
```

Omit the system test file if the existing preflight test remains sufficient.

### Task 5: Record Honest Evidence And Harden The Ledger Boundary

**Files:**
- Modify: `crates/rem6/tests/source_policy.rs`
- Modify: `docs/architecture/gem5-to-rem6-migration.md`

- [ ] **Step 1: Make the malformed evidence field fail source policy**

In `gem5_migration_sections_are_auditable`, replace the loose field-presence
check with a line-owned check:

```rust
for required in ["**Migrated:**", "**Not migrated:**", "**Next evidence:**"] {
    assert!(
        body.lines().any(|line| line.starts_with(required)),
        "`{heading}` is missing standalone field `{required}`"
    );
}
```

- [ ] **Step 2: Run source policy and verify the CPU section fails**

Run:

```text
cargo test -p rem6 --test source_policy gem5_migration_sections_are_auditable -- --exact --nocapture
```

Expected: FAIL because the CPU `**Next evidence:**` marker is embedded after
other prose instead of starting its own line.

- [ ] **Step 3: Update the CPU evidence without changing score or checklist**

Edit only the CPU component section and `tests/gem5/cpu_tests` table row:

1. Add the new direct, cache/fabric/DRAM, resident, timing, handoff, and
   checkpoint test names to the existing standalone-load/ALU evidence paragraph.
2. State the exact four-row ROB, one-row LSQ, no-rename terminal branch,
   deterministic direct-conditional squash, wrong-path store suppression,
   schema-v6 handoff, and checkpoint rejection/drain evidence.
3. Keep `8 of 10`, `80% raw`, `74% representative`, and both unchecked items.
4. Move the existing CPU `**Next evidence:**` marker to the start of its own
   line. Preserve the direct-call/return evidence by folding its sentence into
   the preceding `**Migrated:**` prose rather than deleting required anchors.
5. Keep the ledger exactly 1,200 lines.
6. Keep the remaining boundary explicit: predicted-path descendants, general
   branch issue, arbitrary mixed memory/control windows, restorable transport
   ownership, and a general O3 engine remain open.

- [ ] **Step 4: Verify ledger mechanics and source policy**

Run:

```text
test "$(wc -l < docs/architecture/gem5-to-rem6-migration.md)" -eq 1200
cargo test -p rem6 --test source_policy gem5_migration_sections_are_auditable -- --exact --nocapture
cargo test -p rem6 --test source_policy gem5_migration_doc_tracks_core_test_anchors -- --exact --nocapture
```

Expected: all pass; component score remains 74% representative.

- [ ] **Step 5: Commit documentation and policy cleanup**

```text
git add crates/rem6/tests/source_policy.rs \
        docs/architecture/gem5-to-rem6-migration.md
git commit -m "docs: record mixed O3 branch evidence"
```

### Task 6: Final Verification And Review

**Files:**
- Review all files changed by Tasks 1-5.

- [ ] **Step 1: Run focused suites**

```text
cargo fmt --all -- --check
cargo test -p rem6-cpu --quiet
cargo test -p rem6-system --quiet
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_load_alu_branch -- --nocapture
cargo test -p rem6 --test cli_run rem6_run_host_switch_transfers_o3_mixed_load_alu_branch_until_squash -- --exact --nocapture
cargo test -p rem6 --test cli_run rem6_run_o3_mixed_branch_checkpoint -- --nocapture
cargo test -p rem6 --test source_policy --quiet
cargo test -p rem6-cpu --test source_policy --quiet
git diff --check
```

Expected: all pass.

- [ ] **Step 2: Run full integration verification**

```text
cargo test -p rem6 --test cli_run --quiet
cargo test --workspace --all-targets --quiet
```

Expected: all tests pass, including all 1,213-plus CLI cases after the new
matrix is added.

- [ ] **Step 3: Request mandatory read-only reviews**

Dispatch a spec-compliance reviewer first, then a code-quality reviewer, and
finally one high-intensity read-only whole-diff reviewer. Require explicit
findings for:

1. accidental speculative branch execution
2. younger-row admission after the terminal branch
3. branch statistics lost before redirect cleanup
4. wrong-path data side effects
5. handoff schema expansion without need
6. checkpoint writes before quiescence rejection
7. inflated migration score or weakened assertions
8. duplicate branch/squash authority or dead compatibility code

Fix every Critical or Important finding and rerun the affected verification.

- [ ] **Step 4: Verify branch and remote state before push**

```text
git status --short --branch
git log --oneline --decorate -8
git diff origin/main...HEAD --check
```

Expected: clean worktree, only the planned commits ahead of `origin/main`, and
no whitespace errors.

- [ ] **Step 5: Push the completed increment**

```text
git push origin main
git status --short --branch
test "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)"
```

Expected: local `main` and `origin/main` match.
