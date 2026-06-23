use std::{collections::BTreeSet, process::Command};

use serde_json::Value;

use crate::support::*;

#[test]
fn rem6_run_executes_riscv_elf_load_store_and_emits_data_stats() {
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
    let path = temp_binary("data-exec", &elf);

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000020:8",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_eq!(
        json.pointer("/instruction_cache_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/instruction_cache_l2_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/instruction_cache_l3_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_protocol").and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_l2_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_l3_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));
    assert!(stdout.contains("\"data_loads\":1"));
    assert!(stdout.contains("\"data_stores\":1"));
    assert!(stdout.contains("\"in_order_pipeline\":{\"cycles\":49,\"in_flight\":0,"));
    assert!(stdout.contains(
        "\"stage_in_flight\":{\"fetch1\":0,\"fetch2\":0,\"decode\":0,\"execute\":0,\"commit\":0}"
    ));
    assert!(stdout.contains("\"retired\":6"));
    assert!(stdout.contains("\"resource_blocked\":28"));
    assert!(stdout.contains("\"stall_cycles\":28"));
    assert!(stdout.contains("\"fetch_wait_cycles\":24"));
    assert!(stdout.contains("\"data_wait_cycles\":8"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"bytes\":8"));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
    assert!(json_u64(&json, "/dram/accesses") > 0);
    assert!(json_u64(&json, "/fabric/transfers") > 0);
    assert!(json_u64(&json, "/simulation/instruction_cache_l2_runs") > 0);
    assert!(json_u64(&json, "/simulation/instruction_cache_l3_runs") > 0);
    assert!(json_u64(&json, "/simulation/data_cache_l2_runs") > 0);
    assert!(json_u64(&json, "/simulation/data_cache_l3_runs") > 0);
    assert!(stdout.contains("\"path\":\"sim.data.loads\""));
    assert!(stdout.contains("\"path\":\"sim.data.stores\""));
    assert_stat(&stdout, "sim.data.load_bytes", "Byte", 8, "monotonic");
    assert_stat(&stdout, "sim.data.store_bytes", "Byte", 8, "monotonic");
    assert!(stdout.contains("\"path\":\"sim.memory.dumps\""));
    assert!(stdout.contains("\"path\":\"sim.cpu0.data.loads\""));
    assert!(stdout.contains("\"path\":\"sim.cpu0.data.stores\""));
    assert_stat(&stdout, "sim.cpu0.data.load_bytes", "Byte", 8, "monotonic");
    assert_stat(&stdout, "sim.cpu0.data.store_bytes", "Byte", 8, "monotonic");
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        49,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.retired",
        "Count",
        6,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.stall_cycles",
        "Cycle",
        28,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.fetch_wait_cycles",
        "Cycle",
        24,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.resource_blocked",
        "Count",
        28,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.data_wait_cycles",
        "Cycle",
        8,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(&stdout, "sim.memory.dram.accesses", "Count", 0, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 6, 24, 4);
    assert_transport_stats(&stdout, "sim.memory.data", 2, 8, 4);
}

#[test]
fn rem6_run_executes_riscv_elf_fetches_through_msi_instruction_cache() {
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
    let path = temp_binary("instruction-cache-exec", &elf);

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--dump-memory",
            "0x80000020:8",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));
    assert!(stdout.contains("\"data_cache_runs\":0"));
    assert!(stdout.contains("\"instruction_cache_runs\":6"));
    assert!(stdout.contains("\"instruction_cache_msi_runs\":6"));
    assert!(stdout.contains("\"instruction_cache_cpu_responses\":6"));
    assert!(stdout.contains("\"instruction_cache_directory_decisions\":2"));
    assert!(stdout.contains("\"instruction_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
    assert_stat(&stdout, "sim.data_cache.runs", "Count", 0, "monotonic");
    assert_stat(
        &stdout,
        "sim.instruction_cache.runs",
        "Count",
        6,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.msi.runs",
        "Count",
        6,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.cpu_responses",
        "Count",
        6,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.directory_decisions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.dram_accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_transport_stats(&stdout, "sim.memory.fetch", 6, 12, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 2, 4, 2);
}

#[test]
fn rem6_run_executes_riscv_elf_load_store_through_msi_data_cache() {
    const DATA_OFFSET: usize = 32;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(0, 2, 0x3, 7, 0x03),                  // ld x7, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    program.extend_from_slice(&[0; 16]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("data-cache-exec", &elf);

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
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--dump-memory",
            "0x80000028:8",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));
    assert!(stdout.contains("\"x7\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"data_loads\":2"));
    assert!(stdout.contains("\"data_stores\":1"));
    assert!(stdout.contains("\"data_cache_runs\":3"));
    assert!(stdout.contains("\"data_cache_msi_runs\":3"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":3"));
    assert!(stdout.contains("\"data_cache_directory_decisions\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"data_cache_bank_accepted\":3"));
    assert!(stdout.contains("\"data_cache_bank_immediate_hits\":1"));
    assert!(stdout.contains("\"data_cache_bank_scheduled_misses\":2"));
    assert!(stdout.contains("\"data_cache_bank_coalesced_misses\":0"));
    assert!(stdout.contains("\"address\":\"0x80000028\""));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
    assert_stat(&stdout, "sim.data_cache.runs", "Count", 3, "monotonic");
    assert_stat(&stdout, "sim.data_cache.msi.runs", "Count", 3, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.cpu_responses",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.directory_decisions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.dram_accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.bank.accepted",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.bank.immediate_hits",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.bank.scheduled_misses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.bank.coalesced_misses",
        "Count",
        0,
        "monotonic",
    );
    assert_transport_stats(&stdout, "sim.memory.fetch", 7, 14, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 3, 6, 2);
}

#[test]
fn rem6_run_counts_dram_backed_msi_data_cache_line_fills_once() {
    const DATA_OFFSET: usize = 32;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(8, 2, 0x3, 6, 0x03),                  // ld x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-backed-msi-data-cache-fill", &elf);

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
            "--cores",
            "1",
            "--dram-memory",
            "--data-cache-protocol",
            "msi",
            "--dump-memory",
            "0x80000020:16",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x99aabbccddeeff00\""));
    assert!(stdout.contains("\"data_loads\":2"));
    assert!(stdout.contains("\"data_cache_runs\":2"));
    assert!(stdout.contains("\"data_cache_msi_runs\":2"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":1"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"887766554433221100ffeeddccbbaa99\""));
    assert_stat(&stdout, "sim.data_cache.runs", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.data_cache.msi.runs", "Count", 2, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.cpu_responses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.dram_accesses",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_routes_instruction_cache_miss_through_l2_and_dram() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(2, 5, 0x0, 6, 0x13), // addi x6, x5, 2
        i_type(3, 6, 0x0, 7, 0x13), // addi x7, x6, 3
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("l2-dram-backed-msi-instruction-cache-fill", &elf);

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
            "--cores",
            "1",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
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
    assert!(stdout.contains("\"instruction_cache_protocol\":\"msi\""));
    assert!(stdout.contains("\"instruction_cache_l2_protocol\":\"msi\""));
    assert!(stdout.contains("\"data_cache_runs\":0"));
    assert!(stdout.contains("\"instruction_cache_runs\":4"));
    assert!(stdout.contains("\"instruction_cache_msi_runs\":4"));
    assert!(stdout.contains("\"instruction_cache_cpu_responses\":4"));
    assert!(stdout.contains("\"instruction_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"instruction_cache_l2_runs\":1"));
    assert!(stdout.contains("\"instruction_cache_l2_msi_runs\":1"));
    assert!(stdout.contains("\"instruction_cache_l2_cpu_responses\":0"));
    assert!(stdout.contains("\"instruction_cache_l2_dram_accesses\":1"));
    assert!(stdout.contains("\"cache\":{\"activity\":5,\"active\":2"));
    assert_stat(
        &stdout,
        "sim.instruction_cache.runs",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.dram_accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.l2.runs",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.l2.msi.runs",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.l2.cpu_responses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.l2.dram_accesses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.activity",
        "Count",
        5,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.active",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_routes_instruction_cache_miss_through_l3_and_dram() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(2, 5, 0x0, 6, 0x13), // addi x6, x5, 2
        i_type(3, 6, 0x0, 7, 0x13), // addi x7, x6, 3
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("l3-dram-backed-msi-instruction-cache-fill", &elf);

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
            "--cores",
            "1",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--instruction-cache-l3-protocol",
            "msi",
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
    assert!(stdout.contains("\"instruction_cache_l3_protocol\":\"msi\""));
    assert!(stdout.contains("\"instruction_cache_runs\":4"));
    assert!(stdout.contains("\"instruction_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"instruction_cache_l2_runs\":1"));
    assert!(stdout.contains("\"instruction_cache_l2_dram_accesses\":0"));
    assert!(stdout.contains("\"instruction_cache_l3_runs\":1"));
    assert!(stdout.contains("\"instruction_cache_l3_msi_runs\":1"));
    assert!(stdout.contains("\"instruction_cache_l3_cpu_responses\":0"));
    assert!(stdout.contains("\"instruction_cache_l3_dram_accesses\":1"));
    assert!(stdout.contains("\"cache\":{\"activity\":6,\"active\":3"));
    assert_stat(
        &stdout,
        "sim.instruction_cache.l3.runs",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.l3.dram_accesses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.activity",
        "Count",
        6,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.active",
        "Count",
        3,
        "monotonic",
    );
}

#[test]
fn rem6_run_routes_data_cache_miss_through_l2_and_dram() {
    const DATA_OFFSET: usize = 32;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(8, 2, 0x3, 6, 0x03),                  // ld x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("l2-dram-backed-msi-data-cache-fill", &elf);

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
            "--cores",
            "1",
            "--dram-memory",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--dump-memory",
            "0x80000020:16",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x99aabbccddeeff00\""));
    assert!(stdout.contains("\"data_loads\":2"));
    assert!(stdout.contains("\"data_cache_runs\":2"));
    assert!(stdout.contains("\"data_cache_msi_runs\":2"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"data_cache_l2_runs\":1"));
    assert!(stdout.contains("\"data_cache_l2_msi_runs\":1"));
    assert!(stdout.contains("\"data_cache_l2_cpu_responses\":0"));
    assert!(stdout.contains("\"data_cache_l2_dram_accesses\":1"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"887766554433221100ffeeddccbbaa99\""));
    assert_stat(&stdout, "sim.data_cache.runs", "Count", 2, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.dram_accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(&stdout, "sim.data_cache.l2.runs", "Count", 1, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.l2.msi.runs",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.l2.cpu_responses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.l2.dram_accesses",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_routes_data_cache_miss_through_l3_and_dram() {
    const DATA_OFFSET: usize = 32;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(8, 2, 0x3, 6, 0x03),                  // ld x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("l3-dram-backed-msi-data-cache-fill", &elf);

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
            "--cores",
            "1",
            "--dram-memory",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--data-cache-l3-protocol",
            "msi",
            "--dump-memory",
            "0x80000020:16",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x99aabbccddeeff00\""));
    assert!(stdout.contains("\"data_cache_l3_protocol\":\"msi\""));
    assert!(stdout.contains("\"data_cache_runs\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"data_cache_l2_runs\":1"));
    assert!(stdout.contains("\"data_cache_l2_dram_accesses\":0"));
    assert!(stdout.contains("\"data_cache_l3_runs\":1"));
    assert!(stdout.contains("\"data_cache_l3_msi_runs\":1"));
    assert!(stdout.contains("\"data_cache_l3_cpu_responses\":0"));
    assert!(stdout.contains("\"data_cache_l3_dram_accesses\":1"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"887766554433221100ffeeddccbbaa99\""));
    assert_stat(&stdout, "sim.data_cache.l3.runs", "Count", 1, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.l3.dram_accesses",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_routes_cache_dram_traffic_through_configured_fabric() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("run-fabric-cache-dram", &elf);

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
            "--cores",
            "1",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--instruction-cache-l3-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--data-cache-l3-protocol",
            "msi",
            "--fabric-link",
            "cpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "8",
            "--fabric-request-virtual-network",
            "3",
            "--fabric-response-virtual-network",
            "4",
            "--fabric-credit-depth",
            "2",
            "--dump-memory",
            "0x80000048:8",
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
    let fabric = json.pointer("/fabric").expect("run fabric summary");

    assert_eq!(fabric.get("link").and_then(Value::as_str), Some("cpu_mem"));
    assert_eq!(
        fabric
            .get("bandwidth_bytes_per_tick")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        fabric
            .get("request_virtual_network")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        fabric
            .get("response_virtual_network")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(fabric.get("credit_depth").and_then(Value::as_u64), Some(2));
    assert!(
        fabric
            .get("active_lanes")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            >= 2
    );
    assert_eq!(
        fabric
            .get("active_virtual_networks")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert!(fabric.get("transfers").and_then(Value::as_u64).unwrap_or(0) > 0);
    let fabric_transfers = fabric
        .get("transfers")
        .and_then(Value::as_u64)
        .expect("fabric transfers");
    let fabric_active_lanes = fabric
        .get("active_lanes")
        .and_then(Value::as_u64)
        .expect("fabric active lanes");
    let fabric_active_virtual_networks = fabric
        .get("active_virtual_networks")
        .and_then(Value::as_u64)
        .expect("fabric active virtual networks");
    let fabric_bytes = fabric
        .get("bytes")
        .and_then(Value::as_u64)
        .expect("fabric bytes");
    let fabric_flits = fabric
        .get("flits")
        .and_then(Value::as_u64)
        .expect("fabric flits");
    let fabric_occupied_ticks = fabric
        .get("occupied_ticks")
        .and_then(Value::as_u64)
        .expect("fabric occupied ticks");
    let fabric_queue_delay_ticks = fabric
        .get("queue_delay_ticks")
        .and_then(Value::as_u64)
        .expect("fabric queue delay ticks");
    let fabric_max_queue_delay_ticks = fabric
        .get("max_queue_delay_ticks")
        .and_then(Value::as_u64)
        .expect("fabric max queue delay ticks");
    let fabric_credit_delay_ticks = fabric
        .get("credit_delay_ticks")
        .and_then(Value::as_u64)
        .expect("fabric credit delay ticks");
    let fabric_max_credit_delay_ticks = fabric
        .get("max_credit_delay_ticks")
        .and_then(Value::as_u64)
        .expect("fabric max credit delay ticks");
    let fabric_contended_lanes = fabric
        .get("contended_lanes")
        .and_then(Value::as_u64)
        .expect("fabric contended lanes");
    let memory_resources = json
        .pointer("/memory_resources")
        .expect("memory resource summary");
    let cache_activity = memory_resources
        .pointer("/cache/activity")
        .and_then(Value::as_u64)
        .expect("cache resource activity");
    let transport_activity = memory_resources
        .pointer("/transport/activity")
        .and_then(Value::as_u64)
        .expect("transport resource activity");
    let dram_activity = memory_resources
        .pointer("/dram/activity")
        .and_then(Value::as_u64)
        .expect("DRAM resource activity");
    let cache_active = memory_resources
        .pointer("/cache/active")
        .and_then(Value::as_u64)
        .expect("active cache resources");
    let transport_active = memory_resources
        .pointer("/transport/active")
        .and_then(Value::as_u64)
        .expect("active transport resources");
    let fetch_transport = json.pointer("/transport/fetch").expect("fetch transport");
    let data_transport = json.pointer("/transport/data").expect("data transport");
    let transport_fetch_activity = memory_resources
        .pointer("/transport/fetch/activity")
        .and_then(Value::as_u64)
        .expect("fetch transport resource activity");
    let transport_data_activity = memory_resources
        .pointer("/transport/data/activity")
        .and_then(Value::as_u64)
        .expect("data transport resource activity");
    let transport_fetch_request_arrivals = memory_resources
        .pointer("/transport/fetch/request_arrivals")
        .and_then(Value::as_u64)
        .expect("fetch transport resource request arrivals");
    let transport_data_request_arrivals = memory_resources
        .pointer("/transport/data/request_arrivals")
        .and_then(Value::as_u64)
        .expect("data transport resource request arrivals");
    let transport_fetch_responses = memory_resources
        .pointer("/transport/fetch/responses")
        .and_then(Value::as_u64)
        .expect("fetch transport resource responses");
    let transport_data_responses = memory_resources
        .pointer("/transport/data/responses")
        .and_then(Value::as_u64)
        .expect("data transport resource responses");
    let transport_fetch_response_arrivals = memory_resources
        .pointer("/transport/fetch/response_arrivals")
        .and_then(Value::as_u64)
        .expect("fetch transport resource response arrivals");
    let transport_data_response_arrivals = memory_resources
        .pointer("/transport/data/response_arrivals")
        .and_then(Value::as_u64)
        .expect("data transport resource response arrivals");
    let transport_fetch_round_trip_ticks = memory_resources
        .pointer("/transport/fetch/round_trip_ticks")
        .and_then(Value::as_u64)
        .expect("fetch transport resource round trip ticks");
    let transport_data_round_trip_ticks = memory_resources
        .pointer("/transport/data/round_trip_ticks")
        .and_then(Value::as_u64)
        .expect("data transport resource round trip ticks");
    let transport_fetch_max_round_trip_ticks = memory_resources
        .pointer("/transport/fetch/max_round_trip_ticks")
        .and_then(Value::as_u64)
        .expect("fetch transport resource max round trip ticks");
    let transport_data_max_round_trip_ticks = memory_resources
        .pointer("/transport/data/max_round_trip_ticks")
        .and_then(Value::as_u64)
        .expect("data transport resource max round trip ticks");
    let transport_fetch_active = memory_resources
        .pointer("/transport/fetch/active")
        .and_then(Value::as_u64)
        .expect("active fetch transport resources");
    let transport_data_active = memory_resources
        .pointer("/transport/data/active")
        .and_then(Value::as_u64)
        .expect("active data transport resources");
    let dram_active = memory_resources
        .pointer("/dram/active")
        .and_then(Value::as_u64)
        .expect("active DRAM resources");
    assert_eq!(
        memory_resources
            .pointer("/fabric/activity")
            .and_then(Value::as_u64),
        Some(fabric_transfers)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/active")
            .and_then(Value::as_u64),
        Some(fabric_active_lanes)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/active_virtual_networks")
            .and_then(Value::as_u64),
        Some(fabric_active_virtual_networks)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/bytes")
            .and_then(Value::as_u64),
        Some(fabric_bytes)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/flits")
            .and_then(Value::as_u64),
        Some(fabric_flits)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/occupied_ticks")
            .and_then(Value::as_u64),
        Some(fabric_occupied_ticks)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/queue_delay_ticks")
            .and_then(Value::as_u64),
        Some(fabric_queue_delay_ticks)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/max_queue_delay_ticks")
            .and_then(Value::as_u64),
        Some(fabric_max_queue_delay_ticks)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/credit_delay_ticks")
            .and_then(Value::as_u64),
        Some(fabric_credit_delay_ticks)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/max_credit_delay_ticks")
            .and_then(Value::as_u64),
        Some(fabric_max_credit_delay_ticks)
    );
    assert_eq!(
        memory_resources
            .pointer("/fabric/contended_lanes")
            .and_then(Value::as_u64),
        Some(fabric_contended_lanes)
    );
    assert_eq!(
        transport_fetch_activity,
        fetch_transport
            .pointer("/requests")
            .and_then(Value::as_u64)
            .expect("fetch transport requests")
    );
    assert_eq!(
        transport_data_activity,
        data_transport
            .pointer("/requests")
            .and_then(Value::as_u64)
            .expect("data transport requests")
    );
    assert_eq!(
        transport_fetch_request_arrivals,
        fetch_transport
            .pointer("/request_arrivals")
            .and_then(Value::as_u64)
            .expect("fetch transport request arrivals")
    );
    assert_eq!(
        transport_data_request_arrivals,
        data_transport
            .pointer("/request_arrivals")
            .and_then(Value::as_u64)
            .expect("data transport request arrivals")
    );
    assert_eq!(
        transport_fetch_responses,
        fetch_transport
            .pointer("/responses")
            .and_then(Value::as_u64)
            .expect("fetch transport responses")
    );
    assert_eq!(
        transport_data_responses,
        data_transport
            .pointer("/responses")
            .and_then(Value::as_u64)
            .expect("data transport responses")
    );
    assert_eq!(
        transport_fetch_response_arrivals,
        fetch_transport
            .pointer("/response_arrivals")
            .and_then(Value::as_u64)
            .expect("fetch transport response arrivals")
    );
    assert_eq!(
        transport_data_response_arrivals,
        data_transport
            .pointer("/response_arrivals")
            .and_then(Value::as_u64)
            .expect("data transport response arrivals")
    );
    assert_eq!(
        transport_fetch_round_trip_ticks,
        fetch_transport
            .pointer("/round_trip_ticks")
            .and_then(Value::as_u64)
            .expect("fetch transport round trip ticks")
    );
    assert_eq!(
        transport_data_round_trip_ticks,
        data_transport
            .pointer("/round_trip_ticks")
            .and_then(Value::as_u64)
            .expect("data transport round trip ticks")
    );
    assert_eq!(
        transport_fetch_max_round_trip_ticks,
        fetch_transport
            .pointer("/max_round_trip_ticks")
            .and_then(Value::as_u64)
            .expect("fetch transport max round trip ticks")
    );
    assert_eq!(
        transport_data_max_round_trip_ticks,
        data_transport
            .pointer("/max_round_trip_ticks")
            .and_then(Value::as_u64)
            .expect("data transport max round trip ticks")
    );
    assert_eq!(
        transport_fetch_active,
        u64::from(transport_fetch_activity != 0)
    );
    assert_eq!(
        transport_data_active,
        u64::from(transport_data_activity != 0)
    );
    assert_eq!(
        transport_activity,
        transport_fetch_activity.saturating_add(transport_data_activity)
    );
    assert_eq!(
        transport_active,
        transport_fetch_active.saturating_add(transport_data_active)
    );
    assert_eq!(
        memory_resources
            .pointer("/activity")
            .and_then(Value::as_u64),
        Some(
            cache_activity
                .saturating_add(transport_activity)
                .saturating_add(fabric_transfers)
                .saturating_add(dram_activity)
        )
    );
    let expected_active_memory_resources = cache_active
        .saturating_add(transport_active)
        .saturating_add(fabric_active_lanes)
        .saturating_add(dram_active);
    assert_eq!(
        memory_resources.pointer("/active").and_then(Value::as_u64),
        Some(expected_active_memory_resources)
    );
    assert!(fabric_bytes > 0);
    assert!(fabric_flits > 0);
    assert!(fabric
        .get("lane_activities")
        .and_then(Value::as_array)
        .is_some_and(|lanes| {
            lanes.len() >= 2
                && lanes.iter().all(|lane| {
                    lane.get("flit_count").and_then(Value::as_u64).is_some()
                        && lane
                            .get("credit_delay_ticks")
                            .and_then(Value::as_u64)
                            .is_some()
                        && lane
                            .get("max_credit_delay_ticks")
                            .and_then(Value::as_u64)
                            .is_some()
                })
        }));
    assert!(fabric
        .get("hop_activities")
        .and_then(Value::as_array)
        .is_some_and(|hops| {
            !hops.is_empty()
                && hops.iter().all(|hop| {
                    hop.get("flits").and_then(Value::as_u64).is_some()
                        && hop
                            .get("credit_delay_ticks")
                            .and_then(Value::as_u64)
                            .is_some()
                })
        }));
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(&stdout, "sim.memory.fabric.bytes", "Byte", 0, "monotonic");
    assert_stat_greater_than(&stdout, "sim.memory.fabric.flits", "Count", 0, "monotonic");
    assert_stat(
        &stdout,
        "sim.memory.fabric.credit_delay_ticks",
        "Tick",
        fabric_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.fabric.max_credit_delay_ticks",
        "Tick",
        fabric_max_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.activity",
        "Count",
        fabric_transfers,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.active",
        "Count",
        fabric_active_lanes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.active_virtual_networks",
        "Count",
        fabric_active_virtual_networks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.bytes",
        "Byte",
        fabric_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.flits",
        "Count",
        fabric_flits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.occupied_ticks",
        "Tick",
        fabric_occupied_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.queue_delay_ticks",
        "Tick",
        fabric_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.max_queue_delay_ticks",
        "Tick",
        fabric_max_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.credit_delay_ticks",
        "Tick",
        fabric_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.max_credit_delay_ticks",
        "Tick",
        fabric_max_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.fabric.contended_lanes",
        "Count",
        fabric_contended_lanes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.fetch.activity",
        "Count",
        transport_fetch_activity,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.fetch.round_trip_ticks",
        "Tick",
        transport_fetch_round_trip_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.fetch.request_arrivals",
        "Count",
        transport_fetch_request_arrivals,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.fetch.responses",
        "Count",
        transport_fetch_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.fetch.response_arrivals",
        "Count",
        transport_fetch_response_arrivals,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.fetch.max_round_trip_ticks",
        "Tick",
        transport_fetch_max_round_trip_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.fetch.active",
        "Count",
        transport_fetch_active,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.data.activity",
        "Count",
        transport_data_activity,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.data.round_trip_ticks",
        "Tick",
        transport_data_round_trip_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.data.request_arrivals",
        "Count",
        transport_data_request_arrivals,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.data.responses",
        "Count",
        transport_data_responses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.data.response_arrivals",
        "Count",
        transport_data_response_arrivals,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.data.max_round_trip_ticks",
        "Tick",
        transport_data_max_round_trip_ticks,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.transport.data.active",
        "Count",
        transport_data_active,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.active",
        "Count",
        expected_active_memory_resources,
        "monotonic",
    );
    assert_run_fabric_virtual_network_stats(&stdout, fabric, 3);
    assert_run_fabric_virtual_network_stats(&stdout, fabric, 4);
    assert_run_fabric_lane_stats(&stdout, fabric);
}

#[test]
fn rem6_run_sanitizes_configured_fabric_link_stat_paths() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("run-fabric-sanitized-link-stats", &elf);

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
            "--cores",
            "1",
            "--dram-memory",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--fabric-link",
            "cpu-mem.link 0",
            "--fabric-bandwidth-bytes-per-tick",
            "8",
            "--fabric-request-virtual-network",
            "3",
            "--fabric-response-virtual-network",
            "4",
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
    let fabric = json.pointer("/fabric").expect("run fabric summary");
    assert_eq!(
        fabric.get("link").and_then(Value::as_str),
        Some("cpu-mem.link 0")
    );
    assert_run_fabric_lane_stats(&stdout, fabric);
}

#[test]
fn rem6_run_memory_system_direct_keeps_cpu_on_direct_transport_path() {
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
    let path = temp_binary("run-direct-memory-system", &elf);

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
            "--memory-system",
            "direct",
            "--dump-memory",
            "0x80000020:8",
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
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("8977665544332211")
    );
    assert_eq!(json_u64(&json, "/simulation/instruction_cache_runs"), 0);
    assert_eq!(json_u64(&json, "/simulation/data_cache_runs"), 0);
    assert!(json.pointer("/fabric/transfers").is_none());
    assert_eq!(
        json.pointer("/dram/accesses").and_then(Value::as_u64),
        Some(0)
    );
    assert_transport_stats(&stdout, "sim.memory.fetch", 6, 12, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 2, 4, 2);
}

#[test]
fn rem6_run_memory_system_preset_routes_cpu_through_cache_fabric_and_dram() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("run-memory-system-preset", &elf);

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
            "--cores",
            "1",
            "--memory-system",
            "cache-fabric-dram",
            "--dump-memory",
            "0x80000048:8",
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
        json.pointer("/cores/0/registers/x5")
            .and_then(Value::as_str),
        Some("0x1122334455667788")
    );
    assert_eq!(
        json.pointer("/cores/0/registers/x6")
            .and_then(Value::as_str),
        Some("0x1122334455667789")
    );
    assert_eq!(
        json.pointer("/memory/0/address").and_then(Value::as_str),
        Some("0x80000048")
    );
    assert_eq!(
        json.pointer("/memory/0/bytes").and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("8977665544332211")
    );
    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_eq!(
        json.pointer("/instruction_cache_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/instruction_cache_l2_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/instruction_cache_l3_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_protocol").and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_l2_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_l3_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert!(json_u64(&json, "/dram/accesses") > 0);
    assert!(json_u64(&json, "/fabric/transfers") > 0);
    assert!(json_u64(&json, "/simulation/instruction_cache_l2_runs") > 0);
    assert!(json_u64(&json, "/simulation/instruction_cache_l3_runs") > 0);
    assert!(json_u64(&json, "/simulation/data_cache_l2_runs") > 0);
    assert!(json_u64(&json, "/simulation/data_cache_l3_runs") > 0);
    let cache_dram_accesses = json_u64(&json, "/memory_resources/cache/dram_accesses");
    let cache_l1_activity = json_u64(&json, "/memory_resources/cache/l1/activity");
    let cache_l2_activity = json_u64(&json, "/memory_resources/cache/l2/activity");
    let cache_l3_activity = json_u64(&json, "/memory_resources/cache/l3/activity");
    let cache_l1_dram_accesses = json_u64(&json, "/memory_resources/cache/l1/dram_accesses");
    let cache_l2_dram_accesses = json_u64(&json, "/memory_resources/cache/l2/dram_accesses");
    let cache_l3_dram_accesses = json_u64(&json, "/memory_resources/cache/l3/dram_accesses");
    let hierarchy_cache_dram_accesses =
        json_u64(&json, "/simulation/instruction_cache_dram_accesses")
            + json_u64(&json, "/simulation/instruction_cache_l2_dram_accesses")
            + json_u64(&json, "/simulation/instruction_cache_l3_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_l2_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_l3_dram_accesses");
    let hierarchy_cache_activity = json_u64(&json, "/simulation/instruction_cache_runs")
        + json_u64(&json, "/simulation/instruction_cache_l2_runs")
        + json_u64(&json, "/simulation/instruction_cache_l3_runs")
        + json_u64(&json, "/simulation/data_cache_runs")
        + json_u64(&json, "/simulation/data_cache_l2_runs")
        + json_u64(&json, "/simulation/data_cache_l3_runs");
    let lower_level_cache_dram_accesses =
        json_u64(&json, "/simulation/instruction_cache_l2_dram_accesses")
            + json_u64(&json, "/simulation/instruction_cache_l3_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_l2_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_l3_dram_accesses");
    assert_eq!(
        cache_l1_activity,
        json_u64(&json, "/simulation/instruction_cache_runs")
            + json_u64(&json, "/simulation/data_cache_runs")
    );
    assert_eq!(
        cache_l2_activity,
        json_u64(&json, "/simulation/instruction_cache_l2_runs")
            + json_u64(&json, "/simulation/data_cache_l2_runs")
    );
    assert_eq!(
        cache_l3_activity,
        json_u64(&json, "/simulation/instruction_cache_l3_runs")
            + json_u64(&json, "/simulation/data_cache_l3_runs")
    );
    assert_eq!(
        cache_l1_dram_accesses,
        json_u64(&json, "/simulation/instruction_cache_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_dram_accesses")
    );
    assert_eq!(
        cache_l2_dram_accesses,
        json_u64(&json, "/simulation/instruction_cache_l2_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_l2_dram_accesses")
    );
    assert_eq!(
        cache_l3_dram_accesses,
        json_u64(&json, "/simulation/instruction_cache_l3_dram_accesses")
            + json_u64(&json, "/simulation/data_cache_l3_dram_accesses")
    );
    assert_eq!(
        cache_l1_activity + cache_l2_activity + cache_l3_activity,
        hierarchy_cache_activity
    );
    assert_eq!(cache_dram_accesses, hierarchy_cache_dram_accesses);
    assert_eq!(
        cache_l1_dram_accesses + cache_l2_dram_accesses + cache_l3_dram_accesses,
        hierarchy_cache_dram_accesses
    );
    assert!(lower_level_cache_dram_accesses > 0);
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.l1.activity",
        "Count",
        cache_l1_activity,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.l2.activity",
        "Count",
        cache_l2_activity,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.l3.activity",
        "Count",
        cache_l3_activity,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.dram_accesses",
        "Count",
        cache_dram_accesses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.l1.dram_accesses",
        "Count",
        cache_l1_dram_accesses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.l2.dram_accesses",
        "Count",
        cache_l2_dram_accesses,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.l3.dram_accesses",
        "Count",
        cache_l3_dram_accesses,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(&stdout, "sim.memory.dram.accesses", "Count", 0, "monotonic");
}

#[test]
fn rem6_run_defaults_riscv_cpu_to_cache_fabric_dram_hierarchy() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("run-default-memory-hierarchy", &elf);

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000048:8",
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
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_eq!(
        json.pointer("/instruction_cache_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/instruction_cache_l2_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/instruction_cache_l3_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_protocol").and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_l2_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/data_cache_l3_protocol")
            .and_then(Value::as_str),
        Some("msi")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("8977665544332211")
    );
    assert!(json_u64(&json, "/dram/accesses") > 0);
    assert!(json_u64(&json, "/fabric/transfers") > 0);
    assert!(json_u64(&json, "/simulation/instruction_cache_l2_runs") > 0);
    assert!(json_u64(&json, "/simulation/instruction_cache_l3_runs") > 0);
    assert!(json_u64(&json, "/simulation/data_cache_l2_runs") > 0);
    assert!(json_u64(&json, "/simulation/data_cache_l3_runs") > 0);
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(&stdout, "sim.memory.dram.accesses", "Count", 0, "monotonic");
}

#[test]
fn rem6_run_toml_memory_system_preset_routes_cpu_through_cache_fabric_and_dram() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let binary = temp_binary("toml-run-memory-system-preset", &elf);
    let config = temp_config(
        "toml-run-memory-system-preset",
        &format!(
            "[run]\nisa = \"riscv\"\nbinary = \"{}\"\nmax_tick = 240\nstats_format = \"json\"\nexecute = true\nmemory_system = \"cache-fabric-dram\"\nmemory_dumps = [\"0x80000048:8\"]\n",
            binary.display()
        ),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(["run", "--config", config.to_str().unwrap()])
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
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("8977665544332211")
    );
    assert!(json_u64(&json, "/dram/accesses") > 0);
    assert!(json_u64(&json, "/fabric/transfers") > 0);
    assert!(json_u64(&json, "/simulation/data_cache_l2_runs") > 0);
    assert!(json_u64(&json, "/simulation/data_cache_l3_runs") > 0);
}

#[test]
fn rem6_run_executes_riscv_elf_load_store_through_mesi_data_cache() {
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
    let path = temp_binary("data-cache-mesi-exec", &elf);

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
            "--cores",
            "1",
            "--data-cache-protocol",
            "mesi",
            "--dump-memory",
            "0x80000020:8",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));
    assert!(stdout.contains("\"data_loads\":1"));
    assert!(stdout.contains("\"data_stores\":1"));
    assert!(stdout.contains("\"data_cache_runs\":2"));
    assert!(stdout.contains("\"data_cache_msi_runs\":0"));
    assert!(stdout.contains("\"data_cache_mesi_runs\":2"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_directory_decisions\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
    assert_stat(&stdout, "sim.data_cache.runs", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.data_cache.msi.runs", "Count", 0, "monotonic");
    assert_stat(&stdout, "sim.data_cache.mesi.runs", "Count", 2, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.cpu_responses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.directory_decisions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.dram_accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_transport_stats(&stdout, "sim.memory.fetch", 6, 12, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 2, 4, 2);
}

#[test]
fn rem6_run_executes_riscv_elf_load_store_through_moesi_data_cache() {
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
    let path = temp_binary("data-cache-moesi-exec", &elf);

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
            "--cores",
            "1",
            "--data-cache-protocol",
            "moesi",
            "--dump-memory",
            "0x80000020:8",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));
    assert!(stdout.contains("\"data_loads\":1"));
    assert!(stdout.contains("\"data_stores\":1"));
    assert!(stdout.contains("\"data_cache_runs\":2"));
    assert!(stdout.contains("\"data_cache_msi_runs\":0"));
    assert!(stdout.contains("\"data_cache_mesi_runs\":0"));
    assert!(stdout.contains("\"data_cache_moesi_runs\":2"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_directory_decisions\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
    assert_stat(&stdout, "sim.data_cache.runs", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.data_cache.msi.runs", "Count", 0, "monotonic");
    assert_stat(&stdout, "sim.data_cache.mesi.runs", "Count", 0, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.moesi.runs",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.cpu_responses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.directory_decisions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.dram_accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_transport_stats(&stdout, "sim.memory.fetch", 6, 12, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 2, 4, 2);
}

#[test]
fn rem6_run_executes_riscv_elf_load_store_through_chi_data_cache() {
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
    let path = temp_binary("data-cache-chi-exec", &elf);

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
            "--cores",
            "1",
            "--data-cache-protocol",
            "chi",
            "--dump-memory",
            "0x80000020:8",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));
    assert!(stdout.contains("\"data_loads\":1"));
    assert!(stdout.contains("\"data_stores\":1"));
    assert!(stdout.contains("\"data_cache_runs\":2"));
    assert!(stdout.contains("\"data_cache_msi_runs\":0"));
    assert!(stdout.contains("\"data_cache_mesi_runs\":0"));
    assert!(stdout.contains("\"data_cache_moesi_runs\":0"));
    assert!(stdout.contains("\"data_cache_chi_runs\":2"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_directory_decisions\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
    assert_stat(&stdout, "sim.data_cache.runs", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.data_cache.msi.runs", "Count", 0, "monotonic");
    assert_stat(&stdout, "sim.data_cache.mesi.runs", "Count", 0, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.moesi.runs",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(&stdout, "sim.data_cache.chi.runs", "Count", 2, "monotonic");
    assert_stat(
        &stdout,
        "sim.data_cache.cpu_responses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.directory_decisions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.dram_accesses",
        "Count",
        0,
        "monotonic",
    );
    assert_transport_stats(&stdout, "sim.memory.fetch", 6, 12, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 2, 4, 2);
}

#[test]
fn rem6_run_msi_data_cache_leaves_partial_final_lines_uncached() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(20, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x6, 5, 0x03),  // lwu x5, 0(x2)
        i_type(4, 2, 0x6, 6, 0x03),  // lwu x6, 4(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&0x1122_3344u32.to_le_bytes());
    program.extend_from_slice(&0x5566_7788u32.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("partial-final-line-data-cache-exec", &elf);

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
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
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
    assert!(stdout.contains("\"x5\":\"0x11223344\""));
    assert!(stdout.contains("\"x6\":\"0x55667788\""));
    assert!(stdout.contains("\"data_loads\":2"));
    assert!(stdout.contains("\"data_cache_runs\":0"));
    assert_stat(&stdout, "sim.data_cache.runs", "Count", 0, "monotonic");
    assert_stat(&stdout, "sim.data_cache.msi.runs", "Count", 0, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.data", 2, 4, 2);
}

#[test]
fn rem6_run_riscv_se_data_cache_observes_guest_memory_writes() {
    const DATA_OFFSET: i32 = 64;
    const DATA_ADDRESS: u64 = 0x8000_0000 + DATA_OFFSET as u64;

    let mut program = riscv64_program(&[
        u_type(0, 5, 0x17),                 // auipc x5, 0
        i_type(DATA_OFFSET, 5, 0, 5, 0x13), // addi x5, x5, data offset
        i_type(0, 5, 0x3, 6, 0x03),         // ld x6, 0(x5)
        i_type(0, 0, 0, 10, 0x13),          // addi a0, x0, 0
        i_type(0, 5, 0, 11, 0x13),          // addi a1, x5, 0
        i_type(8, 0, 0, 12, 0x13),          // addi a2, x0, 8
        i_type(63, 0, 0, 17, 0x13),         // addi a7, x0, read
        0x0000_0073,                        // ecall
        i_type(0, 5, 0x3, 10, 0x03),        // ld a0, 0(x5)
        i_type(93, 0, 0, 17, 0x13),         // addi a7, x0, exit
        0x0000_0073,                        // ecall
    ]);
    program.extend_from_slice(&[0; 20]);
    program.extend_from_slice(&0x11u64.to_le_bytes());
    program.extend_from_slice(&[0; 24]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("se-data-cache-guest-writes", &elf);
    let stdin = temp_binary("se-data-cache-stdin", &0x42u64.to_le_bytes());

    for protocol in ["msi", "mesi", "moesi", "chi"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run",
                "--isa",
                "riscv",
                "--binary",
                path.to_str().unwrap(),
                "--max-tick",
                "200",
                "--stats-format",
                "json",
                "--execute",
                "--cores",
                "1",
                "--riscv-se",
                "--riscv-se-stdin",
                stdin.to_str().unwrap(),
                "--data-cache-protocol",
                protocol,
                "--dump-memory",
                &format!("0x{DATA_ADDRESS:x}:8"),
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{protocol} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("\"status\":\"stopped_by_host\""));
        assert!(stdout.contains("\"stop_code\":66"), "{protocol}: {stdout}");
        assert!(stdout.contains("\"x6\":\"0x11\""));
        assert!(stdout.contains("\"x10\":\"0x42\""));
        assert!(stdout.contains("\"data_loads\":2"));
        assert!(stdout.contains("\"data_cache_runs\":2"));
        assert!(
            stdout.contains(&format!("\"data_cache_{protocol}_runs\":2")),
            "{protocol}: {stdout}"
        );
        assert!(stdout.contains(&format!("\"address\":\"0x{DATA_ADDRESS:x}\"")));
        assert!(
            stdout.contains("\"hex\":\"4200000000000000\""),
            "{protocol}: {stdout}"
        );
        assert_stat(&stdout, "sim.riscv.se", "Count", 1, "constant");
        assert_stat(&stdout, "sim.data_cache.runs", "Count", 2, "monotonic");
        assert_stat(
            &stdout,
            &format!("sim.data_cache.{protocol}.runs"),
            "Count",
            2,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_riscv_se_data_cache_observes_fixed_mmap_replacement() {
    const DATA_OFFSET: usize = 0x1000;
    const DATA_ADDRESS: u64 = 0x8000_0000 + DATA_OFFSET as u64;

    let mut program = riscv64_program(&[
        u_type(DATA_OFFSET as i32, 5, 0x17), // auipc x5, data page
        i_type(0, 5, 0x3, 6, 0x03),          // ld x6, 0(x5)
        i_type(222, 0, 0, 17, 0x13),         // addi a7, x0, mmap
        i_type(0, 5, 0, 10, 0x13),           // addi a0, x5, 0
        i_type(64, 0, 0, 11, 0x13),          // addi a1, x0, 64
        i_type(3, 0, 0, 12, 0x13),           // addi a2, x0, 3
        i_type(50, 0, 0, 13, 0x13),          // addi a3, x0, MAP_FIXED|MAP_ANON|MAP_PRIVATE
        i_type(-1, 0, 0, 14, 0x13),          // addi a4, x0, -1
        i_type(0, 0, 0, 15, 0x13),           // addi a5, x0, 0
        0x0000_0073,                         // ecall
        i_type(0, 5, 0x3, 7, 0x03),          // ld x7, 0(x5)
        i_type(0, 7, 0, 10, 0x13),           // addi a0, x7, 0
        i_type(93, 0, 0, 17, 0x13),          // addi a7, x0, exit
        0x0000_0073,                         // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x11u64.to_le_bytes());
    program.extend_from_slice(&[0; 8]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("se-data-cache-fixed-mmap", &elf);

    for protocol in ["msi", "mesi", "moesi", "chi"] {
        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args([
                "run",
                "--isa",
                "riscv",
                "--binary",
                path.to_str().unwrap(),
                "--max-tick",
                "260",
                "--stats-format",
                "json",
                "--execute",
                "--cores",
                "1",
                "--riscv-se",
                "--data-cache-protocol",
                protocol,
                "--dump-memory",
                &format!("0x{DATA_ADDRESS:x}:8"),
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "{protocol} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("\"status\":\"stopped_by_host\""));
        assert!(stdout.contains("\"stop_code\":0"), "{protocol}: {stdout}");
        assert!(stdout.contains("\"x6\":\"0x11\""));
        assert!(stdout.contains("\"data_loads\":2"));
        assert!(stdout.contains("\"data_cache_runs\":2"));
        assert!(
            stdout.contains(&format!("\"data_cache_{protocol}_runs\":2")),
            "{protocol}: {stdout}"
        );
        assert!(stdout.contains(&format!("\"address\":\"0x{DATA_ADDRESS:x}\"")));
        assert!(
            stdout.contains("\"hex\":\"0000000000000000\""),
            "{protocol}: {stdout}"
        );
        assert_stat(&stdout, "sim.data_cache.runs", "Count", 2, "monotonic");
        assert_stat(
            &stdout,
            &format!("sim.data_cache.{protocol}.runs"),
            "Count",
            2,
            "monotonic",
        );
    }
}

#[test]
fn rem6_run_emits_riscv_data_access_probe_stack_distance_stats() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        i_type(16, 2, 0x3, 6, 0x03), // ld x6, 16(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&[0; 12]);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&[0; 8]);
    program.extend_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("data-probe-stack-distance", &elf);

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
            "--cores",
            "1",
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
    assert!(stdout.contains("\"data_access_probes\":{\"sample_count\":2"));
    assert!(stdout.contains(
        "\"stack_distance\":{\"infinite_samples\":2,\"finite_samples\":0,\"stack_depth\":2}"
    ));
    assert!(stdout.contains(
        "\"memory_footprint\":{\"cache_line_bytes\":32,\"cache_line_total_bytes\":32,\"page_bytes\":4096,\"page_total_bytes\":4096}"
    ));
    assert_stat(&stdout, "sim.data.probes.samples", "Count", 2, "monotonic");
    assert_stat(
        &stdout,
        "sim.data.probes.stack_distance.infinite_samples",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data.probes.stack_distance.finite_samples",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data.probes.stack_distance.stack_depth",
        "Count",
        2,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.data.probes.memory_footprint.cache_line_bytes",
        "Byte",
        32,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.data.probes.memory_footprint.cache_line_total_bytes",
        "Byte",
        32,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data.probes.memory_footprint.page_bytes",
        "Byte",
        4096,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.data.probes.memory_footprint.page_total_bytes",
        "Byte",
        4096,
        "monotonic",
    );
}

#[test]
fn rem6_run_emits_riscv_retired_instruction_probe_stats() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(2, 5, 0x0, 6, 0x13), // addi x6, x5, 2
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("retired-instruction-probes", &elf);

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
            "--cores",
            "1",
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
    assert!(stdout.contains("\"committed_instructions\":3"));
    assert!(stdout.contains(
        "\"instruction_probes\":{\"event_count\":3,\"retired_instruction_events\":3,\"tracked_instructions\":3,\"pc_sample_events\":0,\"pc_target_counters\":0}"
    ));
    assert_stat(
        &stdout,
        "sim.instructions.probes.events",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instructions.probes.retired_events",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instructions.probes.tracked_insts",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instructions.probes.pc_sample_events",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instructions.probes.pc_target_counters",
        "Count",
        0,
        "constant",
    );
}

#[test]
fn rem6_run_emits_riscv_data_access_probe_stack_distance_histogram_stats() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        i_type(0, 2, 0x3, 6, 0x03),  // ld x6, 0(x2)
        s_type(8, 5, 2, 0x3),        // sd x5, 8(x2)
        s_type(8, 6, 2, 0x3),        // sd x6, 8(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&[0; 4]);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("data-probe-stack-distance-histogram", &elf);

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
            "--cores",
            "1",
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
        stdout.contains(
            "\"stack_distance\":{\"infinite_samples\":1,\"finite_samples\":3,\"stack_depth\":1}"
        ),
        "{stdout}"
    );
    assert_histogram_stat(
        &stdout,
        "sim.data.probes.stack_distance.read_linear",
        "Count",
        1,
        "monotonic",
        &[(0, 1)],
    );
    assert_histogram_stat(
        &stdout,
        "sim.data.probes.stack_distance.read_log",
        "Count",
        1,
        "monotonic",
        &[(1, 1)],
    );
    assert_histogram_stat(
        &stdout,
        "sim.data.probes.stack_distance.write_linear",
        "Count",
        2,
        "monotonic",
        &[(0, 2)],
    );
    assert_histogram_stat(
        &stdout,
        "sim.data.probes.stack_distance.write_log",
        "Count",
        2,
        "monotonic",
        &[(1, 2)],
    );
}

#[test]
fn rem6_run_executes_riscv_elf_load_store_through_nvm_profile_and_emits_nvm_stats() {
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
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("nvm-profile-data-exec", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "600",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "nvm",
            "--dump-memory",
            "0x80000020:8",
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
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
    assert!(stdout.contains("\"technology\":\"nvm\""));
    assert!(stdout.contains("\"parallel_port_label\":\"controller\""));
    assert!(stdout.contains("\"topology_unit_label\":\"media_bank\""));
    assert!(stdout.contains("\"parallel_ports\":2"));
    assert!(stdout.contains("\"topology_units\":8"));
    assert!(stdout.contains("\"scheduler_banks\":8"));
    assert!(stdout.contains("\"topology_banks\":32"));
    assert!(stdout.contains("\"geometry\":{\"bank_count\":4"));
    assert!(stdout.contains("\"row_size\":64"));
    assert!(stdout.contains("\"line_size\":16"));
    assert!(stdout.contains("\"lines_per_row\":4"));
    assert!(stdout.contains("\"bank_group_count\":0"));
    assert!(stdout.contains("\"timing\":{\"activate_latency\":3"));
    assert!(stdout.contains("\"read_latency\":5"));
    assert!(stdout.contains("\"write_latency\":7"));
    assert!(stdout.contains("\"precharge_latency\":2"));
    assert!(stdout.contains("\"bus_turnaround\":4"));
    assert!(stdout.contains("\"command_window\":{\"window_cycles\":16"));
    assert!(stdout.contains("\"max_commands\":2"));
    assert!(stdout.contains("\"low_power_timing\":{\"precharge_powerdown_entry_delay\":20"));
    assert!(stdout.contains("\"self_refresh_entry_delay\":80"));
    assert!(stdout.contains("\"exit_latency\":7"));
    assert!(stdout.contains("\"self_refresh_exit_latency\":17"));
    assert!(stdout.contains("\"nvm_media\":{\"read_media_latency\":30"));
    assert!(stdout.contains("\"write_media_latency\":50"));
    assert!(stdout.contains("\"send_latency\":6"));
    assert!(stdout.contains("\"max_pending_reads\":4"));
    assert!(stdout.contains("\"max_pending_writes\":1"));
    assert!(stdout.contains("\"nvm\":{\"persistent_writes\":1"));
    assert!(stdout.contains("\"persistent_write_bytes\":8"));
    assert!(stdout.contains("\"max_pending_reads\":1,\"max_pending_persistent_writes\":1"));
    assert!(stdout.contains("\"max_pending_persistent_writes\":1"));
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.technology.nvm",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.parallel_ports",
        "Count",
        2,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.topology_units",
        "Count",
        8,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.scheduler_banks",
        "Count",
        8,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.topology_banks",
        "Count",
        32,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.geometry.bank_count",
        "Count",
        4,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.geometry.row_size",
        "Byte",
        64,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.geometry.line_size",
        "Byte",
        16,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.geometry.lines_per_row",
        "Count",
        4,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.geometry.bank_group_count",
        "Count",
        0,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.activate_latency",
        "Tick",
        3,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.read_latency",
        "Tick",
        5,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.write_latency",
        "Tick",
        7,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.precharge_latency",
        "Tick",
        2,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.bus_turnaround",
        "Tick",
        4,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.command_window.window_cycles",
        "Tick",
        16,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.timing.command_window.max_commands",
        "Count",
        2,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.precharge_powerdown_entry_delay",
        "Tick",
        20,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.self_refresh_entry_delay",
        "Tick",
        80,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.exit_latency",
        "Tick",
        7,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.low_power_timing.self_refresh_exit_latency",
        "Tick",
        17,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.nvm_media.read_media_latency",
        "Tick",
        30,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.nvm_media.write_media_latency",
        "Tick",
        50,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.nvm_media.send_latency",
        "Tick",
        6,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.nvm_media.max_pending_reads",
        "Count",
        4,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.nvm_media.max_pending_writes",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.nvm.persistent_writes",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.nvm.persistent_write_bytes",
        "Byte",
        8,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.nvm.max_pending_reads",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.nvm.max_pending_persistent_writes",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_executes_riscv_elf_with_loaded_blob_memory() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("blob-exec", &elf);
    let blob_path = temp_binary("blob-exec-data", &[0xde, 0xad, 0xbe, 0xef]);

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
            "--cores",
            "1",
            "--load-blob",
            &format!("0x80001000:{}", blob_path.display()),
            "--dump-memory",
            "0x80001000:4",
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
    assert!(stdout.contains("\"address\":\"0x80001000\""));
    assert!(stdout.contains("\"bytes\":4"));
    assert!(stdout.contains("\"hex\":\"deadbeef\""));
    assert_stat(&stdout, "sim.load_blobs", "Count", 1, "constant");
    assert_stat(&stdout, "sim.load_blob_bytes", "Byte", 4, "constant");
}

#[test]
fn rem6_run_executes_riscv_elf_with_adjacent_loaded_blob() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("adjacent-blob-exec", &elf);
    let blob_path = temp_binary("adjacent-blob-data", &[0xaa, 0xbb]);

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
            "--cores",
            "1",
            "--load-blob",
            &format!("0x80000004:{}", blob_path.display()),
            "--dump-memory",
            "0x80000000:6",
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
    assert!(stdout.contains("\"address\":\"0x80000000\""));
    assert!(stdout.contains("\"hex\":\"73000000aabb\""));
}

#[test]
fn rem6_run_executes_riscv_elf_with_adjacent_loaded_blobs() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("adjacent-blobs-exec", &elf);
    let first_blob_path = temp_binary("adjacent-blobs-first", &[0xaa, 0xbb]);
    let second_blob_path = temp_binary("adjacent-blobs-second", &[0xcc, 0xdd]);

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
            "--cores",
            "1",
            "--load-blob",
            &format!("0x80000004:{}", first_blob_path.display()),
            "--load-blob",
            &format!("0x80000006:{}", second_blob_path.display()),
            "--dump-memory",
            "0x80000004:4",
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
    assert!(stdout.contains("\"address\":\"0x80000004\""));
    assert!(stdout.contains("\"hex\":\"aabbccdd\""));
    assert_stat(&stdout, "sim.load_blobs", "Count", 2, "constant");
    assert_stat(&stdout, "sim.load_blob_bytes", "Byte", 4, "constant");
}

#[test]
fn rem6_run_executes_riscv_guest_load_across_adjacent_loaded_blobs() {
    let program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(12, 2, 0x2, 5, 0x03), // lw x5, blob offset(x2)
        0x0000_0073,                 // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("adjacent-blob-load-exec", &elf);
    let first_blob_path = temp_binary("adjacent-blob-load-first", &[0x11, 0x22]);
    let second_blob_path = temp_binary("adjacent-blob-load-second", &[0x33, 0x44]);

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
            "--cores",
            "1",
            "--load-blob",
            &format!("0x8000000c:{}", first_blob_path.display()),
            "--load-blob",
            &format!("0x8000000e:{}", second_blob_path.display()),
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
    assert!(stdout.contains("\"x5\":\"0x44332211\""));
}

#[test]
fn rem6_run_executes_riscv_atomic_memory_op_and_emits_atomic_byte_stats() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                            // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13),                   // addi x2, x2, data offset
        i_type(5, 0, 0x0, 6, 0x13),                    // addi x6, x0, 5
        atomic_type(0x00, false, false, 6, 2, 0x3, 7), // amoadd.d x7, x6, (x2)
        0x0000_0073,                                   // ecall
        0x0000_0013,                                   // padding before data
    ]);
    program.extend_from_slice(&9u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("atomic-exec", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "100",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dump-memory",
            "0x80000018:8",
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
    assert!(stdout.contains("\"x7\":\"0x9\""));
    assert!(stdout.contains("\"data_atomics\":1"));
    assert!(stdout.contains("\"address\":\"0x80000018\""));
    assert!(stdout.contains("\"hex\":\"0e00000000000000\""));
    assert_stat(&stdout, "sim.data.atomic_bytes", "Byte", 8, "monotonic");
    assert_stat(
        &stdout,
        "sim.cpu0.data.atomic_bytes",
        "Byte",
        8,
        "monotonic",
    );
    assert_transport_stats(&stdout, "sim.memory.fetch", 5, 20, 4);
    assert_transport_stats(&stdout, "sim.memory.data", 1, 4, 4);
}

#[test]
fn rem6_run_exposes_distinct_riscv_hart_ids_to_parallel_cores() {
    let mut program = riscv64_program(&[
        csr_read(0xf14, 5),          // csrr x5, mhartid
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(28, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        b_type(12, 0, 5, 0x1),       // bne x5, x0, hart one store
        s_type(0, 5, 2, 0x3),        // sd x5, 0(x2)
        0x0000_0073,                 // ecall
        s_type(8, 5, 2, 0x3),        // sd x5, 8(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&0u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("hartid-exec", &elf);

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
            "--cores",
            "2",
            "--dump-memory",
            "0x80000020:16",
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
    assert!(stdout.contains("\"cpu\":0"));
    assert!(stdout.contains("\"cpu\":1"));
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains("\"path\":\"sim.data.stores\""));
    assert!(stdout.contains("\"value\":2"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"bytes\":16"));
    assert!(stdout.contains("\"hex\":\"00000000000000000100000000000000\""));
    assert!(stdout.contains("\"path\":\"sim.cpu0.data.stores\""));
    assert!(stdout.contains("\"path\":\"sim.cpu1.data.stores\""));
    assert_transport_stats(&stdout, "sim.memory.data.route1.source.cpu0.dmem", 1, 4, 4);
    assert_transport_stats(&stdout, "sim.memory.data.route3.source.cpu1.dmem", 1, 4, 4);
}

#[test]
fn rem6_run_executes_riscv_counter_csr_reads() {
    let program = riscv64_program(&[
        i_type(9, 0, 0x0, 7, 0x13), // addi x7, x0, 9
        csr_read(0xc00, 5),         // rdcycle x5
        csr_read(0xc02, 6),         // rdinstret x6
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("counter-csr-exec", &elf);

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
            "--cores",
            "1",
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
    assert!(stdout.contains("\"x5\":\"0x1\""));
    assert!(stdout.contains("\"x6\":\"0x2\""));
    assert!(stdout.contains("\"x7\":\"0x9\""));
}

fn json_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 JSON field {pointer}"))
}

fn assert_run_fabric_virtual_network_stats(stdout: &str, fabric: &Value, virtual_network: u64) {
    let lanes = fabric
        .get("lane_activities")
        .and_then(Value::as_array)
        .expect("fabric lane activities");
    let mut active_links = BTreeSet::new();
    let mut contended_links = BTreeSet::new();
    let mut transfers = 0;
    let mut bytes = 0;
    let mut flits = 0;
    let mut occupied_ticks = 0;
    let mut queue_delay_ticks = 0;
    let mut max_queue_delay_ticks = 0;
    let mut credit_delay_ticks = 0;
    let mut max_credit_delay_ticks = 0;

    for lane in lanes {
        if lane.get("virtual_network").and_then(Value::as_u64) != Some(virtual_network) {
            continue;
        }
        let link = lane
            .get("link")
            .and_then(Value::as_str)
            .expect("fabric lane link");
        active_links.insert(link);
        let lane_queue_delay_ticks = lane_u64(lane, "queue_delay_ticks");
        if lane_queue_delay_ticks != 0 {
            contended_links.insert(link);
        }
        transfers += lane_u64(lane, "transfer_count");
        bytes += lane_u64(lane, "byte_count");
        flits += lane_u64(lane, "flit_count");
        occupied_ticks += lane_u64(lane, "occupied_ticks");
        queue_delay_ticks += lane_queue_delay_ticks;
        max_queue_delay_ticks = max_queue_delay_ticks.max(lane_u64(lane, "max_queue_delay_ticks"));
        credit_delay_ticks += lane_u64(lane, "credit_delay_ticks");
        max_credit_delay_ticks =
            max_credit_delay_ticks.max(lane_u64(lane, "max_credit_delay_ticks"));
    }

    assert!(
        !active_links.is_empty(),
        "missing VN{virtual_network} lane activity"
    );
    let prefix = format!("sim.memory.fabric.vn{virtual_network}");
    assert_stat(
        stdout,
        &format!("{prefix}.active_lanes"),
        "Count",
        active_links.len() as u64,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.transfers"),
        "Count",
        transfers,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.bytes"),
        "Byte",
        bytes,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.flits"),
        "Count",
        flits,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        occupied_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        max_queue_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.max_credit_delay_ticks"),
        "Tick",
        max_credit_delay_ticks,
        "monotonic",
    );
    assert_stat(
        stdout,
        &format!("{prefix}.contended_lanes"),
        "Count",
        contended_links.len() as u64,
        "monotonic",
    );
}

fn assert_run_fabric_lane_stats(stdout: &str, fabric: &Value) {
    let lanes = fabric
        .get("lane_activities")
        .and_then(Value::as_array)
        .expect("fabric lane activities");
    assert!(!lanes.is_empty(), "missing fabric lane activity");

    for lane in lanes {
        let link = lane
            .get("link")
            .and_then(Value::as_str)
            .expect("fabric lane link");
        let virtual_network = lane
            .get("virtual_network")
            .and_then(Value::as_u64)
            .expect("fabric lane virtual network");
        let prefix = format!(
            "sim.memory.fabric.link.{}.vn{virtual_network}",
            stat_path_segment(link)
        );
        assert_stat(
            stdout,
            &format!("{prefix}.transfers"),
            "Count",
            lane_u64(lane, "transfer_count"),
            "monotonic",
        );
        assert_stat(
            stdout,
            &format!("{prefix}.bytes"),
            "Byte",
            lane_u64(lane, "byte_count"),
            "monotonic",
        );
        assert_stat(
            stdout,
            &format!("{prefix}.flits"),
            "Count",
            lane_u64(lane, "flit_count"),
            "monotonic",
        );
        assert_stat(
            stdout,
            &format!("{prefix}.occupied_ticks"),
            "Tick",
            lane_u64(lane, "occupied_ticks"),
            "monotonic",
        );
        assert_stat(
            stdout,
            &format!("{prefix}.queue_delay_ticks"),
            "Tick",
            lane_u64(lane, "queue_delay_ticks"),
            "monotonic",
        );
        assert_stat(
            stdout,
            &format!("{prefix}.max_queue_delay_ticks"),
            "Tick",
            lane_u64(lane, "max_queue_delay_ticks"),
            "monotonic",
        );
        assert_stat(
            stdout,
            &format!("{prefix}.credit_delay_ticks"),
            "Tick",
            lane_u64(lane, "credit_delay_ticks"),
            "monotonic",
        );
        assert_stat(
            stdout,
            &format!("{prefix}.max_credit_delay_ticks"),
            "Tick",
            lane_u64(lane, "max_credit_delay_ticks"),
            "monotonic",
        );
    }
}

fn lane_u64(lane: &Value, field: &str) -> u64 {
    lane.get(field).and_then(Value::as_u64).unwrap_or(0)
}
