use super::{round_trip::CoroutineRoundTripCase, *};

pub(super) fn assert_coroutine_round_trip_final_state(
    json: &Value,
    case: CoroutineRoundTripCase,
    context: &str,
) {
    for (register, expected) in [("x1", case.final_x1), ("x5", case.final_x5)] {
        assert_eq!(
            register_value(json, register),
            expected,
            "{}: unexpected {context} {register}: {json}",
            case.label
        );
    }
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(case.memory_hex),
        "{}: unexpected {context} memory: {json}",
        case.label
    );
    assert_no_data_address(json, WRONG_STORE_ADDRESS);
}

pub(super) fn coroutine_o3_events_at_pc<'a>(
    json: &'a Value,
    pc: &str,
    context: &str,
) -> Vec<&'a Value> {
    let events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context}: missing O3 events: {json}"));
    events
        .iter()
        .filter(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .collect()
}

pub(super) fn exact_coroutine_o3_event_at_pc<'a>(
    json: &'a Value,
    pc: &str,
    context: &str,
) -> &'a Value {
    let matches = coroutine_o3_events_at_pc(json, pc, context);
    assert_eq!(
        matches.len(),
        1,
        "{context}: expected exactly one O3 event at {pc}: {matches:?}"
    );
    matches[0]
}

pub(super) fn exact_coroutine_timing_switch<'a>(json: &'a Value, context: &str) -> &'a Value {
    let switches = json
        .pointer("/host_actions/execution_mode_switches")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context}: missing execution-mode switches: {json}"));
    let matches = switches
        .iter()
        .filter(|switch| {
            switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                && switch.pointer("/previous_mode").and_then(Value::as_str) == Some("detailed")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{context}: expected exactly one cpu0 detailed-to-timing switch: {switches:?}"
    );
    matches[0]
}

pub(super) fn coroutine_data_trace_counts(json: &Value, context: &str) -> [usize; 2] {
    let records = json
        .pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context}: missing data trace: {json}"));
    let count = |kind, address| {
        records
            .iter()
            .filter(|record| {
                record.pointer("/kind").and_then(Value::as_str) == Some(kind)
                    && record.pointer("/address").and_then(Value::as_str) == Some(address)
            })
            .count()
    };
    [
        count("load", DATA_ADDRESS),
        count("store", SUCCESS_STORE_ADDRESS),
    ]
}

pub(super) fn assert_coroutine_round_trip_resident_window(
    json: &Value,
    case: CoroutineRoundTripCase,
    context: &str,
) {
    assert_eq!(
        resident_rob_pcs(json),
        [
            case.load_pc,
            case.call_pc,
            case.coroutine_pc,
            case.return_pc
        ],
        "{}: unexpected {context} round-trip ROB: {json}",
        case.label
    );
    assert_eq!(
        json.pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1),
        "{}: expected one {context} LSQ row: {json}",
        case.label
    );
}
