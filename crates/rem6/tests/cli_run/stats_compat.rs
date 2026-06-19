use std::{env, process::Command};

use crate::support::*;

#[test]
fn rem6_run_text_stats_emit_gem5_final_tick_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-final-tick-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("---------- Begin Simulation Statistics ----------"));

    let final_tick = text_stat_value(&stdout, "sim.final_tick");
    assert!(final_tick > 0);
    assert_eq!(text_stat_value(&stdout, "simTicks"), final_tick);
    assert_eq!(text_stat_value(&stdout, "finalTick"), final_tick);
}

#[test]
fn rem6_run_text_stats_emit_gem5_frequency_alias() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-frequency-alias", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_eq!(text_stat_value(&stdout, "simFreq"), 1_000_000_000_000);
}

#[test]
fn rem6_run_text_stats_emit_gem5_instruction_alias() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-instruction-alias", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let committed = text_stat_value(&stdout, "sim.instructions.committed");
    assert_eq!(committed, 2);
    assert_eq!(text_stat_value(&stdout, "simInsts"), committed);
}

#[test]
fn rem6_run_text_stats_emit_gem5_cpu_numinsts_and_numcycles_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-cpu-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_eq!(
        text_stat_value(&stdout, "system.cpu.numInsts"),
        text_stat_value(&stdout, "sim.cpu0.instructions.committed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.numCycles"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.cycles")
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_multicore_cpu_aliases_without_ambiguous_cpu_path() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-multicore-cpu-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    for cpu in [0, 1] {
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.numInsts")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.instructions.committed"))
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.pipeline.in_order.cycles"))
        );
    }
    assert!(!has_text_stat(&stdout, "system.cpu.numInsts"));
    assert!(!has_text_stat(&stdout, "system.cpu.numCycles"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_seconds_and_ops_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-seconds-ops-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "text",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("---------- Begin Simulation Statistics ----------\n"));
    assert_eq!(
        text_stat_value(&stdout, "simOps"),
        text_stat_value(&stdout, "simInsts")
    );
    assert_eq!(
        text_stat_decimal(&stdout, "simSeconds"),
        format!(
            "{:.12}",
            text_stat_value(&stdout, "finalTick") as f64 / 1_000_000_000_000_f64
        )
    );
}

#[test]
fn rem6_run_stats_emit_in_order_pipeline_cycles_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-pipeline-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"committed_instructions\":2"));
    assert!(stdout.contains("\"in_order_pipeline\":{\"cycles\":6,\"in_flight\":0,\"retired\":2,"));
    assert!(stdout.contains("\"data_wait_cycles\":0"));
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        6,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.retired",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.data_wait_cycles",
        "Cycle",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_stats_emit_checker_cpu_counts_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("checker-cpu-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--checker-cpu",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"checker\":{\"checked_instructions\":2,\"mismatches\":0}"));
    assert_stat(
        &stdout,
        "sim.cpu0.checker.checked_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.checker.mismatches",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_stats_include_issued_fetch_ahead_before_response() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-issued-fetch-ahead-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "12",
            "--memory-route-delay",
            "5",
            "--stats-format",
            "json",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_tick_limit\""));
    assert_eq!(json_u64_field(&stdout, "\"committed_instructions\":"), 1);
    assert_eq!(json_u64_field(&stdout, "\"in_flight\":"), 1);
    let advanced = json_u64_field(&stdout, "\"advanced\":");
    let retired = json_u64_field(&stdout, "\"retired\":");
    assert!(
        advanced > retired,
        "pipeline advance history should include non-retire cycles: {stdout}"
    );
    assert_stat_greater_than(
        &stdout,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.retired",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.advanced",
        "Count",
        advanced,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.in_flight",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.fetch.requests",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.fetch.responses",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_stats_emit_in_order_fetch_wait_cycles_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-fetch-wait-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
            "5",
            "--stats-format",
            "json",
            "--execute",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let fetch_wait_cycles = json_u64_field(&stdout, "\"fetch_wait_cycles\":");
    assert!(fetch_wait_cycles > 0);
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.fetch_wait_cycles"),
        fetch_wait_cycles
    );
    assert_stat_greater_than(
        &stdout,
        "sim.cpu0.pipeline.in_order.fetch_wait_cycles",
        "Cycle",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_stats_emit_in_order_branch_redirects_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293,          // addi x5, x0, 7
        b_type(8, 0, 0, 0x0), // beq x0, x0, target
        0x0010_0313,          // addi x6, x0, 1
        0x0000_0073,          // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-redirect-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
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
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let branch_predictions = json_u64_field(&stdout, "\"branch_predictions\":");
    let branch_mispredictions = json_u64_field(&stdout, "\"branch_mispredictions\":");
    let advanced = json_u64_field(&stdout, "\"advanced\":");
    let flushed = json_u64_field(&stdout, "\"flushed\":");
    let resource_blocked = json_u64_field(&stdout, "\"resource_blocked\":");
    let ordering_blocked = json_u64_field(&stdout, "\"ordering_blocked\":");
    let branch_prediction_flushes = json_u64_field(&stdout, "\"branch_prediction_flushes\":");
    let redirects = json_u64_field(&stdout, "\"redirects\":");

    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.branch_predictions"),
        branch_predictions
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.branch_mispredictions"),
        branch_mispredictions
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.advanced"),
        advanced
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.flushed"),
        flushed
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.resource_blocked"),
        resource_blocked
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.ordering_blocked"),
        ordering_blocked
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_prediction_flushes"
        ),
        branch_prediction_flushes
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.redirects"),
        redirects
    );
    assert!(branch_predictions > 0);
    assert!(branch_mispredictions > 0);
    assert!(advanced > 0);
    assert!(flushed > 0);
    assert!(flushed >= branch_prediction_flushes);
    assert!(branch_prediction_flushes > 0);
    assert!(redirects > 0);
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x1\""));
}

fn json_u64_field(stdout: &str, marker: &str) -> u64 {
    let start = stdout
        .find(marker)
        .unwrap_or_else(|| panic!("missing JSON field {marker} in output:\n{stdout}"))
        + marker.len();
    let end = stdout[start..]
        .find(|character: char| !character.is_ascii_digit())
        .map(|offset| start + offset)
        .unwrap_or(stdout.len());
    stdout[start..end]
        .parse::<u64>()
        .unwrap_or_else(|error| panic!("invalid numeric JSON field {marker}: {error}"))
}

fn text_stat_decimal(stdout: &str, path: &str) -> String {
    stdout
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            if fields.next()? != path {
                return None;
            }
            Some(fields.next()?.to_string())
        })
        .unwrap_or_else(|| panic!("missing text stat {path} in output:\n{stdout}"))
}

fn text_stat_value(stdout: &str, path: &str) -> u64 {
    stdout
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            if fields.next()? != path {
                return None;
            }
            fields.next()?.parse().ok()
        })
        .unwrap_or_else(|| panic!("missing text stat {path} in output:\n{stdout}"))
}

fn has_text_stat(stdout: &str, path: &str) -> bool {
    stdout
        .lines()
        .any(|line| line.split_whitespace().next() == Some(path))
}
