use super::*;

fn scheduling_candidate(
    fixture: &ScalarIssueFixture,
    request_index: usize,
) -> O3LiveIssueSchedulingCandidate {
    let request = &fixture.requests[request_index];
    fixture
        .runtime
        .live_issue_scheduling_candidate(
            request_index,
            request.pc(),
            request.instruction(),
            request.consumed_requests(),
        )
        .unwrap()
}

fn sequence_at(fixture: &ScalarIssueFixture, pc: u64) -> u64 {
    fixture
        .runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(pc))
        .unwrap()
        .sequence()
}

fn prepared_issue(
    fixture: &ScalarIssueFixture,
    request_index: usize,
    issue_tick: u64,
) -> O3PreparedLiveIssue {
    let request = &fixture.requests[request_index];
    let scheduling = scheduling_candidate(fixture, request_index);
    let candidate = fixture
        .runtime
        .materialize_live_speculative_issue_candidate(&scheduling)
        .unwrap();
    let mut hart = fixture.hart.clone();
    for write in candidate.forwarded_register_writes() {
        hart.write(write.register(), write.value());
    }
    hart.set_pc(request.pc().get());
    let execution = hart.execute_decoded(request.decoded()).unwrap();
    O3PreparedLiveIssue {
        candidate,
        consumed_requests: request.consumed_requests().to_vec(),
        issue_tick,
        execution,
    }
}

#[test]
fn scheduling_metadata_exists_before_forwarded_values() {
    let fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);
    let candidate = scheduling_candidate(&fixture, 2);
    assert_eq!(candidate.data_producers().len(), 1);
    assert_eq!(
        candidate.data_producers()[0].sequence(),
        sequence_at(&fixture, SECOND_PC)
    );
    assert!(fixture
        .runtime
        .materialize_live_speculative_issue_candidate(&candidate)
        .is_none());
}

#[test]
fn dependency_table_keeps_data_and_control_release_ticks_distinct() {
    let mut fixture = ScalarIssueFixture::new(3, ScalarIssueCase::SameWindowCoroutine);
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20, &fixture.requests[..2])
        .unwrap();
    let candidate = scheduling_candidate(&fixture, 2);
    let table = O3LiveIssueDependencyTable::new(&fixture.runtime, std::slice::from_ref(&candidate))
        .unwrap();
    let scoped = table.scoped_instruction(&candidate);
    let admitted = fixture.execution_at(SECOND_PC).admitted_writeback_tick;
    assert_eq!(scoped.waits_on().len(), 2);
    assert_eq!(table.resolved_scopes_at(admitted).len(), 1);
    assert_eq!(table.resolved_scopes_at(admitted + 1).len(), 2);
}

#[test]
fn dependency_table_encodes_two_source_fan_in() {
    let fixture = ScalarIssueFixture::new(4, ScalarIssueCase::FanIn);
    let candidate = scheduling_candidate(&fixture, 2);
    let table = O3LiveIssueDependencyTable::new(&fixture.runtime, std::slice::from_ref(&candidate))
        .unwrap();
    let scoped = table.scoped_instruction(&candidate);
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
