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
