use super::*;

#[path = "live_window_depth/fixture.rs"]
mod fixture;
use fixture::*;

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

fn assert_final_witness<const N: usize>(
    json: &Value,
    expected_memory: &str,
    expected_registers: [(&str, &str); N],
) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host")
    );
    assert_eq!(
        json.pointer("/memory/0/hex").and_then(Value::as_str),
        Some(expected_memory)
    );
    for (register, value) in expected_registers {
        assert_eq!(
            json.pointer(&format!("/cores/0/registers/{register}"))
                .and_then(Value::as_str),
            Some(value)
        );
    }
}

#[test]
fn rem6_run_o3_scalar_live_depth_eight_resides_with_one_lsq_row() {
    let path = scalar_live_window_binary("o3-scalar-live-depth-eight-resident", false);
    let completed = scalar_live_window_json(&path, "direct", 8, 4, 4_000);
    assert_final_witness(
        &completed,
        FINAL_MEMORY,
        [
            ("x5", "0x9"),
            ("x6", "0x6"),
            ("x7", "0x14"),
            ("x8", "0x7"),
            ("x9", "0x1a"),
            ("x14", "0x8"),
            ("x16", "0x21"),
            ("x17", "0x2a"),
        ],
    );
    let response_tick = event_u64(event_at_pc(&completed, LOAD_PC), "lsq_data_response_tick");
    let resident = scalar_live_window_json(&path, "direct", 8, 4, response_tick - 1);
    let rob = resident
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(rob.len(), 8, "resident deep window: {resident}");
    assert_eq!(
        rob.iter()
            .map(|entry| entry.pointer("/pc").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        std::iter::once(LOAD_PC).chain(ROW_PCS).collect::<Vec<_>>()
    );
    assert!(rob
        .iter()
        .all(|entry| entry.pointer("/live_staged").and_then(Value::as_bool) == Some(true)));
    assert_eq!(
        resident
            .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_json_stat(
        &resident,
        "sim.cpu0.o3.max_rob_occupancy",
        "Count",
        8,
        "monotonic",
    );
    assert_json_stat(
        &resident,
        "sim.cpu0.o3.max_lsq_occupancy",
        "Count",
        1,
        "monotonic",
    );
}
