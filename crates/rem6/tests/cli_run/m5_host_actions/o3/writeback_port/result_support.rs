use super::*;

pub(super) fn data_trace(json: &Value) -> &[Value] {
    json.pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("result run must expose Data trace: {json}"))
}

pub(super) fn event_str<'a>(event: &'a Value, field: &str) -> &'a str {
    event
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("event field {field} missing: {event}"))
}

pub(super) fn json_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("JSON field {pointer} missing: {json}"))
}

pub(super) fn assert_event_order(events: [&Value; 3], field: &str, strict: bool) {
    let [first, second, third] = events.map(|event| event_u64(event, field));
    assert!(first <= second && second <= third);
    assert!(!strict || first < second && second < third);
}

pub(super) fn assert_resource_counter(json: &Value, suffix: &str, expected: u64) {
    let pointer = format!("/memory_resources/{}", suffix.replace('.', "/"));
    assert_eq!(json_u64(json, &pointer), expected, "{pointer}: {json}");
    let stat = format!("sim.memory.resources.{suffix}");
    assert_json_stat(json, &stat, "Count", expected, "monotonic");
}

pub(super) fn memory_dump_hex(json: &Value, address: u64) -> Option<&str> {
    let address = format!("0x{address:x}");
    json.pointer("/memory")
        .and_then(Value::as_array)
        .and_then(|dumps| {
            dumps.iter().find(|dump| {
                dump.pointer("/address").and_then(Value::as_str) == Some(address.as_str())
            })
        })
        .and_then(|dump| dump.pointer("/hex").and_then(Value::as_str))
}

pub(super) fn assert_register(json: &Value, register: &str, expected: &str) {
    let pointer = format!("/cores/0/registers/{register}");
    assert_eq!(
        json.pointer(&pointer).and_then(Value::as_str),
        Some(expected),
        "{register}: {json}"
    );
}

pub(super) fn assert_register_absent(json: &Value, register: &str) {
    let pointer = format!("/cores/0/registers/{register}");
    assert_eq!(
        json.pointer(&pointer),
        None,
        "{register} must remain unpublished: {json}"
    );
}

fn rob_entries(json: &Value) -> &[Value] {
    json.pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("O3 ROB snapshot missing: {json}"))
}

pub(super) fn rob_entry_at_sequence(json: &Value, sequence: u64) -> &Value {
    rob_entries(json)
        .iter()
        .find(|entry| event_u64(entry, "sequence") == sequence)
        .unwrap_or_else(|| panic!("O3 ROB should retain live sequence {sequence}: {json}"))
}

pub(super) fn assert_rob_sequence_absent(json: &Value, sequence: u64) {
    assert!(
        rob_entries(json)
            .iter()
            .all(|entry| event_u64(entry, "sequence") != sequence),
        "committed result sequence {sequence} must leave the admission ROB: {json}"
    );
}

pub(super) fn memory_result_event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    let mut matches = o3_trace_events(json).iter().filter(|event| {
        event.pointer("/pc").and_then(Value::as_str) == Some(pc)
            && event
                .pointer("/lsq_data_response_tick")
                .and_then(Value::as_u64)
                .is_some_and(|tick| tick > 0)
    });
    let result = matches.next().unwrap_or_else(|| {
        panic!("O3 trace should include a positive memory result at {pc}: {json}")
    });
    assert!(
        matches.next().is_none(),
        "O3 trace must include exactly one positive memory result at {pc}: {json}"
    );
    result
}

pub(super) fn result_memory_trace(json: &Value) -> (&Value, &Value, &Value) {
    let records = json
        .pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .expect("result memory trace")
        .iter()
        .filter(|record| event_str(record, "channel") == "data")
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 3, "exact result request/response trace");
    let (sent, arrived, response) = (records[0], records[1], records[2]);
    for (record, kind, endpoint) in [
        (sent, "request_sent", "cpu0.dmem"),
        (arrived, "request_arrived", "memory"),
        (response, "response_arrived", "cpu0.dmem"),
    ] {
        assert_eq!(event_str(record, "kind"), kind);
        assert_eq!(event_str(record, "endpoint"), endpoint);
        assert_eq!(event_u64(record, "request"), event_u64(sent, "request"));
        assert_eq!(event_u64(record, "route"), event_u64(sent, "route"));
    }
    (sent, arrived, response)
}
