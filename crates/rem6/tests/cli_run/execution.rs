use std::process::Command;

use crate::support::*;

#[test]
fn rem6_run_executes_riscv_elf_on_parallel_cores_and_emits_core_stats() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("parallel-exec", &elf);

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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"cores\":2"));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"trap\":\"environment_call\""));
    assert!(stdout.contains("\"parallel\":{\"scheduler\":{"));
    assert!(stdout.contains("\"worker_limit\":2"));
    assert!(stdout.contains("\"worker_slots\":[{\"slot\":0"));
    assert!(stdout.contains("\"worker_lanes\":[{\"lane\":0,\"partition\":0"));
    assert!(stdout.contains("{\"lane\":1,\"partition\":1"));
    assert!(stdout.contains("\"partitions\":[{\"partition\":0"));
    assert!(stdout.contains("\"transport\":{\"fetch\":{\"requests\":4"));
    assert!(stdout.contains("\"route\":0,\"source\":\"cpu0.ifetch\",\"requests\":2"));
    assert!(stdout.contains("\"route\":2,\"source\":\"cpu1.ifetch\",\"requests\":2"));
    assert!(stdout.contains("\"data\":{\"requests\":0"));
    assert!(stdout.contains("\"cpu\":0"));
    assert!(stdout.contains("\"cpu\":1"));
    assert!(stdout.contains("\"pc\":\"0x80000004\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(stdout.contains("\"path\":\"sim.instructions.committed\""));
    assert!(stdout.contains("\"path\":\"sim.cpu0.instructions.committed\""));
    assert!(stdout.contains("\"path\":\"sim.cpu1.instructions.committed\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.max_workers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.dispatches\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.batches\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.total_workers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.active_partitions\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.remote_sends\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.batch.worker_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.batch.worker_capacity_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.batch.idle_worker_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.frontiers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.final_frontiers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.ready_partitions\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.frontier0.partition\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.frontier0.now\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.frontier0.safe_until\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.frontier0.pending_events\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.final_frontier0.partition\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.final_frontier0.now\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.ready_partition0.partition\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.ready_partition0.next_tick\""));
    assert!(stdout.contains("\"frontiers\":[{\"partition\":0"));
    assert!(stdout.contains("\"final_frontiers\":[{\"partition\":0"));
    assert!(stdout.contains("\"ready_partitions\":[{\"partition\":0,\"next_tick\":0}"));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.frontier.now\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.frontier.safe_until\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.frontier.pending_events\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.frontier.final_now\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker0.active_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker0.idle_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker1.active_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker1.idle_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker0.partition0.active_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.scheduler.worker1.partition1.active_ticks\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.scheduler.dispatches\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition0.scheduler.workers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition1.scheduler.dispatches\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition1.scheduler.workers\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition2.scheduler.remote_receives\""));
    assert!(stdout.contains("\"path\":\"sim.parallel.partition3.scheduler.remote_receives\""));
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        2,
        4,
        2,
    );
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route2.source.cpu1.ifetch",
        2,
        4,
        2,
    );
    assert!(stdout.contains("\"value\":4"));
    assert!(stdout.contains("\"value\":2"));
}

#[test]
fn rem6_run_respects_explicit_parallel_worker_limit() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("parallel-worker-limit", &elf);

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
            "2",
            "--parallel-workers",
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
    assert_stat(
        &stdout,
        "sim.parallel.scheduler.worker_limit",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.parallel.scheduler.max_workers",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_stops_riscv_execution_at_instruction_limit() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("instruction-limit", &elf);

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
            "--max-instructions",
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
    assert!(stdout.contains("\"status\":\"stopped_at_instruction_limit\""));
    assert!(stdout.contains("\"stop_reason\":\"instruction_limit\""));
    assert!(stdout.contains("\"instruction_limit\":1"));
    assert!(stdout.contains("\"committed_instructions\":1"));
    assert!(stdout.contains("\"pc\":\"0x80000004\""));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\""));
    assert!(!stdout.contains("\"stop_code\""));
    assert!(!stdout.contains("\"trap\""));
    assert_stat(&stdout, "sim.instructions.limit", "Count", 1, "constant");
    assert_stat(
        &stdout,
        "sim.stop.instruction_limit",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.instructions.committed",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_instruction_limit_is_a_hard_cap_across_parallel_cores() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("parallel-instruction-limit", &elf);

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
            "2",
            "--max-instructions",
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
    assert!(stdout.contains("\"status\":\"stopped_at_instruction_limit\""));
    assert!(stdout.contains("\"cores\":2"));
    assert!(stdout.contains("\"committed_instructions\":1"));
    assert_stat(
        &stdout,
        "sim.instructions.committed",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.stop.instruction_limit",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.instructions.committed",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu1.instructions.committed",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_stops_riscv_execution_at_tick_limit() {
    let program = riscv64_program(&[
        b_type(0, 0, 0, 0x0), // beq x0, x0, self
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("tick-limit", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "4",
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
    assert!(stdout.contains("\"status\":\"stopped_at_tick_limit\""));
    assert!(stdout.contains("\"stop_reason\":\"tick_limit\""));
    assert!(stdout.contains("\"executed_ticks\":4"));
    assert!(stdout.contains("\"final_tick\":4"));
    assert!(stdout.contains("\"tick_limit\":4"));
    assert!(!stdout.contains("\"stop_code\""));
    assert!(!stdout.contains("\"trap\""));
    assert_stat(&stdout, "sim.final_tick", "Tick", 4, "monotonic");
    assert_stat(&stdout, "sim.stop.tick_limit", "Count", 1, "constant");
}

#[test]
fn rem6_run_accepts_scheduler_min_remote_delay_runtime_option() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("min-remote-delay", &elf);

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
            "--min-remote-delay",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"stop_reason\":\"host_trap\""));
    assert!(stdout.contains("\"stop_code\":0"));
    assert!(stdout.contains("\"min_remote_delay\":4"));
    assert!(stdout.contains("\"host_event_delay\":4"));
    assert!(stdout.contains("\"executed_ticks\":20"));
    assert!(stdout.contains("\"final_tick\":20"));
    assert_stat(
        &stdout,
        "sim.parallel.scheduler.min_remote_delay",
        "Tick",
        4,
        "constant",
    );
    assert_stat(&stdout, "sim.host.event_delay", "Tick", 4, "constant");
    assert_stat(&stdout, "sim.final_tick", "Tick", 20, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 2, 16, 8);
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        2,
        16,
        8,
    );
}

#[test]
fn rem6_run_accepts_memory_route_delay_runtime_option() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(16, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&7u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("memory-route-delay", &elf);

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
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
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
    assert!(stdout.contains("\"status\":\"executed_until_trap\""));
    assert!(stdout.contains("\"memory_route_delay\":5"));
    assert!(stdout.contains("\"min_remote_delay\":2"));
    assert!(stdout.contains("\"executed_ticks\":52"));
    assert!(stdout.contains("\"final_tick\":52"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert_stat(&stdout, "sim.memory.route_delay", "Tick", 5, "constant");
    assert_stat(&stdout, "sim.final_tick", "Tick", 52, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 4, 40, 10);
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        4,
        40,
        10,
    );
    assert_transport_stats(&stdout, "sim.memory.data", 1, 10, 10);
    assert_transport_stats(
        &stdout,
        "sim.memory.data.route1.source.cpu0.dmem",
        1,
        10,
        10,
    );
}

#[test]
fn rem6_run_can_execute_riscv_elf_through_dram_memory_and_emit_dram_stats() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-memory", &elf);

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
            "--dram-memory",
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
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(stdout.contains("\"dram\":{\"active_targets\":1"));
    assert!(stdout.contains("\"accesses\":2"));
    assert!(stdout.contains("\"reads\":2"));
    assert!(stdout.contains("\"row_hits\":1"));
    assert!(stdout.contains("\"row_misses\":1"));
    assert!(stdout.contains("\"total_ready_latency_ticks\":13"));
    assert!(stdout.contains("\"max_ready_latency_ticks\":8"));
    assert_stat(
        &stdout,
        "sim.memory.dram.active_targets",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(&stdout, "sim.memory.dram.accesses", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.memory.dram.reads", "Count", 2, "monotonic");
    assert_stat(&stdout, "sim.memory.dram.row_hits", "Count", 1, "monotonic");
    assert_stat(
        &stdout,
        "sim.memory.dram.row_misses",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.total_ready_latency_ticks",
        "Tick",
        13,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.max_ready_latency_ticks",
        "Tick",
        8,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.parallel_ports",
        "Count",
        1,
        "constant",
    );
    assert_stat(
        &stdout,
        "sim.memory.dram.profile.scheduler_banks",
        "Count",
        4,
        "constant",
    );
}

#[test]
fn rem6_run_can_select_external_memory_profile_for_dram_backed_execution() {
    struct Case {
        profile: &'static str,
        parallel_port_label: &'static str,
        topology_unit_label: &'static str,
        parallel_ports: u64,
        topology_units: u64,
        scheduler_banks: u64,
        topology_banks: u64,
        bank_group_count: u64,
        scheduler_bank_groups: u64,
        same_bank_group_burst_spacing: u64,
    }

    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("dram-memory-profile", &elf);

    for case in [
        Case {
            profile: "hbm",
            parallel_port_label: "pseudo_channel",
            topology_unit_label: "pseudo_channel",
            parallel_ports: 4,
            topology_units: 4,
            scheduler_banks: 16,
            topology_banks: 16,
            bank_group_count: 2,
            scheduler_bank_groups: 8,
            same_bank_group_burst_spacing: 6,
        },
        Case {
            profile: "lpddr",
            parallel_port_label: "channel",
            topology_unit_label: "die",
            parallel_ports: 2,
            topology_units: 4,
            scheduler_banks: 8,
            topology_banks: 16,
            bank_group_count: 0,
            scheduler_bank_groups: 0,
            same_bank_group_burst_spacing: 0,
        },
    ] {
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
                "--dram-memory",
                "--dram-memory-profile",
                case.profile,
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr for {}: {}",
            case.profile,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(stdout.contains("\"status\":\"executed_until_trap\""));
        assert!(stdout.contains("\"x5\":\"0x7\""));
        assert!(stdout.contains(&format!("\"technology\":\"{}\"", case.profile)));
        assert!(stdout.contains(&format!(
            "\"parallel_port_label\":\"{}\"",
            case.parallel_port_label
        )));
        assert!(stdout.contains(&format!(
            "\"topology_unit_label\":\"{}\"",
            case.topology_unit_label
        )));
        assert!(stdout.contains(&format!("\"parallel_ports\":{}", case.parallel_ports)));
        assert!(stdout.contains(&format!("\"topology_units\":{}", case.topology_units)));
        assert!(stdout.contains(&format!("\"scheduler_banks\":{}", case.scheduler_banks)));
        assert!(stdout.contains(&format!("\"topology_banks\":{}", case.topology_banks)));
        assert!(stdout.contains(&format!("\"bank_group_count\":{}", case.bank_group_count)));
        assert!(stdout.contains(&format!(
            "\"same_bank_group_burst_spacing\":{}",
            case.same_bank_group_burst_spacing
        )));
        assert!(stdout.contains(&format!(
            "\"scheduler_bank_groups\":{}",
            case.scheduler_bank_groups
        )));
        assert_stat(
            &stdout,
            &format!("sim.memory.dram.profile.technology.{}", case.profile),
            "Count",
            1,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.parallel_ports",
            "Count",
            case.parallel_ports,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.topology_units",
            "Count",
            case.topology_units,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.scheduler_banks",
            "Count",
            case.scheduler_banks,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.topology_banks",
            "Count",
            case.topology_banks,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.geometry.bank_group_count",
            "Count",
            case.bank_group_count,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.timing.same_bank_group_burst_spacing",
            "Tick",
            case.same_bank_group_burst_spacing,
            "constant",
        );
        assert_stat(
            &stdout,
            "sim.memory.dram.profile.scheduler_bank_groups",
            "Count",
            case.scheduler_bank_groups,
            "constant",
        );
    }
}

#[test]
fn rem6_run_accepts_host_event_delay_runtime_option() {
    let program = riscv64_program(&[
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("host-event-delay", &elf);

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
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
            "5",
            "--host-event-delay",
            "7",
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
    assert!(stdout.contains("\"host_event_delay\":7"));
    assert!(stdout.contains("\"memory_route_delay\":5"));
    assert!(stdout.contains("\"min_remote_delay\":2"));
    assert!(stdout.contains("\"executed_ticks\":27"));
    assert!(stdout.contains("\"final_tick\":27"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert_stat(&stdout, "sim.host.event_delay", "Tick", 7, "constant");
    assert_stat(&stdout, "sim.final_tick", "Tick", 27, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 2, 20, 10);
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        2,
        20,
        10,
    );
}

#[test]
fn rem6_run_accepts_start_address_runtime_option() {
    let program = riscv64_program(&[
        i_type(3, 0, 0x0, 6, 0x13), // addi x6, x0, 3
        i_type(7, 6, 0x0, 5, 0x13), // addi x5, x6, 7
        0x0000_0073,                // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("start-address", &elf);

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
            "--start-address",
            "0x80000004",
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
    assert!(stdout.contains("\"entry\":\"0x80000000\""));
    assert!(stdout.contains("\"start_address\":\"0x80000004\""));
    assert!(stdout.contains("\"executed_ticks\":5"));
    assert!(stdout.contains("\"final_tick\":5"));
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert_stat(
        &stdout,
        "sim.start_address",
        "Address",
        0x8000_0004,
        "constant",
    );
    assert_stat(&stdout, "sim.final_tick", "Tick", 5, "monotonic");
    assert_transport_stats(&stdout, "sim.memory.fetch", 2, 4, 2);
    assert_transport_stats(
        &stdout,
        "sim.memory.fetch.route0.source.cpu0.ifetch",
        2,
        4,
        2,
    );
}

#[test]
fn rem6_run_accepts_riscv_boot_register_runtime_options() {
    let program = riscv64_program(&[0x0000_0073]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("riscv-boot-registers", &elf);

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
            "--riscv-boot-a0",
            "0x123",
            "--riscv-boot-a1",
            "0X80001000",
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
    assert!(stdout.contains("\"riscv_boot\":{\"a0\":\"0x123\",\"a1\":\"0x80001000\"}"));
    assert!(stdout.contains("\"executed_ticks\":3"));
    assert!(stdout.contains("\"final_tick\":3"));
    assert!(stdout.contains("\"x10\":\"0x123\""));
    assert!(stdout.contains("\"x11\":\"0x80001000\""));
    assert_stat(&stdout, "sim.riscv.boot.a0", "Value", 0x123, "constant");
    assert_stat(
        &stdout,
        "sim.riscv.boot.a1",
        "Value",
        0x8000_1000,
        "constant",
    );
}
