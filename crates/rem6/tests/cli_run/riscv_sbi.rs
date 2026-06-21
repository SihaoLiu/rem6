use std::process::Command;

use crate::support::*;

const SBI_BASE_GET_SPEC_VERSION: i32 = 0;
const SBI_BASE_PROBE_EXTENSION: i32 = 3;
const SBI_BASE_EXTENSION: i32 = 0x10;
const SBI_TIME_EXTENSION: i32 = 0x5449_4d45u32 as i32;
const SBI_TIME_SET_TIMER: i32 = 0;
const SBI_DEBUG_CONSOLE_EXTENSION: i32 = 0x4442_434e;
const SBI_DEBUG_CONSOLE_WRITE: i32 = 0;
const SBI_HSM_EXTENSION: i32 = 0x0048_534d;
const SBI_HSM_HART_START: i32 = 0;
const SBI_ERR_ALREADY_AVAILABLE: u64 = (-6_i64) as u64;
const SBI_SPEC_VERSION_2_0: u64 = 2 << 24;
const RISCV_SBI_ENTRY: u64 = 0x8000_0000;

fn load_dbcn_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_DEBUG_CONSOLE_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_DEBUG_CONSOLE_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

fn load_time_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_TIME_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_TIME_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

fn load_hsm_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_HSM_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_HSM_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

#[test]
fn rem6_run_riscv_sbi_reports_time_set_timer_deadline() {
    let mut words = Vec::new();
    words.extend([
        load_time_extension(17)[0],
        load_time_extension(17)[1],
        i_type(SBI_TIME_SET_TIMER, 0, 0x0, 16, 0x13),
        i_type(96, 0, 0x0, 10, 0x13),
        0x0000_0073,
        i_type(0, 10, 0x0, 5, 0x13),
        i_type(0, 11, 0x0, 6, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-time-set-timer", &elf);

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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(!stdout.contains("\"x6\":\""));
    assert!(stdout.contains("\"riscv_sbi_timers\":[{\"cpu\":0,\"deadline\":96}]"));
    assert_stat(
        &stdout,
        "sim.riscv.sbi.timer.deadlines",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.timer.next_deadline",
        "Tick",
        96,
        "constant",
    );
}

#[test]
fn rem6_run_without_riscv_sbi_omits_sbi_timer_stats() {
    let elf = riscv64_elf(
        RISCV_SBI_ENTRY,
        RISCV_SBI_ENTRY,
        &riscv64_program(&[0x0010_0073]),
    );
    let path = temp_binary("riscv-no-sbi-timer-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "32",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"riscv_sbi_timers\":[]"));
    assert!(!stdout.contains("sim.riscv.sbi.timer."));
}

#[test]
fn rem6_run_riscv_sbi_handles_supervisor_base_ecall() {
    let program = riscv64_program(&[
        i_type(SBI_BASE_EXTENSION, 0, 0x0, 17, 0x13),
        i_type(SBI_BASE_GET_SPEC_VERSION, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0, 11, 0x0, 6, 0x13),
        b_type(12, 0, 10, 0x1),
        i_type(1, 0, 0x0, 5, 0x13),
        0x0010_0073,
        i_type(2, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-sbi-base", &elf);

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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(
        stdout.contains("\"riscv_boot\":{\"a0\":\"0x0\",\"a1\":\"0x0\",\"sbi\":true,\"se\":false}")
    );
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains(&format!("\"x6\":\"0x{:x}\"", SBI_SPEC_VERSION_2_0)));
    assert_stat(&stdout, "sim.riscv.sbi", "Count", 1, "constant");
}

#[test]
fn rem6_run_riscv_sbi_handles_dbcn_shared_memory_write() {
    let message = b"rem6-dbcn\n";
    let mut words = Vec::new();
    words.extend(load_dbcn_extension(10));
    words.extend([
        i_type(SBI_BASE_EXTENSION, 0, 0x0, 17, 0x13),
        i_type(SBI_BASE_PROBE_EXTENSION, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0, 11, 0x0, 5, 0x13),
        i_type(message.len() as i32, 0, 0x0, 10, 0x13),
    ]);
    let auipc_index = words.len();
    words.push(u_type(0, 11, 0x17));
    words.push(i_type(0, 11, 0x0, 11, 0x13));
    words.extend([
        i_type(0, 0, 0x0, 12, 0x13),
        load_dbcn_extension(17)[0],
        load_dbcn_extension(17)[1],
        i_type(SBI_DEBUG_CONSOLE_WRITE, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0, 11, 0x0, 6, 0x13),
        0x0010_0073,
    ]);

    let mut program = riscv64_program(&words);
    let message_offset = program.len() as i32;
    let auipc_offset = (auipc_index * 4) as i32;
    words[auipc_index + 1] = i_type(message_offset - auipc_offset, 11, 0x0, 11, 0x13);
    program = riscv64_program(&words);
    program.extend_from_slice(message);

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &program);
    let path = temp_binary("riscv-sbi-dbcn-write", &elf);

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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains(&format!("\"x6\":\"0x{:x}\"", message.len())));
    assert!(stdout.contains(
        "\"riscv_sbi_console\":{\"bytes\":10,\"text\":\"rem6-dbcn\\n\",\"hex\":\"72656d362d6462636e0a\"}"
    ));
    assert_stat(
        &stdout,
        "sim.riscv.sbi.dbcn.console_bytes",
        "Byte",
        message.len() as u64,
        "constant",
    );
}

#[test]
fn rem6_run_riscv_sbi_starts_secondary_hart_through_hsm() {
    let message = b"hsm-start:1:55\n";
    let mut words = Vec::new();
    words.push(i_type(1, 0, 0x0, 10, 0x13));
    let secondary_auipc_index = words.len();
    words.push(u_type(0, 11, 0x17));
    words.push(i_type(0, 11, 0x0, 11, 0x13));
    words.extend([
        i_type(0x55, 0, 0x0, 12, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_START, 0, 0x0, 16, 0x13),
        0x0000_0073,
        b_type(8, 0, 10, 0x1),
        j_type(0, 0),
        i_type(0x7e, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);

    let secondary_index = words.len();
    words.extend([
        i_type(1, 0, 0x0, 5, 0x13),
        b_type(56, 5, 10, 0x1),
        i_type(0x55, 0, 0x0, 5, 0x13),
        b_type(48, 5, 11, 0x1),
        i_type(message.len() as i32, 0, 0x0, 10, 0x13),
    ]);
    let message_auipc_index = words.len();
    words.push(u_type(0, 11, 0x17));
    words.push(i_type(0, 11, 0x0, 11, 0x13));
    words.extend([
        i_type(0, 0, 0x0, 12, 0x13),
        load_dbcn_extension(17)[0],
        load_dbcn_extension(17)[1],
        i_type(SBI_DEBUG_CONSOLE_WRITE, 0, 0x0, 16, 0x13),
        0x0000_0073,
        b_type(12, 0, 10, 0x1),
        i_type(1, 0, 0x0, 5, 0x13),
        0x0010_0073,
        i_type(0x7f, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);

    let mut program = riscv64_program(&words);
    let secondary_offset = (secondary_index * 4) as i32;
    let message_offset = program.len() as i32;
    words[secondary_auipc_index + 1] = i_type(
        secondary_offset - (secondary_auipc_index * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );
    words[message_auipc_index + 1] = i_type(
        message_offset - (message_auipc_index * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );
    program = riscv64_program(&words);
    program.extend_from_slice(message);

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &program);
    let path = temp_binary("riscv-sbi-hsm-start", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "320",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"cores\":2"));
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains(
        "\"riscv_sbi_console\":{\"bytes\":15,\"text\":\"hsm-start:1:55\\n\",\"hex\":\"68736d2d73746172743a313a35350a\"}"
    ));
    assert_stat(&stdout, "sim.riscv.sbi", "Count", 1, "constant");
    assert_stat(
        &stdout,
        "sim.riscv.sbi.dbcn.console_bytes",
        "Byte",
        message.len() as u64,
        "constant",
    );
}

#[test]
fn rem6_run_riscv_sbi_hart_start_reports_already_available_for_boot_hart() {
    let mut words = Vec::new();
    words.extend([
        i_type(0, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        i_type(0, 0, 0x0, 12, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_START, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0, 10, 0x0, 5, 0x13),
        i_type(0, 11, 0x0, 6, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-hsm-start-boot-hart", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains(&format!("\"x5\":\"0x{:x}\"", SBI_ERR_ALREADY_AVAILABLE)));
    assert!(!stdout.contains("\"x6\":\""));
    assert_stat(&stdout, "sim.riscv.sbi", "Count", 1, "constant");
}

#[test]
fn rem6_run_rejects_riscv_sbi_without_execution() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-sbi-without-execute", &elf);

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
            "--riscv-sbi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-sbi requires --execute"));
}

#[test]
fn rem6_run_rejects_riscv_sbi_without_riscv_isa() {
    let elf = x86_64_elf(0x1000_0000, 0x1000_0000, &[0x90]);
    let path = temp_binary("riscv-sbi-without-riscv", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "x86",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-format",
            "json",
            "--riscv-sbi",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-sbi requires --isa riscv"));
}

#[test]
fn rem6_run_rejects_riscv_sbi_with_riscv_se() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-sbi-with-riscv-se", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-format",
            "json",
            "--riscv-sbi",
            "--riscv-se",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-sbi cannot be combined with --riscv-se"));
}

#[test]
fn rem6_run_rejects_riscv_sbi_with_explicit_boot_a0() {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &[0x13, 0, 0, 0]);
    let path = temp_binary("riscv-sbi-with-boot-a0", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--execute",
            "--stats-format",
            "json",
            "--riscv-sbi",
            "--riscv-boot-a0",
            "7",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--riscv-sbi requires --riscv-boot-a0 0"));
}
