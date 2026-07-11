use std::collections::BTreeSet;

use super::*;

const OLDER_LOAD_PC: &str = "0x8000000c";
const YOUNGER_LOAD_PC: &str = "0x80000010";
const DATA_ADDRESS: &str = "0x80000080";
const YOUNGER_DATA_ADDRESS: &str = "0x800000c0";
const EXPECTED_RESULTS_LOWER: &str = "2a000000000000002a00000063000000";
const EXPECTED_RESULTS_UPPER: &str = "2b00000065000000";
const THIRD_LOAD_PC: &str = "0x80000014";
const THREE_LOAD_DATA_ADDRESS: &str = "0x80000100";
const THREE_LOAD_MIDDLE_ADDRESS: &str = "0x80000140";
const THREE_LOAD_YOUNGER_ADDRESS: &str = "0x80000180";
const THREE_LOAD_RESULTS_LOWER: &str = "2a000000000000002a00000063000000";
const THREE_LOAD_RESULTS_UPPER: &str = "770000002b000000650000007a000000";
const FOURTH_LOAD_PC: &str = "0x80000018";
const FOUR_LOAD_DATA_ADDRESSES: [&str; 4] =
    ["0x80000180", "0x800001c0", "0x80000200", "0x80000240"];
const FOUR_LOAD_RESULTS_LOWER: &str = "2a000000630000007700000088000000";
const FOUR_LOAD_RESULTS_UPPER: &str = "2b000000650000007a0000008c000000";

#[test]
fn rem6_run_o3_detailed_two_scalar_loads_overlap_before_first_response_direct() {
    let path = two_scalar_load_binary("o3-two-scalar-loads-direct");
    let json = two_scalar_load_json(&path, "direct", 900, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_completed_two_scalar_loads(&json);
}

#[test]
fn rem6_run_o3_detailed_two_scalar_loads_overlap_through_cache_fabric_dram() {
    let path = two_scalar_load_binary("o3-two-scalar-loads-cache-fabric-dram");
    let json = two_scalar_load_json(&path, "cache-fabric-dram", 1400, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_completed_two_scalar_loads(&json);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed two-load run should expose {pointer}: {json}"
        );
    }
    for path in [
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(&json, path, "Count", 1, "monotonic");
    }
}

#[test]
fn rem6_run_o3_detailed_two_scalar_loads_remain_resident_at_tick_limit() {
    let path = two_scalar_load_binary("o3-two-scalar-loads-resident");
    let completed = two_scalar_load_json(&path, "direct", 900, None);
    let older = event_at_pc(&completed, OLDER_LOAD_PC);
    let younger = event_at_pc(&completed, YOUNGER_LOAD_PC);
    let younger_issue_tick = event_u64(younger, "issue_tick");
    let older_response_tick = event_u64(older, "lsq_data_response_tick");
    let stop_tick = younger_issue_tick
        .saturating_add(older_response_tick.saturating_sub(younger_issue_tick) / 2);
    assert!(younger_issue_tick < stop_tick && stop_tick < older_response_tick);

    let json = two_scalar_load_json(&path, "direct", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        json.pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(stop_tick)
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident two-load run should expose ROB rows: {json}"));
    let resident_loads = [OLDER_LOAD_PC, YOUNGER_LOAD_PC]
        .into_iter()
        .map(|pc| {
            rob.iter()
                .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
                .unwrap_or_else(|| panic!("resident two-load ROB should include {pc}: {json}"))
        })
        .collect::<Vec<_>>();
    assert!(resident_loads.iter().all(|entry| {
        entry.pointer("/ready").and_then(Value::as_bool) == Some(false)
            && entry.pointer("/live_staged").and_then(Value::as_bool) == Some(true)
    }));
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident two-load run should expose LSQ rows: {json}"));
    assert_eq!(lsq.len(), 2);
    assert_eq!(
        lsq.iter()
            .filter_map(|entry| entry.pointer("/address").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec![DATA_ADDRESS, YOUNGER_DATA_ADDRESS]
    );
    assert!(lsq.iter().all(|entry| {
        entry.pointer("/kind").and_then(Value::as_str) == Some("load")
            && entry.pointer("/bytes").and_then(Value::as_u64) == Some(4)
            && entry.pointer("/completed").and_then(Value::as_bool) == Some(false)
    }));
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_timing_two_scalar_loads_preserve_architecture_without_o3_window() {
    let path = two_scalar_load_binary("o3-two-scalar-loads-timing");
    let json = two_scalar_load_json(&path, "direct", 900, Some("timing"));

    assert_final_architecture(&json);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("run JSON stats array")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "timing mode should suppress two-load O3 aliases: {unexpected:?}"
    );
}

#[test]
fn rem6_run_o3_detailed_three_scalar_loads_overlap_before_first_response_direct() {
    let path = three_scalar_load_binary("o3-three-scalar-loads-direct");
    let json = three_scalar_load_json(&path, "direct", 1100, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_completed_three_scalar_loads(&json);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64).unwrap_or(0),
            0,
            "direct three-load run should not exercise {pointer}: {json}"
        );
    }
}

#[test]
fn rem6_run_o3_detailed_three_scalar_loads_overlap_through_cache_fabric_dram() {
    let path = three_scalar_load_binary("o3-three-scalar-loads-cache-fabric-dram");
    let json = three_scalar_load_json(&path, "cache-fabric-dram", 1700, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_completed_three_scalar_loads(&json);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed three-load run should expose {pointer}: {json}"
        );
    }
}

#[test]
fn rem6_run_o3_detailed_three_scalar_loads_remain_resident_at_tick_limit() {
    let path = three_scalar_load_binary("o3-three-scalar-loads-resident");
    let completed = three_scalar_load_json(&path, "direct", 1100, None);
    let events = [
        event_at_pc(&completed, OLDER_LOAD_PC),
        event_at_pc(&completed, YOUNGER_LOAD_PC),
        event_at_pc(&completed, THIRD_LOAD_PC),
    ];
    let third_issue_tick = event_u64(events[2], "issue_tick");
    let first_response_tick = events
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .expect("three load response ticks");
    let stop_tick =
        third_issue_tick.saturating_add(first_response_tick.saturating_sub(third_issue_tick) / 2);
    assert!(third_issue_tick < stop_tick && stop_tick < first_response_tick);

    let json = three_scalar_load_json(&path, "direct", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        json.pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(stop_tick)
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident three-load run should expose ROB rows: {json}"));
    for pc in [OLDER_LOAD_PC, YOUNGER_LOAD_PC, THIRD_LOAD_PC] {
        let entry = rob
            .iter()
            .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .unwrap_or_else(|| panic!("resident three-load ROB should include {pc}: {json}"));
        assert_eq!(
            entry.pointer("/ready").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            entry.pointer("/live_staged").and_then(Value::as_bool),
            Some(true)
        );
    }
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident three-load run should expose LSQ rows: {json}"));
    assert_eq!(lsq.len(), 3);
    assert_eq!(
        lsq.iter()
            .filter_map(|entry| entry.pointer("/address").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        vec![
            THREE_LOAD_DATA_ADDRESS,
            THREE_LOAD_MIDDLE_ADDRESS,
            THREE_LOAD_YOUNGER_ADDRESS,
        ]
    );
    assert!(lsq.iter().all(|entry| {
        entry.pointer("/kind").and_then(Value::as_str) == Some("load")
            && entry.pointer("/bytes").and_then(Value::as_u64) == Some(4)
            && entry.pointer("/completed").and_then(Value::as_bool) == Some(false)
    }));
}

#[test]
fn rem6_run_o3_timing_three_scalar_loads_preserve_architecture_without_o3_window() {
    let path = three_scalar_load_binary("o3-three-scalar-loads-timing");
    let json = three_scalar_load_json(&path, "direct", 1100, Some("timing"));

    assert_three_load_architecture(&json);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("run JSON stats array")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "timing mode should suppress three-load O3 aliases: {unexpected:?}"
    );
}

#[test]
fn rem6_run_o3_detailed_four_scalar_loads_overlap_before_first_response_direct() {
    let path = four_scalar_load_binary("o3-four-scalar-loads-direct");
    let json = four_scalar_load_json(&path, "direct", 1400, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("direct")
    );
    assert_four_load_architecture(&json);
    assert_four_load_data_trace(&json);
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        4,
        "monotonic",
    );

    let events = [
        event_at_pc(&json, OLDER_LOAD_PC),
        event_at_pc(&json, YOUNGER_LOAD_PC),
        event_at_pc(&json, THIRD_LOAD_PC),
        event_at_pc(&json, FOURTH_LOAD_PC),
    ];
    let first_response_tick = events
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .expect("four load response ticks");
    assert!(
        events
            .iter()
            .all(|event| event_u64(event, "issue_tick") < first_response_tick),
        "all four loads should issue before the first response: {events:?}"
    );
    assert!(events
        .windows(2)
        .all(|pair| event_u64(pair[0], "commit_tick") <= event_u64(pair[1], "commit_tick")));
    assert_eq!(data_memory_request_count(&json), 12);
    assert_eq!(data_requests_sent_before_first_response(&json), 4);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64).unwrap_or(0),
            0,
            "direct four-load run should not exercise {pointer}: {json}"
        );
    }
}

#[test]
fn rem6_run_o3_detailed_four_scalar_loads_overlap_through_cache_fabric_dram() {
    let path = four_scalar_load_binary("o3-four-scalar-loads-cache-fabric-dram");
    let json = four_scalar_load_json(&path, "cache-fabric-dram", 2200, None);

    assert_eq!(
        json.pointer("/simulation/memory_system")
            .and_then(Value::as_str),
        Some("cache-fabric-dram")
    );
    assert_four_load_architecture(&json);
    assert_four_load_data_trace(&json);
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        4,
        "monotonic",
    );
    let events = [
        event_at_pc(&json, OLDER_LOAD_PC),
        event_at_pc(&json, YOUNGER_LOAD_PC),
        event_at_pc(&json, THIRD_LOAD_PC),
        event_at_pc(&json, FOURTH_LOAD_PC),
    ];
    let first_response_tick = events
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .expect("four load response ticks");
    assert!(events
        .iter()
        .all(|event| event_u64(event, "issue_tick") < first_response_tick));
    assert!(events
        .windows(2)
        .all(|pair| event_u64(pair[0], "commit_tick") <= event_u64(pair[1], "commit_tick")));
    assert_eq!(data_memory_request_count(&json), 12);
    assert_eq!(data_requests_sent_before_first_response(&json), 4);
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_u64)
                .is_some_and(|value| value > 0),
            "hierarchy-backed four-load run should expose {pointer}: {json}"
        );
    }
    for path in [
        "sim.memory.resources.cache.data.activity",
        "sim.memory.resources.transport.data.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_json_stat_at_least(&json, path, "Count", 1, "monotonic");
    }
}

#[test]
fn rem6_run_o3_detailed_four_scalar_loads_remain_resident_at_tick_limit() {
    let path = four_scalar_load_binary("o3-four-scalar-loads-resident");
    let completed = four_scalar_load_json(&path, "direct", 1400, None);
    let events = [
        event_at_pc(&completed, OLDER_LOAD_PC),
        event_at_pc(&completed, YOUNGER_LOAD_PC),
        event_at_pc(&completed, THIRD_LOAD_PC),
        event_at_pc(&completed, FOURTH_LOAD_PC),
    ];
    let fourth_issue_tick = event_u64(events[3], "issue_tick");
    let first_response_tick = events
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .expect("four load response ticks");
    let stop_tick =
        fourth_issue_tick.saturating_add(first_response_tick.saturating_sub(fourth_issue_tick) / 2);
    assert!(fourth_issue_tick < stop_tick && stop_tick < first_response_tick);

    let json = four_scalar_load_json(&path, "direct", stop_tick, None);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_at_tick_limit")
    );
    assert_eq!(
        json.pointer("/simulation/final_tick")
            .and_then(Value::as_u64),
        Some(stop_tick)
    );
    let rob = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident four-load run should expose ROB rows: {json}"));
    assert_eq!(rob.len(), 4);
    for pc in [
        OLDER_LOAD_PC,
        YOUNGER_LOAD_PC,
        THIRD_LOAD_PC,
        FOURTH_LOAD_PC,
    ] {
        let entry = rob
            .iter()
            .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(pc))
            .unwrap_or_else(|| panic!("resident four-load ROB should include {pc}: {json}"));
        assert_eq!(
            entry.pointer("/ready").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            entry.pointer("/live_staged").and_then(Value::as_bool),
            Some(true)
        );
    }
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("resident four-load run should expose LSQ rows: {json}"));
    assert_eq!(lsq.len(), 4);
    assert_eq!(
        lsq.iter()
            .filter_map(|entry| entry.pointer("/address").and_then(Value::as_str))
            .collect::<Vec<_>>(),
        FOUR_LOAD_DATA_ADDRESSES
    );
    assert!(lsq.iter().all(|entry| {
        entry.pointer("/kind").and_then(Value::as_str) == Some("load")
            && entry.pointer("/bytes").and_then(Value::as_u64) == Some(4)
            && entry.pointer("/completed").and_then(Value::as_bool) == Some(false)
    }));
    assert_json_stat(
        &json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        4,
        "monotonic",
    );
}

#[test]
fn rem6_run_o3_timing_four_scalar_loads_preserve_architecture_without_o3_window() {
    let path = four_scalar_load_binary("o3-four-scalar-loads-timing");
    let json = four_scalar_load_json(&path, "direct", 1400, Some("timing"));

    assert_four_load_architecture(&json);
    assert_eq!(data_requests_sent_before_first_response(&json), 1);
    assert!(json.pointer("/cores/0/o3_runtime").is_none());
    assert!(json
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    let unexpected = json
        .pointer("/stats")
        .and_then(Value::as_array)
        .expect("run JSON stats array")
        .iter()
        .filter_map(|sample| sample.pointer("/path").and_then(Value::as_str))
        .filter(|path| {
            path.starts_with("sim.cpu0.o3.")
                || [
                    "system.cpu.rob.",
                    "system.cpu.lsq0.",
                    "system.cpu.rename.",
                    "system.cpu.iq.",
                    "system.cpu.iew.",
                    "system.cpu.commit.",
                    "system.cpu.ftq.",
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "timing mode should suppress four-load O3 aliases: {unexpected:?}"
    );
}

#[test]
fn rem6_run_o3_scalar_memory_depth_config_and_cli_precedence_drive_four_load_window() {
    let path = four_scalar_load_binary("o3-four-scalar-loads-config-precedence");
    let config = temp_output("o3-four-scalar-loads-config-precedence.toml");
    std::fs::write(&config, "[run]\nriscv_o3_scalar_memory_depth = 4\n").unwrap();

    let from_config =
        four_scalar_load_json_with_depth_source(&path, "direct", 1400, None, Some(&config), None);
    assert_json_stat(
        &from_config,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_eq!(data_requests_sent_before_first_response(&from_config), 4);

    std::fs::write(&config, "[run]\nriscv_o3_scalar_memory_depth = 1\n").unwrap();
    let overridden = four_scalar_load_json_with_depth_source(
        &path,
        "direct",
        1400,
        None,
        Some(&config),
        Some(4),
    );
    assert_json_stat(
        &overridden,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        4,
        "monotonic",
    );
    assert_eq!(data_requests_sent_before_first_response(&overridden), 4);
}

fn assert_completed_three_scalar_loads(json: &Value) {
    assert_three_load_architecture(json);
    assert_json_stat_at_least(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        3,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        3,
        "monotonic",
    );

    let events = [
        event_at_pc(json, OLDER_LOAD_PC),
        event_at_pc(json, YOUNGER_LOAD_PC),
        event_at_pc(json, THIRD_LOAD_PC),
    ];
    let first_response_tick = events
        .iter()
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .min()
        .expect("three load response ticks");
    assert!(
        events
            .iter()
            .all(|event| event_u64(event, "issue_tick") < first_response_tick),
        "all three loads should issue before the first response: {events:?}"
    );
    assert!(events
        .windows(2)
        .all(|pair| event_u64(pair[0], "commit_tick") <= event_u64(pair[1], "commit_tick")));
    assert_eq!(data_memory_request_count(json), 9);
    assert_eq!(data_requests_sent_before_first_response(json), 3);

    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("three-load run should expose Data trace: {json}"));
    assert_eq!(data_trace.len(), 9);
    let observed = data_trace
        .iter()
        .map(|record| {
            (
                record.pointer("/kind").and_then(Value::as_str).unwrap(),
                record.pointer("/address").and_then(Value::as_str).unwrap(),
                record.pointer("/size").and_then(Value::as_u64).unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed,
        BTreeSet::from([
            ("load", THREE_LOAD_DATA_ADDRESS, 4),
            ("load", THREE_LOAD_MIDDLE_ADDRESS, 4),
            ("load", THREE_LOAD_YOUNGER_ADDRESS, 4),
            ("store", "0x80000108", 4),
            ("store", "0x8000010c", 4),
            ("store", "0x80000110", 4),
            ("store", "0x80000114", 4),
            ("store", "0x80000118", 4),
            ("store", "0x8000011c", 4),
        ])
    );
}

fn assert_three_load_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(THREE_LOAD_RESULTS_LOWER)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some(THREE_LOAD_RESULTS_UPPER)
    );
    assert_eq!(
        json.pointer("/memory/2/hex").and_then(Value::as_str),
        Some("63000000")
    );
    assert_eq!(
        json.pointer("/memory/3/hex").and_then(Value::as_str),
        Some("77000000")
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x63"),
        ("x14", "0x77"),
        ("x15", "0x2b"),
        ("x16", "0x65"),
        ("x17", "0x7a"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should preserve three-load semantics: {json}"
        );
    }
}

fn three_scalar_load_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    switch_mode: Option<&str>,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--riscv-branch-lookahead",
        "2",
        "--debug-flags",
        "O3,Data,Memory",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--dump-memory",
        "0x80000100:16",
        "--dump-memory",
        "0x80000110:16",
        "--dump-memory",
        "0x80000140:4",
        "--dump-memory",
        "0x80000180:4",
    ]);
    if let Some(switch_mode) = switch_mode {
        command.args(["--m5-switch-cpu-mode", switch_mode]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn four_scalar_load_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    switch_mode: Option<&str>,
) -> Value {
    four_scalar_load_json_with_depth_source(
        path,
        memory_system,
        max_tick,
        switch_mode,
        None,
        Some(4),
    )
}

fn four_scalar_load_json_with_depth_source(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    switch_mode: Option<&str>,
    config: Option<&Path>,
    cli_depth: Option<usize>,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--riscv-branch-lookahead",
        "2",
        "--debug-flags",
        "O3,Data,Memory",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--dump-memory",
        "0x80000190:16",
        "--dump-memory",
        "0x800001a0:16",
    ]);
    if let Some(config) = config {
        command.args(["--config", config.to_str().unwrap()]);
    }
    if let Some(cli_depth) = cli_depth {
        let cli_depth = cli_depth.to_string();
        command.args(["--riscv-o3-scalar-memory-depth", cli_depth.as_str()]);
    }
    if let Some(switch_mode) = switch_mode {
        command.args(["--m5-switch-cpu-mode", switch_mode]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn assert_four_load_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(FOUR_LOAD_RESULTS_LOWER)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some(FOUR_LOAD_RESULTS_UPPER)
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x63"),
        ("x14", "0x77"),
        ("x15", "0x88"),
        ("x16", "0x2b"),
        ("x17", "0x65"),
        ("x18", "0x7a"),
        ("x19", "0x8c"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should preserve four-load semantics: {json}"
        );
    }
}

fn assert_four_load_data_trace(json: &Value) {
    let data_trace = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("four-load run should expose Data trace: {json}"));
    assert_eq!(data_trace.len(), 12);
    let observed = data_trace
        .iter()
        .map(|record| {
            (
                record.pointer("/kind").and_then(Value::as_str).unwrap(),
                record.pointer("/address").and_then(Value::as_str).unwrap(),
                record.pointer("/size").and_then(Value::as_u64).unwrap(),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        observed,
        BTreeSet::from([
            ("load", FOUR_LOAD_DATA_ADDRESSES[0], 4),
            ("load", FOUR_LOAD_DATA_ADDRESSES[1], 4),
            ("load", FOUR_LOAD_DATA_ADDRESSES[2], 4),
            ("load", FOUR_LOAD_DATA_ADDRESSES[3], 4),
            ("store", "0x80000190", 4),
            ("store", "0x80000194", 4),
            ("store", "0x80000198", 4),
            ("store", "0x8000019c", 4),
            ("store", "0x800001a0", 4),
            ("store", "0x800001a4", 4),
            ("store", "0x800001a8", 4),
            ("store", "0x800001ac", 4),
        ])
    );
}

fn data_memory_request_count(json: &Value) -> usize {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("run JSON Memory trace")
        .iter()
        .filter(|record| record.pointer("/channel").and_then(Value::as_str) == Some("data"))
        .map(|record| {
            (
                record
                    .pointer("/request_agent")
                    .and_then(Value::as_u64)
                    .expect("memory trace request agent"),
                record
                    .pointer("/request")
                    .and_then(Value::as_u64)
                    .expect("memory trace request sequence"),
            )
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn data_requests_sent_before_first_response(json: &Value) -> usize {
    let trace = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("run JSON Memory trace");
    let first_response_tick = trace
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("response_arrived")
        })
        .filter_map(|record| record.pointer("/tick").and_then(Value::as_u64))
        .min()
        .expect("data response trace record");
    trace
        .iter()
        .filter(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record
                    .pointer("/tick")
                    .and_then(Value::as_u64)
                    .is_some_and(|tick| tick < first_response_tick)
        })
        .map(|record| {
            (
                record
                    .pointer("/request_agent")
                    .and_then(Value::as_u64)
                    .expect("memory trace request agent"),
                record
                    .pointer("/request")
                    .and_then(Value::as_u64)
                    .expect("memory trace request sequence"),
            )
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn assert_completed_two_scalar_loads(json: &Value) {
    assert_final_architecture(json);
    assert_json_stat_at_least(
        json,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        json,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        2,
        "monotonic",
    );
    assert_json_stat(
        json,
        "system.cpu.lsq0.maxOccupancy",
        "Count",
        2,
        "monotonic",
    );

    let older = event_at_pc(json, OLDER_LOAD_PC);
    let younger = event_at_pc(json, YOUNGER_LOAD_PC);
    assert_eq!(
        older.pointer("/lsq_operation").and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        younger.pointer("/lsq_operation").and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        older.pointer("/lsq_load_address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        younger.pointer("/lsq_load_address").and_then(Value::as_str),
        Some(YOUNGER_DATA_ADDRESS)
    );
    assert!(
        event_u64(younger, "issue_tick") < event_u64(older, "lsq_data_response_tick"),
        "younger load should issue before the older response: older={older}, younger={younger}"
    );
    assert!(
        event_u64(older, "commit_tick") <= event_u64(younger, "commit_tick"),
        "two scalar loads must retire in program order: older={older}, younger={younger}"
    );
}

fn assert_final_architecture(json: &Value) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(EXPECTED_RESULTS_LOWER)
    );
    assert_eq!(
        json.pointer("/memory/1/hex").and_then(Value::as_str),
        Some(EXPECTED_RESULTS_UPPER)
    );
    for (register, value) in [
        ("x12", "0x2a"),
        ("x13", "0x63"),
        ("x14", "0x2b"),
        ("x15", "0x65"),
    ] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "final register {register} should preserve two-load semantics: {json}"
        );
    }
}

fn two_scalar_load_json(
    path: &Path,
    memory_system: &str,
    max_tick: u64,
    switch_mode: Option<&str>,
) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        &max_tick.to_string(),
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3",
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        "16",
        "--dump-memory",
        "0x80000080:16",
        "--dump-memory",
        "0x80000090:8",
    ]);
    if let Some(switch_mode) = switch_mode {
        command.args(["--m5-switch-cpu-mode", switch_mode]);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
        .unwrap_or_else(|| panic!("O3 trace should include event at {pc}: {json}"))
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .get(field)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("O3 event should expose {field}: {event}"))
}

fn two_scalar_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 128_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(64, 10, 0b010, 13, 0x03),
        i_type(1, 12, 0x0, 14, 0x13),
        i_type(2, 13, 0x0, 15, 0x13),
        s_type(8, 12, 10, 0b010),
        s_type(12, 13, 10, 0b010),
        s_type(16, 14, 10, 0b010),
        s_type(20, 15, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x2a, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x63]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn three_scalar_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(64, 10, 0b010, 13, 0x03),
        i_type(128, 10, 0b010, 14, 0x03),
        i_type(1, 12, 0x0, 15, 0x13),
        i_type(2, 13, 0x0, 16, 0x13),
        i_type(3, 14, 0x0, 17, 0x13),
        s_type(8, 12, 10, 0b010),
        s_type(12, 13, 10, 0b010),
        s_type(16, 14, 10, 0b010),
        s_type(20, 15, 10, 0b010),
        s_type(24, 16, 10, 0b010),
        s_type(28, 17, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.push(0x2a);
    while words.len() * 4 < data_start as usize + 64 {
        words.push(0);
    }
    words.push(0x63);
    while words.len() * 4 < data_start as usize + 128 {
        words.push(0);
    }
    words.push(0x77);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn four_scalar_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 384_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(64, 10, 0b010, 13, 0x03),
        i_type(128, 10, 0b010, 14, 0x03),
        i_type(192, 10, 0b010, 15, 0x03),
        i_type(1, 12, 0x0, 16, 0x13),
        i_type(2, 13, 0x0, 17, 0x13),
        i_type(3, 14, 0x0, 18, 0x13),
        i_type(4, 15, 0x0, 19, 0x13),
        s_type(16, 12, 10, 0b010),
        s_type(20, 13, 10, 0b010),
        s_type(24, 14, 10, 0b010),
        s_type(28, 15, 10, 0b010),
        s_type(32, 16, 10, 0b010),
        s_type(36, 17, 10, 0b010),
        s_type(40, 18, 10, 0b010),
        s_type(44, 19, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    for (offset, value) in [(0, 0x2a), (64, 0x63), (128, 0x77), (192, 0x88)] {
        while words.len() * 4 < data_start as usize + offset {
            words.push(0);
        }
        words.push(value);
    }
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}
