#[test]
fn rem6_run_host_switch_transfers_o3_same_window_coroutine() {
    let path = direct_coroutine_binary("o3-same-window-coroutine-switch", 0);
    let baseline = run_coroutine_json(&path, "direct", 2_500, "detailed", 2, &DIRECT_WIDTH_ARGS);
    let load = event_at_pc(&baseline, "0x8000000c");
    let switch_tick = event_u64(event_at_pc(&baseline, "0x80000014"), "issue_tick") + 1;
    assert!(
        switch_tick < event_u64(load, "lsq_data_response_tick"),
        "coroutine switch tick must precede load response: load={load}, switch_tick={switch_tick}"
    );

    let switch_arg = format!("{switch_tick}:cpu0:timing");
    let mut switch_args = DIRECT_WIDTH_ARGS.to_vec();
    switch_args.extend(["--host-switch-cpu-mode", switch_arg.as_str()]);
    let switched = run_coroutine_json(&path, "direct", 2_500, "detailed", 2, &switch_args);

    assert_stopped_by_host(&switched);
    assert_final_execution_mode(&switched, "timing");
    assert_eq!(register_value(&switched, "x1"), 0x8000_0014);
    assert_eq!(register_value(&switched, "x5"), 0x8000_0020);
    assert_eq!(register_value(&switched, "x13"), 0x8000_0020);
    assert_eq!(
        switched.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000200000800000000000000000"),
        "unexpected switched coroutine memory: {switched}"
    );
    assert_eq!(
        switched.pointer("/memory/0/hex").and_then(Value::as_str),
        baseline.pointer("/memory/0/hex").and_then(Value::as_str),
        "coroutine switch must preserve final memory: baseline={baseline}, switched={switched}"
    );
    assert_no_data_address(&switched, WRONG_STORE_ADDRESS);

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
        .unwrap_or_else(|| panic!("missing same-window coroutine timing switch: {switched}"));
    let transfer = timing_switch
        .pointer("/state_transfer")
        .expect("same-window coroutine state transfer");
    assert_eq!(
        transfer.pointer("/restorable").and_then(Value::as_bool),
        Some(false)
    );
    let runtime = transfer_o3_runtime_chunk(transfer, "cpu0");
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
        Some(1)
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
            "unexpected coroutine handoff field {pointer}: {handoff}"
        );
    }
    assert_eq!(
        handoff.pointer("/first_operation").and_then(Value::as_str),
        Some("load")
    );
    assert_eq!(
        handoff
            .pointer("/first_target/kind")
            .and_then(Value::as_str),
        Some("memory")
    );
    assert_eq!(
        handoff.pointer("/first_address").and_then(Value::as_str),
        Some(DATA_ADDRESS)
    );
    for pc in ["0x8000000c", "0x80000010", "0x8000001c", "0x80000014"] {
        let expected = event_at_pc(&baseline, pc);
        let actual = event_at_pc(&switched, pc);
        for field in ["issue_tick", "writeback_tick", "commit_tick"] {
            assert_eq!(
                event_u64(actual, field),
                event_u64(expected, field),
                "coroutine transfer must preserve {field} for {pc}: expected={expected} actual={actual}"
            );
        }
    }
    for (pointer, expected) in [
        ("/cores/0/branch_predictor/lookups/call_direct", 1),
        ("/cores/0/branch_predictor/lookups/return", 1),
        ("/cores/0/branch_predictor/committed/call_direct", 1),
        ("/cores/0/branch_predictor/committed/return", 1),
        ("/cores/0/branch_predictor/squashes/call_direct", 0),
        ("/cores/0/branch_predictor/squashes/return", 0),
        ("/cores/0/branch_predictor/ras/pushes", 2),
        ("/cores/0/branch_predictor/ras/pops", 1),
        ("/cores/0/branch_predictor/ras/squashes", 0),
        ("/cores/0/branch_predictor/ras/used", 1),
        ("/cores/0/branch_predictor/ras/correct", 1),
        ("/cores/0/branch_predictor/ras/incorrect", 0),
        ("/cores/0/branch_predictor/target_provider/ras", 1),
    ] {
        let baseline_value = baseline.pointer(pointer).and_then(Value::as_u64);
        assert_eq!(
            baseline_value,
            Some(expected),
            "unexpected baseline coroutine counter {pointer}: {baseline}"
        );
        assert_eq!(
            switched.pointer(pointer).and_then(Value::as_u64),
            baseline_value,
            "coroutine transfer must preserve {pointer}: baseline={baseline}, switched={switched}"
        );
    }
    assert_drained_control_runtime(&switched);
}

#[test]
fn rem6_run_o3_same_window_coroutine_checkpoint_boundary() {
    let path = direct_coroutine_binary("o3-same-window-coroutine-checkpoint", 8);
    let baseline = run_coroutine_json(&path, "direct", 2_500, "detailed", 2, &DIRECT_WIDTH_ARGS);
    let load = event_at_pc(&baseline, "0x8000000c");
    let live_tick = event_u64(event_at_pc(&baseline, "0x80000014"), "issue_tick") + 1;
    assert!(
        live_tick < event_u64(load, "lsq_data_response_tick"),
        "coroutine checkpoint live tick must precede load response: load={load}, live_tick={live_tick}"
    );

    let live_arg = format!("{live_tick}:coroutine-live");
    let mut live_command =
        control_window_command(&path, "direct", 2_500, "detailed", 2, DATA_ADDRESS, 16);
    let mut live_args = DIRECT_WIDTH_ARGS.to_vec();
    live_args.extend(["--host-checkpoint", live_arg.as_str()]);
    live_command.args(live_args.iter().copied());
    let output = live_command.output().unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("checkpoint component is not quiescent: cpu0"),
        "live coroutine checkpoint should fail closed: {stderr}"
    );

    let checkpoint_tick = event_u64(event_at_pc(&baseline, "0x80000028"), "commit_tick") + 1;
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
    let restored = run_coroutine_json(&path, "direct", 2_500, "detailed", 2, &restore_args);

    assert_stopped_by_host(&restored);
    assert_eq!(register_value(&restored, "x1"), 0x8000_0014);
    assert_eq!(register_value(&restored, "x5"), 0x8000_0020);
    assert_eq!(register_value(&restored, "x13"), 0x8000_0020);
    assert_eq!(
        restored.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000200000800000000000000000"),
        "unexpected restored coroutine memory: {restored}"
    );
    assert_eq!(
        restored.pointer("/memory/0/hex").and_then(Value::as_str),
        baseline.pointer("/memory/0/hex").and_then(Value::as_str),
        "coroutine checkpoint restore must preserve final memory: baseline={baseline}, restored={restored}"
    );
    assert_no_data_address(&restored, WRONG_STORE_ADDRESS);
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        restored
            .pointer("/host_actions/checkpoint_restored_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let checkpoint = restored
        .pointer("/host_actions/checkpoints/0")
        .expect("drained coroutine checkpoint");
    let cpu0 = checkpoint_component(checkpoint, "cpu0");
    assert!(checkpoint_component_chunks(cpu0).iter().all(|chunk| {
        chunk.pointer("/name").and_then(Value::as_str) != Some("o3-live-data-handoff")
    }));
    let runtime = checkpoint_component_chunks(cpu0)
        .iter()
        .find(|chunk| chunk.pointer("/name").and_then(Value::as_str) == Some("o3-runtime-state"))
        .and_then(|chunk| chunk.pointer("/o3_runtime"))
        .expect("decoded drained coroutine O3 runtime checkpoint");
    assert_eq!(
        runtime
            .pointer("/snapshot_rob_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        runtime
            .pointer("/snapshot_lsq_entries")
            .and_then(Value::as_u64),
        Some(0)
    );
    assert_drained_control_runtime(&restored);
}

#[test]
fn rem6_run_timing_suppresses_o3_same_window_coroutine() {
    let path = direct_coroutine_binary("o3-same-window-coroutine-timing", 0);
    let timing = run_coroutine_json(&path, "direct", 2_500, "timing", 2, &[]);

    assert_stopped_by_host(&timing);
    assert_final_execution_mode(&timing, "timing");
    assert_eq!(register_value(&timing, "x1"), 0x8000_0014);
    assert_eq!(register_value(&timing, "x5"), 0x8000_0020);
    assert_eq!(register_value(&timing, "x13"), 0x8000_0020);
    assert_eq!(
        timing.pointer("/memory/0/hex").and_then(Value::as_str),
        Some("2a000000200000800000000000000000")
    );
    assert_no_data_address(&timing, WRONG_STORE_ADDRESS);
    assert!(timing.pointer("/cores/0/o3_runtime").is_none());
    assert!(timing
        .pointer("/debug/o3_trace")
        .and_then(Value::as_array)
        .is_some_and(Vec::is_empty));
    assert_no_o3_stats(&timing);
}
