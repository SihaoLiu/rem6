use std::process::Command;

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_delays_architectural_visibility_until_scheduled_commit_stage() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-scheduled-commit", &elf);

    for (memory_system, cores, completed_tick_limit, expected_after_commit_retirements) in [
        ("direct", 1, 120, 2),
        ("direct", 2, 120, 2),
        ("cache-fabric-dram", 1, 600, 1),
    ] {
        let completed = run_pipeline_timing(&path, cores, completed_tick_limit, memory_system);
        let first_fetch_response_tick = completed
            .pointer("/debug/memory_trace")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .filter(|record| record.pointer("/channel").and_then(Value::as_str) == Some("fetch"))
            .filter(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("response_arrived")
            })
            .filter_map(|record| record.pointer("/tick").and_then(Value::as_u64))
            .min()
            .expect("completed run should expose the first fetch response");
        let commit_ready_tick = first_fetch_response_tick + 4;

        let before_commit = run_pipeline_timing(&path, cores, commit_ready_tick, memory_system);
        assert_eq!(
            before_commit
                .pointer("/simulation/status")
                .and_then(Value::as_str),
            Some("stopped_at_tick_limit")
        );
        assert_eq!(
            before_commit
                .pointer("/simulation/final_tick")
                .and_then(Value::as_u64),
            Some(commit_ready_tick)
        );
        for cpu in 0..cores {
            let core = &before_commit["cores"][cpu];
            assert_eq!(core["committed_instructions"].as_u64(), Some(0));
            assert_eq!(core.pointer("/registers/x5"), None);
            assert_eq!(
                core.pointer("/in_order_pipeline/stage_in_flight/commit")
                    .and_then(Value::as_u64),
                Some(1)
            );
            assert_eq!(
                core.pointer("/in_order_pipeline/stage_retired/commit")
                    .and_then(Value::as_u64),
                Some(0)
            );
        }

        let after_commit = run_pipeline_timing(&path, cores, commit_ready_tick + 1, memory_system);
        for cpu in 0..cores {
            let core = &after_commit["cores"][cpu];
            assert_eq!(
                core["committed_instructions"].as_u64(),
                Some(expected_after_commit_retirements)
            );
            assert_eq!(
                core.pointer("/registers/x5").and_then(Value::as_str),
                Some("0x7")
            );
            assert_eq!(
                core.pointer("/in_order_pipeline/stage_retired/commit")
                    .and_then(Value::as_u64),
                Some(expected_after_commit_retirements)
            );
        }
    }
}

#[test]
fn rem6_run_keeps_direct_single_core_mul_invisible_through_execute_wait() {
    assert_execute_wait_visibility(ExecuteWaitVisibilityCase {
        name: "pipeline-mul-direct-single-core-execute-wait-visibility",
        operation: ScheduledIntegerOperation::Mul,
        memory_system: "direct",
        cores: 1,
        completed_tick_limit: 180,
        expected_execute_wait_cycles: 2,
        expected_last_execute_wait_cycle: 17,
        expected_last_execute_wait_tick: 12,
    });
}

#[test]
fn rem6_run_keeps_direct_two_core_div_invisible_through_execute_wait() {
    assert_execute_wait_visibility(ExecuteWaitVisibilityCase {
        name: "pipeline-div-direct-two-core-execute-wait-visibility",
        operation: ScheduledIntegerOperation::Div,
        memory_system: "direct",
        cores: 2,
        completed_tick_limit: 260,
        expected_execute_wait_cycles: 19,
        expected_last_execute_wait_cycle: 34,
        expected_last_execute_wait_tick: 29,
    });
}

#[test]
fn rem6_run_keeps_cache_fabric_dram_mul_invisible_through_execute_wait() {
    assert_execute_wait_visibility(ExecuteWaitVisibilityCase {
        name: "pipeline-mul-cache-fabric-dram-execute-wait-visibility",
        operation: ScheduledIntegerOperation::Mul,
        memory_system: "cache-fabric-dram",
        cores: 1,
        completed_tick_limit: 700,
        expected_execute_wait_cycles: 2,
        expected_last_execute_wait_cycle: 25,
        expected_last_execute_wait_tick: 17,
    });
}

#[test]
fn rem6_run_keeps_scalar_float_result_invisible_through_execute_wait() {
    let program = riscv64_program(&[
        fp_r_type(0x70, 0, 0, 0x1, 5), // fclass.s x5, f0
        0x0000_0073,                   // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("pipeline-fclass-direct-execute-wait-visibility", &elf);
    let completed = run_pipeline_timing_with_debug_flags(&path, 1, 180, "direct", "Pipeline");

    assert_eq!(
        completed
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        completed
            .pointer("/cores/0/registers/x5")
            .and_then(Value::as_str),
        Some("0x200")
    );
    assert_eq!(
        completed
            .pointer("/cores/0/in_order_pipeline/execute_wait_cycles")
            .and_then(Value::as_u64),
        Some(2)
    );
    let wait_cycles = execute_wait_cycles_by_cpu(&completed, 0);
    assert_eq!(wait_cycles.len(), 2);

    let probe_tick = *wait_cycles.last().unwrap();
    let during_wait =
        run_pipeline_timing_with_debug_flags(&path, 1, probe_tick, "direct", "Pipeline");
    assert_eq!(
        during_wait
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        during_wait
            .pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(probe_tick)
    );
    assert_eq!(during_wait.pointer("/cores/0/registers/x5"), None);
    assert_eq!(
        during_wait
            .pointer("/cores/0/committed_instructions")
            .and_then(Value::as_u64),
        Some(0)
    );
}

#[test]
fn rem6_run_direct_single_core_alu_has_no_execute_wait_visibility_gate() {
    let case = ExecuteWaitVisibilityCase {
        name: "pipeline-alu-direct-single-core-no-execute-wait",
        operation: ScheduledIntegerOperation::Add,
        memory_system: "direct",
        cores: 1,
        completed_tick_limit: 180,
        expected_execute_wait_cycles: 0,
        expected_last_execute_wait_cycle: 0,
        expected_last_execute_wait_tick: 0,
    };
    let path = pipeline_visibility_program_path(case.name, case.operation);
    let completed = run_pipeline_timing_with_debug_flags(
        &path,
        case.cores,
        case.completed_tick_limit,
        case.memory_system,
        "Pipeline",
    );

    assert_completed_visibility_case(&completed, case);
    assert_eq!(
        completed
            .pointer("/cores/0/in_order_pipeline/execute_wait_cycles")
            .and_then(Value::as_u64),
        Some(0),
        "{} should not manufacture execute-wait cycles for a zero-extra-wait ALU op",
        case.name
    );
    assert!(
        execute_wait_cycles_by_cpu(&completed, 0).is_empty(),
        "{} should not have Pipeline execute-wait trace rows",
        case.name
    );
}

#[derive(Clone, Copy)]
struct ExecuteWaitVisibilityCase {
    name: &'static str,
    operation: ScheduledIntegerOperation,
    memory_system: &'static str,
    cores: usize,
    completed_tick_limit: u64,
    expected_execute_wait_cycles: u64,
    expected_last_execute_wait_cycle: u64,
    expected_last_execute_wait_tick: u64,
}

#[derive(Clone, Copy)]
enum ScheduledIntegerOperation {
    Add,
    Mul,
    Div,
}

impl ScheduledIntegerOperation {
    fn program_words(self) -> Vec<u32> {
        match self {
            Self::Add => vec![
                i_type(3, 0, 0x0, 5, 0x13),       // addi x5, x0, 3
                i_type(7, 0, 0x0, 6, 0x13),       // addi x6, x0, 7
                r_type(0x00, 6, 5, 0x0, 5, 0x33), // add x5, x5, x6
                0x0000_0073,                      // ecall
            ],
            Self::Mul => vec![
                i_type(3, 0, 0x0, 5, 0x13),       // addi x5, x0, 3
                i_type(7, 0, 0x0, 6, 0x13),       // addi x6, x0, 7
                r_type(0x01, 6, 5, 0x0, 5, 0x33), // mul x5, x5, x6
                0x0000_0073,                      // ecall
            ],
            Self::Div => vec![
                i_type(99, 0, 0x0, 5, 0x13),      // addi x5, x0, 99
                i_type(3, 0, 0x0, 6, 0x13),       // addi x6, x0, 3
                r_type(0x01, 6, 5, 0x4, 5, 0x33), // div x5, x5, x6
                0x0000_0073,                      // ecall
            ],
        }
    }

    const fn old_x5(self) -> &'static str {
        match self {
            Self::Add | Self::Mul => "0x3",
            Self::Div => "0x63",
        }
    }

    const fn final_x5(self) -> &'static str {
        match self {
            Self::Add => "0xa",
            Self::Mul => "0x15",
            Self::Div => "0x21",
        }
    }
}

fn assert_execute_wait_visibility(case: ExecuteWaitVisibilityCase) {
    let path = pipeline_visibility_program_path(case.name, case.operation);
    let completed = run_pipeline_timing_with_debug_flags(
        &path,
        case.cores,
        case.completed_tick_limit,
        case.memory_system,
        "Pipeline",
    );
    assert_completed_visibility_case(&completed, case);

    assert_execute_wait_trace(&completed, case);

    let wait_probe_tick = case.expected_last_execute_wait_tick;
    let during_wait = run_pipeline_timing_with_debug_flags(
        &path,
        case.cores,
        wait_probe_tick,
        case.memory_system,
        "Pipeline",
    );
    let status = during_wait
        .pointer("/simulation/status")
        .and_then(Value::as_str);
    let final_tick = during_wait
        .pointer("/simulation/final_tick")
        .and_then(Value::as_u64);
    for cpu in 0..case.cores {
        let core = &during_wait["cores"][cpu];
        assert_eq!(
            core.pointer("/registers/x5").and_then(Value::as_str),
            Some(case.operation.old_x5()),
            "{} cpu{cpu} must keep the MUL/DIV destination at the old architectural value through execute wait; probe_tick={wait_probe_tick} status={status:?} final_tick={final_tick:?}",
            case.name
        );
        assert_eq!(
            core["committed_instructions"].as_u64(),
            Some(2),
            "{} cpu{cpu} must not architecturally commit the producer during execute wait; probe_tick={wait_probe_tick} status={status:?} final_tick={final_tick:?}",
            case.name
        );
        assert_eq!(
            core.pointer("/in_order_pipeline/execute_wait_cycles")
                .and_then(Value::as_u64),
            Some(case.expected_execute_wait_cycles),
            "{} cpu{cpu} should expose exact scheduler-owned execute-wait evidence at the last wait tick; probe_tick={wait_probe_tick} status={status:?} final_tick={final_tick:?}",
            case.name
        );
    }
    assert_eq!(
        status,
        Some("stopped_at_tick_limit"),
        "{} should stop inside the scheduled execute-wait window; probe_tick={wait_probe_tick} final_tick={final_tick:?}",
        case.name
    );
    assert_eq!(
        final_tick,
        Some(wait_probe_tick),
        "{} should stop at the selected execute-wait cycle; status={status:?}",
        case.name
    );
}

fn assert_completed_visibility_case(completed: &Value, case: ExecuteWaitVisibilityCase) {
    let status = completed
        .pointer("/simulation/status")
        .and_then(Value::as_str);
    assert_eq!(
        status,
        Some("executed_until_trap"),
        "{} should complete the representative program before probing",
        case.name
    );
    let memory_system = completed
        .pointer("/simulation/memory_system")
        .and_then(Value::as_str);
    assert_eq!(
        memory_system,
        Some(case.memory_system),
        "{} should exercise the requested memory-system row",
        case.name
    );
    for cpu in 0..case.cores {
        let core = &completed["cores"][cpu];
        assert_eq!(
            core.pointer("/registers/x5").and_then(Value::as_str),
            Some(case.operation.final_x5()),
            "{} cpu{cpu} should eventually publish the final x5 value",
            case.name
        );
        assert_eq!(
            core.pointer("/in_order_pipeline/execute_wait_cycles")
                .and_then(Value::as_u64),
            Some(case.expected_execute_wait_cycles),
            "{} cpu{cpu} should have exact execute-wait evidence for the row",
            case.name
        );
    }
}

fn pipeline_visibility_program_path(
    name: &str,
    operation: ScheduledIntegerOperation,
) -> std::path::PathBuf {
    let words = operation.program_words();
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn assert_execute_wait_trace(completed: &Value, case: ExecuteWaitVisibilityCase) {
    for cpu in 0..case.cores {
        let cycles = execute_wait_cycles_by_cpu(completed, cpu);
        assert_eq!(
            cycles.len() as u64,
            case.expected_execute_wait_cycles,
            "{} cpu{cpu} should expose exact execute-wait trace rows",
            case.name
        );
        assert_eq!(
            cycles.last().copied(),
            Some(case.expected_last_execute_wait_cycle),
            "{} cpu{cpu} should end execute wait on the exact pipeline cycle",
            case.name
        );
    }
}

fn execute_wait_cycles_by_cpu(completed: &Value, cpu: usize) -> Vec<u64> {
    completed
        .pointer("/debug/pipeline_trace")
        .and_then(Value::as_array)
        .expect("completed run should expose Pipeline debug trace")
        .iter()
        .filter(|record| record.pointer("/cpu").and_then(Value::as_u64) == Some(cpu as u64))
        .filter(|record| {
            record.pointer("/stall_cause").and_then(Value::as_str) == Some("execute_wait")
        })
        .filter_map(|record| record.pointer("/cycle").and_then(Value::as_u64))
        .collect()
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn fp_r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    r_type(funct7, rs2, rs1, funct3, rd, 0x53)
}

fn run_pipeline_timing(
    path: &std::path::Path,
    cores: usize,
    max_tick: u64,
    memory_system: &str,
) -> Value {
    run_pipeline_timing_with_debug_flags(path, cores, max_tick, memory_system, "Memory")
}

fn run_pipeline_timing_with_debug_flags(
    path: &std::path::Path,
    cores: usize,
    max_tick: u64,
    memory_system: &str,
    debug_flags: &str,
) -> Value {
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
            "--memory-system",
            memory_system,
            "--cores",
            &cores.to_string(),
            "--debug-flags",
            debug_flags,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
