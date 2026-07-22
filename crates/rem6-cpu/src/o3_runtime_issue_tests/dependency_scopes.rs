use super::*;

fn sequence_at(fixture: &ScalarIssueFixture, pc: u64) -> u64 {
    fixture.sequence(pc)
}

fn prepared_issue(
    fixture: &ScalarIssueFixture,
    row_index: usize,
    issue_tick: u64,
) -> O3PreparedLiveIssue {
    let queue = fixture.queue();
    let (pc, _, _) = fixture.rows[row_index];
    let entry = queue.entry(sequence_at(fixture, pc)).unwrap();
    let candidate = fixture
        .runtime
        .materialize_live_speculative_issue_candidate(entry.scheduling())
        .unwrap();
    let mut hart = fixture.hart.clone();
    for write in candidate.forwarded_register_writes() {
        hart.write(write.register(), write.value());
    }
    hart.set_pc(entry.scheduling().pc().get());
    let execution = hart.execute_decoded(entry.packet().decoded()).unwrap();
    O3PreparedLiveIssue {
        candidate,
        consumed_requests: entry.packet().consumed_requests().to_vec(),
        issue_tick,
        execution,
    }
}

#[test]
fn scheduling_metadata_exists_before_forwarded_values() {
    let fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);
    let queue = fixture.queue();
    let candidate = queue
        .entry(sequence_at(&fixture, THIRD_PC))
        .unwrap()
        .scheduling();
    assert_eq!(candidate.data_producers().len(), 1);
    assert_eq!(
        candidate.data_producers()[0].sequence(),
        sequence_at(&fixture, SECOND_PC)
    );
    assert!(fixture
        .runtime
        .materialize_live_speculative_issue_candidate(candidate)
        .is_none());
}

#[test]
fn dependency_table_keeps_data_and_control_release_ticks_distinct() {
    let mut fixture = ScalarIssueFixture::new_unbound(3, ScalarIssueCase::SameWindowCoroutine);
    fixture.bind_row(0);
    fixture.bind_row(1);
    fixture.schedule(20);
    fixture.bind_row(2);
    let queue = fixture.queue();
    let entry = queue.entry(sequence_at(&fixture, THIRD_PC)).unwrap();
    let table = O3LiveIssueDependencyTable::new(&fixture.runtime, queue.entries()).unwrap();
    let scoped = table.scoped_instruction(entry);
    let admitted = fixture.execution_at(SECOND_PC).admitted_writeback_tick;
    assert_eq!(scoped.waits_on().len(), 2);
    assert_eq!(table.resolved_scopes_at(admitted).len(), 1);
    assert_eq!(table.resolved_scopes_at(admitted + 1).len(), 2);
}

#[test]
fn dependency_table_encodes_two_source_fan_in() {
    let fixture = ScalarIssueFixture::new(4, ScalarIssueCase::FanIn);
    let queue = fixture.queue();
    let entry = queue.entry(sequence_at(&fixture, THIRD_PC)).unwrap();
    let candidate = entry.scheduling();
    let table = O3LiveIssueDependencyTable::new(&fixture.runtime, queue.entries()).unwrap();
    let scoped = table.scoped_instruction(entry);
    assert_eq!(
        candidate
            .data_producers()
            .iter()
            .map(|producer| producer.sequence())
            .collect::<Vec<_>>(),
        vec![
            sequence_at(&fixture, BRANCH_PC),
            sequence_at(&fixture, SECOND_PC)
        ]
    );
    assert_eq!(scoped.waits_on().len(), 2);
    assert_ne!(scoped.waits_on()[0], scoped.waits_on()[1]);
}

#[test]
fn selected_issue_batch_failure_records_no_partial_state() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::CrossResource);
    let mut prepared = vec![
        prepared_issue(&fixture, 0, 20),
        prepared_issue(&fixture, 1, 20),
    ];
    prepared[1].consumed_requests.push(request(999));
    let before = fixture.runtime.clone();
    assert!(matches!(
        fixture.runtime.record_live_issue_batch(prepared),
        Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { .. })
    ));
    assert_eq!(fixture.runtime, before);
}
