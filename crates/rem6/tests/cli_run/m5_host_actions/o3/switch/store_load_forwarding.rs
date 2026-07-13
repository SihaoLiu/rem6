use std::collections::BTreeSet;

use rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK;

use super::*;

const OLDER_STORE_PC: &str = "0x80000010";
const YOUNGER_LOAD_PC: &str = "0x80000014";
const DEPENDENT_ALU_PC: &str = "0x80000018";
const INDEPENDENT_YOUNGER_ALU_PC: &str = "0x80000018";
const MULTI_SOURCE_OLDER_STORE_PC: &str = "0x80000018";
const MULTI_SOURCE_MIDDLE_STORE_PC: &str = "0x8000001c";
const MULTI_SOURCE_YOUNGEST_STORE_PC: &str = "0x80000020";
const MULTI_SOURCE_YOUNGER_LOAD_PC: &str = "0x80000024";
const MULTI_SOURCE_YOUNGER_ROW_OLDER_STORE_PC: &str = "0x80000014";
const MULTI_SOURCE_YOUNGER_ROW_YOUNGEST_STORE_PC: &str = "0x80000018";
const MULTI_SOURCE_YOUNGER_ROW_LOAD_PC: &str = "0x8000001c";
const MULTI_SOURCE_YOUNGER_ROW_ALU_PC: &str = "0x80000020";
const DATA_ADDRESS: &str = "0x80000100";
const RESULTS: &str = "2a0000002a0000002b000000";

#[test]
fn rem6_run_host_switch_transfers_full_forwarded_store_load_direct() {
    assert_full_forwarded_store_load_handoff("direct");
}

#[test]
fn rem6_run_host_switch_transfers_full_forwarded_store_load_cache_fabric_dram() {
    assert_full_forwarded_store_load_handoff("cache-fabric-dram");
}

#[test]
fn rem6_run_host_switch_transfers_partial_forwarded_store_load_direct() {
    assert_partial_forwarded_store_load_handoff("direct");
}

#[test]
fn rem6_run_host_switch_transfers_partial_forwarded_store_load_cache_fabric_dram() {
    assert_partial_forwarded_store_load_handoff("cache-fabric-dram");
}

#[test]
fn rem6_run_host_switch_transfers_multi_source_partial_forwarded_store_load_direct() {
    assert_multi_source_partial_forwarded_store_load_handoff("direct");
}

#[test]
fn rem6_run_host_switch_transfers_multi_source_partial_forwarded_store_load_cache_fabric_dram() {
    assert_multi_source_partial_forwarded_store_load_handoff("cache-fabric-dram");
}

#[test]
fn rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_direct() {
    assert_completed_multi_source_partial_forwarded_store_load_handoff("direct");
}

#[test]
fn rem6_run_host_switch_transfers_completed_multi_source_partial_forwarded_store_load_cache_fabric_dram(
) {
    assert_completed_multi_source_partial_forwarded_store_load_handoff("cache-fabric-dram");
}

struct CompletedMultiSourceRun {
    baseline: Value,
    switched: Value,
    load_response: u64,
    switch_tick: u64,
    next_source_response: u64,
}

fn assert_completed_multi_source_partial_forwarded_store_load_handoff(memory_system: &str) {
    let run = run_completed_multi_source_partial_handoff(memory_system);
    let baseline = &run.baseline;
    let switched = &run.switched;
    let load = event_at_pc(baseline, MULTI_SOURCE_YOUNGER_LOAD_PC);
    let load_issue = event_u64(load, "issue_tick");
    assert!(run.load_response < run.switch_tick);
    assert!(run.switch_tick < run.next_source_response);
    assert_eq!(
        switched
            .pointer("/simulation/status")
            .and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        switched.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("aa00dd0655667788aa00dd06")
    );
    for (register, value) in [("x14", "0x8877665506dd00aa"), ("x15", "0x8877665506dd00ab")] {
        assert_eq!(
            switched
                .pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "completed multi-source handoff must preserve {register}: {switched}"
        );
    }

    let timing_switch = completed_multi_source_timing_switch(switched);
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("completed multi-source switch action tick");
    assert!(run.load_response < timing_action_tick);
    assert!(timing_action_tick < run.next_source_response);
    let transfer = completed_multi_source_transfer(switched);
    assert_eq!(
        transfer
            .pointer("/live_data_handoff")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        transfer
            .pointer("/quiescence_gate/validated")
            .and_then(Value::as_bool),
        Some(false)
    );

    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(3)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(3)
    );

    let handoff = transfer_handoff_chunk(transfer, "cpu0");
    for (field, expected) in [
        ("schema_version", 7),
        ("outstanding_requests", 1),
        ("resident_rows", 3),
        ("transport_owned_rows", 1),
        ("buffered_store_rows", 1),
        ("forwarded_rows", 0),
        ("partial_overlay_rows", 0),
        ("partial_overlay_source_rows", 0),
        ("completed_partial_overlay_rows", 1),
        ("completed_partial_overlay_source_rows", 2),
        ("younger_rows", 0),
        ("first_bytes", 2),
        ("first_o3_sequence", 6),
        ("first_completed_partial_overlay_issue_tick", load_issue),
        (
            "first_completed_partial_overlay_response_tick",
            run.load_response,
        ),
        ("first_completed_partial_overlay_bytes", 8),
        (
            "first_completed_partial_overlay_original_forwarded_mask",
            0x0f,
        ),
        (
            "first_completed_partial_overlay_original_response_mask",
            0xf0,
        ),
        ("first_completed_partial_overlay_live_forwarded_mask", 0x0c),
        (
            "first_completed_partial_overlay_retired_forwarded_mask",
            0x03,
        ),
        (
            "first_completed_partial_overlay_original_forwarded_bytes",
            4,
        ),
        ("first_completed_partial_overlay_live_forwarded_bytes", 2),
        ("first_completed_partial_overlay_retired_forwarded_bytes", 2),
        ("first_completed_partial_overlay_o3_sequence", 8),
    ] {
        assert_eq!(
            handoff
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "completed multi-source handoff field {field}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_operation").and_then(Value::as_str),
        Some("store")
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some("0x80000102")
    );
    assert_eq!(
        handoff
            .pointer("/first_completed_partial_overlay_operation")
            .and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        handoff
            .pointer("/first_completed_partial_overlay_address")
            .and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_completed_partial_overlay_data_hex")
            .and_then(Value::as_str),
        Some("aa00dd0655667788")
    );
    assert_eq!(
        handoff.pointer("/last_issue_tick").and_then(Value::as_u64),
        Some(load_issue)
    );
    assert!(handoff
        .pointer("/first_completed_partial_overlay_fetch_request_sequence")
        .and_then(Value::as_u64)
        .is_some());
    assert!(handoff
        .pointer("/first_completed_partial_overlay_load_data_request_sequence")
        .and_then(Value::as_u64)
        .is_some());

    let sources = handoff
        .pointer("/first_completed_partial_overlay_sources")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing completed multi-source provenance: {handoff}"));
    assert_eq!(sources.len(), 2);
    for (source, address, bytes, ownership_mask, data) in [
        (&sources[0], "0x80000102", 2, 0x08, "0006"),
        (&sources[1], "0x80000102", 1, 0x04, "dd"),
    ] {
        assert_eq!(
            source.pointer("/source_address").and_then(Value::as_str),
            Some(address)
        );
        assert_eq!(
            source.pointer("/source_bytes").and_then(Value::as_u64),
            Some(bytes)
        );
        assert_eq!(
            source.pointer("/ownership_mask").and_then(Value::as_u64),
            Some(ownership_mask)
        );
        assert_eq!(
            source.pointer("/source_data_hex").and_then(Value::as_str),
            Some(data)
        );
    }
    let source_requests = sources
        .iter()
        .map(|source| {
            (
                source
                    .pointer("/source_data_request_agent")
                    .and_then(Value::as_u64)
                    .expect("completed source request agent"),
                source
                    .pointer("/source_data_request_sequence")
                    .and_then(Value::as_u64)
                    .expect("completed source request sequence"),
            )
        })
        .collect::<Vec<_>>();
    assert!(source_requests[0] < source_requests[1]);
    assert_eq!(
        source_requests[0],
        (
            handoff
                .pointer("/first_data_request_agent")
                .and_then(Value::as_u64)
                .expect("first completed source agent"),
            handoff
                .pointer("/first_data_request_sequence")
                .and_then(Value::as_u64)
                .expect("first completed source sequence"),
        )
    );
    let completed_load_request = (
        handoff
            .pointer("/first_completed_partial_overlay_load_data_request_agent")
            .and_then(Value::as_u64)
            .expect("completed load request agent"),
        handoff
            .pointer("/first_completed_partial_overlay_load_data_request_sequence")
            .and_then(Value::as_u64)
            .expect("completed load request sequence"),
    );
    assert!(source_requests[1] < completed_load_request);

    let buffered_stores = handoff
        .pointer("/buffered_stores")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing completed buffered-store ownership: {handoff}"));
    assert_eq!(buffered_stores.len(), 1);
    let buffered = &buffered_stores[0];
    assert_eq!(
        (
            buffered
                .pointer("/data_request_agent")
                .and_then(Value::as_u64)
                .expect("completed buffered request agent"),
            buffered
                .pointer("/data_request_sequence")
                .and_then(Value::as_u64)
                .expect("completed buffered request sequence"),
        ),
        source_requests[1]
    );
    assert_eq!(
        (
            buffered
                .pointer("/predecessor_data_request_agent")
                .and_then(Value::as_u64)
                .expect("completed predecessor request agent"),
            buffered
                .pointer("/predecessor_data_request_sequence")
                .and_then(Value::as_u64)
                .expect("completed predecessor request sequence"),
        ),
        source_requests[0]
    );

    for pc in [
        MULTI_SOURCE_OLDER_STORE_PC,
        MULTI_SOURCE_MIDDLE_STORE_PC,
        MULTI_SOURCE_YOUNGEST_STORE_PC,
        MULTI_SOURCE_YOUNGER_LOAD_PC,
    ] {
        let baseline_event = event_at_pc(baseline, pc);
        let transferred = event_at_pc(switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(transferred, field),
                event_u64(baseline_event, field),
                "completed multi-source handoff must preserve {field} for {pc}: {transferred}"
            );
        }
    }
    assert_eq!(data_memory_request_count(switched), 6);
    assert_memory_resources(switched, memory_system);
    assert_json_stat(
        switched,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );

    let trace_switch = switched
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
                    && record.pointer("/tick").and_then(Value::as_u64) == Some(timing_action_tick)
            })
        })
        .unwrap_or_else(|| panic!("missing HostAction completed multi-source trace: {switched}"));
    assert_eq!(
        transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction completed multi-source state transfer"),
            "cpu0",
        ),
        handoff
    );
}

fn run_completed_multi_source_partial_handoff(memory_system: &str) -> CompletedMultiSourceRun {
    let path = multi_source_partial_forwarded_store_load_binary(&format!(
        "host-switch-completed-multi-source-partial-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_store_load_handoff(&path, memory_system, None, 4);
    let load = event_at_pc(&baseline, MULTI_SOURCE_YOUNGER_LOAD_PC);
    let load_response = event_u64(load, "lsq_data_response_tick");
    let next_source_response = [
        MULTI_SOURCE_OLDER_STORE_PC,
        MULTI_SOURCE_MIDDLE_STORE_PC,
        MULTI_SOURCE_YOUNGEST_STORE_PC,
    ]
    .into_iter()
    .map(|pc| event_u64(event_at_pc(&baseline, pc), "lsq_data_response_tick"))
    .filter(|tick| *tick > load_response)
    .min()
    .expect("live source store after completed load");
    let switch_tick =
        load_response.saturating_add(next_source_response.saturating_sub(load_response) / 2);
    assert!(
        load_response < switch_tick && switch_tick < next_source_response,
        "completed partial window must follow load response and precede the next source response: load_response={load_response}, switch_tick={switch_tick}, next_source_response={next_source_response}"
    );
    let switched = run_store_load_handoff(&path, memory_system, Some(switch_tick), 4);
    CompletedMultiSourceRun {
        baseline,
        switched,
        load_response,
        switch_tick,
        next_source_response,
    }
}

fn completed_multi_source_timing_switch(json: &Value) -> &Value {
    json.pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing completed partial timing switch: {json}"))
}

fn completed_multi_source_transfer(json: &Value) -> &Value {
    completed_multi_source_timing_switch(json)
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing completed partial state transfer: {json}"))
}

fn assert_multi_source_partial_forwarded_store_load_handoff(memory_system: &str) {
    let path = multi_source_partial_forwarded_store_load_binary(&format!(
        "host-switch-multi-source-partial-forwarded-store-load-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_store_load_handoff(&path, memory_system, None, 4);
    let source_pcs = [
        MULTI_SOURCE_OLDER_STORE_PC,
        MULTI_SOURCE_MIDDLE_STORE_PC,
        MULTI_SOURCE_YOUNGEST_STORE_PC,
    ];
    let load = event_at_pc(&baseline, MULTI_SOURCE_YOUNGER_LOAD_PC);
    let load_issue = event_u64(load, "issue_tick");
    let first_response = source_pcs
        .iter()
        .map(|pc| event_u64(event_at_pc(&baseline, pc), "lsq_data_response_tick"))
        .chain(std::iter::once(event_u64(load, "lsq_data_response_tick")))
        .min()
        .expect("multi-source response tick");
    let switch_tick = load_issue.saturating_add(first_response.saturating_sub(load_issue) / 2);
    assert!(
        load_issue < switch_tick && switch_tick < first_response,
        "multi-source window must precede the first response: load_issue={load_issue}, switch_tick={switch_tick}, first_response={first_response}"
    );

    let json = run_store_load_handoff(&path, memory_system, Some(switch_tick), 4);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("aa00dd0655667788aa00dd06")
    );
    for (register, value) in [("x14", "0x8877665506dd00aa"), ("x15", "0x8877665506dd00ab")] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "multi-source handoff must preserve {register}: {json}"
        );
    }

    let timing_switch = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing multi-source timing switch: {json}"));
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("multi-source switch action tick");
    assert!(load_issue < timing_action_tick && timing_action_tick < first_response);

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing multi-source state transfer: {timing_switch}"));
    assert_eq!(
        transfer
            .pointer("/live_data_handoff")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        transfer
            .pointer("/quiescence_gate/validated")
            .and_then(Value::as_bool),
        Some(false)
    );

    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(4)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(4)
    );

    let handoff = transfer_handoff_chunk(transfer, "cpu0");
    for (field, expected) in [
        ("schema_version", 7),
        ("outstanding_requests", 2),
        ("resident_rows", 4),
        ("transport_owned_rows", 2),
        ("buffered_store_rows", 2),
        ("forwarded_rows", 0),
        ("partial_overlay_rows", 1),
        ("partial_overlay_source_rows", 3),
        ("younger_rows", 0),
        ("first_bytes", 4),
        ("first_partial_overlay_bytes", 8),
        ("first_partial_overlay_forwarded_mask", 15),
        ("first_partial_overlay_response_owned_mask", 240),
        ("first_partial_overlay_forwarded_bytes", 4),
    ] {
        assert_eq!(
            handoff
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "multi-source handoff field {field}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_operation").and_then(Value::as_str),
        Some("store")
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_partial_overlay_address")
            .and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_partial_overlay_forwarded_data_hex")
            .and_then(Value::as_str),
        Some("aa00dd0600000000")
    );
    assert_eq!(
        handoff.pointer("/last_issue_tick").and_then(Value::as_u64),
        Some(load_issue)
    );

    let sources = handoff
        .pointer("/first_partial_overlay_sources")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing multi-source provenance: {handoff}"));
    assert_eq!(sources.len(), 3);
    for (source, address, bytes, ownership_mask, data) in [
        (&sources[0], DATA_ADDRESS, 4, 3, "aa000000"),
        (&sources[1], "0x80000102", 2, 8, "0006"),
        (&sources[2], "0x80000102", 1, 4, "dd"),
    ] {
        assert_eq!(
            source.pointer("/source_address").and_then(Value::as_str),
            Some(address)
        );
        assert_eq!(
            source.pointer("/source_bytes").and_then(Value::as_u64),
            Some(bytes)
        );
        assert_eq!(
            source.pointer("/ownership_mask").and_then(Value::as_u64),
            Some(ownership_mask)
        );
        assert_eq!(
            source.pointer("/source_data_hex").and_then(Value::as_str),
            Some(data)
        );
    }
    let source_requests = sources
        .iter()
        .map(|source| {
            (
                source
                    .pointer("/source_data_request_agent")
                    .and_then(Value::as_u64)
                    .expect("source request agent"),
                source
                    .pointer("/source_data_request_sequence")
                    .and_then(Value::as_u64)
                    .expect("source request sequence"),
            )
        })
        .collect::<Vec<_>>();
    assert!(source_requests.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        source_requests[0],
        (
            handoff
                .pointer("/first_data_request_agent")
                .and_then(Value::as_u64)
                .expect("first data request agent"),
            handoff
                .pointer("/first_data_request_sequence")
                .and_then(Value::as_u64)
                .expect("first data request sequence"),
        )
    );
    let buffered_stores = handoff
        .pointer("/buffered_stores")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing buffered-store ownership: {handoff}"));
    assert_eq!(buffered_stores.len(), 2);
    for (buffered, request, predecessor) in [
        (&buffered_stores[0], source_requests[1], source_requests[0]),
        (&buffered_stores[1], source_requests[2], source_requests[1]),
    ] {
        assert_eq!(
            (
                buffered
                    .pointer("/data_request_agent")
                    .and_then(Value::as_u64)
                    .expect("buffered data request agent"),
                buffered
                    .pointer("/data_request_sequence")
                    .and_then(Value::as_u64)
                    .expect("buffered data request sequence"),
            ),
            request
        );
        assert_eq!(
            (
                buffered
                    .pointer("/predecessor_data_request_agent")
                    .and_then(Value::as_u64)
                    .expect("buffered predecessor request agent"),
                buffered
                    .pointer("/predecessor_data_request_sequence")
                    .and_then(Value::as_u64)
                    .expect("buffered predecessor request sequence"),
            ),
            predecessor
        );
    }

    for pc in source_pcs
        .into_iter()
        .chain(std::iter::once(MULTI_SOURCE_YOUNGER_LOAD_PC))
    {
        let baseline_event = event_at_pc(&baseline, pc);
        let transferred = event_at_pc(&json, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(transferred, field),
                event_u64(baseline_event, field),
                "multi-source handoff must preserve {field} for {pc}: {transferred}"
            );
        }
    }
    assert_eq!(data_memory_request_count(&json), 6);
    assert_memory_resources(&json, memory_system);
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );

    let trace_switch = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
                    && record.pointer("/tick").and_then(Value::as_u64) == Some(timing_action_tick)
            })
        })
        .unwrap_or_else(|| panic!("missing HostAction multi-source trace: {json}"));
    assert_eq!(
        transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction multi-source state transfer"),
            "cpu0",
        ),
        handoff
    );
}

#[test]
fn rem6_run_host_switch_rejects_partial_forwarded_store_load_with_younger_row() {
    const ROUTE_DELAY: u64 = 32;
    let path = partial_forwarded_store_load_with_younger_binary(
        "host-switch-partial-forwarded-store-load-younger-row",
    );
    let baseline = run_store_load_handoff_with_delay(&path, "direct", None, 3, ROUTE_DELAY);
    let store = event_at_pc(&baseline, OLDER_STORE_PC);
    let load = event_at_pc(&baseline, YOUNGER_LOAD_PC);
    let younger = event_at_pc(&baseline, INDEPENDENT_YOUNGER_ALU_PC);
    let load_issue = event_u64(load, "issue_tick");
    let younger_issue = event_u64(younger, "issue_tick");
    let first_response =
        event_u64(load, "lsq_data_response_tick").min(event_u64(store, "lsq_data_response_tick"));
    let switch_floor = load_issue.max(younger_issue);
    let switch_tick = switch_floor.saturating_add(first_response.saturating_sub(switch_floor) / 2);
    assert!(
        switch_floor < switch_tick && switch_tick < first_response,
        "younger-row window must follow issue and precede the first response: load_issue={load_issue}, younger_issue={younger_issue}, switch_tick={switch_tick}, first_response={first_response}"
    );

    let output =
        store_load_handoff_command_with_delay(&path, "direct", Some(switch_tick), 3, ROUTE_DELAY)
            .output()
            .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("checkpoint component is not quiescent: cpu0"),
        "unexpected partial-forward handoff with younger row error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rem6_run_host_switch_rejects_multi_source_partial_forwarded_store_load_with_younger_row() {
    const ROUTE_DELAY: u64 = 32;
    let path = multi_source_partial_forwarded_store_load_with_younger_binary(
        "host-switch-multi-source-partial-forwarded-store-load-younger-row",
    );
    let baseline = run_store_load_handoff_with_delay(&path, "direct", None, 4, ROUTE_DELAY);
    let source_pcs = [
        MULTI_SOURCE_YOUNGER_ROW_OLDER_STORE_PC,
        MULTI_SOURCE_YOUNGER_ROW_YOUNGEST_STORE_PC,
    ];
    let load = event_at_pc(&baseline, MULTI_SOURCE_YOUNGER_ROW_LOAD_PC);
    let younger = event_at_pc(&baseline, MULTI_SOURCE_YOUNGER_ROW_ALU_PC);
    let load_issue = event_u64(load, "issue_tick");
    let younger_issue = event_u64(younger, "issue_tick");
    let first_response = source_pcs
        .iter()
        .map(|pc| event_u64(event_at_pc(&baseline, pc), "lsq_data_response_tick"))
        .chain(std::iter::once(event_u64(load, "lsq_data_response_tick")))
        .min()
        .expect("multi-source younger-row response tick");
    let switch_floor = load_issue.max(younger_issue);
    let switch_tick = switch_floor.saturating_add(first_response.saturating_sub(switch_floor) / 2);
    assert!(
        switch_floor < switch_tick && switch_tick < first_response,
        "multi-source younger-row window must follow issue and precede the first response: load_issue={load_issue}, younger_issue={younger_issue}, switch_tick={switch_tick}, first_response={first_response}"
    );

    let output =
        store_load_handoff_command_with_delay(&path, "direct", Some(switch_tick), 4, ROUTE_DELAY)
            .output()
            .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("checkpoint component is not quiescent: cpu0"),
        "unexpected multi-source partial-forward handoff with younger row error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rem6_run_host_switch_rejects_completed_partial_forwarded_store_load_with_younger_row() {
    const ROUTE_DELAY: u64 = 32;
    let path = multi_source_partial_forwarded_store_load_with_younger_binary(
        "host-switch-completed-partial-forwarded-store-load-younger-row",
    );
    let baseline = run_store_load_handoff_with_delay(&path, "direct", None, 4, ROUTE_DELAY);
    let load = event_at_pc(&baseline, MULTI_SOURCE_YOUNGER_ROW_LOAD_PC);
    let load_response = event_u64(load, "lsq_data_response_tick");
    let next_source_response = [
        MULTI_SOURCE_YOUNGER_ROW_OLDER_STORE_PC,
        MULTI_SOURCE_YOUNGER_ROW_YOUNGEST_STORE_PC,
    ]
    .into_iter()
    .map(|pc| event_u64(event_at_pc(&baseline, pc), "lsq_data_response_tick"))
    .filter(|tick| *tick > load_response)
    .min()
    .expect("completed partial live source store after load response");
    let switch_tick =
        load_response.saturating_add(next_source_response.saturating_sub(load_response) / 2);
    assert!(
        load_response < switch_tick && switch_tick < next_source_response,
        "completed partial younger-row window must follow load response and precede the next source response: load_response={load_response}, switch_tick={switch_tick}, next_source_response={next_source_response}"
    );

    let output =
        store_load_handoff_command_with_delay(&path, "direct", Some(switch_tick), 4, ROUTE_DELAY)
            .output()
            .unwrap();

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("checkpoint component is not quiescent: cpu0"),
        "unexpected completed partial handoff with younger row error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn assert_partial_forwarded_store_load_handoff(memory_system: &str) {
    let path = partial_forwarded_store_load_binary(&format!(
        "host-switch-partial-forwarded-store-load-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_store_load_handoff(&path, memory_system, None, 2);
    let baseline_store = event_at_pc(&baseline, OLDER_STORE_PC);
    let baseline_load = event_at_pc(&baseline, YOUNGER_LOAD_PC);
    let load_issue = event_u64(baseline_load, "issue_tick");
    let first_response = event_u64(baseline_load, "lsq_data_response_tick")
        .min(event_u64(baseline_store, "lsq_data_response_tick"));
    let switch_tick = load_issue.saturating_add(first_response.saturating_sub(load_issue) / 2);
    assert!(load_issue < switch_tick && switch_tick < first_response);

    let json = run_store_load_handoff(&path, memory_system, Some(switch_tick), 2);

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("115a3380115a3380125a3380")
    );
    for (register, value) in [("x12", "0xffffffff80335a11"), ("x13", "0xffffffff80335a12")] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "partial-overlay handoff must preserve {register}: {json}"
        );
    }

    let timing_switch = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing partial-overlay timing switch: {json}"));
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("partial-overlay switch action tick");
    assert!(load_issue < timing_action_tick && timing_action_tick < first_response);

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing partial-overlay state transfer: {timing_switch}"));
    assert_eq!(
        transfer
            .pointer("/live_data_handoff")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        transfer
            .pointer("/quiescence_gate/validated")
            .and_then(Value::as_bool),
        Some(false)
    );

    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(2)
    );

    let handoff = transfer_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(7)
    );
    for (field, expected) in [
        ("outstanding_requests", 2),
        ("resident_rows", 2),
        ("transport_owned_rows", 2),
        ("buffered_store_rows", 0),
        ("forwarded_rows", 0),
        ("partial_overlay_rows", 1),
        ("partial_overlay_source_rows", 1),
        ("younger_rows", 0),
        ("first_bytes", 1),
        ("first_partial_overlay_bytes", 4),
        ("first_partial_overlay_source_bytes", 1),
        ("first_partial_overlay_forwarded_mask", 2),
        ("first_partial_overlay_response_owned_mask", 13),
        ("first_partial_overlay_forwarded_bytes", 1),
    ] {
        assert_eq!(
            handoff
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "partial-overlay handoff field {field}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_operation").and_then(Value::as_str),
        Some("store")
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some("0x80000101")
    );
    assert_eq!(
        handoff
            .pointer("/first_partial_overlay_operation")
            .and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        handoff
            .pointer("/first_partial_overlay_address")
            .and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_partial_overlay_source_address")
            .and_then(Value::as_str),
        Some("0x80000101")
    );
    assert_eq!(
        handoff
            .pointer("/first_partial_overlay_source_data_hex")
            .and_then(Value::as_str),
        Some("5a")
    );
    assert_eq!(
        handoff
            .pointer("/first_partial_overlay_forwarded_data_hex")
            .and_then(Value::as_str),
        Some("005a0000")
    );
    assert_eq!(
        handoff.pointer("/last_issue_tick").and_then(Value::as_u64),
        Some(load_issue)
    );
    assert_eq!(
        handoff.pointer("/first_partial_overlay_source_data_request_agent"),
        handoff.pointer("/first_data_request_agent")
    );
    assert_eq!(
        handoff.pointer("/first_partial_overlay_source_data_request_sequence"),
        handoff.pointer("/first_data_request_sequence")
    );
    assert!(
        handoff
            .pointer("/first_partial_overlay_load_data_request_sequence")
            .and_then(Value::as_u64)
            .is_some_and(|sequence| {
                handoff
                    .pointer("/first_data_request_sequence")
                    .and_then(Value::as_u64)
                    != Some(sequence)
            }),
        "overlay load must retain its own transport request: {handoff}"
    );

    for pc in [OLDER_STORE_PC, YOUNGER_LOAD_PC] {
        let baseline_event = event_at_pc(&baseline, pc);
        let transferred = event_at_pc(&json, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(transferred, field),
                event_u64(baseline_event, field),
                "partial-overlay handoff must preserve {field} for {pc}: {transferred}"
            );
        }
    }
    assert!(event_at_pc_if_present(&json, DEPENDENT_ALU_PC).is_none());
    assert_eq!(data_memory_request_count(&json), 4);
    assert_memory_resources(&json, memory_system);
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );

    let trace_switch = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
                    && record.pointer("/tick").and_then(Value::as_u64) == Some(timing_action_tick)
            })
        })
        .unwrap_or_else(|| panic!("missing HostAction partial-overlay trace: {json}"));
    assert_eq!(
        transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction partial-overlay state transfer"),
            "cpu0",
        ),
        handoff
    );
}

fn assert_full_forwarded_store_load_handoff(memory_system: &str) {
    let path = full_forwarded_store_load_binary(&format!(
        "host-switch-full-forwarded-store-load-{}",
        memory_system.replace('-', "_")
    ));
    let baseline = run_full_forwarded_store_load_handoff(&path, memory_system, None);
    let baseline_store = event_at_pc(&baseline, OLDER_STORE_PC);
    let baseline_load = event_at_pc(&baseline, YOUNGER_LOAD_PC);
    let load_issue = event_u64(baseline_load, "issue_tick");
    let load_response = event_u64(baseline_load, "lsq_data_response_tick");
    let store_response = event_u64(baseline_store, "lsq_data_response_tick");
    let switch_tick =
        load_response.saturating_add(store_response.saturating_sub(load_response) / 2);
    assert!(load_response < switch_tick && switch_tick < store_response);

    let json = run_full_forwarded_store_load_handoff(&path, memory_system, Some(switch_tick));

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(RESULTS)
    );
    for (register, value) in [("x12", "0x2a"), ("x13", "0x2b")] {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value),
            "forwarded handoff must preserve {register}: {json}"
        );
    }

    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing execution-mode switches: {json}"));
    let timing_switch = switches
        .iter()
        .find(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
        })
        .unwrap_or_else(|| panic!("missing forwarded store-load timing switch: {switches:?}"));
    let timing_action_tick = timing_switch
        .pointer("/tick")
        .and_then(Value::as_u64)
        .expect("timing switch action tick");
    assert!(load_response < timing_action_tick && timing_action_tick < store_response);
    assert!(timing_action_tick >= switch_tick);

    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing forwarded store-load handoff: {timing_switch}"));
    assert_eq!(
        transfer
            .pointer("/live_data_handoff")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        transfer
            .pointer("/quiescence_gate/validated")
            .and_then(Value::as_bool),
        Some(false)
    );

    let runtime = latest_transfer_o3_runtime_chunk(transfer, "cpu0");
    assert_eq!(
        runtime.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(2)
    );

    let handoff = transfer_handoff_chunk(transfer, "cpu0");
    assert_eq!(
        handoff.pointer("/decode_error").and_then(Value::as_bool),
        Some(false)
    );
    assert_eq!(
        handoff.pointer("/schema_version").and_then(Value::as_u64),
        Some(7)
    );
    for (field, expected) in [
        ("outstanding_requests", 1),
        ("resident_rows", 2),
        ("transport_owned_rows", 1),
        ("buffered_store_rows", 0),
        ("forwarded_rows", 1),
        ("partial_overlay_source_rows", 0),
        ("younger_rows", 0),
        ("first_bytes", 4),
        ("first_forwarded_bytes", 4),
    ] {
        assert_eq!(
            handoff
                .pointer(&format!("/{field}"))
                .and_then(Value::as_u64),
            Some(expected),
            "forwarded handoff field {field}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_operation").and_then(Value::as_str),
        Some("store")
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_operation")
            .and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_address")
            .and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_data_hex")
            .and_then(Value::as_str),
        Some("2a000000")
    );
    assert_eq!(
        handoff.pointer("/last_issue_tick").and_then(Value::as_u64),
        Some(load_issue)
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_issue_tick")
            .and_then(Value::as_u64),
        Some(load_issue)
    );
    assert_eq!(
        handoff
            .pointer("/first_forwarded_response_tick")
            .and_then(Value::as_u64),
        Some(load_response)
    );
    assert_eq!(
        handoff.pointer("/first_forwarding_source_data_request_agent"),
        handoff.pointer("/first_data_request_agent")
    );
    assert_eq!(
        handoff.pointer("/first_forwarding_source_data_request_sequence"),
        handoff.pointer("/first_data_request_sequence")
    );

    for pc in [OLDER_STORE_PC, YOUNGER_LOAD_PC] {
        let baseline_event = event_at_pc(&baseline, pc);
        let transferred = event_at_pc(&json, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(transferred, field),
                event_u64(baseline_event, field),
                "handoff must preserve {field} for {pc}: {transferred}"
            );
        }
    }
    assert!(event_at_pc_if_present(&json, DEPENDENT_ALU_PC).is_none());
    assert_eq!(data_memory_request_count(&json), 3);
    assert_forwarding_trace(&json, timing_action_tick, handoff);
    assert_memory_resources(&json, memory_system);
    assert_json_stat(
        &json,
        "sim.cpu0.o3.lsq_store_to_load_forwarding_matches",
        "Count",
        1,
        "monotonic",
    );

    let trace_switch = json
        .pointer("/debug/host_action_trace")
        .and_then(Value::as_array)
        .and_then(|records| {
            records.iter().find(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some("execution_mode_switch")
                    && record.pointer("/tick").and_then(Value::as_u64) == Some(timing_action_tick)
            })
        })
        .unwrap_or_else(|| panic!("missing HostAction forwarded handoff trace: {json}"));
    assert_eq!(
        transfer_handoff_chunk(
            trace_switch
                .pointer("/state_transfer")
                .expect("HostAction state transfer"),
            "cpu0",
        ),
        handoff
    );
}

fn assert_forwarding_trace(json: &Value, switch_tick: u64, handoff: &Value) {
    let data = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing forwarded Data trace: {json}"));
    assert_eq!(data.len(), 4);
    let observed = data
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
            ("load", DATA_ADDRESS, 4),
            ("store", DATA_ADDRESS, 4),
            ("store", "0x80000104", 4),
            ("store", "0x80000108", 4),
        ])
    );

    let memory = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing forwarded Memory trace: {json}"));
    let request = memory
        .iter()
        .find(|record| {
            record.pointer("/channel").and_then(Value::as_str) == Some("data")
                && record.pointer("/kind").and_then(Value::as_str) == Some("request_sent")
                && record
                    .pointer("/tick")
                    .and_then(Value::as_u64)
                    .is_some_and(|tick| tick < switch_tick)
        })
        .unwrap_or_else(|| panic!("missing pre-switch store request: {memory:?}"));
    assert_eq!(
        handoff
            .pointer("/first_data_request_agent")
            .and_then(Value::as_u64),
        request.pointer("/request_agent").and_then(Value::as_u64)
    );
    assert_eq!(
        handoff
            .pointer("/first_data_request_sequence")
            .and_then(Value::as_u64),
        request.pointer("/request").and_then(Value::as_u64)
    );
}

fn assert_memory_resources(json: &Value, memory_system: &str) {
    for pointer in [
        "/memory_resources/cache/data/activity",
        "/memory_resources/transport/data/activity",
        "/memory_resources/fabric/activity",
        "/memory_resources/dram/activity",
    ] {
        let value = json.pointer(pointer).and_then(Value::as_u64).unwrap_or(0);
        if memory_system == "direct" && pointer != "/memory_resources/transport/data/activity" {
            assert_eq!(value, 0, "direct handoff should bypass {pointer}: {json}");
        } else {
            assert!(value > 0, "handoff should exercise {pointer}: {json}");
        }
    }
}

fn run_full_forwarded_store_load_handoff(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
) -> Value {
    run_store_load_handoff(path, memory_system, switch_tick, 2)
}

fn run_store_load_handoff(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
    depth: usize,
) -> Value {
    run_store_load_handoff_with_delay(path, memory_system, switch_tick, depth, 16)
}

fn run_store_load_handoff_with_delay(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
    depth: usize,
    route_delay: u64,
) -> Value {
    let output =
        store_load_handoff_command_with_delay(path, memory_system, switch_tick, depth, route_delay)
            .output()
            .unwrap();
    assert!(
        output.status.success(),
        "{memory_system} switch {switch_tick:?}; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn store_load_handoff_command_with_delay(
    path: &Path,
    memory_system: &str,
    switch_tick: Option<u64>,
    depth: usize,
    route_delay: u64,
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        "1500",
        "--stats-format",
        "json",
        "--execute",
        "--debug-flags",
        "O3,Data,Memory,HostAction",
        "--riscv-o3-scalar-memory-depth",
        &depth.to_string(),
        "--memory-system",
        memory_system,
        "--memory-route-delay",
        &route_delay.to_string(),
        "--m5-switch-cpu-mode",
        "detailed",
        "--dump-memory",
        "0x80000100:12",
    ]);
    if let Some(tick) = switch_tick {
        command.args(["--host-switch-cpu-mode", &format!("{tick}:cpu0:timing")]);
    }
    command
}

fn full_forwarded_store_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x2a, 0, 0x0, 11, 0x13),
        s_type(0, 11, 10, 0b010),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(4, 12, 10, 0b010),
        s_type(8, 13, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0; 16]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn partial_forwarded_store_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(1, 11, 10, 0b000),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(1, 12, 0x0, 13, 0x13),
        s_type(4, 12, 10, 0b010),
        s_type(8, 13, 10, 0b010),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x8033_2211, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn partial_forwarded_store_load_with_younger_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0x5a, 0, 0x0, 11, 0x13),
        s_type(1, 11, 10, 0b000),
        i_type(0, 10, 0b010, 12, 0x03),
        i_type(7, 0, 0x0, 14, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x8033_2211, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multi_source_partial_forwarded_store_load_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0xaa, 0, 0x0, 11, 0x13),
        i_type(0x6bb, 0, 0x0, 12, 0x13),
        i_type(0xdd, 0, 0x0, 13, 0x13),
        s_type(0, 11, 10, 0b010),
        s_type(2, 12, 10, 0b001),
        s_type(2, 13, 10, 0b000),
        i_type(0, 10, 0b011, 14, 0x03),
        i_type(1, 14, 0x0, 15, 0x13),
        s_type(8, 14, 10, 0b011),
        s_type(16, 15, 10, 0b011),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x4433_2211, 0x8877_6655, 0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn multi_source_partial_forwarded_store_load_with_younger_binary(name: &str) -> std::path::PathBuf {
    let data_start = 256_i32;
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    words.extend([
        u_type(0, 10, 0x17),
        i_type(data_start - auipc_pc, 10, 0x0, 10, 0x13),
        i_type(0xaa, 0, 0x0, 11, 0x13),
        i_type(0xdd, 0, 0x0, 12, 0x13),
        s_type(0, 11, 10, 0b010),
        s_type(2, 12, 10, 0b000),
        i_type(0, 10, 0b011, 13, 0x03),
        i_type(7, 0, 0x0, 14, 0x13),
    ]);
    append_host_stop(&mut words);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x4433_2211, 0x8877_6655, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    event_at_pc_if_present(json, pc).unwrap_or_else(|| panic!("missing O3 event at {pc}: {json}"))
}

fn event_at_pc_if_present<'a>(json: &'a Value, pc: &str) -> Option<&'a Value> {
    json.pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .find(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        })
}

fn event_u64(event: &Value, field: &str) -> u64 {
    event
        .pointer(&format!("/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing {field}: {event}"))
}

fn data_memory_request_count(json: &Value) -> usize {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("run JSON Memory trace")
        .iter()
        .filter(|record| record.pointer("/channel").and_then(Value::as_str) == Some("data"))
        .map(|record| {
            (
                record.pointer("/request_agent").and_then(Value::as_u64),
                record.pointer("/request").and_then(Value::as_u64),
            )
        })
        .collect::<BTreeSet<_>>()
        .len()
}

fn transfer_handoff_chunk<'a>(transfer: &'a Value, component: &str) -> &'a Value {
    transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|entry| {
                entry.pointer("/component").and_then(Value::as_str) == Some(component)
            })
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str)
                    == Some(RISCV_O3_LIVE_DATA_HANDOFF_CHUNK)
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_live_data_handoff"))
        .unwrap_or_else(|| panic!("missing decoded live-data handoff chunk: {transfer}"))
}
