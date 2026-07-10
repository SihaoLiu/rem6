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
    let icache_mshr_hits = text_stat_value(&stdout, "sim.instruction_cache.bank.coalesced_misses");
    let icache_mshr_misses =
        text_stat_value(&stdout, "sim.instruction_cache.bank.scheduled_misses");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandMshrHits"),
        icache_mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallMshrHits"),
        icache_mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandMshrMisses"),
        icache_mshr_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallMshrMisses"),
        icache_mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.icache.demandMshrMissRate"),
        fixed_ratio(icache_mshr_misses, icache_accesses)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.icache.overallMshrMissRate"),
        fixed_ratio(icache_mshr_misses, icache_accesses)
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
    let dcache_mshr_hits = text_stat_value(&stdout, "sim.data_cache.bank.coalesced_misses");
    let dcache_mshr_misses = text_stat_value(&stdout, "sim.data_cache.bank.scheduled_misses");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandMshrHits"),
        dcache_mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallMshrHits"),
        dcache_mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandMshrMisses"),
        dcache_mshr_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallMshrMisses"),
        dcache_mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.demandMshrMissRate"),
        fixed_ratio(dcache_mshr_misses, dcache_accesses)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.overallMshrMissRate"),
        fixed_ratio(dcache_mshr_misses, dcache_accesses)
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.demandMshrHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.demandMshrMissRate").contains("unit=Ratio"),
        "{stdout}"
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
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMshrMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMshrMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMshrMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMshrMissRate\""));
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
    let mshr_hits = text_stat_value(&stdout, "sim.instruction_cache.l2.bank.coalesced_misses")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.coalesced_misses");
    let mshr_misses = text_stat_value(&stdout, "sim.instruction_cache.l2.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.scheduled_misses");
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
    assert_eq!(
        text_stat_value(&stdout, "system.l2.overallMshrHits"),
        mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.l2.overallMshrMisses"),
        mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.l2.overallMshrMissRate"),
        fixed_ratio(mshr_misses, accesses)
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallMshrHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallMshrMissRate").contains("unit=Ratio"),
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
    assert!(!stdout.contains("\"path\":\"system.l2.overallMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMshrMissRate\""));
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
    let mshr_hits = text_stat_value(&stdout, "sim.instruction_cache.l3.bank.coalesced_misses")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.coalesced_misses");
    let mshr_misses = text_stat_value(&stdout, "sim.instruction_cache.l3.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.scheduled_misses");
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
    assert_eq!(
        text_stat_value(&stdout, "system.l3.overallMshrHits"),
        mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.l3.overallMshrMisses"),
        mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.l3.overallMshrMissRate"),
        fixed_ratio(mshr_misses, accesses)
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallMshrHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallMshrMissRate").contains("unit=Ratio"),
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
    assert!(!stdout.contains("\"path\":\"system.l3.overallMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMshrMissRate\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_ruby_network_flit_aliases() {
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
    let path = temp_binary("gem5-ruby-network-flit-aliases", &elf);

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
            "--fabric-router",
            "router0",
            "--fabric-router-input-port",
            "1",
            "--fabric-router-output-port",
            "2",
            "--fabric-router-virtual-channel",
            "3",
            "--fabric-router-latency",
            "5",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let request_flits = text_stat_value(&stdout, "sim.memory.fabric.vn3.flits");
    let response_flits = text_stat_value(&stdout, "sim.memory.fabric.vn4.flits");
    assert!(request_flits > 0, "{stdout}");
    assert!(response_flits > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.ruby.network.flits_injected::vnet-3"),
        request_flits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.ruby.network.flits_received::vnet-4"),
        response_flits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.ruby.network.flits_injected::total"),
        request_flits + response_flits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.ruby.network.flits_received::total"),
        request_flits + response_flits
    );
    assert!(
        text_stat_line(&stdout, "system.ruby.network.flits_injected::vnet-3")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.ruby.network.flits_received::vnet-4")
            .contains("unit=Count"),
        "{stdout}"
    );
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
    assert!(has_text_stat(&stdout, "system.cpu.dcache.overallMshrHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandMisses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandAccesses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandMissRate"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandMshrHits"));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.dcache.demandMshrMisses"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.dcache.demandMshrMissRate"
    ));
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
