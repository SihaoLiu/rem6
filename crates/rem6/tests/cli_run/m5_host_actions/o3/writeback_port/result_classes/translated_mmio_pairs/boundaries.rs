use super::*;

const HOST_EVENT_DELAY: u64 = 1;

pub(super) fn assert_mixed_pair(
    fixture: &TranslatedMemoryPairFixture,
    memory_system: &str,
    route_delay: u64,
) {
    let completed = fixture.run_mixed(memory_system, route_delay, PAIR_MAX_TICK);
    let first = memory_result_event_at_pc(&completed, FIRST_PC);
    let second = memory_result_event_at_pc(&completed, SECOND_PC);
    assert_data_completion_at_pc(&completed, first, FIRST_PC, FIRST_PHYSICAL_PAGE);
    assert_data_completion_at_pc(&completed, second, SECOND_PC, fixture::MMIO_PAGE);
    let first_issue = event_u64(first, "issue_tick");
    let second_issue = event_u64(second, "issue_tick");
    assert!(first_issue < second_issue);
    assert_ne!(event_u64(first, "sequence"), event_u64(second, "sequence"));

    let earliest_response = [first, second]
        .map(|event| event_u64(event, "lsq_data_response_tick"))
        .into_iter()
        .min()
        .unwrap();
    assert!(second_issue < earliest_response);
    let resident = fixture.run_mixed(
        memory_system,
        route_delay,
        earliest_response.saturating_sub(1),
    );
    assert_pre_response_residency(&resident, first_issue);

    let first_request = sole_data_request_at_tick(&completed, first_issue, FIRST_PC);
    assert_no_data_request_at_tick(&completed, second_issue, SECOND_PC);
    let first_identity = request_identity(first_request);
    assert!(data_record_for_identity(&completed, "response_arrived", first_identity).is_some());

    let pair_fetches = [FIRST_PC, SECOND_PC]
        .map(|pc| fetch_request_identity(&completed, fetch_record_at_pc(&completed, pc)));
    assert_ne!(pair_fetches[0], pair_fetches[1]);
    assert_mixed_live_handoff(
        fixture,
        memory_system,
        route_delay,
        first_issue,
        second_issue,
        earliest_response,
    );
    assert_mixed_final_witness(&completed);
    assert_oldest_first_commit(&completed);

    let before = fixture.run_mixed(memory_system, route_delay, first_issue.saturating_sub(1));
    let through = fixture.run_mixed(
        memory_system,
        route_delay,
        [first, second]
            .map(|event| event_u64(event, "lsq_data_response_tick"))
            .into_iter()
            .max()
            .unwrap()
            + 1,
    );
    assert_mixed_route_resources(&before, &through, memory_system);
}

fn assert_mixed_live_handoff(
    fixture: &TranslatedMemoryPairFixture,
    memory_system: &str,
    route_delay: u64,
    first_issue: u64,
    second_issue: u64,
    earliest_response: u64,
) {
    let latest_source = earliest_response
        .checked_sub(HOST_EVENT_DELAY + 1)
        .expect("mixed pair response leaves room for a host action");
    assert!(second_issue <= latest_source);
    let switch_tick = second_issue + (latest_source - second_issue) / 2;
    let switched = fixture.run_mixed_with_switch(memory_system, route_delay, switch_tick);
    let switched_first = memory_result_event_at_pc(&switched, FIRST_PC);
    let switched_second = memory_result_event_at_pc(&switched, SECOND_PC);
    assert_eq!(event_u64(switched_first, "issue_tick"), first_issue);
    assert_eq!(event_u64(switched_second, "issue_tick"), second_issue);
    let first_request = sole_data_request_at_tick(&switched, first_issue, FIRST_PC);
    assert_no_data_request_at_tick(&switched, second_issue, SECOND_PC);

    let timing_switch = switched
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .and_then(|switches| {
            switches.iter().find(|switch| {
                switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                    && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                    && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
            })
        })
        .unwrap_or_else(|| panic!("missing mixed translated timing switch: {switched}"));
    let action_tick = event_u64(timing_switch, "tick");
    assert_eq!(action_tick, switch_tick + HOST_EVENT_DELAY);
    assert!(second_issue < action_tick && action_tick < earliest_response);
    let transfer = timing_switch
        .pointer("/state_transfer")
        .unwrap_or_else(|| panic!("missing mixed translated live handoff: {timing_switch}"));
    assert_eq!(
        transfer
            .pointer("/live_data_handoff")
            .and_then(Value::as_bool),
        Some(true)
    );
    let handoff = translated_handoff_chunk(transfer);
    for (pointer, expected) in [
        ("/outstanding_requests", 2),
        ("/resident_rows", 2),
        ("/transport_owned_rows", 2),
        ("/first_issue_tick", first_issue),
        ("/last_issue_tick", second_issue),
        ("/first_o3_sequence", event_u64(switched_first, "sequence")),
    ] {
        assert_eq!(
            handoff.pointer(pointer).and_then(Value::as_u64),
            Some(expected),
            "mixed handoff field {pointer}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some("0x80001000")
    );
    assert_eq!(
        handoff
            .pointer("/first_target/kind")
            .and_then(Value::as_str),
        Some("memory")
    );
    let first_identity = request_identity(first_request);
    assert_eq!(
        handoff
            .pointer("/first_data_request_agent")
            .and_then(Value::as_u64),
        Some(first_identity.agent)
    );
    assert_eq!(
        handoff
            .pointer("/first_data_request_sequence")
            .and_then(Value::as_u64),
        Some(first_identity.sequence)
    );
    assert_data_completion_at_pc(&switched, switched_second, SECOND_PC, fixture::MMIO_PAGE);
}

fn sole_data_request_at_tick<'a>(json: &'a Value, tick: u64, pc: &str) -> &'a Value {
    let records = data_request_sent_records(json)
        .into_iter()
        .filter(|record| event_u64(record, "tick") == tick)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "exact ordinary request for {pc}");
    records[0]
}

fn assert_no_data_request_at_tick(json: &Value, tick: u64, pc: &str) {
    let records = data_request_sent_records(json)
        .into_iter()
        .filter(|record| event_u64(record, "tick") == tick)
        .collect::<Vec<_>>();
    assert!(
        records.is_empty(),
        "{pc} must use the MMIO path: {records:?}"
    );
}

fn translated_handoff_chunk(transfer: &Value) -> &Value {
    transfer
        .pointer("/components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components
                .iter()
                .find(|entry| entry.pointer("/component").and_then(Value::as_str) == Some("cpu0"))
        })
        .and_then(|component| component.pointer("/chunks").and_then(Value::as_array))
        .and_then(|chunks| {
            chunks.iter().find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str)
                    == Some(rem6_system::RISCV_O3_LIVE_DATA_HANDOFF_CHUNK)
            })
        })
        .and_then(|chunk| chunk.pointer("/o3_live_data_handoff"))
        .unwrap_or_else(|| panic!("missing mixed translated handoff chunk: {transfer}"))
}

fn assert_mixed_final_witness(completed: &Value) {
    assert_eq!(json_u64(completed, "/readfiles/0/bytes"), 8);
    assert_register(completed, "x11", "0x11");
    assert_register(completed, "x12", "0x33");
    assert_register(completed, "x13", "0x34");
    assert_eq!(
        page_witness_hex(completed, FIRST_PHYSICAL_PAGE).as_deref(),
        Some("110000000000000011000000000000002a00000000000000")
    );
    assert_eq!(
        memory_dump_hex(completed, FIRST_PHYSICAL_PAGE + 24),
        Some("3300000000000000")
    );
    assert_eq!(
        memory_dump_hex(completed, FIRST_PHYSICAL_PAGE + 32),
        Some("3400000000000000")
    );
}

fn assert_mixed_route_resources(before: &Value, through: &Value, memory_system: &str) {
    assert_eq!(
        fixture::resource_delta(before, through, "/memory_resources/transport/data/activity"),
        1
    );
    match memory_system {
        "direct" => {
            for pointer in [
                "/memory_resources/cache/data/activity",
                "/memory_resources/fabric/activity",
                "/memory_resources/dram/activity",
            ] {
                assert_eq!(
                    fixture::resource_delta(before, through, pointer),
                    0,
                    "{pointer}"
                );
            }
        }
        "cache-fabric-dram" => {
            for (pointer, expected) in [
                ("/memory_resources/cache/data/activity", 3),
                ("/memory_resources/cache/data/dram_accesses", 1),
                ("/memory_resources/cache/data/l1/activity", 1),
                ("/memory_resources/cache/data/l2/activity", 1),
                ("/memory_resources/cache/data/l3/activity", 1),
                ("/memory_resources/fabric/activity", 2),
                ("/memory_resources/dram/activity", 0),
                ("/memory_resources/dram/accesses", 1),
                ("/memory_resources/dram/reads", 1),
            ] {
                assert_eq!(
                    fixture::resource_delta(before, through, pointer),
                    expected,
                    "{pointer}"
                );
            }
        }
        _ => unreachable!(),
    }
}
