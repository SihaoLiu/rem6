use super::*;

#[test]
fn rem6_run_o3_general_iq_oldest_ready_width_one_direct() {
    assert_general_iq_oldest_ready(1);
}

#[test]
fn rem6_run_o3_general_iq_oldest_ready_width_two_direct() {
    assert_general_iq_oldest_ready(2);
}

fn assert_general_iq_oldest_ready(issue_width: usize) {
    let path =
        general_iq_oldest_ready_binary(&format!("o3-general-iq-oldest-ready-width-{issue_width}",));
    let json = general_iq_oldest_ready_json(&path, issue_width, 4_000);
    assert_final_witness(
        &json,
        GENERAL_IQ_RESULTS,
        [
            ("x5", "0x9"),
            ("x6", "0xc"),
            ("x7", "0x1"),
            ("x8", "0x5"),
            ("x9", "0x24c"),
        ],
    );

    let load = event_at_pc(&json, GENERAL_IQ_LOAD_PC);
    let producer = event_at_pc(&json, GENERAL_IQ_PRODUCER_PC);
    let blocked = event_at_pc(&json, GENERAL_IQ_BLOCKED_PC);
    let alu = event_at_pc(&json, GENERAL_IQ_ALU_PC);
    let multiply = event_at_pc(&json, GENERAL_IQ_MUL_PC);
    let load_issue_tick = event_u64(load, "issue_tick");
    for pc in [
        GENERAL_IQ_PRODUCER_PC,
        GENERAL_IQ_BLOCKED_PC,
        GENERAL_IQ_ALU_PC,
        GENERAL_IQ_MUL_PC,
    ] {
        assert!(
            fetch_tick_at_pc(&json, pc) < load_issue_tick,
            "general-IQ row {pc} must be fetched before the load issues: load={load}"
        );
    }
    let sequences = [producer, blocked, alu, multiply].map(|event| event_u64(event, "sequence"));
    assert!(sequences.windows(2).all(|pair| pair[0] < pair[1]));
    assert_eq!(
        event_u64(blocked, "issue_tick"),
        event_u64(producer, "writeback_tick")
    );
    assert!(
        event_u64(alu, "issue_tick") < event_u64(blocked, "issue_tick"),
        "ready ALU must issue before the older blocked row: load={load}, producer={producer}, blocked={blocked}, alu={alu}, multiply={multiply}"
    );
    assert!(
        event_u64(multiply, "issue_tick") < event_u64(blocked, "issue_tick"),
        "ready multiply must issue before the older blocked row: load={load}, producer={producer}, blocked={blocked}, alu={alu}, multiply={multiply}"
    );
    let issue = scoped_issue_artifact(&json);
    if issue_width == 1 {
        assert!(
            event_u64(alu, "issue_tick") < event_u64(multiply, "issue_tick"),
            "width one must serialize ready rows: alu={alu}, multiply={multiply}, issue={issue}"
        );
    } else {
        assert_eq!(
            event_u64(alu, "issue_tick"),
            event_u64(multiply, "issue_tick"),
            "width two must co-issue ready rows: alu={alu}, multiply={multiply}, issue={issue}"
        );
    }
    let commits =
        [load, producer, blocked, alu, multiply].map(|event| event_u64(event, "commit_tick"));
    assert!(commits.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(issue_u64(issue, "issued_rows"), 4);
    assert_eq!(issue_u64(issue, "max_rows_per_cycle"), issue_width as u64);
}

fn fetch_tick_at_pc(json: &Value, pc: &str) -> u64 {
    json.pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing general-IQ fetch trace: {json}"))
        .iter()
        .find(|record| record.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .and_then(|record| record.pointer("/tick"))
        .and_then(Value::as_u64)
        .unwrap_or_else(|| panic!("missing general-IQ fetch tick for {pc}: {json}"))
}
