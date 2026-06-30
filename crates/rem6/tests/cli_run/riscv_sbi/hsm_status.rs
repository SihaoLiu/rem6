use std::process::Command;

use serde_json::Value;

use crate::support::*;

use super::{load_hsm_extension, RISCV_SBI_ENTRY, SBI_HSM_HART_GET_STATUS, SBI_HSM_HART_START};

const SBI_HSM_HART_STARTED: u64 = 0;
const SBI_HSM_HART_STOPPED: u64 = 1;

#[test]
fn rem6_run_riscv_sbi_hsm_status_queries_emit_artifacts_and_stats() {
    let mut words = Vec::new();
    words.extend([
        i_type(0, 0, 0x0, 10, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_GET_STATUS, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0x10, 11, 0x0, 5, 0x13),
        i_type(1, 0, 0x0, 10, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_GET_STATUS, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0, 11, 0x0, 6, 0x13),
        i_type(1, 0, 0x0, 10, 0x13),
    ]);
    let secondary_auipc_index = words.len();
    words.push(u_type(0, 11, 0x17));
    words.push(i_type(0, 11, 0x0, 11, 0x13));
    words.extend([
        i_type(0x44, 0, 0x0, 12, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_START, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    let hsm_start_error_branch_index = words.len();
    words.push(b_type(0, 0, 10, 0x1));

    let poll_index = words.len();
    words.extend([
        i_type(1, 0, 0x0, 10, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_GET_STATUS, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0x20, 11, 0x0, 7, 0x13),
    ]);
    let poll_branch_index = words.len();
    words.push(b_type(
        ((poll_index as isize - poll_branch_index as isize) * 4) as i32,
        0,
        11,
        0x1,
    ));
    words.push(0x0010_0073);

    let failure_index = words.len();
    words.extend([i_type(0x7e, 0, 0x0, 5, 0x13), 0x0010_0073]);
    let secondary_index = words.len();
    words.push(j_type(0, 0));

    words[secondary_auipc_index + 1] = i_type(
        ((secondary_index - secondary_auipc_index) * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );
    words[hsm_start_error_branch_index] = b_type(
        ((failure_index - hsm_start_error_branch_index) * 4) as i32,
        0,
        10,
        0x1,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-hsm-status-artifacts", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "480",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
            "--riscv-sbi",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/simulation/trap").and_then(Value::as_str),
        Some("breakpoint")
    );
    assert!(stdout.contains("\"x5\":\"0x10\""));
    assert!(stdout.contains("\"x6\":\"0x1\""));
    assert!(stdout.contains("\"x7\":\"0x20\""));

    let statuses = json
        .pointer("/riscv_sbi_hsm_statuses")
        .and_then(Value::as_array)
        .unwrap();
    assert!(statuses.len() >= 3);
    assert_hsm_status(&statuses[0], 0, 0, SBI_HSM_HART_STARTED, "started");
    assert_hsm_status(&statuses[1], 0, 1, SBI_HSM_HART_STOPPED, "stopped");
    assert_hsm_status(
        statuses.last().unwrap(),
        0,
        1,
        SBI_HSM_HART_STARTED,
        "started",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.hsm.status_queries",
        "Count",
        statuses.len() as u64,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.hsm.status.started",
        "Count",
        2,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.hsm.status.stopped",
        "Count",
        1,
        "constant",
    );
}

fn assert_hsm_status(
    value: &Value,
    source_cpu: u64,
    target_hart: u64,
    status: u64,
    status_name: &str,
) {
    assert_eq!(
        value.get("source_cpu").and_then(Value::as_u64),
        Some(source_cpu)
    );
    assert_eq!(
        value.get("target_hart").and_then(Value::as_u64),
        Some(target_hart)
    );
    assert_eq!(value.get("status").and_then(Value::as_u64), Some(status));
    assert_eq!(
        value.get("status_name").and_then(Value::as_str),
        Some(status_name)
    );
}
