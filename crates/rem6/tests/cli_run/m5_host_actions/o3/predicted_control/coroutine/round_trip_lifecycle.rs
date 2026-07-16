fn assert_coroutine_round_trip_final_state(
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

fn coroutine_o3_events_at_pc<'a>(json: &'a Value, pc: &str, context: &str) -> Vec<&'a Value> {
    let events = json
        .pointer("/debug/o3_trace/0/events")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{context}: missing O3 events: {json}"));
    events
        .iter()
        .filter(|event| event.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .collect()
}

fn exact_coroutine_o3_event_at_pc<'a>(
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

fn exact_coroutine_timing_switch<'a>(json: &'a Value, context: &str) -> &'a Value {
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

fn coroutine_data_trace_counts(json: &Value, context: &str) -> [usize; 2] {
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
    [count("load", DATA_ADDRESS), count("store", SUCCESS_STORE_ADDRESS)]
}

fn assert_coroutine_round_trip_resident_window(
    json: &Value,
    case: CoroutineRoundTripCase,
    context: &str,
) {
    assert_eq!(
        resident_rob_pcs(json),
        [case.load_pc, case.call_pc, case.coroutine_pc, case.return_pc],
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

#[test]
fn rem6_run_host_switch_transfers_o3_same_window_coroutine_round_trip() {
    for case in COROUTINE_ROUND_TRIP_CASES {
        let path = (case.binary)(
            &format!("o3-coroutine-round-trip-switch-{}", case.label),
            0,
        );
        let baseline = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &DIRECT_WIDTH_ARGS,
        );
        let event_pcs = [
            case.load_pc,
            case.call_pc,
            case.coroutine_pc,
            case.return_pc,
            case.success_store_pc,
        ];
        let baseline_events = event_pcs.map(|pc| {
            exact_coroutine_o3_event_at_pc(&baseline, pc, &format!("{} baseline", case.label))
        });
        let load = baseline_events[0];
        let return_jump = baseline_events[3];
        let switch_source_tick = event_u64(return_jump, "issue_tick") + 1;
        let switch_tick = switch_source_tick + 1;
        assert!(
            switch_tick < event_u64(load, "lsq_data_response_tick"),
            "{}: round-trip switch tick must precede load response: load={load}, switch_tick={switch_tick}",
            case.label
        );

        let resident = run_coroutine_json(
            &path,
            case.memory_system,
            switch_source_tick,
            "detailed",
            3,
            &DIRECT_WIDTH_ARGS,
        );
        assert_coroutine_round_trip_resident_window(&resident, case, "switch-source resident");
        for register in ["x1", "x5"] {
            assert_register_absent_or_zero_with_context(&resident, register, case.label);
        }
        for (row_pc, register) in [
            (case.call_pc, case.call_destination),
            (case.coroutine_pc, case.coroutine_destination),
        ] {
            assert_coroutine_lifecycle_rename_maps_to_row_destination(
                &resident,
                row_pc,
                u64::from(register),
                case.label,
            );
        }
        let resident_return = resident
            .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
            .and_then(Value::as_array)
            .and_then(|entries| {
                entries.iter().find(|entry| {
                    entry.pointer("/pc").and_then(Value::as_str) == Some(case.return_pc)
                })
            })
            .unwrap_or_else(|| {
                panic!(
                    "{}: missing resident round-trip return {}: {resident}",
                    case.label, case.return_pc
                )
            });
        assert!(resident_return
            .pointer("/destination")
            .is_some_and(Value::is_null));

        let switch_arg = format!("{switch_source_tick}:cpu0:timing");
        let mut switch_args = DIRECT_WIDTH_ARGS.to_vec();
        switch_args.extend(["--host-switch-cpu-mode", switch_arg.as_str()]);
        let switched = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &switch_args,
        );

        assert_coroutine_lifecycle_stopped_by_host(&switched, case.label);
        assert_coroutine_lifecycle_execution_mode(&switched, "timing", case.label);
        assert_coroutine_round_trip_final_state(&baseline, case, "baseline");
        assert_coroutine_round_trip_final_state(&switched, case, "switched");
        for register in ["x1", "x5"] {
            assert_eq!(
                register_value(&switched, register),
                register_value(&baseline, register),
                "{}: switch must preserve {register}: baseline={baseline}, switched={switched}",
                case.label
            );
        }
        assert_eq!(
            switched.pointer("/memory/0/hex").and_then(Value::as_str),
            baseline.pointer("/memory/0/hex").and_then(Value::as_str),
            "{}: switch must preserve final memory: baseline={baseline}, switched={switched}",
            case.label
        );

        let timing_switch = exact_coroutine_timing_switch(
            &switched,
            &format!("{} live switched", case.label),
        );
        assert_eq!(
            timing_switch.pointer("/tick").and_then(Value::as_u64),
            Some(switch_tick),
            "{}: unexpected round-trip switch tick: {timing_switch}",
            case.label
        );
        let transfer = timing_switch
            .pointer("/state_transfer")
            .unwrap_or_else(|| panic!("{}: missing round-trip state transfer", case.label));
        assert_eq!(
            transfer.pointer("/restorable").and_then(Value::as_bool),
            Some(false),
            "{}: live round-trip transfer must not be restorable: {transfer}",
            case.label
        );
        let runtime = transfer_o3_runtime_chunk_with_context(transfer, "cpu0", case.label);
        assert_eq!(
            runtime
                .pointer("/snapshot_rob_entries")
                .and_then(Value::as_u64),
            Some(4),
            "{}: unexpected transferred ROB snapshot: {runtime}",
            case.label
        );
        assert_eq!(
            runtime
                .pointer("/snapshot_lsq_entries")
                .and_then(Value::as_u64),
            Some(1),
            "{}: unexpected transferred LSQ snapshot: {runtime}",
            case.label
        );
        let handoff =
            transfer_live_data_handoff_chunk_with_context(transfer, "cpu0", case.label);
        for (pointer, expected) in [
            ("/schema_version", 7),
            ("/outstanding_requests", 1),
            ("/resident_rows", 1),
            ("/younger_rows", 3),
            ("/first_target/source_partition", 0),
            ("/first_bytes", 4),
        ] {
            assert_eq!(
                handoff.pointer(pointer).and_then(Value::as_u64),
                Some(expected),
                "{}: unexpected handoff field {pointer}: {handoff}",
                case.label
            );
        }
        for (pointer, expected) in [
            ("/first_operation", "load"),
            ("/first_target/kind", "memory"),
            ("/first_address", DATA_ADDRESS),
        ] {
            assert_eq!(
                handoff.pointer(pointer).and_then(Value::as_str),
                Some(expected),
                "{}: unexpected handoff field {pointer}: {handoff}",
                case.label
            );
        }
        for (index, pc) in event_pcs[..4].iter().copied().enumerate() {
            let expected = baseline_events[index];
            let actual = exact_coroutine_o3_event_at_pc(
                &switched,
                pc,
                &format!("{} live switched", case.label),
            );
            for field in ["issue_tick", "writeback_tick", "commit_tick"] {
                assert_eq!(
                    event_u64(actual, field),
                    event_u64(expected, field),
                    "{}: switch must preserve {field} for {pc}: expected={expected} actual={actual}",
                    case.label
                );
            }
        }
        assert_eq!(
            coroutine_o3_events_at_pc(
                &switched,
                case.success_store_pc,
                &format!("{} live switched", case.label),
            )
            .len(),
            0,
            "{}: timing-executed success store must not be replayed as an O3 event: {switched}",
            case.label
        );
        let baseline_data_counts = coroutine_data_trace_counts(&baseline, case.label);
        assert_eq!(
            baseline_data_counts,
            [1, 1],
            "{}: unexpected baseline [load, success store] data-trace counts",
            case.label
        );
        assert_eq!(
            coroutine_data_trace_counts(&switched, case.label),
            baseline_data_counts,
            "{}: live switch must not replay the architectural load or success store",
            case.label
        );
        let (first_request, response_latency) = match case.memory_system {
            "direct" => (9, 32),
            "cache-fabric-dram" => (11, 34),
            other => panic!("{}: unsupported memory system {other}", case.label),
        };
        let expected_memory = CoroutineMemoryTraceSnapshot {
            kind_counts: [2, 2, 0, 2],
            first_request,
            first_request_kind_counts: [1, 1, 0, 1],
            request_agents: vec![0],
            routes: vec![1],
            response_latencies: vec![response_latency; 2],
        };
        let baseline_memory = coroutine_memory_trace_snapshot(&baseline, case.label);
        assert_eq!(
            baseline_memory, expected_memory,
            "{}: unexpected baseline data-channel memory trace",
            case.label
        );
        assert_eq!(
            coroutine_memory_trace_snapshot(&switched, case.label),
            baseline_memory,
            "{}: live switch must preserve exact data-channel memory events",
            case.label
        );
        let counts = match case.memory_system {
            "direct" => [2, 2, 2, 2, 64, 32],
            "cache-fabric-dram" => [2, 2, 2, 2, 68, 34],
            other => panic!("{}: unsupported memory system {other}", case.label),
        };
        let expected_transport = CoroutineTransportSnapshot {
            route: 1,
            source: "cpu0.dmem",
            aggregate: counts,
            per_route: counts,
        };
        let baseline_transport = coroutine_transport_snapshot(&baseline, case.label);
        assert_eq!(baseline_transport, expected_transport);
        assert_eq!(
            coroutine_transport_snapshot(&switched, case.label),
            baseline_transport,
            "{}: live switch must preserve aggregate and per-route transport",
            case.label
        );

        let opposite_call_kind = match case.call_kind {
            "call_direct" => "call_indirect",
            "call_indirect" => "call_direct",
            other => panic!("{}: unsupported call kind {other}", case.label),
        };
        let predictor_expectations = [
            (format!("/cores/0/branch_predictor/lookups/{}", case.call_kind), 1),
            (format!("/cores/0/branch_predictor/lookups/{opposite_call_kind}"), 0),
            ("/cores/0/branch_predictor/lookups/return".to_owned(), 2),
            ("/cores/0/branch_predictor/lookups/total".to_owned(), 3),
            (format!("/cores/0/branch_predictor/committed/{}", case.call_kind), 1),
            (format!("/cores/0/branch_predictor/committed/{opposite_call_kind}"), 0),
            ("/cores/0/branch_predictor/committed/return".to_owned(), 2),
            ("/cores/0/branch_predictor/committed/total".to_owned(), 3),
            (format!("/cores/0/branch_predictor/squashes/{}", case.call_kind), 0),
            (format!("/cores/0/branch_predictor/squashes/{opposite_call_kind}"), 0),
            ("/cores/0/branch_predictor/squashes/return".to_owned(), 0),
            ("/cores/0/branch_predictor/squashes/total".to_owned(), 0),
            (format!("/cores/0/branch_predictor/mispredicted/{}", case.call_kind), 0),
            (format!("/cores/0/branch_predictor/mispredicted/{opposite_call_kind}"), 0),
            ("/cores/0/branch_predictor/mispredicted/return".to_owned(), 0),
            ("/cores/0/branch_predictor/mispredicted/total".to_owned(), 0),
            ("/cores/0/branch_predictor/target_provider/no_target".to_owned(), case.provider_no_target),
            ("/cores/0/branch_predictor/target_provider/indirect".to_owned(), case.provider_indirect),
            ("/cores/0/branch_predictor/target_provider/btb".to_owned(), 0),
            ("/cores/0/branch_predictor/target_provider/ras".to_owned(), 2),
            ("/cores/0/branch_predictor/target_provider/total".to_owned(), 3),
            ("/cores/0/branch_predictor/ras/pushes".to_owned(), 2),
            ("/cores/0/branch_predictor/ras/pops".to_owned(), 2),
            ("/cores/0/branch_predictor/ras/squashes".to_owned(), 0),
            ("/cores/0/branch_predictor/ras/used".to_owned(), 2),
            ("/cores/0/branch_predictor/ras/correct".to_owned(), 2),
            ("/cores/0/branch_predictor/ras/incorrect".to_owned(), 0),
            ("/cores/0/branch_predictor/indirect_hits".to_owned(), case.provider_indirect),
            ("/cores/0/branch_predictor/indirect_mispredicted".to_owned(), 0),
        ];
        for (pointer, expected) in predictor_expectations {
            let baseline_value = baseline.pointer(&pointer).and_then(Value::as_u64);
            assert_eq!(
                baseline_value,
                Some(expected),
                "{}: unexpected baseline counter {pointer}: {baseline}",
                case.label
            );
            assert_eq!(
                switched.pointer(&pointer).and_then(Value::as_u64),
                baseline_value,
                "{}: switch must preserve {pointer}: baseline={baseline}, switched={switched}",
                case.label
            );
        }
        assert_coroutine_lifecycle_runtime_drained(&switched, case.label);
    }
}

#[test]
fn rem6_run_o3_same_window_coroutine_round_trip_checkpoint_boundary() {
    for case in COROUTINE_ROUND_TRIP_CASES {
        let path = (case.binary)(
            &format!("o3-coroutine-round-trip-checkpoint-{}", case.label),
            8,
        );
        let baseline = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &DIRECT_WIDTH_ARGS,
        );
        assert_coroutine_round_trip_final_state(&baseline, case, "baseline");
        let load = exact_coroutine_o3_event_at_pc(
            &baseline,
            case.load_pc,
            &format!("{} checkpoint baseline", case.label),
        );
        let live_checkpoint_source_tick = event_u64(
            exact_coroutine_o3_event_at_pc(
                &baseline,
                case.return_pc,
                &format!("{} checkpoint baseline", case.label),
            ),
            "issue_tick",
        ) + 1;
        let live_checkpoint_tick = live_checkpoint_source_tick + 1;
        assert!(
            live_checkpoint_tick < event_u64(load, "lsq_data_response_tick"),
            "{}: live checkpoint delivery tick must precede load response: load={load}, live_checkpoint_tick={live_checkpoint_tick}",
            case.label
        );

        let live_resident = run_coroutine_json(
            &path,
            case.memory_system,
            live_checkpoint_tick,
            "detailed",
            3,
            &DIRECT_WIDTH_ARGS,
        );
        assert_coroutine_round_trip_resident_window(
            &live_resident,
            case,
            "live-checkpoint delivery",
        );

        let live_arg =
            format!("{live_checkpoint_source_tick}:coroutine-round-trip-live");
        let mut live_command = control_window_command(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            DATA_ADDRESS,
            16,
        );
        let mut live_args = DIRECT_WIDTH_ARGS.to_vec();
        live_args.extend(["--host-checkpoint", live_arg.as_str()]);
        live_command.args(live_args.iter().copied());
        let output = live_command.output().unwrap_or_else(|error| {
            panic!("{}: failed to run live checkpoint: {error}", case.label)
        });
        assert!(
            !output.status.success(),
            "{}: live round-trip checkpoint unexpectedly succeeded",
            case.label
        );
        assert!(
            output.stdout.is_empty(),
            "{}: live checkpoint emitted stdout: {}",
            case.label,
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("checkpoint component is not quiescent: cpu0"),
            "{}: live checkpoint should fail closed: {stderr}",
            case.label
        );

        let checkpoint_source_tick =
            event_u64(
                exact_coroutine_o3_event_at_pc(
                    &baseline,
                    case.success_store_pc,
                    &format!("{} checkpoint baseline", case.label),
                ),
                "commit_tick",
            ) + 1;
        let checkpoint_tick = checkpoint_source_tick + 1;
        let restore_source_tick = checkpoint_source_tick + 1;
        let restore_tick = restore_source_tick + 1;
        let checkpoint_arg =
            format!("{checkpoint_source_tick}:coroutine-round-trip-drained");
        let restore_arg = format!("{restore_source_tick}:coroutine-round-trip-drained");
        let mut restore_args = DIRECT_WIDTH_ARGS.to_vec();
        restore_args.extend([
            "--host-checkpoint",
            checkpoint_arg.as_str(),
            "--host-restore-checkpoint",
            restore_arg.as_str(),
        ]);
        let restored = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            3,
            &restore_args,
        );

        assert_coroutine_lifecycle_stopped_by_host(&restored, case.label);
        assert_coroutine_round_trip_final_state(&restored, case, "restored");
        for register in ["x1", "x5"] {
            assert_eq!(
                register_value(&restored, register),
                register_value(&baseline, register),
                "{}: restore must preserve {register}: baseline={baseline}, restored={restored}",
                case.label
            );
        }
        assert_eq!(
            restored.pointer("/memory/0/hex").and_then(Value::as_str),
            baseline.pointer("/memory/0/hex").and_then(Value::as_str),
            "{}: restore must preserve memory: baseline={baseline}, restored={restored}",
            case.label
        );
        assert_eq!(
            restored
                .pointer("/host_actions/checkpoint_count")
                .and_then(Value::as_u64),
            Some(1),
            "{}: expected one checkpoint: {restored}",
            case.label
        );
        assert_eq!(
            restored
                .pointer("/host_actions/checkpoint_restored_count")
                .and_then(Value::as_u64),
            Some(1),
            "{}: expected one restore: {restored}",
            case.label
        );
        let checkpoints = restored
            .pointer("/host_actions/checkpoints")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{}: missing drained checkpoints: {restored}", case.label));
        assert_eq!(
            checkpoints.len(),
            1,
            "{}: expected exactly one drained checkpoint: {checkpoints:?}",
            case.label
        );
        let checkpoint = &checkpoints[0];
        assert_eq!(
            checkpoint.pointer("/tick").and_then(Value::as_u64),
            Some(checkpoint_tick),
            "{}: unexpected drained checkpoint tick: {checkpoint}",
            case.label
        );
        assert_eq!(
            checkpoint.pointer("/label").and_then(Value::as_str),
            Some("coroutine-round-trip-drained"),
            "{}: unexpected drained checkpoint label: {checkpoint}",
            case.label
        );
        let checkpoint_restores = restored
            .pointer("/host_actions/checkpoint_restores")
            .and_then(Value::as_array)
            .unwrap_or_else(|| {
                panic!("{}: missing drained checkpoint restores: {restored}", case.label)
            });
        assert_eq!(
            checkpoint_restores.len(),
            1,
            "{}: expected exactly one drained restore: {checkpoint_restores:?}",
            case.label
        );
        let checkpoint_restore = &checkpoint_restores[0];
        assert_eq!(
            checkpoint_restore.pointer("/tick").and_then(Value::as_u64),
            Some(restore_tick),
            "{}: unexpected drained restore tick: {checkpoint_restore}",
            case.label
        );
        assert_eq!(
            checkpoint_restore.pointer("/label").and_then(Value::as_str),
            Some("coroutine-round-trip-drained"),
            "{}: unexpected drained restore label: {checkpoint_restore}",
            case.label
        );
        let cpu0 = checkpoint_component_with_context(checkpoint, "cpu0", case.label);
        let chunks = checkpoint_component_chunks_with_context(cpu0, case.label);
        assert!(
            chunks.iter().all(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str)
                    != Some("o3-live-data-handoff")
            }),
            "{}: drained checkpoint retained live handoff: {cpu0}",
            case.label
        );
        let runtime_chunk =
            checkpoint_component_chunk_with_context(chunks, "o3-runtime-state", case.label);
        let runtime = runtime_chunk
            .pointer("/o3_runtime")
            .unwrap_or_else(|| panic!("{}: missing decoded O3 runtime: {cpu0}", case.label));
        assert_eq!(
            runtime
                .pointer("/snapshot_rob_entries")
                .and_then(Value::as_u64),
            Some(0),
            "{}: drained checkpoint retained ROB rows: {runtime}",
            case.label
        );
        assert_eq!(
            runtime
                .pointer("/snapshot_lsq_entries")
                .and_then(Value::as_u64),
            Some(0),
            "{}: drained checkpoint retained LSQ rows: {runtime}",
            case.label
        );
        assert_coroutine_lifecycle_runtime_drained(&restored, case.label);
    }
}

#[test]
fn rem6_run_timing_suppresses_o3_same_window_coroutine_round_trip() {
    for case in COROUTINE_ROUND_TRIP_CASES {
        let path = (case.binary)(
            &format!("o3-coroutine-round-trip-timing-{}", case.label),
            0,
        );
        let timing = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "timing",
            3,
            &[],
        );

        assert_coroutine_lifecycle_stopped_by_host(&timing, case.label);
        assert_coroutine_lifecycle_execution_mode(&timing, "timing", case.label);
        assert_coroutine_round_trip_final_state(&timing, case, "timing");
        assert!(
            timing.pointer("/cores/0/o3_runtime").is_none(),
            "{}: timing mode exposed an O3 runtime: {timing}",
            case.label
        );
        assert!(
            timing
                .pointer("/debug/o3_trace")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "{}: timing mode must keep an empty O3 trace: {timing}",
            case.label
        );
        assert_no_o3_stats_with_context(&timing, case.label);
        for path in [
            "sim.debug.o3_trace.records",
            "sim.debug.o3_trace.instructions",
            "sim.debug.o3_trace.max_rob_occupancy",
            "sim.debug.o3_trace.max_lsq_occupancy",
            "sim.debug.o3_trace.execution_mode.timing",
            "sim.debug.o3_trace.execution_mode.detailed",
            "sim.debug.o3_trace.execution_mode_authority.mode.timing",
        ] {
            assert_json_stat(&timing, path, "Count", 0, "monotonic");
        }
    }
}
