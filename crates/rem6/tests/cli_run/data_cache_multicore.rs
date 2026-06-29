use std::process::Command;

use crate::support::*;

#[test]
fn rem6_run_routes_two_cores_through_shared_msi_data_cache() {
    assert_multicore_data_cache("msi", "data_cache_msi_runs", "sim.data_cache.msi.runs", 2);
}

#[test]
fn rem6_run_routes_two_cores_through_shared_mesi_data_cache() {
    assert_multicore_data_cache(
        "mesi",
        "data_cache_mesi_runs",
        "sim.data_cache.mesi.runs",
        2,
    );
}

#[test]
fn rem6_run_routes_two_cores_through_shared_moesi_data_cache() {
    assert_multicore_data_cache(
        "moesi",
        "data_cache_moesi_runs",
        "sim.data_cache.moesi.runs",
        2,
    );
}

#[test]
fn rem6_run_routes_two_cores_through_shared_chi_data_cache() {
    assert_multicore_data_cache("chi", "data_cache_chi_runs", "sim.data_cache.chi.runs", 2);
}

#[test]
fn rem6_run_routes_three_cores_through_shared_msi_data_cache() {
    assert_multicore_data_cache("msi", "data_cache_msi_runs", "sim.data_cache.msi.runs", 3);
}

#[test]
fn rem6_run_routes_three_cores_through_shared_mesi_data_cache() {
    assert_multicore_data_cache(
        "mesi",
        "data_cache_mesi_runs",
        "sim.data_cache.mesi.runs",
        3,
    );
}

#[test]
fn rem6_run_routes_three_cores_through_shared_moesi_data_cache() {
    assert_multicore_data_cache(
        "moesi",
        "data_cache_moesi_runs",
        "sim.data_cache.moesi.runs",
        3,
    );
}

#[test]
fn rem6_run_routes_three_cores_through_shared_chi_data_cache() {
    assert_multicore_data_cache("chi", "data_cache_chi_runs", "sim.data_cache.chi.runs", 3);
}

#[test]
fn rem6_run_routes_multicore_fabric_with_configured_qos_queue_policy() {
    const DATA_OFFSET: usize = 88;

    let mut program = riscv64_program(&[
        csr_read(0xf14, 5),                                 // csrr x5, mhartid
        b_type(36, 0, 5, 0x0),                              // beq x5, x0, core0 path
        u_type(0, 2, 0x17),                                 // auipc x2, 0
        i_type((DATA_OFFSET - 8) as i32, 2, 0x0, 2, 0x13),  // addi x2, x2, data
        i_type(0, 2, 0x3, 6, 0x03),                         // ld x6, 0(x2)
        i_type(20, 0, 0x0, 8, 0x13),                        // addi x8, x0, 20
        i_type(-1, 8, 0x0, 8, 0x13),                        // addi x8, x8, -1
        b_type(-4, 0, 8, 0x1),                              // bne x8, x0, loop
        i_type(0, 2, 0x3, 7, 0x03),                         // ld x7, 0(x2)
        0x0000_0073,                                        // ecall
        i_type(8, 0, 0x0, 8, 0x13),                         // addi x8, x0, 8
        i_type(-1, 8, 0x0, 8, 0x13),                        // addi x8, x8, -1
        b_type(-4, 0, 8, 0x1),                              // bne x8, x0, loop
        u_type(0, 2, 0x17),                                 // auipc x2, 0
        i_type((DATA_OFFSET - 52) as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data
        i_type(7, 0, 0x0, 9, 0x13),                         // addi x9, x0, 7
        s_type(0, 9, 2, 0x3),                               // sd x9, 0(x2)
        i_type(40, 0, 0x0, 8, 0x13),                        // addi x8, x0, 40
        i_type(-1, 8, 0x0, 8, 0x13),                        // addi x8, x8, -1
        b_type(-4, 0, 8, 0x1),                              // bne x8, x0, loop
        0x0000_0073,                                        // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&3u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("multicore-fabric-qos-queue-policy", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "400",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "3",
            "--parallel-workers",
            "3",
            "--memory-system",
            "cache-fabric-dram",
            "--data-cache-protocol",
            "msi",
            "--fabric-bandwidth-bytes-per-tick",
            "4",
            "--fabric-credit-depth",
            "2",
            "--fabric-qos-queue-policy",
            "least-recently-granted",
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
    assert!(stdout.contains("\"cores\":3"));
    assert!(stdout.contains("\"qos_queue_policy\":\"least-recently-granted\""));
    assert!(stdout.contains("\"x7\":\"0x7\""));
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.queue_delay_ticks",
        "Tick",
        0,
        "monotonic",
    );
}

fn assert_multicore_data_cache(
    protocol: &str,
    summary_field: &str,
    protocol_stat: &str,
    cores: u32,
) {
    const DATA_OFFSET: usize = 88;
    let expected_runs = 1 + 2 * u64::from(cores - 1);

    let mut program = riscv64_program(&[
        csr_read(0xf14, 5),                                 // csrr x5, mhartid
        b_type(36, 0, 5, 0x0),                              // beq x5, x0, core0 path
        u_type(0, 2, 0x17),                                 // auipc x2, 0
        i_type((DATA_OFFSET - 8) as i32, 2, 0x0, 2, 0x13),  // addi x2, x2, data
        i_type(0, 2, 0x3, 6, 0x03),                         // ld x6, 0(x2)
        i_type(50, 0, 0x0, 8, 0x13),                        // addi x8, x0, 50
        i_type(-1, 8, 0x0, 8, 0x13),                        // addi x8, x8, -1
        b_type(-4, 0, 8, 0x1),                              // bne x8, x0, loop
        i_type(0, 2, 0x3, 7, 0x03),                         // ld x7, 0(x2)
        0x0000_0073,                                        // ecall
        i_type(10, 0, 0x0, 8, 0x13),                        // addi x8, x0, 10
        i_type(-1, 8, 0x0, 8, 0x13),                        // addi x8, x8, -1
        b_type(-4, 0, 8, 0x1),                              // bne x8, x0, loop
        u_type(0, 2, 0x17),                                 // auipc x2, 0
        i_type((DATA_OFFSET - 52) as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data
        i_type(7, 0, 0x0, 9, 0x13),                         // addi x9, x0, 7
        s_type(0, 9, 2, 0x3),                               // sd x9, 0(x2)
        i_type(100, 0, 0x0, 8, 0x13),                       // addi x8, x0, 100
        i_type(-1, 8, 0x0, 8, 0x13),                        // addi x8, x8, -1
        b_type(-4, 0, 8, 0x1),                              // bne x8, x0, loop
        0x0000_0073,                                        // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&3u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(&format!("multicore-{cores}-{protocol}-data-cache"), &elf);

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
            &cores.to_string(),
            "--data-cache-protocol",
            protocol,
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
    assert!(stdout.contains(&format!("\"cores\":{cores}")));
    assert!(stdout.contains("\"cpu\":0"));
    assert!(stdout.contains("\"cpu\":1"));
    if cores == 3 {
        assert!(stdout.contains("\"cpu\":2"));
    }
    assert!(stdout.contains("\"x6\":\"0x3\""));
    assert!(stdout.contains("\"x7\":\"0x7\""));
    assert!(stdout.contains(&format!("\"data_cache_runs\":{expected_runs}")));
    assert!(stdout.contains(&format!("\"{summary_field}\":{expected_runs}")));
    assert!(stdout.contains(&format!("\"data_cache_cpu_responses\":{expected_runs}")));
    assert_stat(
        &stdout,
        "sim.data_cache.runs",
        "Count",
        expected_runs,
        "monotonic",
    );
    assert_stat(&stdout, protocol_stat, "Count", expected_runs, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.cpu_responses",
        "Count",
        expected_runs,
        "monotonic",
    );
    assert_stat(&stdout, "sim.cpu0.data.loads", "Count", 0, "monotonic");
    assert_stat(&stdout, "sim.cpu0.data.stores", "Count", 1, "monotonic");
    assert_stat(&stdout, "sim.cpu1.data.loads", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.cpu1.data.stores", "Count", 0, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.data.route1.source.cpu0.dmem", 1, 2, 2);
    assert_transport_stats(&stdout, "sim.memory.data.route3.source.cpu1.dmem", 2, 4, 2);
    if cores == 3 {
        assert_stat(&stdout, "sim.cpu2.data.loads", "Count", 2, "monotonic");
        assert_stat(&stdout, "sim.cpu2.data.stores", "Count", 0, "monotonic");
        assert_transport_stats(&stdout, "sim.memory.data.route5.source.cpu2.dmem", 2, 4, 2);
    }
}

#[test]
fn rem6_run_routes_two_cores_through_msi_instruction_cache() {
    assert_multicore_instruction_cache(
        "msi",
        "instruction_cache_msi_runs",
        "sim.instruction_cache.msi.runs",
        2,
    );
}

#[test]
fn rem6_run_routes_two_cores_through_mesi_instruction_cache() {
    assert_multicore_instruction_cache(
        "mesi",
        "instruction_cache_mesi_runs",
        "sim.instruction_cache.mesi.runs",
        2,
    );
}

#[test]
fn rem6_run_routes_two_cores_through_moesi_instruction_cache() {
    assert_multicore_instruction_cache(
        "moesi",
        "instruction_cache_moesi_runs",
        "sim.instruction_cache.moesi.runs",
        2,
    );
}

#[test]
fn rem6_run_routes_two_cores_through_chi_instruction_cache() {
    assert_multicore_instruction_cache(
        "chi",
        "instruction_cache_chi_runs",
        "sim.instruction_cache.chi.runs",
        2,
    );
}

#[test]
fn rem6_run_routes_three_cores_through_msi_instruction_cache() {
    assert_multicore_instruction_cache(
        "msi",
        "instruction_cache_msi_runs",
        "sim.instruction_cache.msi.runs",
        3,
    );
}

#[test]
fn rem6_run_routes_three_cores_through_mesi_instruction_cache() {
    assert_multicore_instruction_cache(
        "mesi",
        "instruction_cache_mesi_runs",
        "sim.instruction_cache.mesi.runs",
        3,
    );
}

#[test]
fn rem6_run_routes_three_cores_through_moesi_instruction_cache() {
    assert_multicore_instruction_cache(
        "moesi",
        "instruction_cache_moesi_runs",
        "sim.instruction_cache.moesi.runs",
        3,
    );
}

#[test]
fn rem6_run_routes_three_cores_through_chi_instruction_cache() {
    assert_multicore_instruction_cache(
        "chi",
        "instruction_cache_chi_runs",
        "sim.instruction_cache.chi.runs",
        3,
    );
}

fn assert_multicore_instruction_cache(
    protocol: &str,
    summary_field: &str,
    protocol_stat: &str,
    cores: u32,
) {
    let expected_runs = 6 * u64::from(cores);
    let expected_directory_decisions = 2 * u64::from(cores);

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),        // sd x6, 8(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    program.extend_from_slice(&[0; 16]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(
        &format!("multicore-{cores}-{protocol}-instruction-cache"),
        &elf,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            &cores.to_string(),
            "--instruction-cache-protocol",
            protocol,
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
    assert!(stdout.contains(&format!("\"cores\":{cores}")));
    assert!(stdout.contains("\"cpu\":0"));
    assert!(stdout.contains("\"cpu\":1"));
    if cores == 3 {
        assert!(stdout.contains("\"cpu\":2"));
    }
    assert!(stdout.contains("\"data_cache_runs\":0"));
    assert!(stdout.contains(&format!("\"instruction_cache_runs\":{expected_runs}")));
    assert!(stdout.contains(&format!("\"{summary_field}\":{expected_runs}")));
    assert!(stdout.contains(&format!(
        "\"instruction_cache_cpu_responses\":{expected_runs}"
    )));
    assert!(stdout.contains(&format!(
        "\"instruction_cache_directory_decisions\":{expected_directory_decisions}"
    )));
    assert_stat(
        &stdout,
        "sim.instruction_cache.runs",
        "Count",
        expected_runs,
        "monotonic",
    );
    assert_stat(&stdout, protocol_stat, "Count", expected_runs, "monotonic");
    assert_stat(
        &stdout,
        "sim.instruction_cache.cpu_responses",
        "Count",
        expected_runs,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.directory_decisions",
        "Count",
        expected_directory_decisions,
        "monotonic",
    );
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        6,
        12,
        2,
    );
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route2.source.cpu1.ifetch",
        6,
        12,
        2,
    );
    if cores == 3 {
        assert_transport_stats(
            &stdout,
            "sim.memory.fetch.route4.source.cpu2.ifetch",
            6,
            12,
            2,
        );
    }
}
