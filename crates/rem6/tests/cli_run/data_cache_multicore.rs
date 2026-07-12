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
fn rem6_run_routes_multicore_fabric_with_qos_queue_policy_matrix() {
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
    let path = temp_binary("multicore-fabric-qos-queue-policy-matrix", &elf);

    for (policy, expected_grants, expected_selected, core1_x7, core2_x7) in [
        (Some("fifo"), &[0, 1, 2][..], &[0, 0, 0][..], "0x7", "0x3"),
        (Some("lifo"), &[2, 1, 0][..], &[2, 1, 0][..], "0x3", "0x7"),
        (
            Some("least-recently-granted"),
            &[0, 1, 2][..],
            &[0, 0, 0][..],
            "0x7",
            "0x3",
        ),
        (None, &[][..], &[][..], "0x7", "0x3"),
    ] {
        let mut args = vec![
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "900",
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
            "--fabric-request-virtual-network",
            "7",
            "--fabric-response-virtual-network",
            "8",
            "--fabric-router",
            "router0",
            "--fabric-router-input-port",
            "1",
            "--fabric-router-output-port",
            "2",
            "--fabric-router-virtual-channel",
            "3",
            "--fabric-request-router-virtual-channel",
            "11",
            "--fabric-response-router-virtual-channel",
            "13",
            "--fabric-router-latency",
            "1",
        ];
        if let Some(policy) = policy {
            args.extend(["--fabric-qos-queue-policy", policy]);
        }

        let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
            .args(args)
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "policy {policy:?} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
        assert!(stdout.contains("\"status\":\"executed_until_trap\""));
        assert!(stdout.contains("\"cores\":3"));
        assert_eq!(
            json.pointer("/fabric/qos_queue_policy")
                .and_then(serde_json::Value::as_str),
            policy
        );
        assert_eq!(run_core_register(&json, 1, "x7"), Some(core1_x7));
        assert_eq!(run_core_register(&json, 2, "x7"), Some(core2_x7));

        let qos_activities = json
            .pointer("/fabric/qos_grant_activities")
            .and_then(serde_json::Value::as_array)
            .expect("run fabric QoS grant activities");
        let request_transfers = json["fabric"]["lane_activities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|activity| activity["virtual_network"].as_u64() == Some(7))
            .and_then(|activity| activity["transfer_count"].as_u64())
            .expect("request virtual-network transfer count");
        assert!(request_transfers > 0);
        if let Some(policy) = policy {
            assert_eq!(qos_activities.len() as u64, request_transfers);
            for activity in qos_activities {
                let packet = activity["grant"]["packet"].as_u64().unwrap();
                let hop = json["fabric"]["hop_activities"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|hop| hop["packet"].as_u64() == Some(packet))
                    .expect("QoS grant packet should have fabric hop activity");
                assert_eq!(hop["virtual_network"].as_u64(), Some(7));
                assert_eq!(hop["router"]["virtual_channel"].as_u64(), Some(11));
            }
            let first_batch = qos_activities
                .iter()
                .filter(|activity| {
                    activity.get("batch").and_then(serde_json::Value::as_u64) == Some(0)
                })
                .collect::<Vec<_>>();
            assert_eq!(first_batch.len(), 3, "policy {policy}: {stdout}");
            assert_eq!(
                first_batch
                    .iter()
                    .map(|activity| activity["grant"]["requestor"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                expected_grants
            );
            assert_eq!(
                first_batch
                    .iter()
                    .map(|activity| activity["selected_queue_index"].as_u64().unwrap())
                    .collect::<Vec<_>>(),
                expected_selected
            );
            assert_eq!(
                first_batch
                    .iter()
                    .map(|activity| activity["candidates"].as_array().unwrap().len())
                    .collect::<Vec<_>>(),
                [3, 2, 1]
            );
            for (grant_index, activity) in first_batch.iter().enumerate() {
                assert_eq!(activity["tick"].as_u64(), Some(0));
                assert_eq!(activity["grant_index"].as_u64(), Some(grant_index as u64));
                assert_eq!(activity["policy"].as_str(), Some(policy));
                assert!(activity["suppressed"].as_array().unwrap().is_empty());
                for candidate in activity["candidates"].as_array().unwrap() {
                    assert_eq!(candidate["packet"], candidate["request_id"]);
                }
            }
            if policy == "least-recently-granted" {
                assert_eq!(
                    first_batch[0]["lrg_requestors_before"],
                    serde_json::json!([])
                );
                assert_eq!(
                    first_batch[0]["lrg_requestors_after"],
                    serde_json::json!([1, 2, 0])
                );
                assert_eq!(
                    first_batch[1]["lrg_requestors_before"],
                    serde_json::json!([1, 2, 0])
                );
                assert_eq!(
                    first_batch[1]["lrg_requestors_after"],
                    serde_json::json!([2, 0, 1])
                );
                assert_eq!(
                    first_batch[2]["lrg_requestors_before"],
                    serde_json::json!([2, 0, 1])
                );
                assert_eq!(
                    first_batch[2]["lrg_requestors_after"],
                    serde_json::json!([0, 1, 2])
                );
            } else {
                assert!(first_batch.iter().all(|activity| {
                    activity["lrg_requestors_before"]
                        .as_array()
                        .unwrap()
                        .is_empty()
                        && activity["lrg_requestors_after"]
                            .as_array()
                            .unwrap()
                            .is_empty()
                }));
            }
            let grants = qos_activities.len() as u64;
            let candidates = qos_activities
                .iter()
                .map(|activity| activity["candidates"].as_array().unwrap().len() as u64)
                .sum();
            let suppressed = qos_activities
                .iter()
                .map(|activity| activity["suppressed"].as_array().unwrap().len() as u64)
                .sum();
            let mut batches = qos_activities
                .iter()
                .map(|activity| activity["batch"].as_u64().unwrap())
                .collect::<Vec<_>>();
            batches.sort_unstable();
            batches.dedup();
            let max_candidates = qos_activities
                .iter()
                .map(|activity| activity["candidates"].as_array().unwrap().len() as u64)
                .max()
                .unwrap();
            for (path, value) in [
                ("sim.memory.fabric.qos.grants", grants),
                ("sim.memory.fabric.qos.candidates", candidates),
                ("sim.memory.fabric.qos.suppressed", suppressed),
                ("sim.memory.fabric.qos.batches", batches.len() as u64),
                ("sim.memory.fabric.qos.max_candidates", max_candidates),
                ("sim.memory.fabric.qos.priority0.grants", grants),
            ] {
                assert_stat(&stdout, path, "Count", value, "monotonic");
            }
            for requestor in 0..3 {
                let grants = qos_activities
                    .iter()
                    .filter(|activity| activity["grant"]["requestor"].as_u64() == Some(requestor))
                    .count() as u64;
                assert_stat(
                    &stdout,
                    &format!("sim.memory.fabric.qos.requestor{requestor}.grants"),
                    "Count",
                    grants,
                    "monotonic",
                );
            }
        } else {
            assert!(qos_activities.is_empty(), "{stdout}");
            for path in [
                "sim.memory.fabric.qos.grants",
                "sim.memory.fabric.qos.candidates",
                "sim.memory.fabric.qos.suppressed",
                "sim.memory.fabric.qos.batches",
                "sim.memory.fabric.qos.max_candidates",
            ] {
                assert_stat(&stdout, path, "Count", 0, "monotonic");
            }
        }

        for virtual_network in [7, 8] {
            assert!(json["fabric"]["lane_activities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|activity| activity["virtual_network"].as_u64() == Some(virtual_network)));
        }
        for virtual_channel in [11, 13] {
            assert!(json["fabric"]["router_activities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|activity| activity["virtual_channel"].as_u64() == Some(virtual_channel)));
        }
        let fabric_wait_edges = json
            .pointer("/fabric/wait_for_edge_count")
            .and_then(serde_json::Value::as_u64)
            .expect("run fabric wait-for edge count");
        assert!(fabric_wait_edges > 0, "{stdout}");
        let wait_kind_windows = json
            .pointer("/fabric/wait_for_edge_kind_windows")
            .and_then(serde_json::Value::as_array)
            .expect("run fabric wait-for kind windows");
        assert!(wait_kind_windows.iter().any(|window| {
            window.get("kind").and_then(serde_json::Value::as_str) == Some("queue")
                && window
                    .get("edge_count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    > 0
                && window
                    .get("last_tick")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0)
                    >= window
                        .get("first_tick")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0)
        }));
        assert!(json
            .pointer("/fabric/wait_for_target_node_windows")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|windows| windows.iter().any(|window| window
                .get("node")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|node| node.starts_with("resource:fabric.")))));
        for (path, unit) in [
            ("sim.memory.fabric.transfers", "Count"),
            ("sim.memory.fabric.queue_delay_ticks", "Tick"),
            ("sim.memory.fabric.wait_for.edges", "Count"),
            ("sim.memory.fabric.wait_for.kind.queue.edges", "Count"),
            ("sim.memory.fabric.wait_for.kind.queue.last_tick", "Tick"),
        ] {
            assert_stat_greater_than(&stdout, path, unit, 0, "monotonic");
        }
    }
}

fn run_core_register<'a>(json: &'a serde_json::Value, cpu: u64, register: &str) -> Option<&'a str> {
    json["cores"]
        .as_array()?
        .iter()
        .find(|core| core["cpu"].as_u64() == Some(cpu))?["registers"][register]
        .as_str()
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
