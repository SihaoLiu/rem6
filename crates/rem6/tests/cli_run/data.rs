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
    assert_transport_stats(&stdout, "sim.memory.fetch", 6, 12, 2);
    assert_transport_stats(&stdout, "sim.memory.data", 2, 4, 2);
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
