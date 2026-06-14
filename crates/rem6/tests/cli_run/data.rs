use std::process::Command;

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
            "80",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x1122334455667789\""));
    assert!(stdout.contains("\"data_loads\":1"));
    assert!(stdout.contains("\"data_stores\":1"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"bytes\":8"));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
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
        30,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.retired",
        "Count",
        6,
        "monotonic",
    );
    assert_transport_stats(&stdout, "sim.memory.fetch", 6, 12, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 2, 4, 2);
}

#[test]
fn rem6_run_executes_riscv_elf_load_store_through_msi_data_cache() {
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
    assert!(stdout.contains("\"data_cache_msi_runs\":2"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_directory_decisions\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":0"));
    assert!(stdout.contains("\"address\":\"0x80000020\""));
    assert!(stdout.contains("\"hex\":\"8977665544332211\""));
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
    assert_transport_stats(&stdout, "sim.memory.fetch", 5, 10, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 1, 2, 2);
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
    assert_transport_stats(&stdout, "sim.memory.data.route1.source.cpu0.dmem", 1, 2, 2);
    assert_transport_stats(&stdout, "sim.memory.data.route3.source.cpu1.dmem", 1, 2, 2);
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
