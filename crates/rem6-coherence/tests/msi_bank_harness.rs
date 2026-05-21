use std::collections::BTreeMap;

use rem6_cache::CacheControllerResultKind;
use rem6_coherence::{
    CpuResponseRecord, HarnessError, MsiBankDirectoryHarness, MsiBankDirectoryHarnessSnapshot,
    SubmitKind,
};
use rem6_directory::{DirectoryDataSource, DirectoryLineState};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_protocol_msi::{MsiLineId, MsiState};

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn line(address: u64) -> MsiLineId {
    MsiLineId::new(Address::new(address))
}

fn request_id(agent_id: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(agent(agent_id), sequence)
}

fn read(agent_id: u32, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(agent_id, sequence),
        Address::new(address),
        size(8),
        layout(),
    )
    .unwrap()
}

fn write(agent_id: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
    let size = size(data.len() as u64);
    MemoryRequest::write(
        request_id(agent_id, sequence),
        Address::new(address),
        size,
        data,
        ByteMask::full(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn line_data(byte: u8) -> Vec<u8> {
    vec![byte; layout().bytes() as usize]
}

fn backing_line_addresses(snapshot: &MsiBankDirectoryHarnessSnapshot) -> Vec<u64> {
    snapshot
        .backing_lines()
        .iter()
        .map(|line| line.line_address().get())
        .collect()
}

#[test]
fn msi_bank_harness_reports_stable_live_indexes() {
    let mut harness =
        MsiBankDirectoryHarness::new(layout(), [agent(3), agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();

    assert_eq!(harness.cache_count(), 3);
    assert_eq!(harness.cache_agents(), vec![agent(1), agent(2), agent(3)]);
    assert_eq!(
        harness.backing_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
}

#[test]
fn msi_bank_harness_snapshot_exposes_stable_indexes() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(2), agent(1)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .submit_cpu_request(agent(2), read(2, 90, 0x1018))
        .unwrap();
    harness
        .submit_cpu_request(agent(1), read(1, 91, 0x1004))
        .unwrap();

    let snapshot = harness.snapshot();
    assert_eq!(snapshot.cache_count(), 2);
    assert_eq!(snapshot.directory_line_count(), 2);
    assert_eq!(snapshot.backing_line_count(), 2);
    assert_eq!(snapshot.cache_agents(), vec![agent(1), agent(2)]);
    assert!(snapshot.cache_snapshot(agent(1)).is_some());
    assert!(snapshot.cache_snapshot(agent(2)).is_some());
    assert!(snapshot.cache_snapshot(agent(3)).is_none());
    assert_eq!(
        snapshot.directory_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );

    let first_line = line_data(0x11);
    let second_line = line_data(0x22);
    assert_eq!(
        snapshot.backing_line(Address::new(0x1004)),
        Some(first_line.as_slice())
    );
    assert_eq!(
        snapshot.backing_line(Address::new(0x1018)),
        Some(second_line.as_slice())
    );
    assert_eq!(snapshot.backing_line(Address::new(0x2000)), None);
}

#[test]
fn msi_bank_harness_keeps_independent_lines_in_one_cache_bank() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();

    let first = harness
        .submit_cpu_request(agent(1), read(1, 10, 0x1004))
        .unwrap();
    assert_eq!(first.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(first.cache_result(), CacheControllerResultKind::Miss);
    assert_eq!(
        first
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::BackingMemory,
    );

    let second = harness
        .submit_cpu_request(agent(1), read(1, 11, 0x1018))
        .unwrap();
    assert_eq!(second.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(second.cache_result(), CacheControllerResultKind::Miss);

    assert_eq!(
        harness.cache_line_addresses(agent(1)).unwrap(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1010)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1000)),
        DirectoryLineState::new(line(0x1000)).with_sharer(agent(1))
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1010)),
        DirectoryLineState::new(line(0x1010)).with_sharer(agent(1))
    );
    assert_eq!(
        harness.directory_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].request(), request_id(1, 10));
    assert_eq!(responses[0].status(), ResponseStatus::Completed);
    assert_eq!(responses[0].data().unwrap(), &[0x11; 8]);
    assert_eq!(responses[1].request(), request_id(1, 11));
    assert_eq!(responses[1].data().unwrap(), &[0x22; 8]);
}

#[test]
fn msi_bank_harness_transfers_modified_owner_data_without_touching_other_lines() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();

    harness
        .submit_cpu_request(agent(1), write(1, 20, 0x1004, vec![0xaa; 8]))
        .unwrap();
    let shared = harness
        .submit_cpu_request(agent(2), read(2, 30, 0x1004))
        .unwrap();

    assert_eq!(shared.kind(), SubmitKind::ScheduledMiss);
    assert_eq!(
        shared
            .directory_decision()
            .unwrap()
            .grant()
            .unwrap()
            .data_source(),
        DirectoryDataSource::ModifiedOwner(agent(1)),
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1000)),
        DirectoryLineState::new(line(0x1000))
            .with_sharer(agent(1))
            .with_sharer(agent(2))
    );
    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.cache_state(agent(2), Address::new(0x1000)).unwrap(),
        Some(MsiState::Shared)
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].request(), request_id(1, 20));
    assert_eq!(responses[0].data(), None);
    assert_eq!(responses[1].request(), request_id(2, 30));
    assert_eq!(responses[1].data().unwrap(), &[0xaa; 8]);
    assert_eq!(
        harness.backing_line(Address::new(0x1010)).unwrap(),
        line_data(0x22).as_slice()
    );
    assert_eq!(
        harness.directory_state(Address::new(0x1010)),
        DirectoryLineState::new(line(0x1010))
    );
}

#[test]
fn msi_bank_harness_parallel_cycle_accepts_independent_lines_in_stable_order() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(2), agent(1)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();

    let run = harness
        .submit_parallel_cycle(
            17,
            [
                (agent(2), read(2, 20, 0x1018)),
                (agent(1), read(1, 10, 0x1004)),
            ],
        )
        .unwrap();

    assert_eq!(run.tick(), 17);
    assert_eq!(run.accepted_count(), 2);
    assert_eq!(
        run.accepted_lines(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(run.accepted()[0].agent(), agent(1));
    assert_eq!(run.accepted()[0].request(), request_id(1, 10));
    assert_eq!(run.accepted()[0].result().kind(), SubmitKind::ScheduledMiss);
    assert_eq!(run.accepted()[1].agent(), agent(2));
    assert_eq!(run.accepted()[1].request(), request_id(2, 20));

    assert_eq!(
        harness.cache_state(agent(1), Address::new(0x1004)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.cache_state(agent(2), Address::new(0x1018)).unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        harness.directory_line_addresses(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].tick(), 17);
    assert_eq!(responses[0].request(), request_id(1, 10));
    assert_eq!(responses[0].data().unwrap(), &[0x11; 8]);
    assert_eq!(responses[1].tick(), 17);
    assert_eq!(responses[1].request(), request_id(2, 20));
    assert_eq!(responses[1].data().unwrap(), &[0x22; 8]);
}

#[test]
fn msi_bank_harness_parallel_cycle_plan_is_stable_and_side_effect_free() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(2), agent(1)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    let before = harness.snapshot();

    let plan = harness
        .plan_parallel_cycle(
            19,
            [
                (agent(2), read(2, 22, 0x1018)),
                (agent(1), read(1, 11, 0x1004)),
            ],
        )
        .unwrap();

    assert_eq!(plan.tick(), 19);
    assert_eq!(plan.entry_count(), 2);
    assert!(plan.has_parallel_work());
    assert_eq!(
        plan.lines(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(plan.entries()[0].agent(), agent(1));
    assert_eq!(plan.entries()[0].request(), request_id(1, 11));
    assert_eq!(plan.entries()[1].agent(), agent(2));
    assert_eq!(plan.entries()[1].request(), request_id(2, 22));
    assert_eq!(harness.snapshot(), before);

    let run = harness.submit_parallel_cycle_plan(plan).unwrap();

    assert_eq!(run.tick(), 19);
    assert_eq!(run.accepted_count(), 2);
    assert_eq!(run.response_count(), 2);
    assert_eq!(run.scheduled_miss_count(), 2);
    assert_eq!(run.immediate_hit_count(), 0);
    assert_ne!(harness.snapshot(), before);
}

#[test]
fn msi_bank_harness_snapshot_restore_preserves_parallel_cycle_history() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();

    let first = source
        .submit_parallel_cycle(
            41,
            [
                (agent(2), read(2, 20, 0x1018)),
                (agent(1), read(1, 10, 0x1004)),
            ],
        )
        .unwrap();
    let second = source
        .submit_parallel_cycle(
            47,
            [
                (agent(1), read(1, 11, 0x1004)),
                (agent(2), read(2, 21, 0x1018)),
            ],
        )
        .unwrap();

    assert_eq!(
        source.parallel_cycle_runs(),
        &[first.clone(), second.clone()]
    );
    let snapshot = source.snapshot();
    assert_eq!(snapshot.parallel_cycle_runs(), source.parallel_cycle_runs());

    let rebuilt = MsiBankDirectoryHarnessSnapshot::from_bytes(&snapshot.to_bytes()).unwrap();
    assert_eq!(rebuilt, snapshot);
    assert_eq!(
        rebuilt.parallel_cycle_runs(),
        &[first.clone(), second.clone()]
    );

    let mut restored = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    restored
        .insert_backing_line(Address::new(0x1000), line_data(0xee))
        .unwrap();
    restored
        .submit_parallel_cycle(53, [(agent(1), read(1, 30, 0x1004))])
        .unwrap();
    assert_ne!(restored.parallel_cycle_runs(), source.parallel_cycle_runs());

    restored.restore(&snapshot).unwrap();

    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(restored.parallel_cycle_runs(), &[first, second]);
    assert_eq!(
        restored
            .parallel_cycle_runs()
            .iter()
            .map(|run| (run.tick(), run.accepted_count(), run.response_count()))
            .collect::<Vec<_>>(),
        vec![(41, 2, 2), (47, 2, 2)]
    );
}

#[test]
fn msi_bank_harness_parallel_cycle_history_summarizes_restored_records() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    source
        .submit_parallel_cycle(
            61,
            [
                (agent(2), read(2, 20, 0x1018)),
                (agent(1), read(1, 10, 0x1004)),
            ],
        )
        .unwrap();
    source
        .submit_parallel_cycle(
            67,
            [
                (agent(1), read(1, 11, 0x1004)),
                (agent(2), read(2, 21, 0x1018)),
            ],
        )
        .unwrap();
    let snapshot = source.snapshot();
    let restored = MsiBankDirectoryHarnessSnapshot::from_bytes(&snapshot.to_bytes()).unwrap();

    let history = restored.parallel_cycle_history();

    assert_eq!(history.cycle_count(), 2);
    assert!(!history.is_empty());
    assert!(history.has_parallel_work());
    assert_eq!(history.parallel_cycle_count(), 2);
    assert_eq!(history.single_request_cycle_count(), 0);
    assert_eq!(history.ticks(), vec![61, 67]);
    assert_eq!(history.total_accepted(), 4);
    assert_eq!(history.total_responses(), 4);
    assert_eq!(history.total_immediate_hits(), 2);
    assert_eq!(history.total_scheduled_misses(), 2);
    assert_eq!(history.max_accepted_per_cycle(), 2);
    assert_eq!(
        history.touched_lines(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(history.accepted_by_tick(61), 2);
    assert_eq!(history.accepted_by_tick(67), 2);
    assert_eq!(history.accepted_by_tick(100), 0);
    assert_eq!(history.accepted_by_line(Address::new(0x1000)), 2);
    assert_eq!(history.accepted_by_line(Address::new(0x1010)), 2);
    assert_eq!(history.accepted_by_line(Address::new(0x2000)), 0);
    assert_eq!(history.accepted_by_agent(agent(1)), 2);
    assert_eq!(history.accepted_by_agent(agent(2)), 2);
    assert_eq!(history.accepted_by_agent(agent(3)), 0);
    assert_eq!(
        history.accepted_by_agent_counts(),
        BTreeMap::from([(agent(1), 2), (agent(2), 2)])
    );
    assert_eq!(
        history.accepted_by_line_counts(),
        BTreeMap::from([(Address::new(0x1000), 2), (Address::new(0x1010), 2)])
    );
    assert_eq!(
        history.accepted_by_tick_counts(),
        BTreeMap::from([(61, 2), (67, 2)])
    );
    assert_eq!(source.parallel_cycle_history(), history);

    let first_run = &restored.parallel_cycle_runs()[0];
    assert_eq!(first_run.agents(), vec![agent(1), agent(2)]);
    assert_eq!(
        first_run.requests(),
        vec![request_id(1, 10), request_id(2, 20)]
    );
    assert_eq!(
        first_run.lines(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert!(first_run.has_agent(agent(1)));
    assert!(first_run.has_agent(agent(2)));
    assert!(!first_run.has_agent(agent(3)));
    assert!(first_run.has_line(Address::new(0x1000)));
    assert!(first_run.has_line(Address::new(0x1010)));
    assert!(!first_run.has_line(Address::new(0x2000)));
}

#[test]
fn msi_bank_harness_parallel_cycle_reports_hit_and_miss_mix() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    harness
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    harness
        .submit_cpu_request(agent(1), read(1, 10, 0x1004))
        .unwrap();

    let run = harness
        .submit_parallel_cycle(
            29,
            [
                (agent(1), read(1, 11, 0x1004)),
                (agent(2), read(2, 20, 0x1018)),
            ],
        )
        .unwrap();

    assert!(run.has_parallel_work());
    assert_eq!(run.accepted_count(), 2);
    assert_eq!(run.immediate_hit_count(), 1);
    assert_eq!(run.scheduled_miss_count(), 1);
    assert_eq!(run.response_count(), 2);
    assert_eq!(run.accepted()[0].result().kind(), SubmitKind::ImmediateHit);
    assert_eq!(run.accepted()[1].result().kind(), SubmitKind::ScheduledMiss);

    let responses = harness.cpu_responses();
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[1].tick(), 29);
    assert_eq!(responses[1].request(), request_id(1, 11));
    assert_eq!(responses[1].data().unwrap(), &[0x11; 8]);
    assert_eq!(responses[2].tick(), 29);
    assert_eq!(responses[2].request(), request_id(2, 20));
    assert_eq!(responses[2].data().unwrap(), &[0x22; 8]);
}

#[test]
fn msi_bank_harness_parallel_cycle_allows_empty_work_without_mutation() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    let before = harness.snapshot();

    let plan = harness
        .plan_parallel_cycle(31, std::iter::empty::<(AgentId, MemoryRequest)>())
        .unwrap();

    assert_eq!(plan.tick(), 31);
    assert!(plan.is_empty());
    assert!(!plan.has_parallel_work());
    assert_eq!(plan.entry_count(), 0);
    assert!(plan.lines().is_empty());
    assert_eq!(harness.snapshot(), before);

    let run = harness.submit_parallel_cycle_plan(plan).unwrap();

    assert_eq!(run.tick(), 31);
    assert!(run.is_empty());
    assert!(!run.has_parallel_work());
    assert_eq!(run.accepted_count(), 0);
    assert_eq!(run.response_count(), 0);
    assert_eq!(harness.snapshot(), before);
    assert!(harness.cpu_responses().is_empty());
}

#[test]
fn msi_bank_harness_parallel_cycle_rejects_same_line_conflict_without_mutation() {
    let mut harness = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    harness
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    let before = harness.snapshot();

    assert_eq!(
        harness
            .submit_parallel_cycle(
                23,
                [
                    (agent(2), read(2, 20, 0x1008)),
                    (agent(1), write(1, 10, 0x1004, vec![0xaa; 8])),
                ],
            )
            .unwrap_err(),
        HarnessError::ParallelLineConflict {
            line: Address::new(0x1000),
            first: request_id(1, 10),
            second: request_id(2, 20),
        }
    );
    assert_eq!(harness.snapshot(), before);
    assert!(harness.cpu_responses().is_empty());
    assert_eq!(
        harness.directory_state(Address::new(0x1000)),
        DirectoryLineState::new(line(0x1000))
    );
}

#[test]
fn msi_bank_harness_snapshot_restore_reinstates_multi_line_state() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .insert_backing_line(Address::new(0x1010), line_data(0x22))
        .unwrap();
    source
        .submit_cpu_request(agent(1), write(1, 20, 0x1004, vec![0xaa; 8]))
        .unwrap();
    source
        .submit_cpu_request(agent(2), read(2, 30, 0x1004))
        .unwrap();
    source
        .submit_cpu_request(agent(1), read(1, 40, 0x1018))
        .unwrap();

    let snapshot = source.snapshot();
    assert_eq!(snapshot.layout(), layout());
    assert_eq!(snapshot.cache_snapshots().len(), 2);
    assert_eq!(
        snapshot.directory_states(),
        &[
            DirectoryLineState::new(line(0x1000))
                .with_sharer(agent(1))
                .with_sharer(agent(2)),
            DirectoryLineState::new(line(0x1010)).with_sharer(agent(1)),
        ]
    );
    assert_eq!(backing_line_addresses(&snapshot), vec![0x1000, 0x1010]);
    assert_eq!(snapshot.backing_lines()[0].data(), line_data(0x11));
    assert_eq!(snapshot.backing_lines()[1].data(), line_data(0x22));
    assert_eq!(snapshot.cpu_responses().len(), 3);
    assert_eq!(snapshot.directory_decisions().len(), 3);

    let mut restored = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    restored
        .insert_backing_line(Address::new(0x1000), line_data(0xee))
        .unwrap();
    restored
        .submit_cpu_request(agent(2), read(2, 50, 0x1004))
        .unwrap();
    assert_ne!(restored.snapshot(), snapshot);

    restored.restore(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored.cache_line_addresses(agent(1)).unwrap(),
        vec![Address::new(0x1000), Address::new(0x1010)]
    );
    assert_eq!(
        restored.cache_line_addresses(agent(2)).unwrap(),
        vec![Address::new(0x1000)]
    );
    assert_eq!(
        restored
            .cache_state(agent(1), Address::new(0x1000))
            .unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        restored
            .cache_state(agent(2), Address::new(0x1000))
            .unwrap(),
        Some(MsiState::Shared)
    );
    assert_eq!(
        restored.backing_line(Address::new(0x1000)).unwrap(),
        line_data(0x11).as_slice()
    );

    let local_hit = restored
        .submit_cpu_request(agent(2), read(2, 60, 0x1004))
        .unwrap();
    assert_eq!(local_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(local_hit.directory_decision(), None);
    assert_eq!(
        restored.cpu_responses().last(),
        Some(&CpuResponseRecord::new(
            0,
            CacheControllerResultKind::Hit,
            request_id(2, 60),
            ResponseStatus::Completed,
            Some(vec![0xaa; 8]),
        ))
    );

    let other_line_hit = restored
        .submit_cpu_request(agent(1), read(1, 61, 0x1018))
        .unwrap();
    assert_eq!(other_line_hit.kind(), SubmitKind::ImmediateHit);
    assert_eq!(
        restored.cpu_responses().last().unwrap().data().unwrap(),
        &[0x22; 8]
    );
}

#[test]
fn msi_bank_harness_restore_rejects_snapshot_layout_mismatch() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .submit_cpu_request(agent(1), read(1, 70, 0x1004))
        .unwrap();
    let snapshot = source.snapshot();

    let other_layout = CacheLineLayout::new(32).unwrap();
    let mut restored = MsiBankDirectoryHarness::new(other_layout, [agent(1)]).unwrap();

    assert_eq!(
        restored.restore(&snapshot).unwrap_err(),
        HarnessError::SnapshotResourceMismatch {
            resource: "msi bank directory harness layout",
        }
    );
}

#[test]
fn msi_bank_harness_restore_rejects_cache_set_mismatch() {
    let mut source = MsiBankDirectoryHarness::new(layout(), [agent(1), agent(2)]).unwrap();
    source
        .insert_backing_line(Address::new(0x1000), line_data(0x11))
        .unwrap();
    source
        .submit_cpu_request(agent(1), read(1, 80, 0x1004))
        .unwrap();
    let snapshot = source.snapshot();

    let mut restored = MsiBankDirectoryHarness::new(layout(), [agent(1)]).unwrap();

    assert_eq!(
        restored.restore(&snapshot).unwrap_err(),
        HarnessError::UnknownCache { agent: agent(2) }
    );
}
