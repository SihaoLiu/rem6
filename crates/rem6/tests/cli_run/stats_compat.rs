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
fn rem6_run_text_stats_emit_gem5_cpu_instruction_cycle_and_rate_aliases() {
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
        text_stat_value(&stdout, "system.cpu.numOps"),
        text_stat_value(&stdout, "sim.cpu0.instructions.committed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.commitStats0.numInsts"),
        text_stat_value(&stdout, "sim.cpu0.instructions.committed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.commitStats0.numOps"),
        text_stat_value(&stdout, "sim.cpu0.instructions.committed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.numCycles"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.cycles")
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.commitStats0.ipc"),
        fixed_ratio(
            text_stat_value(&stdout, "system.cpu.commitStats0.numInsts"),
            text_stat_value(&stdout, "system.cpu.numCycles")
        )
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.commitStats0.cpi"),
        fixed_ratio(
            text_stat_value(&stdout, "system.cpu.numCycles"),
            text_stat_value(&stdout, "system.cpu.commitStats0.numInsts")
        )
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.commitStats0.ipc").contains("unit=(Count/Cycle)"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.commitStats0.cpi").contains("unit=(Cycle/Count)"),
        "{stdout}"
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.ipc"),
        fixed_ratio(
            text_stat_value(&stdout, "system.cpu.numInsts"),
            text_stat_value(&stdout, "system.cpu.numCycles")
        )
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.cpi"),
        fixed_ratio(
            text_stat_value(&stdout, "system.cpu.numCycles"),
            text_stat_value(&stdout, "system.cpu.numInsts")
        )
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_cpu_rate_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-cpu-rate-aliases-json", &elf);

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

    assert!(stdout.contains("\"path\":\"system.cpu.numCycles\""));
    assert!(stdout.contains("\"path\":\"system.cpu.commitStats0.numInsts\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.ipc\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.cpi\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.commitStats0.ipc\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.commitStats0.cpi\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_multicore_cpu_aliases_and_rates_without_ambiguous_cpu_path() {
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
            text_stat_value(&stdout, &format!("system.cpu{cpu}.numOps")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.instructions.committed"))
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.commitStats0.numInsts")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.instructions.committed"))
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.commitStats0.numOps")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.instructions.committed"))
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.pipeline.in_order.cycles"))
        );
        assert_eq!(
            text_stat_decimal(&stdout, &format!("system.cpu{cpu}.commitStats0.ipc")),
            fixed_ratio(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.commitStats0.numInsts")),
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles"))
            )
        );
        assert_eq!(
            text_stat_decimal(&stdout, &format!("system.cpu{cpu}.commitStats0.cpi")),
            fixed_ratio(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles")),
                text_stat_value(&stdout, &format!("system.cpu{cpu}.commitStats0.numInsts"))
            )
        );
        assert_eq!(
            text_stat_decimal(&stdout, &format!("system.cpu{cpu}.ipc")),
            fixed_ratio(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numInsts")),
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles"))
            )
        );
        assert_eq!(
            text_stat_decimal(&stdout, &format!("system.cpu{cpu}.cpi")),
            fixed_ratio(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles")),
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numInsts"))
            )
        );
    }
    assert!(!has_text_stat(&stdout, "system.cpu.numInsts"));
    assert!(!has_text_stat(&stdout, "system.cpu.numOps"));
    assert!(!has_text_stat(&stdout, "system.cpu.numCycles"));
    assert!(!has_text_stat(&stdout, "system.cpu.commitStats0.numInsts"));
    assert!(!has_text_stat(&stdout, "system.cpu.commitStats0.numOps"));
    assert!(!has_text_stat(&stdout, "system.cpu.ipc"));
    assert!(!has_text_stat(&stdout, "system.cpu.cpi"));
    assert!(!has_text_stat(&stdout, "system.cpu.commitStats0.ipc"));
    assert!(!has_text_stat(&stdout, "system.cpu.commitStats0.cpi"));
}

#[test]
fn rem6_run_text_stats_omit_ambiguous_gem5_l1_cache_aliases_for_multicore() {
    let path = gem5_l1_cache_alias_binary("gem5-l1-cache-aliases-multicore");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "2",
            "--instruction-cache-protocol",
            "msi",
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

    assert!(text_stat_value(&stdout, "sim.instruction_cache.bank.accepted") > 0);
    assert!(text_stat_value(&stdout, "sim.data_cache.bank.accepted") > 0);
    assert!(!has_text_stat(&stdout, "system.cpu.icache.overallHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.icache.overallMisses"));
    assert!(!has_text_stat(&stdout, "system.cpu.icache.overallAccesses"));
    assert!(!has_text_stat(&stdout, "system.cpu.icache.overallMissRate"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.overallHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.overallMisses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.overallAccesses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.overallMissRate"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_cache_hit_miss_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l1-cache-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
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

    let icache_hits = text_stat_value(&stdout, "sim.instruction_cache.bank.immediate_hits");
    let icache_misses = text_stat_value(&stdout, "sim.instruction_cache.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.instruction_cache.bank.coalesced_misses");
    assert!(icache_hits > 0, "{stdout}");
    assert!(icache_misses > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallHits"),
        icache_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallMisses"),
        icache_misses
    );
    let icache_accesses = icache_hits + icache_misses;
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandHits"),
        icache_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandMisses"),
        icache_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandAccesses"),
        icache_accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.icache.demandMissRate"),
        fixed_ratio(icache_misses, icache_accesses)
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallAccesses"),
        icache_accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.icache.overallMissRate"),
        fixed_ratio(icache_misses, icache_accesses)
    );

    let dcache_hits = text_stat_value(&stdout, "sim.data_cache.bank.immediate_hits");
    let dcache_misses = text_stat_value(&stdout, "sim.data_cache.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.bank.coalesced_misses");
    assert!(dcache_hits > 0, "{stdout}");
    assert!(dcache_misses > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallHits"),
        dcache_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallMisses"),
        dcache_misses
    );
    let dcache_accesses = dcache_hits + dcache_misses;
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandHits"),
        dcache_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandMisses"),
        dcache_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandAccesses"),
        dcache_accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.demandMissRate"),
        fixed_ratio(dcache_misses, dcache_accesses)
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallAccesses"),
        dcache_accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.overallMissRate"),
        fixed_ratio(dcache_misses, dcache_accesses)
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_l1_cache_hit_miss_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l1-cache-aliases-json");

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
            "--instruction-cache-protocol",
            "msi",
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

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.bank.immediate_hits",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.bank.immediate_hits",
        "Count",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMissRate\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l2_cache_overall_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l2-cache-overall-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
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

    let hits = text_stat_value(&stdout, "sim.instruction_cache.l2.bank.immediate_hits")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.immediate_hits");
    let misses = text_stat_value(&stdout, "sim.instruction_cache.l2.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.instruction_cache.l2.bank.coalesced_misses")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.coalesced_misses");
    let accesses = hits + misses;
    assert!(misses > 0, "{stdout}");
    assert_eq!(text_stat_value(&stdout, "system.l2.overallHits"), hits);
    assert_eq!(text_stat_value(&stdout, "system.l2.overallMisses"), misses);
    assert_eq!(
        text_stat_value(&stdout, "system.l2.overallAccesses"),
        accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.l2.overallMissRate"),
        fixed_ratio(misses, accesses)
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_l2_cache_overall_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l2-cache-overall-aliases-json");

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
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
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

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.l2.bank.scheduled_misses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.l2.bank.scheduled_misses",
        "Count",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.l2.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMissRate\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l3_cache_overall_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l3-cache-overall-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let hits = text_stat_value(&stdout, "sim.instruction_cache.l3.bank.immediate_hits")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.immediate_hits");
    let misses = text_stat_value(&stdout, "sim.instruction_cache.l3.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.instruction_cache.l3.bank.coalesced_misses")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.coalesced_misses");
    let accesses = hits + misses;
    assert!(misses > 0, "{stdout}");
    assert_eq!(text_stat_value(&stdout, "system.l3.overallHits"), hits);
    assert_eq!(text_stat_value(&stdout, "system.l3.overallMisses"), misses);
    assert_eq!(
        text_stat_value(&stdout, "system.l3.overallAccesses"),
        accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.l3.overallMissRate"),
        fixed_ratio(misses, accesses)
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_l3_cache_overall_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l3-cache-overall-aliases-json");

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

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.l3.bank.scheduled_misses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.l3.bank.scheduled_misses",
        "Count",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.l3.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMissRate\""));
}

#[test]
fn rem6_run_text_stats_omit_gem5_l1_demand_aliases_when_prefetch_issued() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-demand-alias-prefetch-omission");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    assert!(text_stat_value(&stdout, "sim.data_cache.prefetch.issued") > 0);
    assert!(has_text_stat(&stdout, "system.cpu.dcache.overallHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandMisses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandAccesses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandMissRate"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_issued_aliases() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-prefetcher-pf-issued-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    let icache_issued = text_stat_value(&stdout, "sim.instruction_cache.prefetch.issued");
    let dcache_issued = text_stat_value(&stdout, "sim.data_cache.prefetch.issued");
    assert!(icache_issued > 0, "{stdout}");
    assert!(dcache_issued > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.prefetcher.pfIssued"),
        icache_issued
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfIssued"),
        dcache_issued
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.icache.prefetcher.pfIssued").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfIssued").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_issued_aliases() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-prefetcher-pf-issued-aliases-json");

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
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.prefetch.issued",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.issued",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.icache.prefetcher.pfIssued"),
        stat_value(&stdout, "sim.instruction_cache.prefetch.issued")
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfIssued"),
        stat_value(&stdout, "sim.data_cache.prefetch.issued")
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_identified_aliases() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-prefetcher-pf-identified-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    let icache_identified = text_stat_value(&stdout, "sim.instruction_cache.prefetch.identified");
    let dcache_identified = text_stat_value(&stdout, "sim.data_cache.prefetch.identified");
    assert!(icache_identified > 0, "{stdout}");
    assert!(dcache_identified > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.prefetcher.pfIdentified"),
        icache_identified
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfIdentified"),
        dcache_identified
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.icache.prefetcher.pfIdentified").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfIdentified").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_identified_aliases() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-prefetcher-pf-identified-aliases-json");

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
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.prefetch.identified",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.identified",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.icache.prefetcher.pfIdentified"),
        stat_value(&stdout, "sim.instruction_cache.prefetch.identified")
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfIdentified"),
        stat_value(&stdout, "sim.data_cache.prefetch.identified")
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_span_page_aliases() {
    let path = page_crossing_prefetch_binary("gem5-l1-prefetcher-pf-span-page-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    let icache_span_page = text_stat_value(&stdout, "sim.instruction_cache.prefetch.span_page");
    let dcache_span_page = text_stat_value(&stdout, "sim.data_cache.prefetch.span_page");
    assert!(icache_span_page > 0, "{stdout}");
    assert!(dcache_span_page > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.prefetcher.pfSpanPage"),
        icache_span_page
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfSpanPage"),
        dcache_span_page
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.icache.prefetcher.pfSpanPage").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfSpanPage").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_span_page_aliases() {
    let path = page_crossing_prefetch_binary("gem5-l1-prefetcher-pf-span-page-aliases-json");

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
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.prefetch.span_page",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.span_page",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.icache.prefetcher.pfSpanPage"),
        stat_value(&stdout, "sim.instruction_cache.prefetch.span_page")
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfSpanPage"),
        stat_value(&stdout, "sim.data_cache.prefetch.span_page")
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_useful_span_page_alias() {
    let path = useful_span_page_prefetch_binary("gem5-l1-prefetcher-pf-useful-span-page-alias");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfUsefulSpanPage"),
        text_stat_value(&stdout, "sim.data_cache.prefetch.useful_span_page")
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfUsefulSpanPage")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.pfUsefulSpanPage"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_in_cache_alias() {
    let path = same_line_data_prefetch_binary("gem5-l1-prefetcher-pf-in-cache-alias");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    let dcache_in_cache = text_stat_value(&stdout, "sim.data_cache.prefetch.in_cache");
    assert!(dcache_in_cache > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfInCache"),
        dcache_in_cache
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfInCache").contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.pfInCache"));
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_in_cache_alias() {
    let path = same_line_data_prefetch_binary("gem5-l1-prefetcher-pf-in-cache-alias-json");

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
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.in_cache",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfInCache"),
        stat_value(&stdout, "sim.data_cache.prefetch.in_cache")
    );
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.prefetcher.pfInCache\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_useful_alias() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-pf-useful-alias");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    let dcache_useful = text_stat_value(&stdout, "sim.data_cache.prefetch.useful");
    assert!(dcache_useful > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfUseful"),
        dcache_useful
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfUseful").contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.pfUseful"));
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_useful_alias() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-pf-useful-alias-json");

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
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.useful",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfUseful"),
        stat_value(&stdout, "sim.data_cache.prefetch.useful")
    );
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.prefetcher.pfUseful\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_useful_but_miss_alias() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-pf-useful-but-miss-alias");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    let dcache_useful_but_miss =
        text_stat_value(&stdout, "sim.data_cache.prefetch.useful_but_miss");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfUsefulButMiss"),
        dcache_useful_but_miss
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfUsefulButMiss")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.pfUsefulButMiss"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_late_and_unused_aliases() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-late-unused-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    for (alias, source) in [
        ("pfUnused", "unused"),
        ("pfHitInCache", "hit_in_cache"),
        ("pfHitInMSHR", "hit_in_mshr"),
        ("pfHitInWB", "hit_in_write_buffer"),
        ("pfLate", "late"),
    ] {
        let source_path = format!("sim.data_cache.prefetch.{source}");
        let alias_path = format!("system.cpu.dcache.prefetcher.{alias}");
        assert_eq!(
            text_stat_value(&stdout, &alias_path),
            text_stat_value(&stdout, &source_path),
            "{stdout}"
        );
        assert!(
            text_stat_line(&stdout, &alias_path).contains("unit=Count"),
            "{stdout}"
        );
        assert!(!stdout.contains(&format!("system.cpu.icache.prefetcher.{alias}")));
    }
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_accuracy_and_coverage_aliases() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-accuracy-coverage-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
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

    let dcache_useful = text_stat_value(&stdout, "sim.data_cache.prefetch.useful");
    let dcache_issued = text_stat_value(&stdout, "sim.data_cache.prefetch.issued");
    let dcache_demand_mshr_misses =
        text_stat_value(&stdout, "sim.data_cache.prefetch.demand_mshr_misses");
    assert_eq!(dcache_useful, 1, "{stdout}");
    assert_eq!(dcache_issued, 2, "{stdout}");
    assert_eq!(dcache_demand_mshr_misses, 1, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.demandMshrMisses"),
        dcache_demand_mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.prefetcher.accuracy"),
        fixed_ratio(dcache_useful, dcache_issued)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.prefetcher.coverage"),
        fixed_ratio(
            dcache_useful,
            dcache_useful.saturating_add(dcache_demand_mshr_misses)
        )
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.accuracy").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.coverage").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.accuracy"));
    assert!(!stdout.contains("system.cpu.icache.prefetcher.coverage"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_icache_prefetcher_pf_useful_alias() {
    let path = useful_instruction_prefetch_binary("gem5-l1-icache-prefetcher-pf-useful-alias");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let icache_useful = text_stat_value(&stdout, "sim.instruction_cache.prefetch.useful");
    assert!(icache_useful > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.prefetcher.pfUseful"),
        icache_useful
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.icache.prefetcher.pfUseful").contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.dcache.prefetcher.pfUseful"));
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_icache_prefetcher_pf_useful_alias() {
    let path = useful_instruction_prefetch_binary("gem5-l1-icache-prefetcher-pf-useful-alias-json");

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
            "1",
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

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.prefetch.useful",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.icache.prefetcher.pfUseful"),
        stat_value(&stdout, "sim.instruction_cache.prefetch.useful")
    );
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.prefetcher.pfUseful\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_mem_ctrl_bandwidth_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-mem-ctrl-bandwidth-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let sim_freq = text_stat_value(&stdout, "simFreq");
    let final_tick = text_stat_value(&stdout, "finalTick");
    let read_bytes = text_stat_value(&stdout, "system.mem_ctrl.bytesReadSys");
    let written_bytes = text_stat_value(&stdout, "system.mem_ctrl.bytesWrittenSys");
    let dram_read_bytes = text_stat_value(&stdout, "system.mem_ctrl.dram.dramBytesRead");
    let dram_written_bytes = text_stat_value(&stdout, "system.mem_ctrl.dram.dramBytesWritten");
    assert!(final_tick > 0, "{stdout}");
    assert!(read_bytes > 0, "{stdout}");
    assert!(written_bytes > 0, "{stdout}");
    assert_eq!(dram_read_bytes, read_bytes);
    assert_eq!(dram_written_bytes, written_bytes);

    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.avgRdBWSys"),
        fixed_ratio_precision(read_bytes * sim_freq, final_tick, 8)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.avgWrBWSys"),
        fixed_ratio_precision(written_bytes * sim_freq, final_tick, 8)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.avgRdBWSys").contains("unit=(Byte/Second)"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.avgWrBWSys").contains("unit=(Byte/Second)"),
        "{stdout}"
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.avgRdBW"),
        fixed_ratio_default_precision(dram_read_bytes * sim_freq, final_tick * 1_000_000)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.avgWrBW"),
        fixed_ratio_default_precision(dram_written_bytes * sim_freq, final_tick * 1_000_000)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.avgRdBW").contains("unit=(Byte/Second)"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.avgWrBW").contains("unit=(Byte/Second)"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_mem_ctrl_bandwidth_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-mem-ctrl-bandwidth-aliases-json");

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

    assert_stat_greater_than(
        &stdout,
        "system.mem_ctrl.bytesReadSys",
        "Byte",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "system.mem_ctrl.bytesWrittenSys",
        "Byte",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.avgRdBWSys\""));
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.avgWrBWSys\""));
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.avgRdBW\""));
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.avgWrBW\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_nvm_interface_byte_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-nvm-interface-byte-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "nvm",
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

    assert_eq!(
        text_stat_value(&stdout, "sim.memory.dram.profile.technology.nvm"),
        1
    );
    let read_bytes = text_stat_value(&stdout, "system.mem_ctrl.bytesReadSys");
    let written_bytes = text_stat_value(&stdout, "system.mem_ctrl.bytesWrittenSys");
    assert!(read_bytes > 0, "{stdout}");
    assert!(written_bytes > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.mem_ctrl.dram.nvmBytesRead"),
        read_bytes
    );
    assert_eq!(
        text_stat_value(&stdout, "system.mem_ctrl.dram.nvmBytesWritten"),
        written_bytes
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.nvmBytesRead").contains("unit=Byte"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.nvmBytesWritten").contains("unit=Byte"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_nvm_interface_byte_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-nvm-interface-byte-aliases-json");

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

    assert_stat(
        &stdout,
        "sim.memory.dram.profile.technology.nvm",
        "Count",
        1,
        "constant",
    );
    let read_bytes = stat_value(&stdout, "system.mem_ctrl.bytesReadSys");
    let written_bytes = stat_value(&stdout, "system.mem_ctrl.bytesWrittenSys");
    assert!(read_bytes > 0, "{stdout}");
    assert!(written_bytes > 0, "{stdout}");
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.nvmBytesRead",
        "Byte",
        read_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.nvmBytesWritten",
        "Byte",
        written_bytes,
        "monotonic",
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_burst_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-burst-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.readBursts");
    let write_bursts = text_stat_value(&stdout, "system.mem_ctrl.writeBursts");
    let dram_read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let dram_write_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    assert!(read_bursts > 0, "{stdout}");
    assert!(write_bursts > 0, "{stdout}");
    assert_eq!(dram_read_bursts, read_bursts);
    assert_eq!(dram_write_bursts, write_bursts);
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.readBursts").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.writeBursts").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_dram_interface_burst_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-burst-aliases-json");

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

    assert_stat_greater_than(
        &stdout,
        "system.mem_ctrl.dram.readBursts",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "system.mem_ctrl.dram.writeBursts",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_per_bank_burst_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-per-bank-burst-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let write_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    let per_bank_reads =
        text_stat_values_with_prefix(&stdout, "system.mem_ctrl.dram.perBankRdBursts.bank");
    let per_bank_writes =
        text_stat_values_with_prefix(&stdout, "system.mem_ctrl.dram.perBankWrBursts.bank");
    assert!(!per_bank_reads.is_empty(), "{stdout}");
    assert!(!per_bank_writes.is_empty(), "{stdout}");
    assert_eq!(per_bank_reads.iter().sum::<u64>(), read_bursts);
    assert_eq!(per_bank_writes.iter().sum::<u64>(), write_bursts);
    assert!(
        text_stat_lines_with_prefix(&stdout, "system.mem_ctrl.dram.perBankRdBursts.bank",)
            .iter()
            .all(|line| line.contains("unit=Count")),
        "{stdout}"
    );
    assert!(
        text_stat_lines_with_prefix(&stdout, "system.mem_ctrl.dram.perBankWrBursts.bank",)
            .iter()
            .all(|line| line.contains("unit=Count")),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_dram_interface_per_bank_burst_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-per-bank-burst-aliases-json");

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

    let read_bursts = stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let write_bursts = stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    let per_bank_reads = json_stat_values_with_prefix(
        &stdout,
        "system.mem_ctrl.dram.perBankRdBursts.bank",
        "Count",
        "monotonic",
    );
    let per_bank_writes = json_stat_values_with_prefix(
        &stdout,
        "system.mem_ctrl.dram.perBankWrBursts.bank",
        "Count",
        "monotonic",
    );
    assert!(!per_bank_reads.is_empty(), "{stdout}");
    assert!(!per_bank_writes.is_empty(), "{stdout}");
    assert_eq!(per_bank_reads.iter().sum::<u64>(), read_bursts);
    assert_eq!(per_bank_writes.iter().sum::<u64>(), write_bursts);
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_mem_acc_latency_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-mem-acc-latency-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let all_access_latency = text_stat_value(&stdout, "sim.memory.dram.total_ready_latency_ticks");
    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let read_latency = text_stat_value(&stdout, "system.mem_ctrl.dram.totMemAccLat");
    assert!(all_access_latency > 0, "{stdout}");
    assert!(read_latency > 0, "{stdout}");
    assert!(read_latency < all_access_latency, "{stdout}");
    assert!(read_bursts > 0, "{stdout}");
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.avgMemAccLat"),
        fixed_ratio_precision(read_latency, read_bursts, 2)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.totMemAccLat").contains("unit=Tick"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.avgMemAccLat").contains("unit=(Tick/Count)"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_dram_interface_mem_acc_latency_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-mem-acc-latency-aliases-json");

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
    let all_access_latency = stat_value(&stdout, "sim.memory.dram.total_ready_latency_ticks");
    let read_latency = stat_value(&stdout, "system.mem_ctrl.dram.totMemAccLat");
    assert!(all_access_latency > 0, "{stdout}");
    assert!(read_latency > 0, "{stdout}");
    assert!(read_latency < all_access_latency, "{stdout}");
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.totMemAccLat",
        "Tick",
        read_latency,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.avgMemAccLat\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_row_hit_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-row-hit-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let row_hits = text_stat_value(&stdout, "sim.memory.dram.row_hits");
    let read_row_hits = text_stat_value(&stdout, "system.mem_ctrl.dram.readRowHits");
    let write_row_hits = text_stat_value(&stdout, "system.mem_ctrl.dram.writeRowHits");
    assert_eq!(read_row_hits + write_row_hits, row_hits);
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.readRowHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.writeRowHits").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_dram_interface_row_hit_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-row-hit-aliases-json");

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

    let row_hits = stat_value(&stdout, "sim.memory.dram.row_hits");
    let read_row_hits = stat_value(&stdout, "system.mem_ctrl.dram.readRowHits");
    let write_row_hits = stat_value(&stdout, "system.mem_ctrl.dram.writeRowHits");
    assert_eq!(read_row_hits + write_row_hits, row_hits);
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.readRowHits",
        "Count",
        read_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.writeRowHits",
        "Count",
        write_row_hits,
        "monotonic",
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_row_hit_rate_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-row-hit-rate-aliases");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let read_row_hits = text_stat_value(&stdout, "system.mem_ctrl.dram.readRowHits");
    let write_row_hits = text_stat_value(&stdout, "system.mem_ctrl.dram.writeRowHits");
    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let write_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    assert!(read_bursts > 0, "{stdout}");
    assert!(write_bursts > 0, "{stdout}");
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.readRowHitRate"),
        fixed_ratio_precision(read_row_hits * 100, read_bursts, 2)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.writeRowHitRate"),
        fixed_ratio_precision(write_row_hits * 100, write_bursts, 2)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.readRowHitRate").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.writeRowHitRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_dram_interface_row_hit_rate_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-row-hit-rate-aliases-json");

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
    let read_row_hits = stat_value(&stdout, "system.mem_ctrl.dram.readRowHits");
    let write_row_hits = stat_value(&stdout, "system.mem_ctrl.dram.writeRowHits");
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.readRowHits",
        "Count",
        read_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.writeRowHits",
        "Count",
        write_row_hits,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.readRowHitRate\""));
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.writeRowHitRate\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_page_hit_rate_alias() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-page-hit-rate-alias");

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
            "text",
            "--execute",
            "--cores",
            "1",
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

    let row_hits = text_stat_value(&stdout, "sim.memory.dram.row_hits");
    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let write_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    let total_bursts = read_bursts + write_bursts;
    assert!(total_bursts > 0, "{stdout}");
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.pageHitRate"),
        fixed_ratio_precision(row_hits * 100, total_bursts, 2)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.pageHitRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_dram_interface_page_hit_rate_alias() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-page-hit-rate-alias-json");

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
    assert_stat_greater_than(&stdout, "sim.memory.dram.accesses", "Count", 0, "monotonic");
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.pageHitRate\""));
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
            "--memory-system",
            "direct",
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
    assert!(stdout.contains("\"in_order_pipeline\":{\"cycles\":10,\"in_flight\":0,"));
    assert!(stdout.contains(
        "\"stage_in_flight\":{\"fetch1\":0,\"fetch2\":0,\"decode\":0,\"execute\":0,\"commit\":0}"
    ));
    assert!(stdout.contains(
        "\"stage_max_in_flight\":{\"fetch1\":1,\"fetch2\":1,\"decode\":1,\"execute\":1,\"commit\":1}"
    ));
    assert!(stdout.contains("\"stage_occupied_cycles\":{\"fetch1\":"));
    assert!(stdout.contains("\"retired\":2"));
    assert!(stdout.contains("\"resource_blocked\":4"));
    assert!(stdout.contains("\"stall_cycles\":4"));
    assert!(stdout.contains("\"fetch_wait_cycles\":4"));
    assert!(stdout.contains("\"data_wait_cycles\":0"));
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        10,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.retired",
        "Count",
        2,
        "monotonic",
    );
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_stat(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.max_in_flight"),
            "Count",
            1,
            "monotonic",
        );
        assert_stat_greater_than(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.occupied_cycles"),
            "Cycle",
            0,
            "monotonic",
        );
    }
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.resource_blocked",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.stall_cycles",
        "Cycle",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.fetch_wait_cycles",
        "Cycle",
        4,
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
fn rem6_run_stats_emit_configured_in_order_pipeline_widths_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-pipeline-width-stats", &elf);

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
            "--memory-system",
            "direct",
            "--riscv-in-order-width",
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
    assert!(stdout.contains("\"committed_instructions\":2"));
    assert!(stdout.contains(
        "\"stage_widths\":{\"fetch1\":2,\"fetch2\":2,\"decode\":2,\"execute\":2,\"commit\":2}"
    ));
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_stat(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.width"),
            "Count",
            2,
            "constant",
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_width_changes_executed_stage_occupancy() {
    let program = riscv64_program(&[
        0x0010_0093, // addi x1, x0, 1
        0x0020_0113, // addi x2, x0, 2
        0x0030_0193, // addi x3, x0, 3
        0x0040_0213, // addi x4, x0, 4
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-pipeline-width-timing", &elf);

    let width_one = in_order_pipeline_stats_for_width(&path, 1);
    let width_two = in_order_pipeline_stats_for_width(&path, 2);

    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_stat(
            &width_one,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.max_in_flight"),
            "Count",
            1,
            "monotonic",
        );
        assert_stat(
            &width_two,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.max_in_flight"),
            "Count",
            2,
            "monotonic",
        );
    }
    assert_stat_greater_than(
        &width_two,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        stat_value(&width_two, "sim.cpu0.instructions.committed"),
        "monotonic",
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_integer_mul_execute_latency() {
    const EXPECTED_MUL_EXTRA_EXECUTE_CYCLES: u64 = 2;

    let add_stats = in_order_pipeline_latency_stats(
        "in-order-add-execute-latency",
        &[
            0x0060_0093, // addi x1, x0, 6
            0x0070_0113, // addi x2, x0, 7
            0x0020_81b3, // add x3, x1, x2
            0x0000_0073, // ecall
        ],
    );
    assert_eq!(stat_value(&add_stats, "sim.cpu0.instructions.committed"), 4);
    assert_eq!(
        stat_value(&add_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
        0
    );

    let add_cycles = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.cycles");
    let add_stall = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
    for (name, word) in [
        ("mul", 0x0220_81b3),
        ("mulh", 0x0220_91b3),
        ("mulhsu", 0x0220_a1b3),
        ("mulhu", 0x0220_b1b3),
    ] {
        let mul_stats = in_order_pipeline_latency_stats(
            &format!("in-order-{name}-execute-latency"),
            &[
                0x0060_0093, // addi x1, x0, 6
                0x0070_0113, // addi x2, x0, 7
                word,
                0x0000_0073, // ecall
            ],
        );

        assert_eq!(stat_value(&mul_stats, "sim.cpu0.instructions.committed"), 4);
        assert_eq!(
            stat_value(&mul_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            0
        );

        let mul_cycles = stat_value(&mul_stats, "sim.cpu0.pipeline.in_order.cycles");
        assert_eq!(
            mul_cycles - add_cycles,
            EXPECTED_MUL_EXTRA_EXECUTE_CYCLES,
            "{name} should consume the fixed extra execute latency: add={add_cycles}, {name}={mul_cycles}\nadd stats:\n{add_stats}\n{name} stats:\n{mul_stats}"
        );

        let mul_stall = stat_value(&mul_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
        assert_eq!(
            mul_stall - add_stall,
            EXPECTED_MUL_EXTRA_EXECUTE_CYCLES,
            "{name} should add the fixed execute-stage pipeline stall cycles: add={add_stall}, {name}={mul_stall}\nadd stats:\n{add_stats}\n{name} stats:\n{mul_stats}"
        );
    }
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

fn in_order_pipeline_latency_stats(name: &str, words: &[u32]) -> String {
    let program = riscv64_program(words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(name, &elf);

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
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn in_order_pipeline_stats_for_width(path: &std::path::Path, width: u64) -> String {
    let width = width.to_string();
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
            "--riscv-branch-lookahead",
            "2",
            "--riscv-in-order-width",
            &width,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"committed_instructions\":5"));
    stdout
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
            "--memory-system",
            "direct",
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
    assert!(stdout.contains("\"stage_in_flight\":{\"fetch1\":"));
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
    let stage_in_flight = [
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stage.fetch1.in_flight"),
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stage.fetch2.in_flight"),
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stage.decode.in_flight"),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.execute.in_flight",
        ),
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stage.commit.in_flight"),
    ];
    assert_eq!(stage_in_flight.iter().sum::<u64>(), 1);
    assert!(stage_in_flight.contains(&1));
    let stage_max_in_flight = [
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.fetch1.max_in_flight",
        ),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.fetch2.max_in_flight",
        ),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.decode.max_in_flight",
        ),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.execute.max_in_flight",
        ),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.commit.max_in_flight",
        ),
    ];
    assert!(stage_max_in_flight
        .iter()
        .zip(stage_in_flight)
        .all(|(max, current)| *max >= current));
    assert!(stage_max_in_flight.iter().sum::<u64>() >= 1);
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
fn rem6_run_stats_issue_jal_fetch_ahead_before_retire() {
    let program = riscv64_program(&[
        0x0070_0293,  // addi x5, x0, 7
        j_type(8, 0), // jal x0, target
        0x0010_0313,  // addi x6, x0, 1
        0x0000_0073,  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-jal-fetch-ahead-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "21",
            "--memory-route-delay",
            "5",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
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
    assert_eq!(
        json_u64_field(&stdout, "\"committed_instructions\":"),
        2,
        "{stdout}"
    );
    assert_eq!(json_u64_field(&stdout, "\"in_flight\":"), 1, "{stdout}");
    assert_eq!(
        stat_value(&stdout, "sim.memory.fetch.requests"),
        3,
        "{stdout}"
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.fetch.responses"),
        2,
        "{stdout}"
    );
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x1\""));
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
fn rem6_run_stats_emit_in_order_resource_stalls_for_pending_parallel_fetch() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-pending-fetch-resource-stalls", &elf);

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
    let stall_cycles = json_u64_field(&stdout, "\"stall_cycles\":");
    let resource_blocked = json_u64_field(&stdout, "\"resource_blocked\":");
    assert!(fetch_wait_cycles > 0, "{stdout}");
    assert!(stall_cycles > 0, "{stdout}");
    assert!(resource_blocked > 0, "{stdout}");
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stall_cycles"),
        stall_cycles
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.resource_blocked"),
        resource_blocked
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

#[test]
fn rem6_run_stats_emit_in_order_nested_branch_speculation_rollback() {
    let program = nested_branch_speculation_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-nested-branch-speculation", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--memory-route-delay",
            "1",
            "--riscv-branch-lookahead",
            "2",
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
    let predictions = json_u64_field(&stdout, "\"branch_speculation_predictions\":");
    let repairs = json_u64_field(&stdout, "\"branch_speculation_repairs\":");
    let removed_youngers = json_u64_field(&stdout, "\"branch_speculation_removed_youngers\":");
    let max_pending = json_u64_field(&stdout, "\"branch_speculation_max_pending\":");

    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_speculation_predictions"
        ),
        predictions
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_speculation_repairs"
        ),
        repairs
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_speculation_removed_youngers"
        ),
        removed_youngers
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_speculation_max_pending"
        ),
        max_pending
    );
    assert_eq!(predictions, 2, "{stdout}");
    assert_eq!(repairs, 1, "{stdout}");
    assert_eq!(removed_youngers, 1, "{stdout}");
    assert_eq!(max_pending, 2, "{stdout}");
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x1\""));
    assert!(!stdout.contains("\"x7\":\"0x2\""));
}

#[test]
fn rem6_run_stats_keep_default_branch_speculation_depth_single_pending() {
    let program = nested_branch_speculation_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-default-branch-speculation", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--memory-route-delay",
            "1",
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
    assert_eq!(
        json_u64_field(&stdout, "\"branch_speculation_predictions\":"),
        1,
        "{stdout}"
    );
    assert_eq!(
        json_u64_field(&stdout, "\"branch_speculation_repairs\":"),
        1,
        "{stdout}"
    );
    assert_eq!(
        json_u64_field(&stdout, "\"branch_speculation_removed_youngers\":"),
        0,
        "{stdout}"
    );
    assert_eq!(
        json_u64_field(&stdout, "\"branch_speculation_max_pending\":"),
        1,
        "{stdout}"
    );
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x1\""));
    assert!(!stdout.contains("\"x7\":\"0x2\""));
}

#[test]
fn rem6_run_stats_use_selected_gshare_branch_predictor_for_fetch_steering() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-gshare-branch-steering", &elf);

    let basic = selected_branch_predictor_stdout(&path, "basic");
    let gshare = selected_branch_predictor_stdout(&path, "gshare");

    let gshare_predictions = json_u64_field(&gshare, "\"branch_speculation_predictions\":");
    let basic_final_tick = json_u64_field(&basic, "\"final_tick\":");
    let gshare_final_tick = json_u64_field(&gshare, "\"final_tick\":");

    assert!(gshare_predictions >= 3, "{gshare}");
    assert_ne!(
        gshare_final_tick, basic_final_tick,
        "basic final_tick={basic_final_tick}, gshare final_tick={gshare_final_tick}\nbasic:\n{basic}\ngshare:\n{gshare}"
    );
    assert!(gshare.contains("\"x5\":\"0x7\""));
    assert!(!gshare.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_use_selected_bimode_branch_predictor_for_fetch_steering() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-bimode-branch-steering", &elf);

    let basic = selected_branch_predictor_stdout(&path, "basic");
    let bimode = selected_branch_predictor_stdout(&path, "bimode");

    let bimode_predictions = json_u64_field(&bimode, "\"branch_speculation_predictions\":");
    let basic_final_tick = json_u64_field(&basic, "\"final_tick\":");
    let bimode_final_tick = json_u64_field(&bimode, "\"final_tick\":");

    assert!(bimode_predictions >= 3, "{bimode}");
    assert_ne!(
        bimode_final_tick, basic_final_tick,
        "basic final_tick={basic_final_tick}, bimode final_tick={bimode_final_tick}\nbasic:\n{basic}\nbimode:\n{bimode}"
    );
    assert!(bimode.contains("\"x5\":\"0x7\""));
    assert!(!bimode.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_use_selected_multiperspective_perceptron_for_fetch_steering() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-multiperspective-perceptron-branch-steering", &elf);

    let basic = selected_branch_predictor_stdout(&path, "basic");
    let perceptron = selected_branch_predictor_stdout(&path, "multiperspective-perceptron");

    let perceptron_predictions = json_u64_field(&perceptron, "\"branch_speculation_predictions\":");
    let basic_final_tick = json_u64_field(&basic, "\"final_tick\":");
    let perceptron_final_tick = json_u64_field(&perceptron, "\"final_tick\":");

    assert!(perceptron_predictions >= 3, "{perceptron}");
    assert_ne!(
        perceptron_final_tick, basic_final_tick,
        "basic final_tick={basic_final_tick}, perceptron final_tick={perceptron_final_tick}\nbasic:\n{basic}\nperceptron:\n{perceptron}"
    );
    assert!(perceptron.contains("\"x5\":\"0x7\""));
    assert!(!perceptron.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_use_selected_tage_sc_l_branch_predictor_for_fetch_steering() {
    let program = tage_sc_l_initial_bias_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-tage-sc-l-initial-branch-steering", &elf);

    let basic = selected_branch_predictor_stdout(&path, "basic");
    let tage_sc_l = selected_branch_predictor_stdout(&path, "tage-sc-l");

    let tage_sc_l_predictions = json_u64_field(&tage_sc_l, "\"branch_speculation_predictions\":");
    let basic_final_tick = json_u64_field(&basic, "\"final_tick\":");
    let tage_sc_l_final_tick = json_u64_field(&tage_sc_l, "\"final_tick\":");

    assert!(tage_sc_l_predictions >= 1, "{tage_sc_l}");
    assert_ne!(
        tage_sc_l_final_tick, basic_final_tick,
        "basic final_tick={basic_final_tick}, tage-sc-l final_tick={tage_sc_l_final_tick}\nbasic:\n{basic}\ntage-sc-l:\n{tage_sc_l}"
    );
    assert!(tage_sc_l.contains("\"x5\":\"0x7\""));
    assert!(!tage_sc_l.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_use_retired_tage_sc_l_training_for_later_fetch_steering() {
    let program = tage_sc_l_repeated_not_taken_training_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-tage-sc-l-training-feedback", &elf);

    let tage_sc_l = selected_branch_predictor_stdout(&path, "tage-sc-l");

    let predictions = json_u64_field(&tage_sc_l, "\"branch_speculation_predictions\":");
    let repairs = json_u64_field(&tage_sc_l, "\"branch_speculation_repairs\":");

    assert_eq!(predictions, 4, "{tage_sc_l}");
    assert_eq!(repairs, 2, "{tage_sc_l}");
    assert!(tage_sc_l.contains("\"x5\":\"0x7\""));
    assert!(!tage_sc_l.contains("\"x6\":\"0x1\""));
}

fn selected_branch_predictor_stdout(path: &std::path::Path, predictor: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--memory-route-delay",
            "1",
            "--riscv-branch-lookahead",
            "2",
            "--riscv-branch-predictor",
            predictor,
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
    String::from_utf8(output.stdout).unwrap()
}

fn nested_branch_speculation_program() -> Vec<u8> {
    riscv64_program(&[
        b_type(16, 0, 0, 0x0), // beq x0, x0, target
        b_type(8, 0, 0, 0x0),  // wrong-path beq x0, x0, skipped
        0x0010_0313,           // addi x6, x0, 1
        0x0020_0393,           // addi x7, x0, 2
        0x0070_0293,           // addi x5, x0, 7
        0x0000_0073,           // ecall
    ])
}

fn selected_branch_predictor_program() -> Vec<u8> {
    riscv64_program(&[
        i_type(1, 8, 0x0, 8, 0x13), // addi x8, x8, 1
        i_type(3, 0, 0x0, 9, 0x13), // addi x9, x0, 3
        b_type(12, 9, 8, 0x4),      // blt x8, x9, loop_body
        0x0070_0293,                // addi x5, x0, 7
        0x0000_0073,                // ecall
        0x0000_0513,                // addi x10, x0, 0
        j_type(-24, 0),             // jal x0, loop
    ])
}

fn tage_sc_l_initial_bias_program() -> Vec<u8> {
    riscv64_program(&[
        b_type(12, 0, 0, 0x1), // bne x0, x0, wrong_path
        0x0070_0293,           // addi x5, x0, 7
        0x0000_0073,           // ecall
        0x0010_0313,           // addi x6, x0, 1
        0x0000_0073,           // ecall
    ])
}

fn tage_sc_l_repeated_not_taken_training_program() -> Vec<u8> {
    riscv64_program(&[
        i_type(0, 0, 0x0, 8, 0x13),    // addi x8, x0, 0
        i_type(0, 0, 0x0, 9, 0x13),    // addi x9, x0, 0
        i_type(2, 0, 0x0, 10, 0x13),   // addi x10, x0, 2
        b_type(20, 9, 8, 0x1),         // bne x8, x9, wrong_path
        i_type(-1, 10, 0x0, 10, 0x13), // addi x10, x10, -1
        b_type(-8, 0, 10, 0x1),        // bne x10, x0, loop
        0x0070_0293,                   // addi x5, x0, 7
        0x0000_0073,                   // ecall
        0x0010_0313,                   // addi x6, x0, 1
        0x0000_0073,                   // ecall
    ])
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

fn text_stat_line<'a>(stdout: &'a str, path: &str) -> &'a str {
    stdout
        .lines()
        .find(|line| line.split_whitespace().next() == Some(path))
        .unwrap_or_else(|| panic!("missing text stat {path} in output:\n{stdout}"))
}

fn fixed_ratio(numerator: u64, denominator: u64) -> String {
    assert_ne!(denominator, 0);
    format!("{:.6}", numerator as f64 / denominator as f64)
}

fn fixed_ratio_precision(numerator: u64, denominator: u64, precision: usize) -> String {
    assert_ne!(denominator, 0);
    format!(
        "{:.precision$}",
        numerator as f64 / denominator as f64,
        precision = precision
    )
}

fn fixed_ratio_default_precision(numerator: u64, denominator: u64) -> String {
    assert_ne!(denominator, 0);
    let value = numerator as f64 / denominator as f64;
    if value == value.round() {
        format!("{value:.0}")
    } else {
        format!("{value:.6}")
    }
}

fn gem5_l1_cache_alias_binary(name: &str) -> std::path::PathBuf {
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
    temp_binary(name, &elf)
}

fn tagged_next_line_prefetch_binary(name: &str) -> std::path::PathBuf {
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
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn page_crossing_prefetch_binary(name: &str) -> std::path::PathBuf {
    const DATA_OFFSET: usize = 0x1000;

    let mut program = riscv64_program(&[
        u_type(0x1000, 2, 0x17),    // auipc x2, 0x1000
        i_type(0, 2, 0x3, 5, 0x03), // ld x5, 0(x2)
        0x0000_0073,                // ecall
    ]);
    program.resize(DATA_OFFSET + 64, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0ff0, 0x8000_0ff0, &program);
    temp_binary(name, &elf)
}

fn useful_span_page_prefetch_binary(name: &str) -> std::path::PathBuf {
    const DATA_OFFSET: usize = 0xfe0;

    let mut program = riscv64_program(&[
        u_type(0x1000, 2, 0x17),      // auipc x2, 0x1000
        i_type(-32, 2, 0x0, 2, 0x13), // addi x2, x2, -32
        i_type(0, 2, 0x3, 5, 0x03),   // ld x5, 0(x2)
        i_type(16, 2, 0x3, 6, 0x03),  // ld x6, 16(x2)
        0x0000_0073,                  // ecall
    ]);
    program.resize(DATA_OFFSET + 64, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 16..DATA_OFFSET + 24]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn same_line_data_prefetch_binary(name: &str) -> std::path::PathBuf {
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
    temp_binary(name, &elf)
}

fn useful_data_prefetch_binary(name: &str) -> std::path::PathBuf {
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
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn useful_instruction_prefetch_binary(name: &str) -> std::path::PathBuf {
    let mut program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        0x0000_0073,                // ecall
    ]);
    program.resize(48, 0);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
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

fn text_stat_values_with_prefix(stdout: &str, prefix: &str) -> Vec<u64> {
    text_stat_lines_with_prefix(stdout, prefix)
        .iter()
        .map(|line| {
            line.split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_else(|| panic!("invalid text stat value in line {line}"))
        })
        .collect()
}

fn text_stat_lines_with_prefix<'a>(stdout: &'a str, prefix: &str) -> Vec<&'a str> {
    stdout
        .lines()
        .filter(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|path| path_has_numeric_suffix(path, prefix))
        })
        .collect()
}

fn json_stat_values_with_prefix(
    stdout: &str,
    prefix: &str,
    unit: &str,
    reset_policy: &str,
) -> Vec<u64> {
    stdout
        .split("{\"id\":")
        .skip(1)
        .filter_map(|tail| {
            let sample_end = tail.find('}').unwrap_or(tail.len());
            let sample = &tail[..sample_end];
            let path_tail = sample.split("\"path\":\"").nth(1)?;
            let path_end = path_tail.find('"')?;
            let path = &path_tail[..path_end];
            if !path_has_numeric_suffix(path, prefix) {
                return None;
            }
            assert!(
                sample.contains(&format!("\"unit\":\"{unit}\"")),
                "missing stat unit {unit} in {sample}"
            );
            assert!(
                sample.contains(&format!("\"reset_policy\":\"{reset_policy}\"")),
                "missing stat reset policy {reset_policy} in {sample}"
            );
            let value_tail = sample
                .split("\"value\":")
                .nth(1)
                .unwrap_or_else(|| panic!("missing stat value in {sample}"));
            let value_end = value_tail
                .find(',')
                .or_else(|| value_tail.find('}'))
                .unwrap_or(value_tail.len());
            Some(
                value_tail[..value_end]
                    .parse::<u64>()
                    .unwrap_or_else(|error| panic!("invalid stat value in {sample}: {error}")),
            )
        })
        .collect()
}

fn path_has_numeric_suffix(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
    })
}

fn has_text_stat(stdout: &str, path: &str) -> bool {
    stdout
        .lines()
        .any(|line| line.split_whitespace().next() == Some(path))
}
