use std::process::Command;

use crate::support::*;

const SBI_BASE_GET_SPEC_VERSION: i32 = 0;
const SBI_BASE_PROBE_EXTENSION: i32 = 3;
const SBI_BASE_EXTENSION: i32 = 0x10;
const SBI_TIME_EXTENSION: i32 = 0x5449_4d45u32 as i32;
const SBI_TIME_SET_TIMER: i32 = 0;
const SBI_IPI_EXTENSION: i32 = 0x0073_5049;
const SBI_IPI_SEND_IPI: i32 = 0;
const SBI_RFENCE_EXTENSION: i32 = 0x5246_4e43;
const SBI_RFENCE_REMOTE_FENCE_I: i32 = 0;
const SBI_SRST_EXTENSION: i32 = 0x5352_5354;
const SBI_SRST_SYSTEM_RESET: i32 = 0;
const SBI_RESET_TYPE_SHUTDOWN: i32 = 0;
const SBI_RESET_TYPE_COLD_REBOOT: i32 = 1;
const SBI_RESET_TYPE_WARM_REBOOT: i32 = 2;
const SBI_DEBUG_CONSOLE_EXTENSION: i32 = 0x4442_434e;
const SBI_DEBUG_CONSOLE_WRITE: i32 = 0;
const SBI_HSM_EXTENSION: i32 = 0x0048_534d;
const SBI_HSM_HART_START: i32 = 0;
const SBI_HSM_HART_STOP: i32 = 1;
const SBI_HSM_HART_GET_STATUS: i32 = 2;
const SBI_HSM_HART_SUSPEND: i32 = 3;
const SBI_HSM_DEFAULT_RETENTIVE_SUSPEND: i32 = 0;
const SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND: i32 = 0x8000_0000u32 as i32;
const SBI_HSM_HART_SUSPENDED: i32 = 4;
const SBI_ERR_ALREADY_AVAILABLE: u64 = (-6_i64) as u64;
const SBI_SPEC_VERSION_2_0: u64 = 2 << 24;
const RISCV_SBI_ENTRY: u64 = 0x8000_0000;
const RISCV_INTERRUPT_BIT: u64 = 1 << 63;
const RISCV_SUPERVISOR_SOFTWARE_INTERRUPT: u64 = 1;
const RISCV_SUPERVISOR_TIMER_INTERRUPT: u64 = 5;

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

fn load_ipi_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_IPI_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_IPI_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

fn load_rfence_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_RFENCE_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_RFENCE_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

fn load_srst_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_SRST_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_SRST_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

fn load_hsm_extension(rd: u8) -> [u32; 2] {
    let upper = (SBI_HSM_EXTENSION + 0x800) & !0xfff;
    let lower = SBI_HSM_EXTENSION - upper;
    [u_type(upper, rd, 0x37), i_type(lower, rd, 0x0, rd, 0x13)]
}

fn csr_write(csr: u32, rs1: u8) -> u32 {
    (csr << 20) | (u32::from(rs1) << 15) | (0x1 << 12) | 0x73
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
fn rem6_run_riscv_sbi_timer_interrupt_reaches_supervisor_handler() {
    let mut words = Vec::new();
    let stvec_auipc_index = words.len();
    words.extend([
        u_type(0, 5, 0x17),
        i_type(0, 5, 0x0, 5, 0x13),
        csr_write(0x105, 5),
        i_type(1 << 5, 0, 0x0, 5, 0x13),
        csr_write(0x104, 5),
        i_type(1 << 1, 0, 0x0, 5, 0x13),
        csr_write(0x100, 5),
        load_time_extension(17)[0],
        load_time_extension(17)[1],
        i_type(SBI_TIME_SET_TIMER, 0, 0x0, 16, 0x13),
        i_type(128, 0, 0x0, 10, 0x13),
        0x0000_0073,
        b_type(0, 0, 0, 0x0),
    ]);
    let handler_index = words.len();
    words.extend([
        csr_read(0x142, 5),
        csr_read(0x141, 6),
        i_type(0x5a, 0, 0x0, 7, 0x13),
        0x0010_0073,
    ]);
    words[stvec_auipc_index + 1] = i_type(
        ((handler_index - stvec_auipc_index) * 4) as i32,
        5,
        0x0,
        5,
        0x13,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-time-interrupt", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "512",
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
    assert!(stdout.contains(&format!(
        "\"x5\":\"0x{:x}\"",
        RISCV_INTERRUPT_BIT | RISCV_SUPERVISOR_TIMER_INTERRUPT
    )));
    assert!(stdout.contains("\"x7\":\"0x5a\""));
    assert!(stdout.contains("\"riscv_sbi_timers\":[{\"cpu\":0,\"deadline\":128}]"));
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
    let secondary_address = RISCV_SBI_ENTRY + secondary_offset as u64;
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
    assert!(stdout.contains(&format!(
        "\"riscv_sbi_hsm_events\":[{{\"source_cpu\":0,\"function\":0,\"target_hart\":1,\"start_addr\":\"0x{:x}\",\"opaque\":\"0x55\"}}]",
        secondary_address
    )));
    assert_stat(&stdout, "sim.riscv.sbi", "Count", 1, "constant");
    assert_stat(&stdout, "sim.riscv.sbi.hsm.starts", "Count", 1, "constant");
    assert_stat(
        &stdout,
        "sim.riscv.sbi.dbcn.console_bytes",
        "Byte",
        message.len() as u64,
        "constant",
    );
}

#[test]
fn rem6_run_riscv_sbi_hart_stop_records_hsm_stop() {
    let mut words = Vec::new();
    words.extend([
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_STOP, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0x7e, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-hsm-stop", &elf);

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
    assert!(stdout.contains("\"status\":\"idle\""));
    assert!(stdout.contains("\"stop_reason\":\"idle\""));
    assert!(!stdout.contains("\"x5\":\"0x7e\""));
    assert!(stdout.contains(
        "\"riscv_sbi_hsm_events\":[{\"source_cpu\":0,\"function\":1,\"target_hart\":0,\"start_addr\":\"0x0\",\"opaque\":\"0x0\"}]"
    ));
    assert_stat(&stdout, "sim.riscv.sbi.hsm.stops", "Count", 1, "constant");
    assert_stat(&stdout, "sim.stop.idle", "Count", 1, "constant");
}

#[test]
fn rem6_run_riscv_sbi_hart_suspend_records_hsm_suspend() {
    let mut words = Vec::new();
    words.extend([i_type(1, 0, 0x0, 10, 0x13), i_type(31, 10, 0x1, 10, 0x13)]);
    let resume_auipc_index = words.len();
    words.extend([
        u_type(0, 11, 0x17),
        i_type(0, 11, 0x0, 11, 0x13),
        i_type(0x55, 0, 0x0, 12, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_SUSPEND, 0, 0x0, 16, 0x13),
        0x0000_0073,
        i_type(0x7e, 0, 0x0, 5, 0x13),
        0x0010_0073,
    ]);
    let resume_index = words.len();
    words.extend([
        i_type(0x53, 0, 0x0, 5, 0x13),
        i_type(0, 11, 0x0, 6, 0x13),
        0x0010_0073,
    ]);
    words[resume_auipc_index + 1] = i_type(
        ((resume_index - resume_auipc_index) * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );

    let resume_addr = RISCV_SBI_ENTRY + (resume_index as u64 * 4);
    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-hsm-suspend", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
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
    assert!(stdout.contains("\"x5\":\"0x53\""));
    assert!(stdout.contains("\"x6\":\"0x55\""));
    assert!(!stdout.contains("\"x5\":\"0x7e\""));
    assert!(stdout.contains(&format!(
        "\"riscv_sbi_hsm_events\":[{{\"source_cpu\":0,\"function\":3,\"suspend_type\":\"0x{:x}\",\"resume_addr\":\"0x{resume_addr:x}\",\"opaque\":\"0x55\"}}]",
        SBI_HSM_DEFAULT_NON_RETENTIVE_SUSPEND as u32
    )));
    assert_stat(
        &stdout,
        "sim.riscv.sbi.hsm.suspends",
        "Count",
        1,
        "constant",
    );
}

#[test]
fn rem6_run_riscv_sbi_secondary_hart_receives_timer_interrupt_after_hsm_start() {
    let mut words = Vec::new();
    words.push(i_type(1, 0, 0x0, 10, 0x13));
    let secondary_auipc_index = words.len();
    words.push(u_type(0, 11, 0x17));
    words.push(i_type(0, 11, 0x0, 11, 0x13));
    words.extend([
        i_type(0x66, 0, 0x0, 12, 0x13),
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
    let stvec_auipc_index = words.len();
    words.extend([
        u_type(0, 5, 0x17),
        i_type(0, 5, 0x0, 5, 0x13),
        csr_write(0x105, 5),
        i_type(1 << 5, 0, 0x0, 5, 0x13),
        csr_write(0x104, 5),
        i_type(1 << 1, 0, 0x0, 5, 0x13),
        csr_write(0x100, 5),
        load_time_extension(17)[0],
        load_time_extension(17)[1],
        i_type(SBI_TIME_SET_TIMER, 0, 0x0, 16, 0x13),
        i_type(144, 0, 0x0, 10, 0x13),
        0x0000_0073,
        b_type(0, 0, 0, 0x0),
    ]);
    let handler_index = words.len();
    words.extend([
        csr_read(0x142, 5),
        i_type(0x6b, 0, 0x0, 7, 0x13),
        0x0010_0073,
    ]);

    words[secondary_auipc_index + 1] = i_type(
        ((secondary_index - secondary_auipc_index) * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );
    words[stvec_auipc_index + 1] = i_type(
        ((handler_index - stvec_auipc_index) * 4) as i32,
        5,
        0x0,
        5,
        0x13,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-secondary-time-interrupt", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "640",
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
    assert!(stdout.contains(&format!(
        "\"x5\":\"0x{:x}\"",
        RISCV_INTERRUPT_BIT | RISCV_SUPERVISOR_TIMER_INTERRUPT
    )));
    assert!(stdout.contains("\"x7\":\"0x6b\""));
    assert!(stdout.contains("\"riscv_sbi_timers\":[{\"cpu\":1,\"deadline\":144}]"));
}

#[test]
fn rem6_run_riscv_sbi_secondary_hart_receives_ipi_interrupt_after_hsm_start() {
    let mut words = Vec::new();
    words.push(i_type(1, 0, 0x0, 10, 0x13));
    let secondary_auipc_index = words.len();
    words.push(u_type(0, 11, 0x17));
    words.push(i_type(0, 11, 0x0, 11, 0x13));
    words.extend([
        i_type(0x77, 0, 0x0, 12, 0x13),
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_START, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    let hsm_error_branch_index = words.len();
    words.push(b_type(0, 0, 10, 0x1));
    words.extend([
        i_type(1 << 1, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        load_ipi_extension(17)[0],
        load_ipi_extension(17)[1],
        i_type(SBI_IPI_SEND_IPI, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    let ipi_error_branch_index = words.len();
    words.push(b_type(0, 0, 10, 0x1));
    words.push(j_type(0, 0));
    let failure_index = words.len();
    words.extend([i_type(0x7e, 0, 0x0, 5, 0x13), 0x0010_0073]);
    words[hsm_error_branch_index] = b_type(
        ((failure_index - hsm_error_branch_index) * 4) as i32,
        0,
        10,
        0x1,
    );
    words[ipi_error_branch_index] = b_type(
        ((failure_index - ipi_error_branch_index) * 4) as i32,
        0,
        10,
        0x1,
    );

    let secondary_index = words.len();
    let stvec_auipc_index = words.len();
    words.extend([
        u_type(0, 5, 0x17),
        i_type(0, 5, 0x0, 5, 0x13),
        csr_write(0x105, 5),
        i_type(1 << 1, 0, 0x0, 5, 0x13),
        csr_write(0x104, 5),
        i_type(1 << 1, 0, 0x0, 5, 0x13),
        csr_write(0x100, 5),
        b_type(0, 0, 0, 0x0),
    ]);
    let handler_index = words.len();
    words.extend([
        csr_read(0x142, 5),
        i_type(0x4d, 0, 0x0, 7, 0x13),
        0x0010_0073,
    ]);

    words[secondary_auipc_index + 1] = i_type(
        ((secondary_index - secondary_auipc_index) * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );
    words[stvec_auipc_index + 1] = i_type(
        ((handler_index - stvec_auipc_index) * 4) as i32,
        5,
        0x0,
        5,
        0x13,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-secondary-ipi-interrupt", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "640",
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
    assert!(stdout.contains(&format!(
        "\"x5\":\"0x{:x}\"",
        RISCV_INTERRUPT_BIT | RISCV_SUPERVISOR_SOFTWARE_INTERRUPT
    )));
    assert!(stdout.contains("\"x7\":\"0x4d\""));
    assert!(stdout.contains(
        "\"riscv_sbi_ipis\":[{\"source_cpu\":0,\"hart_mask\":\"0x2\",\"hart_mask_base\":\"0x0\",\"targets\":[1]}]"
    ));
    assert_stat(
        &stdout,
        "sim.riscv.sbi.ipi.requests",
        "Count",
        1,
        "constant",
    );
    assert_stat(&stdout, "sim.riscv.sbi.ipi.targets", "Count", 1, "constant");
}

#[test]
fn rem6_run_riscv_sbi_ipi_wakes_retentive_hart_suspend() {
    let mut words = Vec::new();
    words.push(i_type(1, 0, 0x0, 10, 0x13));
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
        i_type(SBI_HSM_HART_SUSPENDED, 0, 0x0, 6, 0x13),
    ]);
    let status_poll_branch_index = words.len();
    words.push(b_type(
        ((poll_index as isize - status_poll_branch_index as isize) * 4) as i32,
        11,
        6,
        0x1,
    ));
    words.extend([
        i_type(1 << 1, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        load_ipi_extension(17)[0],
        load_ipi_extension(17)[1],
        i_type(SBI_IPI_SEND_IPI, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    let ipi_error_branch_index = words.len();
    words.push(b_type(0, 0, 10, 0x1));
    words.push(j_type(0, 0));
    let failure_index = words.len();
    words.extend([i_type(0x7e, 0, 0x0, 5, 0x13), 0x0010_0073]);

    let secondary_index = words.len();
    words.extend([
        load_hsm_extension(17)[0],
        load_hsm_extension(17)[1],
        i_type(SBI_HSM_HART_SUSPEND, 0, 0x0, 16, 0x13),
        i_type(SBI_HSM_DEFAULT_RETENTIVE_SUSPEND, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        i_type(0x5a, 0, 0x0, 12, 0x13),
        0x0000_0073,
        i_type(0x5c, 0, 0x0, 7, 0x13),
        0x0010_0073,
    ]);

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
    words[ipi_error_branch_index] = b_type(
        ((failure_index - ipi_error_branch_index) * 4) as i32,
        0,
        10,
        0x1,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-hsm-retentive-ipi-wake", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "720",
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
    assert!(stdout.contains("\"x7\":\"0x5c\""));
    assert!(!stdout.contains("\"x5\":\"0x7e\""));
    assert!(stdout.contains(
        "\"riscv_sbi_hsm_wakes\":[{\"source_cpu\":0,\"target_hart\":1,\"interrupt_bits\":\"0x2\"}]"
    ));
    assert_stat(&stdout, "sim.riscv.sbi.hsm.wakes", "Count", 1, "constant");
}

#[test]
fn rem6_run_riscv_sbi_remote_fence_i_records_rfence_request() {
    let mut words = Vec::new();
    words.push(i_type(1, 0, 0x0, 10, 0x13));
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
    let hsm_error_branch_index = words.len();
    words.push(b_type(0, 0, 10, 0x1));
    words.extend([
        i_type(1 << 1, 0, 0x0, 10, 0x13),
        i_type(0, 0, 0x0, 11, 0x13),
        load_rfence_extension(17)[0],
        load_rfence_extension(17)[1],
        i_type(SBI_RFENCE_REMOTE_FENCE_I, 0, 0x0, 16, 0x13),
        0x0000_0073,
    ]);
    let rfence_error_branch_index = words.len();
    words.push(b_type(0, 0, 10, 0x1));
    words.extend([i_type(0x33, 0, 0x0, 5, 0x13), 0x0010_0073]);
    let failure_index = words.len();
    words.extend([i_type(0x7e, 0, 0x0, 5, 0x13), 0x0010_0073]);
    words[hsm_error_branch_index] = b_type(
        ((failure_index - hsm_error_branch_index) * 4) as i32,
        0,
        10,
        0x1,
    );
    words[rfence_error_branch_index] = b_type(
        ((failure_index - rfence_error_branch_index) * 4) as i32,
        0,
        10,
        0x1,
    );

    let secondary_index = words.len();
    words.extend([i_type(1, 0, 0x0, 5, 0x13), j_type(0, 0)]);
    words[secondary_auipc_index + 1] = i_type(
        ((secondary_index - secondary_auipc_index) * 4) as i32,
        11,
        0x0,
        11,
        0x13,
    );

    let elf = riscv64_elf(RISCV_SBI_ENTRY, RISCV_SBI_ENTRY, &riscv64_program(&words));
    let path = temp_binary("riscv-sbi-remote-fence-i", &elf);

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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"cores\":2"));
    assert!(stdout.contains("\"trap\":\"breakpoint\""));
    assert!(stdout.contains("\"x5\":\"0x33\""));
    assert!(stdout.contains(
        "\"riscv_sbi_rfences\":[{\"source_cpu\":0,\"function\":0,\"hart_mask\":\"0x2\",\"hart_mask_base\":\"0x0\",\"start_addr\":\"0x0\",\"size\":\"0x0\",\"address_space\":null,\"targets\":[1]}]"
    ));
    assert_stat(
        &stdout,
        "sim.riscv.sbi.rfence.requests",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.riscv.sbi.rfence.targets",
        "Count",
        1,
        "constant",
    );
}

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
