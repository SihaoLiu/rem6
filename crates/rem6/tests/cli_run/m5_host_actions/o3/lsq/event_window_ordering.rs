use super::*;

#[test]
fn rem6_run_o3_event_window_preserves_lsq_ordering_provenance() {
    for (ordering, acquire, release, memory_system, max_tick) in [
        ("acquire", true, false, "direct", "180"),
        ("release", false, true, "direct", "180"),
        ("acquire_release", true, true, "direct", "180"),
        ("acquire_release", true, true, "cache-fabric-dram", "280"),
    ] {
        let path = detailed_o3_event_window_ordering_binary(
            &format!("m5-switch-cpu-o3-event-window-ordering-{ordering}-{memory_system}"),
            acquire,
            release,
        );
        let json = event_window_ordering_json(&path, memory_system, max_tick);

        assert_eq!(
            json.pointer("/simulation/memory_system")
                .and_then(Value::as_str),
            Some(memory_system)
        );
        assert_eq!(
            json.pointer("/memory/0/hex").and_then(Value::as_str),
            Some("0400000000000000"),
            "ordered AMO should update memory through {memory_system}: {json}"
        );

        let events = json
            .pointer("/debug/o3_trace/0/events")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("O3 debug trace should expose events: {json}"));
        let atomic_events = events
            .iter()
            .filter(|event| {
                event.pointer("/lsq_operation").and_then(Value::as_str) == Some("atomic")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            atomic_events.len(),
            1,
            "O3 debug trace should expose exactly one AMO event: {events:?}"
        );
        let atomic = atomic_events[0];
        assert_eq!(
            atomic.pointer("/lsq_ordering").and_then(Value::as_str),
            Some(ordering),
            "raw AMO event should retain its ordering lane: {atomic}"
        );

        for pointer in [
            "/cores/0/o3_runtime/event_window/max_lsq_data_latency/lsq_ordering",
            "/cores/0/o3_runtime/event_summary/event_window/max_lsq_data_latency/lsq_ordering",
            "/debug/o3_trace/0/event_summary/event_window/max_lsq_data_latency/lsq_ordering",
        ] {
            assert_eq!(
                json.pointer(pointer).and_then(Value::as_str),
                Some(ordering),
                "selected O3 event-window row should retain ordering at {pointer}: {json}"
            );
        }

        for prefix in [
            "sim.cpu0.o3.event_window.max_lsq_data_latency.lsq_ordering",
            "sim.cpu0.o3.event_summary.event_window.max_lsq_data_latency.lsq_ordering",
            "sim.debug.o3_trace.event_window.max_lsq_data_latency.lsq_ordering",
            "sim.debug.o3_trace.cpu.cpu0.event_window.max_lsq_data_latency.lsq_ordering",
        ] {
            assert_ordering_stat_lanes(&json, prefix, ordering);
        }

        let dump = json
            .pointer("/host_actions/stats_dumps/0")
            .unwrap_or_else(|| panic!("ordered AMO run should emit one stats dump: {json}"));
        for lane in ["acquire", "release", "acquire_release"] {
            assert_stats_dump_sample(
                dump,
                &format!(
                    "sim.host_actions.stats_dump.cpu0.o3.event_window.max_lsq_data_latency.lsq_ordering.{lane}"
                ),
                "counter",
                "Count",
                u64::from(lane == ordering),
                "resettable",
            );
        }

        if memory_system == "cache-fabric-dram" {
            for pointer in [
                "/memory_resources/transport/data/activity",
                "/memory_resources/fabric/activity",
                "/memory_resources/dram/activity",
            ] {
                assert!(
                    json.pointer(pointer)
                        .and_then(Value::as_u64)
                        .is_some_and(|value| value > 0),
                    "hierarchy-backed ordered AMO should expose resource activity at {pointer}: {json}"
                );
            }
            assert_eq!(
                json.pointer("/memory_resources/cache/data/activity")
                    .and_then(Value::as_u64),
                Some(0),
                "ordered AMOs should preserve the current data-cache bypass boundary: {json}"
            );
        }
    }
}

fn event_window_ordering_json(path: &Path, memory_system: &str, max_tick: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            max_tick,
            "--stats-format",
            "json",
            "--execute",
            "--debug-flags",
            "O3",
            "--memory-system",
            memory_system,
            "--dump-memory",
            "0x80000040:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("invalid stdout JSON: {error}"))
}

fn assert_ordering_stat_lanes(json: &Value, prefix: &str, ordering: &str) {
    for lane in ["acquire", "release", "acquire_release"] {
        assert_json_stat(
            json,
            &format!("{prefix}.{lane}"),
            "Count",
            u64::from(lane == ordering),
            "monotonic",
        );
    }
}
