use super::*;

pub(super) fn assert_mixed_live_switch_rejects(
    fixture: &TranslatedMemoryPairFixture,
    memory_system: &str,
    route_delay: u64,
    second_issue: u64,
    earliest_response: u64,
) {
    let latest_source = earliest_response
        .checked_sub(HOST_EVENT_DELAY + 1)
        .expect("mixed pair response leaves room for a host action");
    assert!(second_issue <= latest_source);
    let switch_tick = second_issue + (latest_source - second_issue) / 2;
    let id = super::super::super::RESULT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let artifact = std::env::temp_dir().join(format!("rem6-translated-pair-live-switch-{id}.json"));
    let _ = std::fs::remove_file(&artifact);
    let output = fixture.output_mixed_with_switch(
        memory_system,
        route_delay,
        switch_tick,
        &["--output", artifact.to_str().unwrap()],
    );

    assert_eq!(
        output.status.code(),
        Some(2),
        "mixed translated live switch: {output:?}"
    );
    assert!(
        output.stdout.is_empty(),
        "mixed translated live switch: {output:?}"
    );
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "failed to execute run: host action failed: checkpoint component is not quiescent: cpu0\n"
    );
    assert!(
        !artifact.exists(),
        "mixed translated live switch emitted {}",
        artifact.display()
    );
}

pub(super) fn sole_data_request_at_tick<'a>(json: &'a Value, tick: u64, pc: &str) -> &'a Value {
    let records = data_request_sent_records(json)
        .into_iter()
        .filter(|record| event_u64(record, "tick") == tick)
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "exact ordinary request for {pc}");
    records[0]
}
