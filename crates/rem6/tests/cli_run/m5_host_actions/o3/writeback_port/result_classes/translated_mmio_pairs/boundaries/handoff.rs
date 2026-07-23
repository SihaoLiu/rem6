use super::*;

pub(super) fn assert_mixed_live_handoff(
    fixture: &TranslatedMemoryPairFixture,
    memory_system: &str,
    route_delay: u64,
    first_issue: u64,
    second_issue: u64,
    earliest_response: u64,
) -> Value {
    let latest_source = earliest_response
        .checked_sub(HOST_EVENT_DELAY + 1)
        .expect("mixed pair response leaves room for a host action");
    assert!(second_issue <= latest_source);
    let switch_tick = second_issue + (latest_source - second_issue) / 2;
    let switched = fixture.run_mixed_with_switch(memory_system, route_delay, switch_tick);
    let switched_first = memory_result_event_at_pc(&switched, FIRST_PC);
    let switched_second = memory_result_event_at_pc(&switched, SECOND_PC);
    let completion_identities = assert_mixed_completion_identities(&switched);
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
    assert_eq!(completion_identities[0].data, first_identity);
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
    request_sent_for_identity(&switched, first_identity).unwrap();
    let o3_pcs = o3_trace_events(&switched)
        .iter()
        .map(|event| event_str(event, "pc"))
        .collect::<Vec<_>>();
    assert_eq!(o3_pcs, PAIR_PCS);
    switched
}

pub(super) fn sole_data_request_at_tick<'a>(json: &'a Value, tick: u64, pc: &str) -> &'a Value {
    let records = data_request_sent_records(json)
        .into_iter()
        .filter(|record| event_u64(record, "tick") == tick)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "exact ordinary request for {pc}");
    records[0]
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
