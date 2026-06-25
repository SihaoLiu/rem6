use std::process::Command;

use crate::support::*;
use serde_json::Value;

fn tagged_next_line_prefetch_two_load_elf() -> Vec<u8> {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(32, 2, 0x3, 6, 0x03),                 // ld x6, 32(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 96, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 32..DATA_OFFSET + 40]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    riscv64_elf(0x8000_0000, 0x8000_0000, &program)
}

fn tagged_next_line_prefetch_useful_elf() -> Vec<u8> {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(16, 2, 0x3, 6, 0x03),                 // ld x6, 16(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 48, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 16..DATA_OFFSET + 24]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    riscv64_elf(0x8000_0000, 0x8000_0000, &program)
}

fn run_tagged_next_line_prefetch(
    path: &std::path::Path,
    max_tick: u64,
    dram_memory: bool,
    instruction_cache: bool,
) -> String {
    let max_tick = max_tick.to_string();
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        max_tick.as_str(),
        "--stats-format",
        "json",
        "--execute",
    ]);
    if dram_memory {
        command.arg("--dram-memory");
    }
    if instruction_cache {
        command.args(["--instruction-cache-protocol", "msi"]);
    }
    command.args([
        "--data-cache-protocol",
        "msi",
        "--data-cache-prefetcher",
        "tagged-next-line",
    ]);
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn rem6_run_data_cache_prefetcher_translates_page_crossing_next_line() {
    const DATA_OFFSET: usize = 0xff0;

    let mut program = riscv64_program(&[
        u_type(0x1000, 2, 0x17),      // auipc x2, 0x1000
        i_type(-16, 2, 0x0, 2, 0x13), // addi x2, x2, -16
        i_type(0, 2, 0x3, 5, 0x03),   // ld x5, 0(x2)
        0x0000_0073,                  // ecall
    ]);
    program.resize(DATA_OFFSET + 64, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("data-cache-prefetch-translation-queue", &elf);
    let stdout = run_tagged_next_line_prefetch(&path, 180, false, false);

    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"data_loads\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_identified\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_issued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_enqueued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_issued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_translation_queue_enqueued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_translation_queue_issued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_translation_queue_translated\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_translation_queue_dropped\":0"));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/prefetch_identified"),
        1
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/data/prefetch_identified"),
        1
    );
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/data/l1/prefetch_translation_queue_translated"
        ),
        1
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.translation_queue.enqueued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.translation_queue.issued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.translation_queue.translated",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.translation_queue.dropped",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.prefetch.translation_queue.translated",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.data.prefetch.translation_queue.enqueued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.data.l1.prefetch.translation_queue.translated",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_data_cache_prefetcher_drops_repeated_page_crossing_next_line_after_translation() {
    const DATA_OFFSET: usize = 0x1000;

    let mut program = riscv64_program(&[
        u_type(0x1000, 2, 0x17),    // auipc x2, 0x1000
        i_type(0, 2, 0x3, 5, 0x03), // ld x5, 0(x2)
        i_type(8, 2, 0x3, 6, 0x03), // ld x6, 8(x2)
        0x0000_0073,                // ecall
    ]);
    program.resize(DATA_OFFSET + 64, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 8..DATA_OFFSET + 16]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0ff0, 0x8000_0ff0, &program);
    let path = temp_binary("data-cache-prefetch-repeated-page-crossing-next-line", &elf);
    let stdout = run_tagged_next_line_prefetch(&path, 240, false, false);

    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x99aabbccddeeff00\""));
    assert!(stdout.contains("\"data_loads\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_identified\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_issued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_span_page\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_in_cache\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_enqueued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_issued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_dropped\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_translation_queue_enqueued\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_translation_queue_issued\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_translation_queue_translated\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_translation_queue_dropped\":0"));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/data/l1/prefetch_in_cache"),
        1
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.in_cache",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.data.l1.prefetch.in_cache",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_data_cache_prefetcher_issues_tagged_next_line_prefetches() {
    let elf = tagged_next_line_prefetch_two_load_elf();
    let path = temp_binary("data-cache-prefetch-tagged-next-line", &elf);
    let stdout = run_tagged_next_line_prefetch(&path, 200, false, false);

    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x99aabbccddeeff00\""));
    assert!(stdout.contains("\"data_loads\":2"));
    assert!(stdout.contains("\"data_cache_runs\":4"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_identified\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_issued\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_enqueued\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_issued\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_dropped\":0"));
    assert_stat(
        &stdout,
        "sim.data_cache.cpu_responses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.identified",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.issued",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.queue.enqueued",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.queue.issued",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_data_cache_prefetcher_counts_prefetched_line_as_useful() {
    let elf = tagged_next_line_prefetch_useful_elf();
    let path = temp_binary("data-cache-prefetch-useful-next-line", &elf);
    let stdout = run_tagged_next_line_prefetch(&path, 200, false, false);

    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x99aabbccddeeff00\""));
    assert!(stdout.contains("\"data_loads\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_issued\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_useful\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_demand_mshr_misses\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_accuracy_ppm\":500000"));
    assert!(stdout.contains("\"data_cache_prefetch_coverage_ppm\":500000"));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/prefetch_useful"),
        1
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/prefetch_demand_mshr_misses"),
        1
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/prefetch_accuracy_ppm"),
        500_000
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/prefetch_coverage_ppm"),
        500_000
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/data/prefetch_useful"),
        1
    );
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/data/prefetch_demand_mshr_misses"
        ),
        1
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/data/prefetch_accuracy_ppm"),
        500_000
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/data/prefetch_coverage_ppm"),
        500_000
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/data/l1/prefetch_useful"),
        1
    );
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/data/l1/prefetch_demand_mshr_misses"
        ),
        1
    );
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/data/l1/prefetch_accuracy_ppm"
        ),
        500_000
    );
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/data/l1/prefetch_coverage_ppm"
        ),
        500_000
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.useful",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.demand_mshr_misses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.accuracy_ppm",
        "Ppm",
        500_000,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.coverage_ppm",
        "Ppm",
        500_000,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.prefetch.useful",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.data.l1.prefetch.useful",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.prefetch.demand_mshr_misses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.prefetch.accuracy_ppm",
        "Ppm",
        500_000,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.prefetch.coverage_ppm",
        "Ppm",
        500_000,
        "monotonic",
    );
}

#[test]
fn rem6_run_data_cache_prefetcher_accounts_dram_line_fills() {
    let elf = tagged_next_line_prefetch_two_load_elf();
    let path = temp_binary("data-cache-prefetch-dram-fills", &elf);
    let stdout = run_tagged_next_line_prefetch(&path, 260, true, true);

    assert!(stdout.contains("\"instruction_cache_dram_accesses\":2"));
    assert!(stdout.contains("\"data_cache_runs\":4"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_dram_accesses\":4"));
    assert!(stdout.contains("\"data_cache_prefetch_issued\":2"));
    assert_stat(
        &stdout,
        "sim.instruction_cache.dram_accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.data_cache.dram_accesses",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(&stdout, "sim.memory.dram.accesses", "Count", 6, "monotonic");
    assert_stat(&stdout, "sim.memory.dram.reads", "Count", 6, "monotonic");
    assert_stat(&stdout, "sim.memory.dram.writes", "Count", 0, "monotonic");
}

#[test]
fn rem6_run_data_cache_prefetcher_does_not_reissue_same_next_line() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(8, 2, 0x3, 6, 0x03),                  // ld x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 32, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 8..DATA_OFFSET + 16]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("data-cache-prefetch-same-next-line", &elf);
    let stdout = run_tagged_next_line_prefetch(&path, 160, false, false);

    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"x6\":\"0x99aabbccddeeff00\""));
    assert!(stdout.contains("\"data_loads\":2"));
    assert!(stdout.contains("\"data_cache_runs\":3"));
    assert!(stdout.contains("\"data_cache_cpu_responses\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_identified\":2"));
    assert!(stdout.contains("\"data_cache_prefetch_issued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_in_cache\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_enqueued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_issued\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_dropped\":1"));
}

#[test]
fn rem6_run_data_cache_prefetcher_drops_unbacked_next_line_before_issue() {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("data-cache-prefetch-unbacked-next-line", &elf);
    let stdout = run_tagged_next_line_prefetch(&path, 120, false, false);

    assert!(stdout.contains("\"x5\":\"0x1122334455667788\""));
    assert!(stdout.contains("\"data_loads\":1"));
    assert!(stdout.contains("\"data_cache_runs\":1"));
    assert!(stdout.contains("\"data_cache_prefetch_identified\":0"));
    assert!(stdout.contains("\"data_cache_prefetch_issued\":0"));
    assert!(stdout.contains("\"data_cache_prefetch_accuracy_ppm\":null"));
    assert!(stdout.contains("\"data_cache_prefetch_coverage_ppm\":0"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_enqueued\":0"));
    assert!(stdout.contains("\"data_cache_prefetch_queue_issued\":0"));
    assert!(!stdout.contains("\"path\":\"sim.data_cache.prefetch.accuracy_ppm\""));
    assert_stat(
        &stdout,
        "sim.data_cache.prefetch.coverage_ppm",
        "Ppm",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_instruction_cache_prefetcher_issues_tagged_next_line_prefetch() {
    let mut program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        0x0000_0073,                // ecall
    ]);
    program.resize(32, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("instruction-cache-prefetch-tagged-next-line", &elf);

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
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
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
    assert!(stdout.contains("\"x5\":\"0x3\""));
    assert!(stdout.contains("\"instruction_cache_runs\":5"));
    assert!(stdout.contains("\"instruction_cache_cpu_responses\":4"));
    assert!(stdout.contains("\"instruction_cache_prefetch_identified\":4"));
    assert!(stdout.contains("\"instruction_cache_prefetch_issued\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_in_cache\":3"));
    assert!(stdout.contains("\"instruction_cache_prefetch_queue_enqueued\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_queue_issued\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_queue_dropped\":3"));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/prefetch_identified"),
        4
    );
    assert_eq!(
        json_u64(&json, "/memory_resources/cache/prefetch_in_cache"),
        3
    );
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/instruction/prefetch_identified"
        ),
        4
    );
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/instruction/prefetch_in_cache"
        ),
        3
    );
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/instruction/l1/prefetch_queue_issued"
        ),
        1
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.prefetch.identified",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.prefetch.issued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.prefetch.in_cache",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.prefetch.issued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.instruction.prefetch.identified",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.instruction.prefetch.in_cache",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.instruction.l1.prefetch.queue.issued",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_instruction_cache_prefetcher_counts_prefetched_line_as_useful() {
    let mut program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        0x0000_0073,                // ecall
    ]);
    program.resize(48, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("instruction-cache-prefetch-useful-next-line", &elf);

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
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
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
    assert!(stdout.contains("\"x5\":\"0x4\""));
    assert!(stdout.contains("\"instruction_cache_prefetch_useful\":1"));
    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json_u64(
            &json,
            "/memory_resources/cache/instruction/l1/prefetch_useful"
        ),
        1
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.prefetch.useful",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.resources.cache.instruction.l1.prefetch.useful",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_instruction_cache_prefetcher_translates_page_crossing_next_line() {
    let mut program = riscv64_program(&[
        0x0000_0073, // ecall
    ]);
    program.resize(64, 0);
    let elf = riscv64_elf(0x8000_0ff0, 0x8000_0ff0, &program);
    let path = temp_binary("instruction-cache-prefetch-translation-queue", &elf);

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
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
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
    assert!(stdout.contains("\"committed_instructions\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_identified\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_issued\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_queue_enqueued\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_queue_issued\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_translation_queue_enqueued\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_translation_queue_issued\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_translation_queue_translated\":1"));
    assert!(stdout.contains("\"instruction_cache_prefetch_translation_queue_dropped\":0"));
    assert_stat(
        &stdout,
        "sim.instruction_cache.prefetch.translation_queue.enqueued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.prefetch.translation_queue.issued",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.prefetch.translation_queue.translated",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.instruction_cache.prefetch.translation_queue.dropped",
        "Count",
        0,
        "monotonic",
    );
}

fn json_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing u64 JSON field {pointer}"))
}

#[test]
fn rem6_run_instruction_cache_prefetcher_works_across_cache_protocols() {
    let mut program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        0x0000_0073,                // ecall
    ]);
    program.resize(32, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("instruction-cache-prefetch-protocol-matrix", &elf);

    for protocol in ["msi", "mesi", "moesi", "chi"] {
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
                "--instruction-cache-protocol",
                protocol,
                "--instruction-cache-prefetcher",
                "tagged-next-line",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "protocol {protocol} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("\"status\":\"executed_until_trap\""),
            "protocol {protocol} stdout: {stdout}"
        );
        assert!(
            stdout.contains("\"instruction_cache_cpu_responses\":4"),
            "protocol {protocol} stdout: {stdout}"
        );
        assert!(
            stdout.contains("\"instruction_cache_prefetch_issued\":1"),
            "protocol {protocol} stdout: {stdout}"
        );
    }
}
