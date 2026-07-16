#[test]
fn rem6_run_host_switch_transfers_o3_same_window_coroutine() {
    for case in COROUTINE_LIFECYCLE_CASES {
        let path = (case.binary)(
            &format!("o3-same-window-coroutine-switch-{}", case.label),
            0,
        );
        let baseline = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            2,
            &DIRECT_WIDTH_ARGS,
        );
        let load = event_at_pc(&baseline, case.load_pc);
        let switch_tick = event_u64(event_at_pc(&baseline, case.descendant_pc), "issue_tick") + 1;
        assert!(
            switch_tick < event_u64(load, "lsq_data_response_tick"),
            "{}: coroutine switch tick must precede load response: load={load}, switch_tick={switch_tick}",
            case.label
        );

        let resident = run_coroutine_json(
            &path,
            case.memory_system,
            switch_tick,
            "detailed",
            2,
            &DIRECT_WIDTH_ARGS,
        );
        assert_eq!(
            resident_rob_pcs(&resident),
            [
                case.load_pc,
                case.call_pc,
                case.coroutine_pc,
                case.descendant_pc,
            ],
            "{}: unexpected resident ROB at switch tick {switch_tick}: {resident}",
            case.label
        );
        assert_eq!(
            resident
                .pointer("/cores/0/o3_runtime/snapshot/lsq/count")
                .and_then(Value::as_u64),
            Some(1),
            "{}: expected one resident LSQ row at switch tick {switch_tick}: {resident}",
            case.label
        );
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

        let switch_arg = format!("{switch_tick}:cpu0:timing");
        let mut switch_args = DIRECT_WIDTH_ARGS.to_vec();
        switch_args.extend(["--host-switch-cpu-mode", switch_arg.as_str()]);
        let switched = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            2,
            &switch_args,
        );

        assert_coroutine_lifecycle_stopped_by_host(&switched, case.label);
        assert_coroutine_lifecycle_execution_mode(&switched, "timing", case.label);
        assert_coroutine_lifecycle_final_state(&baseline, case, "baseline");
        assert_coroutine_lifecycle_final_state(&switched, case, "switched");
        for register in ["x1", "x5", "x13"] {
            assert_eq!(
                register_value(&switched, register),
                register_value(&baseline, register),
                "{}: coroutine switch must preserve {register}: baseline={baseline}, switched={switched}",
                case.label
            );
        }
        assert_eq!(
            switched.pointer("/memory/0/hex").and_then(Value::as_str),
            baseline.pointer("/memory/0/hex").and_then(Value::as_str),
            "{}: coroutine switch must preserve final memory: baseline={baseline}, switched={switched}",
            case.label
        );
        assert_coroutine_lifecycle_no_data_address(
            &switched,
            WRONG_STORE_ADDRESS,
            case.label,
        );

        let timing_switch = switched
            .pointer("/host_actions/execution_mode_switches")
            .and_then(Value::as_array)
            .and_then(|switches| {
                switches.iter().find(|switch| {
                    switch.pointer("/target").and_then(Value::as_str) == Some("cpu0")
                        && switch.pointer("/mode").and_then(Value::as_str) == Some("timing")
                        && switch.pointer("/previous_mode").and_then(Value::as_str)
                            == Some("detailed")
                })
            })
            .unwrap_or_else(|| {
                panic!(
                    "{}: missing same-window coroutine detailed-to-timing switch: {switched}",
                    case.label
                )
            });
        let transfer = timing_switch
            .pointer("/state_transfer")
            .unwrap_or_else(|| panic!("{}: missing coroutine state transfer", case.label));
        assert_eq!(
            transfer.pointer("/restorable").and_then(Value::as_bool),
            Some(false),
            "{}: live coroutine transfer must not be restorable: {transfer}",
            case.label
        );
        let runtime = transfer_o3_runtime_chunk(transfer, "cpu0");
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
        let handoff = transfer_live_data_handoff_chunk(transfer, "cpu0");
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
                "{}: unexpected coroutine handoff field {pointer}: {handoff}",
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
                "{}: unexpected coroutine handoff field {pointer}: {handoff}",
                case.label
            );
        }
        for pc in [
            case.load_pc,
            case.call_pc,
            case.coroutine_pc,
            case.descendant_pc,
        ] {
            let expected = event_at_pc(&baseline, pc);
            let actual = event_at_pc(&switched, pc);
            for field in ["issue_tick", "writeback_tick", "commit_tick"] {
                assert_eq!(
                    event_u64(actual, field),
                    event_u64(expected, field),
                    "{}: coroutine transfer must preserve {field} for {pc}: expected={expected} actual={actual}",
                    case.label
                );
            }
        }

        let opposite_call_kind = match case.call_kind {
            "call_direct" => "call_indirect",
            "call_indirect" => "call_direct",
            other => panic!("{}: unsupported lifecycle call kind {other}", case.label),
        };
        let predictor_expectations = [
            (
                format!(
                    "/cores/0/branch_predictor/lookups/{}",
                    case.call_kind
                ),
                1,
            ),
            (
                format!(
                    "/cores/0/branch_predictor/lookups/{opposite_call_kind}"
                ),
                0,
            ),
            (
                "/cores/0/branch_predictor/lookups/return".to_owned(),
                1,
            ),
            (
                format!(
                    "/cores/0/branch_predictor/committed/{}",
                    case.call_kind
                ),
                1,
            ),
            (
                format!(
                    "/cores/0/branch_predictor/committed/{opposite_call_kind}"
                ),
                0,
            ),
            (
                "/cores/0/branch_predictor/committed/return".to_owned(),
                1,
            ),
            (
                format!(
                    "/cores/0/branch_predictor/squashes/{}",
                    case.call_kind
                ),
                0,
            ),
            (
                format!(
                    "/cores/0/branch_predictor/squashes/{opposite_call_kind}"
                ),
                0,
            ),
            (
                "/cores/0/branch_predictor/squashes/return".to_owned(),
                0,
            ),
            (
                "/cores/0/branch_predictor/target_provider/no_target".to_owned(),
                case.provider_no_target,
            ),
            (
                "/cores/0/branch_predictor/target_provider/indirect".to_owned(),
                case.provider_indirect,
            ),
            (
                "/cores/0/branch_predictor/target_provider/btb".to_owned(),
                0,
            ),
            (
                "/cores/0/branch_predictor/target_provider/ras".to_owned(),
                1,
            ),
            (
                "/cores/0/branch_predictor/target_provider/total".to_owned(),
                2,
            ),
            (
                "/cores/0/branch_predictor/ras/pushes".to_owned(),
                2,
            ),
            ("/cores/0/branch_predictor/ras/pops".to_owned(), 1),
            (
                "/cores/0/branch_predictor/ras/squashes".to_owned(),
                0,
            ),
            ("/cores/0/branch_predictor/ras/used".to_owned(), 1),
            (
                "/cores/0/branch_predictor/ras/correct".to_owned(),
                1,
            ),
            (
                "/cores/0/branch_predictor/ras/incorrect".to_owned(),
                0,
            ),
            (
                "/cores/0/branch_predictor/indirect_hits".to_owned(),
                case.provider_indirect,
            ),
        ];
        for (pointer, expected) in predictor_expectations {
            let baseline_value = baseline.pointer(&pointer).and_then(Value::as_u64);
            assert_eq!(
                baseline_value,
                Some(expected),
                "{}: unexpected baseline coroutine counter {pointer}: {baseline}",
                case.label
            );
            assert_eq!(
                switched.pointer(&pointer).and_then(Value::as_u64),
                baseline_value,
                "{}: coroutine transfer must preserve {pointer}: baseline={baseline}, switched={switched}",
                case.label
            );
        }
        assert_coroutine_lifecycle_runtime_drained(&switched, case.label);
    }
}

#[test]
fn rem6_run_o3_same_window_coroutine_checkpoint_boundary() {
    for case in COROUTINE_LIFECYCLE_CASES {
        let path = (case.binary)(
            &format!("o3-same-window-coroutine-checkpoint-{}", case.label),
            8,
        );
        let baseline = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            2,
            &DIRECT_WIDTH_ARGS,
        );
        assert_coroutine_lifecycle_final_state(&baseline, case, "baseline");
        let load = event_at_pc(&baseline, case.load_pc);
        let live_tick = event_u64(event_at_pc(&baseline, case.descendant_pc), "issue_tick") + 1;
        assert!(
            live_tick < event_u64(load, "lsq_data_response_tick"),
            "{}: coroutine checkpoint live tick must precede load response: load={load}, live_tick={live_tick}",
            case.label
        );

        let live_arg = format!("{live_tick}:coroutine-live");
        let mut live_command = control_window_command(
            &path,
            case.memory_system,
            case.max_tick,
            "detailed",
            2,
            DATA_ADDRESS,
            16,
        );
        let mut live_args = DIRECT_WIDTH_ARGS.to_vec();
        live_args.extend(["--host-checkpoint", live_arg.as_str()]);
        live_command.args(live_args.iter().copied());
        let output = live_command.output().unwrap_or_else(|error| {
            panic!(
                "{}: failed to run live coroutine checkpoint command: {error}",
                case.label
            )
        });
        assert!(
            !output.status.success(),
            "{}: live coroutine checkpoint unexpectedly succeeded",
            case.label
        );
        assert!(
            output.stdout.is_empty(),
            "{}: live coroutine checkpoint emitted stdout: {}",
            case.label,
            String::from_utf8_lossy(&output.stdout)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("checkpoint component is not quiescent: cpu0"),
            "{}: live coroutine checkpoint should fail closed: {stderr}",
            case.label
        );

        let checkpoint_tick =
            event_u64(event_at_pc(&baseline, case.success_store_pc), "commit_tick") + 1;
        let restore_tick = checkpoint_tick + 1;
        let checkpoint_arg = format!("{checkpoint_tick}:coroutine-drained");
        let restore_arg = format!("{restore_tick}:coroutine-drained");
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
            2,
            &restore_args,
        );

        assert_coroutine_lifecycle_stopped_by_host(&restored, case.label);
        assert_coroutine_lifecycle_final_state(&restored, case, "restored");
        for register in ["x1", "x5", "x13"] {
            assert_eq!(
                register_value(&restored, register),
                register_value(&baseline, register),
                "{}: coroutine checkpoint restore must preserve {register}: baseline={baseline}, restored={restored}",
                case.label
            );
        }
        assert_eq!(
            restored.pointer("/memory/0/hex").and_then(Value::as_str),
            baseline.pointer("/memory/0/hex").and_then(Value::as_str),
            "{}: coroutine checkpoint restore must preserve final memory: baseline={baseline}, restored={restored}",
            case.label
        );
        assert_coroutine_lifecycle_no_data_address(
            &restored,
            WRONG_STORE_ADDRESS,
            case.label,
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
            "{}: expected one checkpoint restore: {restored}",
            case.label
        );
        let checkpoint = restored
            .pointer("/host_actions/checkpoints/0")
            .unwrap_or_else(|| panic!("{}: missing drained coroutine checkpoint", case.label));
        let cpu0 = checkpoint_component(checkpoint, "cpu0");
        let chunks = checkpoint_component_chunks(cpu0);
        assert!(
            chunks.iter().all(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str)
                    != Some("o3-live-data-handoff")
            }),
            "{}: drained checkpoint must not contain a live-data handoff: {cpu0}",
            case.label
        );
        let runtime = chunks
            .iter()
            .find(|chunk| {
                chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state")
            })
            .and_then(|chunk| chunk.pointer("/o3_runtime"))
            .unwrap_or_else(|| {
                panic!(
                    "{}: missing decoded drained coroutine O3 runtime checkpoint: {cpu0}",
                    case.label
                )
            });
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
fn rem6_run_timing_suppresses_o3_same_window_coroutine() {
    for case in COROUTINE_LIFECYCLE_CASES {
        let path = (case.binary)(
            &format!("o3-same-window-coroutine-timing-{}", case.label),
            0,
        );
        let timing = run_coroutine_json(
            &path,
            case.memory_system,
            case.max_tick,
            "timing",
            2,
            &[],
        );

        assert_coroutine_lifecycle_stopped_by_host(&timing, case.label);
        assert_coroutine_lifecycle_execution_mode(&timing, "timing", case.label);
        assert_coroutine_lifecycle_final_state(&timing, case, "timing");
        assert_coroutine_lifecycle_no_data_address(&timing, WRONG_STORE_ADDRESS, case.label);
        assert!(
            timing.pointer("/cores/0/o3_runtime").is_none(),
            "{}: timing mode exposed an O3 runtime snapshot: {timing}",
            case.label
        );
        assert!(
            timing
                .pointer("/debug/o3_trace")
                .and_then(Value::as_array)
                .is_some_and(Vec::is_empty),
            "{}: timing mode must preserve an empty O3 trace schema: {timing}",
            case.label
        );
        assert_no_o3_stats_with_context(&timing, case.label);
    }
}

fn assert_coroutine_lifecycle_stopped_by_host(json: &Value, label: &str) {
    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("stopped_by_host"),
        "{label}: coroutine lifecycle run did not stop by host: {json}"
    );
}

fn assert_coroutine_lifecycle_execution_mode(json: &Value, expected: &str, label: &str) {
    let execution_modes = json
        .pointer("/host_actions/execution_modes")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{label}: missing final execution mode: {json}"));
    assert_eq!(
        execution_modes.len(),
        1,
        "{label}: unexpected final execution mode count: {execution_modes:?}"
    );
    assert_eq!(
        execution_modes[0]
            .pointer("/target")
            .and_then(Value::as_str),
        Some("cpu0"),
        "{label}: unexpected final execution mode target: {execution_modes:?}"
    );
    assert_eq!(
        execution_modes[0].pointer("/mode").and_then(Value::as_str),
        Some(expected),
        "{label}: unexpected final execution mode: {execution_modes:?}"
    );
}

fn assert_coroutine_lifecycle_final_state(
    json: &Value,
    case: CoroutineLifecycleCase,
    context: &str,
) {
    for (register, expected) in [
        ("x1", case.final_x1),
        ("x5", case.final_x5),
        ("x13", case.final_x13),
    ] {
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
        "{}: unexpected {context} coroutine memory: {json}",
        case.label
    );
}

fn assert_coroutine_lifecycle_rename_maps_to_row_destination(
    json: &Value,
    row_pc: &str,
    register: u64,
    label: &str,
) {
    let rows = json
        .pointer("/cores/0/o3_runtime/snapshot/rob/entries")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{label}: missing resident coroutine ROB: {json}"));
    let row = rows
        .iter()
        .find(|entry| entry.pointer("/pc").and_then(Value::as_str) == Some(row_pc))
        .unwrap_or_else(|| panic!("{label}: missing resident integer row {row_pc}: {json}"));
    let destination = row
        .pointer("/destination")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            panic!("{label}: integer row {row_pc} should own a destination: {row}")
        });
    let rename_entry = json
        .pointer("/cores/0/o3_runtime/snapshot/rename_map/entries")
        .and_then(Value::as_array)
        .and_then(|entries| {
            entries.iter().find(|entry| {
                entry.pointer("/register_class").and_then(Value::as_str) == Some("integer")
                    && entry.pointer("/architectural").and_then(Value::as_u64) == Some(register)
            })
        })
        .unwrap_or_else(|| panic!("{label}: missing live rename for x{register}: {json}"));
    assert_eq!(
        rename_entry.pointer("/physical").and_then(Value::as_u64),
        Some(destination),
        "{label}: x{register} should map to the destination owned by {row_pc}"
    );
}

fn assert_coroutine_lifecycle_no_data_address(json: &Value, address: &str, label: &str) {
    for pointer in ["/debug/data_trace", "/debug/memory_trace"] {
        assert!(
            json.pointer(pointer)
                .and_then(Value::as_array)
                .is_some_and(|records| records.iter().all(|record| {
                    record.pointer("/address").and_then(Value::as_str) != Some(address)
                })),
            "{label}: unexpected data access at {address} in {pointer}: {json}"
        );
    }
}

fn assert_coroutine_lifecycle_runtime_drained(json: &Value, label: &str) {
    for pointer in [
        "/cores/0/o3_runtime/snapshot/rob/count",
        "/cores/0/o3_runtime/snapshot/lsq/count",
    ] {
        assert_eq!(
            json.pointer(pointer).and_then(Value::as_u64),
            Some(0),
            "{label}: final coroutine runtime was not drained at {pointer}: {json}"
        );
    }
}
