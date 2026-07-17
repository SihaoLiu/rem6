fn data_trace(json: &Value) -> &[Value] {
    json.pointer("/debug/data_trace")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("result run must expose Data trace: {json}"))
}

fn event_str<'a>(event: &'a Value, field: &str) -> &'a str {
    event
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("event field {field} missing: {event}"))
}

fn json_u64(json: &Value, pointer: &str) -> u64 {
    json.pointer(pointer)
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("JSON field {pointer} missing: {json}"))
}

fn assert_event_order(events: [&Value; 3], field: &str, strict: bool) {
    let [first, second, third] = events.map(|event| event_u64(event, field));
    assert!(if strict {
        first < second && second < third
    } else {
        first <= second && second <= third
    });
}

fn assert_resource_counter(json: &Value, suffix: &str, expected: u64) {
    let pointer = format!("/memory_resources/{}", suffix.replace('.', "/"));
    assert_eq!(json_u64(json, &pointer), expected, "{pointer}: {json}");
    assert_json_stat(
        json,
        &format!("sim.memory.resources.{suffix}"),
        "Count",
        expected,
        "monotonic",
    );
}

fn memory_dump_hex(json: &Value, address: u64) -> Option<&str> {
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

fn assert_register(json: &Value, register: &str, expected: &str) {
    assert_eq!(
        json.pointer(&format!("/cores/0/registers/{register}"))
            .and_then(Value::as_str),
        Some(expected),
        "{register}: {json}"
    );
}

fn assert_register_absent(json: &Value, register: &str) {
    assert_eq!(
        json.pointer(&format!("/cores/0/registers/{register}")),
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

fn rob_entry_at_sequence(json: &Value, sequence: u64) -> &Value {
    rob_entries(json)
        .iter()
        .find(|entry| event_u64(entry, "sequence") == sequence)
        .unwrap_or_else(|| panic!("O3 ROB should retain live sequence {sequence}: {json}"))
}

fn assert_rob_sequence_absent(json: &Value, sequence: u64) {
    assert!(
        rob_entries(json)
            .iter()
            .all(|entry| event_u64(entry, "sequence") != sequence),
        "committed result sequence {sequence} must leave the admission ROB: {json}"
    );
}

fn memory_result_event_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
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

fn result_data_record(json: &Value, class: MemoryResultClass) -> &Value {
    let (_, kind, address) = class.request_evidence();
    let mut matches = data_trace(json).iter().filter(|record| {
        event_str(record, "kind") == kind
            && event_str(record, "address") == address
            && event_u64(record, "size") == 8
    });
    let record = matches.next().unwrap_or_else(|| {
        panic!(
            "{} result data request missing: {:?}",
            class.label(),
            data_trace(json)
        )
    });
    assert!(
        matches.next().is_none(),
        "{} result request repeated",
        class.label()
    );
    record
}
