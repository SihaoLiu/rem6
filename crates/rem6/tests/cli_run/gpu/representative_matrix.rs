use super::*;

#[derive(Clone, Copy)]
struct GpuHierarchyRow {
    name: &'static str,
    extra_args: &'static [&'static str],
    expected_final_tick: u64,
    expected_cache_runs: u64,
    expected_dram_accesses: u64,
    expected_round_trip_ticks: u64,
    expected_max_round_trip_ticks: u64,
    expected_fabric_transfers: Option<u64>,
    expected_cu_transport: [[u64; 4]; 2],
}

const GPU_HIERARCHY_ROWS: &[GpuHierarchyRow] = &[
    GpuHierarchyRow {
        name: "direct",
        extra_args: &[],
        expected_final_tick: 11,
        expected_cache_runs: 0,
        expected_dram_accesses: 0,
        expected_round_trip_ticks: 18,
        expected_max_round_trip_ticks: 2,
        expected_fabric_transfers: None,
        expected_cu_transport: [[12, 2, 11, 11], [6, 2, 11, 11]],
    },
    GpuHierarchyRow {
        name: "cache-dram",
        extra_args: &["--data-cache-protocol", "msi", "--dram-memory"],
        expected_final_tick: 24,
        expected_cache_runs: 9,
        expected_dram_accesses: 6,
        expected_round_trip_ticks: 105,
        expected_max_round_trip_ticks: 15,
        expected_fabric_transfers: None,
        expected_cu_transport: [[70, 15, 19, 24], [35, 15, 19, 24]],
    },
    GpuHierarchyRow {
        name: "fabric-cache-dram",
        extra_args: &[
            "--data-cache-protocol",
            "msi",
            "--dram-memory",
            "--fabric-link",
            "gpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "16",
            "--fabric-request-virtual-network",
            "7",
            "--fabric-response-virtual-network",
            "8",
            "--fabric-credit-depth",
            "2",
        ],
        expected_final_tick: 29,
        expected_cache_runs: 9,
        expected_dram_accesses: 6,
        expected_round_trip_ticks: 144,
        expected_max_round_trip_ticks: 20,
        expected_fabric_transfers: Some(18),
        expected_cu_transport: [[93, 19, 21, 28], [51, 20, 23, 29]],
    },
];

const GPU_MATRIX_COMMON_ARGS: &[&str] = &[
    "gpu-run",
    "--workgroups",
    "3",
    "--compute-units",
    "2",
    "--wave-slots-per-compute-unit",
    "1",
    "--workgroup-cycles",
    "4",
    "--global-load",
    "0x303f:1:0:4",
    "--global-store",
    "0x3080:4:4:4",
    "--memory-start",
    "0x3000",
    "--memory-size",
    "256",
    "--max-tick",
    "200",
    "--stats-format",
    "json",
];

fn run_gpu_hierarchy_row(row: GpuHierarchyRow) -> (String, Value) {
    let mut args = GPU_MATRIX_COMMON_ARGS.to_vec();
    args.extend_from_slice(row.extra_args);
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args(args)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "row {} stderr: {}",
        row.name,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact = serde_json::from_str(&stdout).unwrap();
    (stdout, artifact)
}

fn compute_unit_activity(artifact: &Value) -> &[Value] {
    artifact["simulation"]["compute_unit_activity"]
        .as_array()
        .expect("GPU compute-unit activity array")
}

fn json_u64(value: &Value, key: &str, row: &str) -> u64 {
    value[key]
        .as_u64()
        .unwrap_or_else(|| panic!("row {row} missing unsigned JSON field {key}"))
}

#[test]
fn rem6_gpu_run_correlates_queued_cu_coalescing_across_memory_hierarchy_rows() {
    for row in GPU_HIERARCHY_ROWS.iter().copied() {
        let (stdout, artifact) = run_gpu_hierarchy_row(row);
        let simulation = &artifact["simulation"];
        let compute_units = compute_unit_activity(&artifact);
        assert_eq!(compute_units.len(), 2, "row {} compute units", row.name);

        for (key, expected) in [
            ("final_tick", row.expected_final_tick),
            ("workgroup_completions", 3),
            ("workgroup_queue_wait_count", 1),
            ("workgroup_queue_wait_ticks", 4),
            ("max_workgroup_queue_wait_ticks", 4),
            ("coalesced_memory_accesses", 9),
            ("global_memory_requests", 9),
            ("memory_responses", 9),
        ] {
            assert_eq!(
                json_u64(simulation, key, row.name),
                expected,
                "row {} simulation field {key}",
                row.name
            );
        }

        for (compute_unit, expected) in
            [(0, [2, 1, 4, 8, 6, 4, 2, 6]), (1, [1, 0, 0, 4, 3, 2, 1, 3])]
        {
            let activity = &compute_units[compute_unit];
            for (key, value) in [
                ("workgroup_completions", expected[0]),
                ("workgroup_queue_wait_count", expected[1]),
                ("workgroup_queue_wait_ticks", expected[2]),
                ("busy_cycles", expected[3]),
                ("coalesced_memory_accesses", expected[4]),
                ("global_memory_reads", expected[5]),
                ("global_memory_writes", expected[6]),
            ] {
                assert_eq!(
                    json_u64(activity, key, row.name),
                    value,
                    "row {} CU{compute_unit} field {key}",
                    row.name
                );
            }
            assert_eq!(
                json_u64(&activity["memory_transport"], "responses", row.name),
                expected[7],
                "row {} CU{compute_unit} transport responses",
                row.name
            );
        }

        let cu0_transport = &compute_units[0]["memory_transport"];
        let cu1_transport = &compute_units[1]["memory_transport"];
        let cu_round_trip_ticks = json_u64(cu0_transport, "round_trip_ticks", row.name)
            .checked_add(json_u64(cu1_transport, "round_trip_ticks", row.name))
            .unwrap();
        let cu_max_round_trip_ticks = json_u64(cu0_transport, "max_round_trip_ticks", row.name)
            .max(json_u64(cu1_transport, "max_round_trip_ticks", row.name));
        assert_eq!(
            json_u64(&artifact["transport"], "responses", row.name),
            9,
            "row {} aggregate responses",
            row.name
        );
        assert_eq!(
            json_u64(&artifact["transport"], "round_trip_ticks", row.name),
            row.expected_round_trip_ticks,
            "row {} aggregate round-trip ticks",
            row.name
        );
        assert_eq!(
            cu_round_trip_ticks, row.expected_round_trip_ticks,
            "row {} per-CU round-trip reconciliation",
            row.name
        );
        assert_eq!(
            json_u64(&artifact["transport"], "max_round_trip_ticks", row.name),
            row.expected_max_round_trip_ticks,
            "row {} aggregate max round-trip ticks",
            row.name
        );
        assert_eq!(
            cu_max_round_trip_ticks, row.expected_max_round_trip_ticks,
            "row {} per-CU max round-trip reconciliation",
            row.name
        );

        for (compute_unit, transport) in [(0, cu0_transport), (1, cu1_transport)] {
            let first_response_at = json_u64(transport, "first_response_at", row.name);
            let last_response_at = json_u64(transport, "last_response_at", row.name);
            let expected = row.expected_cu_transport[compute_unit];
            assert_eq!(
                json_u64(transport, "round_trip_ticks", row.name),
                expected[0],
                "row {} CU{compute_unit} round-trip ticks",
                row.name
            );
            assert_eq!(
                json_u64(transport, "max_round_trip_ticks", row.name),
                expected[1],
                "row {} CU{compute_unit} max round-trip ticks",
                row.name
            );
            assert_eq!(
                first_response_at, expected[2],
                "row {} CU{compute_unit} first response tick",
                row.name
            );
            assert_eq!(
                last_response_at, expected[3],
                "row {} CU{compute_unit} last response tick",
                row.name
            );
            assert!(
                first_response_at <= last_response_at,
                "row {} CU{compute_unit} response window",
                row.name
            );
            assert!(
                last_response_at <= row.expected_final_tick,
                "row {} CU{compute_unit} response after final tick",
                row.name
            );
            let prefix = format!("sim.gpu_run.compute_unit.cu{compute_unit}.memory_transport");
            assert_stat(
                &stdout,
                &format!("{prefix}.responses"),
                "Count",
                json_u64(transport, "responses", row.name),
                "monotonic",
            );
            assert_stat(
                &stdout,
                &format!("{prefix}.round_trip_ticks"),
                "Tick",
                json_u64(transport, "round_trip_ticks", row.name),
                "monotonic",
            );
            assert_stat(
                &stdout,
                &format!("{prefix}.max_round_trip_ticks"),
                "Tick",
                json_u64(transport, "max_round_trip_ticks", row.name),
                "monotonic",
            );
            assert_stat(
                &stdout,
                &format!("{prefix}.first_response_at"),
                "Tick",
                first_response_at,
                "monotonic",
            );
            assert_stat(
                &stdout,
                &format!("{prefix}.last_response_at"),
                "Tick",
                last_response_at,
                "monotonic",
            );
        }

        assert_eq!(
            artifact["data_cache"]["data_cache_runs"].as_u64(),
            Some(row.expected_cache_runs),
            "row {} cache runs",
            row.name
        );
        assert_eq!(
            artifact["dram"]["accesses"].as_u64(),
            Some(row.expected_dram_accesses),
            "row {} DRAM accesses",
            row.name
        );
        match row.expected_fabric_transfers {
            Some(transfers) => {
                assert_eq!(
                    artifact["fabric"]["transfers"].as_u64(),
                    Some(transfers),
                    "row {} fabric transfers",
                    row.name
                );
                assert_eq!(
                    artifact["fabric"]["active_virtual_networks"].as_u64(),
                    Some(2),
                    "row {} active virtual networks",
                    row.name
                );
                assert!(
                    artifact["fabric"]["queue_delay_ticks"]
                        .as_u64()
                        .is_some_and(|ticks| ticks > 0),
                    "row {} fabric queue delay",
                    row.name
                );
            }
            None => assert!(
                artifact["fabric"].is_null(),
                "row {} should suppress fabric summary",
                row.name
            ),
        }
    }
}

#[test]
fn rem6_gpu_run_reports_per_compute_unit_activity() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "5",
            "--compute-units",
            "2",
            "--wave-slots-per-compute-unit",
            "1",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3000:4:4:4",
            "--memory-start",
            "0x3000",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let simulation = artifact.get("simulation").unwrap();
    let compute_units = simulation
        .get("compute_unit_activity")
        .and_then(Value::as_array)
        .unwrap();
    assert!(stdout.contains("\"workgroup_completions\":5"));
    assert_eq!(
        compute_units[0]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        compute_units[0].get("busy_cycles").and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        compute_units[1]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[1].get("busy_cycles").and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        simulation
            .get("workgroup_queue_wait_count")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        simulation
            .get("workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(16)
    );
    assert_eq!(
        simulation
            .get("max_workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        compute_units[0]
            .get("workgroup_queue_wait_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0]
            .get("workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(12)
    );
    assert_eq!(
        compute_units[0]
            .get("max_workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        compute_units[1]
            .get("workgroup_queue_wait_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        compute_units[1]
            .get("workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        compute_units[1]
            .get("max_workgroup_queue_wait_ticks")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.workgroup_completions",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.busy_cycles",
        "Cycle",
        12,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.first_started_at",
        "Tick",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.last_completed_at",
        "Tick",
        13,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.workgroup_queue_wait_count",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.workgroup_queue_wait_ticks",
        "Tick",
        12,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.max_workgroup_queue_wait_ticks",
        "Tick",
        8,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_completions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.busy_cycles",
        "Cycle",
        8,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.first_started_at",
        "Tick",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.last_completed_at",
        "Tick",
        9,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_queue_wait_count",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_queue_wait_ticks",
        "Tick",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.max_workgroup_queue_wait_ticks",
        "Tick",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.workgroup_queue_wait_count",
        "Count",
        3,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.workgroup_queue_wait_ticks",
        "Tick",
        16,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.max_workgroup_queue_wait_ticks",
        "Tick",
        8,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_reports_per_compute_unit_memory_activity() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "3",
            "--compute-units",
            "2",
            "--wave-slots-per-compute-unit",
            "1",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3200:4:4:4",
            "--global-store",
            "0x3240:4:4:4",
            "--memory-start",
            "0x3200",
            "--memory-size",
            "128",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let simulation = artifact.get("simulation").unwrap();
    let compute_units = simulation
        .get("compute_unit_activity")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        simulation
            .get("global_memory_requests")
            .and_then(Value::as_u64),
        Some(6)
    );
    assert_eq!(
        compute_units[0]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0].get("busy_cycles").and_then(Value::as_u64),
        Some(8)
    );
    assert_eq!(
        compute_units[0]
            .get("coalesced_memory_accesses")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        compute_units[0]
            .get("global_memory_reads")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0]
            .get("global_memory_writes")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[1]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        compute_units[1].get("busy_cycles").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        compute_units[1]
            .get("coalesced_memory_accesses")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[1]
            .get("global_memory_reads")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        compute_units[1]
            .get("global_memory_writes")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.coalesced_memory_accesses",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.global_memory_reads",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.global_memory_writes",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.coalesced_memory_accesses",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.global_memory_reads",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.global_memory_writes",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_merges_overlapping_wave_slots_for_compute_unit_busy_cycles() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "2",
            "--compute-units",
            "1",
            "--wave-slots-per-compute-unit",
            "2",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3400:4:4:4",
            "--memory-start",
            "0x3400",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let simulation = artifact.get("simulation").unwrap();
    let compute_units = simulation
        .get("compute_unit_activity")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        compute_units[0]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0].get("busy_cycles").and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        compute_units[0]
            .get("coalesced_memory_accesses")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0]
            .get("global_memory_reads")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        compute_units[0]
            .get("global_memory_writes")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[0]
            .get("first_started_at")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        compute_units[0]
            .get("last_completed_at")
            .and_then(Value::as_u64),
        Some(5)
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu0.busy_cycles",
        "Cycle",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_gpu_run_omits_activity_window_stats_for_inactive_compute_units() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "1",
            "--compute-units",
            "2",
            "--wave-slots-per-compute-unit",
            "1",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x3800:4:4:4",
            "--memory-start",
            "0x3800",
            "--memory-size",
            "64",
            "--max-tick",
            "80",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let simulation = artifact.get("simulation").unwrap();
    let compute_units = simulation
        .get("compute_unit_activity")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(
        compute_units[1]
            .get("workgroup_completions")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1].get("busy_cycles").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1]
            .get("coalesced_memory_accesses")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1]
            .get("global_memory_reads")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1]
            .get("global_memory_writes")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        compute_units[1].get("first_started_at").unwrap(),
        &Value::Null
    );
    assert_eq!(
        compute_units[1].get("last_completed_at").unwrap(),
        &Value::Null
    );
    assert_eq!(
        compute_units[1]["memory_transport"]["responses"].as_u64(),
        Some(0)
    );
    assert_eq!(
        compute_units[1]["memory_transport"]["round_trip_ticks"].as_u64(),
        Some(0)
    );
    assert_eq!(
        compute_units[1]["memory_transport"]["max_round_trip_ticks"].as_u64(),
        Some(0)
    );
    assert!(compute_units[1]["memory_transport"]["first_response_at"].is_null());
    assert!(compute_units[1]["memory_transport"]["last_response_at"].is_null());
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.workgroup_completions",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.busy_cycles",
        "Cycle",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.memory_transport.responses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.memory_transport.round_trip_ticks",
        "Tick",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.gpu_run.compute_unit.cu1.memory_transport.max_round_trip_ticks",
        "Tick",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("sim.gpu_run.compute_unit.cu1.first_started_at"));
    assert!(!stdout.contains("sim.gpu_run.compute_unit.cu1.last_completed_at"));
    assert!(!stdout.contains("sim.gpu_run.compute_unit.cu1.memory_transport.first_response_at"));
    assert!(!stdout.contains("sim.gpu_run.compute_unit.cu1.memory_transport.last_response_at"));
}

#[test]
fn rem6_gpu_run_representative_hierarchy_rejects_response_after_max_tick() {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "gpu-run",
            "--workgroups",
            "3",
            "--compute-units",
            "2",
            "--wave-slots-per-compute-unit",
            "1",
            "--workgroup-cycles",
            "4",
            "--global-load",
            "0x303f:1:0:4",
            "--global-store",
            "0x3080:4:4:4",
            "--memory-start",
            "0x3000",
            "--memory-size",
            "256",
            "--data-cache-protocol",
            "msi",
            "--dram-memory",
            "--fabric-link",
            "gpu_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "16",
            "--fabric-request-virtual-network",
            "7",
            "--fabric-response-virtual-network",
            "8",
            "--fabric-credit-depth",
            "2",
            "--max-tick",
            "20",
            "--stats-format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("GPU final tick 29"), "stderr: {stderr}");
    assert!(stderr.contains("exceeded max tick 20"), "stderr: {stderr}");
}
