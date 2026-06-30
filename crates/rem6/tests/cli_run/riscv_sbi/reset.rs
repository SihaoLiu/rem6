use std::process::Command;

use crate::support::*;

use super::{
    load_srst_extension, RISCV_SBI_ENTRY, SBI_RESET_TYPE_COLD_REBOOT, SBI_RESET_TYPE_SHUTDOWN,
    SBI_RESET_TYPE_WARM_REBOOT, SBI_SRST_SYSTEM_RESET,
};

#[test]
fn rem6_run_riscv_sbi_system_reset_records_reset_request() {
    let mut words = Vec::new();
    words.extend([
        load_srst_extension(17)[0],
        load_srst_extension(17)[1],
        i_type(SBI_SRST_SYSTEM_RESET, 0, 0x0, 16, 0x13),
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(1, 0, 0x0, 11, 0x13),
        0x0000_0073,
        i_type(0x7e, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-system-reset", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
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
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":1"));
    assert!(stdout.contains(
        "\"riscv_sbi_resets\":[{\"cpu\":0,\"reset_type\":0,\"reset_reason\":1,\"code\":1}]"
    ));
    assert!(stdout.contains("\"riscv_sbi_ipis\":[]"));
    assert!(stdout.contains("\"riscv_sbi_timers\":[]"));
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.requests",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.shutdowns",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.cold_reboots",
        "Count",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.warm_reboots",
        "Count",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.system_failures",
        "Count",
        1,
        "constant",
    );
}

#[test]
fn rem6_run_riscv_sbi_shutdown_reset_records_shutdown_stat() {
    let mut words = Vec::new();
    words.extend([
        load_srst_extension(17)[0],
        load_srst_extension(17)[1],
        i_type(SBI_SRST_SYSTEM_RESET, 0, 0x0, 16, 0x13),
        i_type(SBI_RESET_TYPE_SHUTDOWN, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        0x0000_0073,
        i_type(0x7e, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-shutdown-reset", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
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
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_reason\":\"host_stop\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(!stdout.contains("\"x5\":\"0x7e\""));
    assert!(stdout.contains(
        "\"riscv_sbi_resets\":[{\"cpu\":0,\"reset_type\":0,\"reset_reason\":0,\"code\":0}]"
    ));
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.requests",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.shutdowns",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.cold_reboots",
        "Count",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.warm_reboots",
        "Count",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.system_failures",
        "Count",
        0,
        "constant",
    );
}

fn run_reset_type_for_stats(reset_type: i32, temp_name: &str) {
    let mut words = Vec::new();
    words.extend([
        load_srst_extension(17)[0],
        load_srst_extension(17)[1],
        i_type(SBI_SRST_SYSTEM_RESET, 0, 0x0, 16, 0x13),
        i_type(reset_type, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        0x0000_0073,
        i_type(0x7e, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary(temp_name, &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "json",
            "--execute",
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
    assert!(stdout.contains("\"status\":\"stopped_by_host\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(!stdout.contains("\"x5\":\"0x7e\""));
    assert!(stdout.contains(&format!(
        "\"riscv_sbi_resets\":[{{\"cpu\":0,\"reset_type\":{reset_type},\"reset_reason\":0,\"code\":0}}]"
    )));
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.requests",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.shutdowns",
        "Count",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.cold_reboots",
        "Count",
        u64::from(reset_type == SBI_RESET_TYPE_COLD_REBOOT),
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.warm_reboots",
        "Count",
        u64::from(reset_type == SBI_RESET_TYPE_WARM_REBOOT),
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.reset.system_failures",
        "Count",
        0,
        "constant",
    );
}

#[test]
fn rem6_run_riscv_sbi_reboot_resets_record_reboot_type_stats() {
    run_reset_type_for_stats(SBI_RESET_TYPE_COLD_REBOOT, "riscv-sbi-cold-reboot-reset");
    run_reset_type_for_stats(SBI_RESET_TYPE_WARM_REBOOT, "riscv-sbi-warm-reboot-reset");
}
