use std::process::Command;

use serde_json::Value;

use crate::support::{
    assert_stat_greater_than, i_type, riscv64_elf, riscv64_program, s_type, temp_binary, u_type,
};

const DATA_OFFSET: usize = 64;

#[test]
fn rem6_run_routes_cache_dram_through_two_fabric_hops_and_routers() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13),
        i_type(0, 2, 0x3, 5, 0x03),
        i_type(1, 5, 0x0, 6, 0x13),
        s_type(8, 6, 2, 0x3),
        0x0000_0073,
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("run-fabric-two-hop-two-router", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "280",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--memory-system",
            "cache-fabric-dram",
            "--fabric-link",
            "cpu_r0,r0_mem",
            "--fabric-bandwidth-bytes-per-tick",
            "8,4",
            "--fabric-request-virtual-network",
            "5",
            "--fabric-response-virtual-network",
            "6",
            "--fabric-credit-depth",
            "2",
            "--fabric-router",
            "router0,router1",
            "--fabric-router-input-port",
            "1,2",
            "--fabric-router-output-port",
            "2,3",
            "--fabric-router-virtual-channel",
            "7,8",
            "--fabric-router-latency",
            "3,5",
            "--dump-memory",
            "0x80000048:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let fabric = json.pointer("/fabric").expect("run fabric summary");

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("8977665544332211")
    );
    assert_eq!(
        fabric
            .pointer("/active_virtual_networks")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert!(
        fabric
            .pointer("/active_routers")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            >= 2
    );
    assert!(has_fabric_hop(fabric, "cpu_r0", 0, 5, "router0", 1, 2, 7));
    assert!(has_fabric_hop(fabric, "r0_mem", 1, 5, "router1", 2, 3, 8));
    assert!(has_fabric_hop(fabric, "cpu_r0", 0, 6, "router0", 1, 2, 7));
    assert!(has_fabric_hop(fabric, "r0_mem", 1, 6, "router1", 2, 3, 8));
    assert_eq!(
        json.pointer("/memory_resources/fabric/hop_activities"),
        fabric.pointer("/hop_activities")
    );
    assert_eq!(
        json.pointer("/memory_resources/fabric/router_activities"),
        fabric.pointer("/router_activities")
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.link.cpu_r0.vn5.hop0.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.link.r0_mem.vn5.hop1.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.link.cpu_r0.vn6.hop0.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.link.r0_mem.vn6.hop1.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.router.router0.in1.out2.vc7.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.fabric.router.router1.in2.out3.vc8.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.resources.fabric.router.router0.in1.out2.vc7.transfers",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.memory.resources.fabric.router.router1.in2.out3.vc8.transfers",
        "Count",
        0,
        "monotonic",
    );
}

fn has_fabric_hop(
    fabric: &Value,
    link: &str,
    hop_index: u64,
    virtual_network: u64,
    router: &str,
    input_port: u64,
    output_port: u64,
    virtual_channel: u64,
) -> bool {
    fabric
        .pointer("/hop_activities")
        .and_then(Value::as_array)
        .is_some_and(|hops| {
            hops.iter().any(|hop| {
                hop.get("link").and_then(Value::as_str) == Some(link)
                    && hop.get("hop_index").and_then(Value::as_u64) == Some(hop_index)
                    && hop.get("virtual_network").and_then(Value::as_u64) == Some(virtual_network)
                    && hop.pointer("/router/router").and_then(Value::as_str) == Some(router)
                    && hop.pointer("/router/input_port").and_then(Value::as_u64) == Some(input_port)
                    && hop.pointer("/router/output_port").and_then(Value::as_u64)
                        == Some(output_port)
                    && hop
                        .pointer("/router/virtual_channel")
                        .and_then(Value::as_u64)
                        == Some(virtual_channel)
            })
        })
}
