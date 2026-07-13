use std::process::Command;

use crate::support::*;

#[test]
fn rem6_run_routes_multicore_two_hop_fabric_with_qos_queue_policy_matrix() {
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
            "1000",
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
            "--fabric-link",
            "cpu_r0,r0_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "4,4",
            "--fabric-credit-depth",
            "2",
            "--fabric-request-virtual-network",
            "7",
            "--fabric-response-virtual-network",
            "8",
            "--fabric-router",
            "router0,router1",
            "--fabric-router-input-port",
            "1,2",
            "--fabric-router-output-port",
            "2,3",
            "--fabric-router-virtual-channel",
            "3,4",
            "--fabric-request-router-virtual-channel",
            "11,12",
            "--fabric-response-router-virtual-channel",
            "13,14",
            "--fabric-router-latency",
            "1,2",
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

        let configured_hops = json
            .pointer("/fabric/hops")
            .and_then(serde_json::Value::as_array)
            .expect("configured fabric hops");
        assert_eq!(configured_hops.len(), 2);
        for (
            hop,
            (
                hop_index,
                link,
                bandwidth,
                router,
                input,
                output,
                vc,
                request_vc,
                response_vc,
                latency,
            ),
        ) in configured_hops.iter().zip([
            (0, "cpu_r0", 4, "router0", 1, 2, 3, 11, 13, 1),
            (1, "r0_mem", 4, "router1", 2, 3, 4, 12, 14, 2),
        ]) {
            assert_eq!(hop["hop_index"].as_u64(), Some(hop_index));
            assert_eq!(hop["link"].as_str(), Some(link));
            assert_eq!(hop["bandwidth_bytes_per_tick"].as_u64(), Some(bandwidth));
            assert_eq!(hop["router_stage"]["router"].as_str(), Some(router));
            assert_eq!(hop["router_stage"]["input_port"].as_u64(), Some(input));
            assert_eq!(hop["router_stage"]["output_port"].as_u64(), Some(output));
            assert_eq!(hop["router_stage"]["virtual_channel"].as_u64(), Some(vc));
            assert_eq!(
                hop["router_stage"]["request_virtual_channel"].as_u64(),
                Some(request_vc)
            );
            assert_eq!(
                hop["router_stage"]["response_virtual_channel"].as_u64(),
                Some(response_vc)
            );
            assert_eq!(hop["router_stage"]["latency_ticks"].as_u64(), Some(latency));
        }
        assert!(json.pointer("/fabric/link").is_none());
        assert!(json.pointer("/fabric/bandwidth_bytes_per_tick").is_none());
        assert!(json.pointer("/fabric/router_stage").is_none());

        let qos_activities = json
            .pointer("/fabric/qos_grant_activities")
            .and_then(serde_json::Value::as_array)
            .expect("run fabric QoS grant activities");
        let request_qos_activities = qos_activities
            .iter()
            .filter(|activity| activity["direction"].as_str() == Some("request"))
            .collect::<Vec<_>>();
        let response_qos_activities = qos_activities
            .iter()
            .filter(|activity| activity["direction"].as_str() == Some("response"))
            .collect::<Vec<_>>();
        assert_eq!(
            request_qos_activities.len() + response_qos_activities.len(),
            qos_activities.len()
        );
        let request_transfers = fabric_vn_transfer_count(&json, 7);
        let response_transfers = fabric_vn_transfer_count(&json, 8);
        assert!(request_transfers > 0);
        assert!(response_transfers > 0);

        if let Some(policy) = policy {
            assert_eq!(request_qos_activities.len() as u64 * 2, request_transfers);
            assert_eq!(response_qos_activities.len() as u64 * 2, response_transfers);
            assert_qos_grant_hops(
                &json,
                &request_qos_activities,
                7,
                &[(0, "cpu_r0", "router0", 11), (1, "r0_mem", "router1", 12)],
            );
            assert_qos_grant_hops(
                &json,
                &response_qos_activities,
                8,
                &[(0, "cpu_r0", "router0", 13), (1, "r0_mem", "router1", 14)],
            );

            let first_batch = request_qos_activities
                .iter()
                .copied()
                .filter(|activity| activity["batch"].as_u64() == Some(0))
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

            assert!(!response_qos_activities.is_empty());
            assert!(response_qos_activities.iter().all(|activity| {
                activity["candidates"].as_array().unwrap().len() == 1
                    && activity["selected_queue_index"].as_u64() == Some(0)
            }));
            for activity in &response_qos_activities {
                let candidates = activity["candidates"].as_array().unwrap();
                let selected = activity["selected_queue_index"].as_u64().unwrap() as usize;
                assert_eq!(activity["policy"].as_str(), Some(policy));
                assert!(selected < candidates.len());
                match policy {
                    "fifo" => assert_eq!(selected, 0),
                    "lifo" => assert_eq!(selected, 0),
                    "least-recently-granted" => assert!(!activity["lrg_requestors_after"]
                        .as_array()
                        .unwrap()
                        .is_empty()),
                    _ => unreachable!(),
                }
            }

            assert_qos_direction_stats(&stdout, "request", &request_qos_activities);
            assert_qos_direction_stats(&stdout, "response", &response_qos_activities);
        } else {
            assert!(qos_activities.is_empty(), "{stdout}");
            assert_qos_direction_stats(&stdout, "request", &[]);
            assert_qos_direction_stats(&stdout, "response", &[]);
        }
        assert!(!stdout.contains("\"path\":\"sim.memory.fabric.qos.grants\""));

        for virtual_network in [7, 8] {
            assert!(json["fabric"]["lane_activities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|activity| activity["virtual_network"].as_u64() == Some(virtual_network)));
        }
        for virtual_channel in [11, 12, 13, 14] {
            assert!(json["fabric"]["router_activities"]
                .as_array()
                .unwrap()
                .iter()
                .any(|activity| activity["virtual_channel"].as_u64() == Some(virtual_channel)));
        }
        for path in [
            "sim.memory.fabric.link.cpu_r0.vn7.hop0.transfers",
            "sim.memory.fabric.link.r0_mem.vn7.hop1.transfers",
            "sim.memory.fabric.link.cpu_r0.vn8.hop0.transfers",
            "sim.memory.fabric.link.r0_mem.vn8.hop1.transfers",
            "sim.memory.fabric.router.router0.in1.out2.vc11.transfers",
            "sim.memory.fabric.router.router1.in2.out3.vc12.transfers",
            "sim.memory.fabric.router.router0.in1.out2.vc13.transfers",
            "sim.memory.fabric.router.router1.in2.out3.vc14.transfers",
        ] {
            assert_stat_greater_than(&stdout, path, "Count", 0, "monotonic");
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

fn fabric_vn_transfer_count(json: &serde_json::Value, virtual_network: u64) -> u64 {
    json["fabric"]["lane_activities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|activity| activity["virtual_network"].as_u64() == Some(virtual_network))
        .map(|activity| activity["transfer_count"].as_u64().unwrap())
        .sum()
}

fn assert_qos_grant_hops(
    json: &serde_json::Value,
    activities: &[&serde_json::Value],
    virtual_network: u64,
    expected_hops: &[(u64, &str, &str, u64)],
) {
    for activity in activities {
        let packet = activity["grant"]["packet"].as_u64().unwrap();
        let packet_hops = json["fabric"]["hop_activities"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|hop| hop["packet"].as_u64() == Some(packet))
            .collect::<Vec<_>>();
        assert_eq!(packet_hops.len(), expected_hops.len());
        for (hop, (hop_index, link, router, virtual_channel)) in
            packet_hops.iter().zip(expected_hops)
        {
            assert_eq!(hop["hop_index"].as_u64(), Some(*hop_index));
            assert_eq!(hop["link"].as_str(), Some(*link));
            assert_eq!(hop["virtual_network"].as_u64(), Some(virtual_network));
            assert_eq!(hop["router"]["router"].as_str(), Some(*router));
            assert_eq!(
                hop["router"]["virtual_channel"].as_u64(),
                Some(*virtual_channel)
            );
        }
    }
}

fn assert_qos_direction_stats(stdout: &str, direction: &str, activities: &[&serde_json::Value]) {
    let grants = activities.len() as u64;
    let candidates = activities
        .iter()
        .map(|activity| activity["candidates"].as_array().unwrap().len() as u64)
        .sum();
    let suppressed = activities
        .iter()
        .map(|activity| activity["suppressed"].as_array().unwrap().len() as u64)
        .sum();
    let mut batches = activities
        .iter()
        .map(|activity| activity["batch"].as_u64().unwrap())
        .collect::<Vec<_>>();
    batches.sort_unstable();
    batches.dedup();
    let max_candidates = activities
        .iter()
        .map(|activity| activity["candidates"].as_array().unwrap().len() as u64)
        .max()
        .unwrap_or(0);
    let prefix = format!("sim.memory.fabric.qos.{direction}");
    for (suffix, value) in [
        ("grants", grants),
        ("candidates", candidates),
        ("suppressed", suppressed),
        ("batches", batches.len() as u64),
        ("max_candidates", max_candidates),
    ] {
        assert_stat(
            stdout,
            &format!("{prefix}.{suffix}"),
            "Count",
            value,
            "monotonic",
        );
    }

    let mut requestors = activities
        .iter()
        .map(|activity| activity["grant"]["requestor"].as_u64().unwrap())
        .collect::<Vec<_>>();
    requestors.sort_unstable();
    requestors.dedup();
    for requestor in requestors {
        let grants = activities
            .iter()
            .filter(|activity| activity["grant"]["requestor"].as_u64() == Some(requestor))
            .count() as u64;
        assert_stat(
            stdout,
            &format!("{prefix}.requestor{requestor}.grants"),
            "Count",
            grants,
            "monotonic",
        );
    }

    let mut priorities = activities
        .iter()
        .map(|activity| activity["grant"]["priority"].as_u64().unwrap())
        .collect::<Vec<_>>();
    priorities.sort_unstable();
    priorities.dedup();
    for priority in priorities {
        let grants = activities
            .iter()
            .filter(|activity| activity["grant"]["priority"].as_u64() == Some(priority))
            .count() as u64;
        assert_stat(
            stdout,
            &format!("{prefix}.priority{priority}.grants"),
            "Count",
            grants,
            "monotonic",
        );
    }
}

fn run_core_register<'a>(json: &'a serde_json::Value, cpu: u64, register: &str) -> Option<&'a str> {
    json["cores"]
        .as_array()?
        .iter()
        .find(|core| core["cpu"].as_u64() == Some(cpu))?["registers"][register]
        .as_str()
}
