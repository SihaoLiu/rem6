use rem6_cache::{
    CacheWriteQueueConfig, CacheWriteQueueEntryKind, CacheWriteQueueError, ChiCacheBank,
    ChiCacheBankError, ChiCacheBankSnapshot, ChiCacheControllerResultKind, MesiCacheBank,
    MesiCacheBankError, MesiCacheBankSnapshot, MesiCacheControllerResultKind, MoesiCacheBank,
    MoesiCacheBankError, MoesiCacheBankSnapshot, MoesiCacheControllerResultKind, MshrEntry,
    MshrQueueConfig, MshrQueueSnapshot, MshrTarget, MshrTargetSource,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryAccessOrdering, MemoryOperation,
    MemoryRequest, MemoryRequestId, MemoryRequestSnapshot, MemoryResponse,
};
use rem6_protocol_chi::{ChiEvent, ChiState};
use rem6_protocol_mesi::{MesiEvent, MesiState};
use rem6_protocol_moesi::{MoesiEvent, MoesiState};
use rem6_transport::TargetOutcome;

fn agent(value: u32) -> AgentId {
    AgentId::new(value)
}

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn size(bytes: u64) -> AccessSize {
    AccessSize::new(bytes).unwrap()
}

fn read(agent_id: AgentId, sequence: u64, address: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(address),
        size(8),
        layout(),
    )
    .unwrap()
}

fn clean_writeback(agent_id: AgentId, sequence: u64, line: u64, value: u8) -> MemoryRequest {
    MemoryRequest::writeback_clean(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
        vec![value; layout().bytes() as usize],
        layout(),
    )
    .unwrap()
}

fn clean_evict(agent_id: AgentId, sequence: u64, line: u64) -> MemoryRequest {
    MemoryRequest::clean_evict(
        MemoryRequestId::new(agent_id, sequence),
        Address::new(line),
        layout(),
    )
    .unwrap()
}

fn invalidate(agent_id: AgentId, sequence: u64, line: u64) -> MemoryRequest {
    let snapshot = MemoryRequestSnapshot::new(
        MemoryRequestId::new(agent_id, sequence),
        MemoryOperation::Invalidate,
        Address::new(line),
        size(layout().bytes()),
        layout(),
        MemoryAccessOrdering::none(),
        false,
        false,
        None,
        None,
        None,
    )
    .unwrap();
    MemoryRequest::from_snapshot(&snapshot).unwrap()
}

fn fill(request: &MemoryRequest, byte: u8) -> MemoryResponse {
    MemoryResponse::completed(request, Some(vec![byte; layout().bytes() as usize])).unwrap()
}

fn response_id(outcome: &TargetOutcome) -> MemoryRequestId {
    match outcome {
        TargetOutcome::Respond(response) => response.request_id(),
        TargetOutcome::RespondAfter { response, .. } => response.request_id(),
        TargetOutcome::NoResponse => panic!("expected response outcome"),
    }
}

macro_rules! protocol_post_fill_bank_tests {
    (
        $module:ident,
        $bank:ty,
        $snapshot:ty,
        $error:ty,
        $result_kind:ty,
        $fill_event:expr,
        $transient_state:expr
    ) => {
        mod $module {
            use super::*;

            fn restored_bank_with_target(
                target: MemoryRequest,
                write_queue_entries: Option<usize>,
            ) -> ($bank, MemoryRequest, MemoryRequest) {
                let cache_agent = agent(7);
                let config = MshrQueueConfig::new(2, 3, 0).unwrap();
                let mut bank = match write_queue_entries {
                    Some(entries) => <$bank>::new_with_mshr_and_write_queue(
                        cache_agent,
                        layout(),
                        config.clone(),
                        CacheWriteQueueConfig::new(entries, 0).unwrap(),
                    ),
                    None => <$bank>::new_with_mshr(cache_agent, layout(), config.clone()),
                };

                let first = read(cache_agent, 100, 0x1004);
                let first_miss = bank.accept_cpu_request(first.clone()).unwrap();
                let first_downstream = first_miss.downstream_request().unwrap().clone();

                let snapshot = bank.snapshot();
                let current_mshr = snapshot.mshr().unwrap();
                let current_entry = &current_mshr.entries()[0];
                let mut targets = current_entry.targets().to_vec();
                targets.push(MshrTarget::from_parts(
                    target,
                    1,
                    1,
                    MshrTargetSource::Demand,
                    false,
                    None,
                ));
                let mshr_snapshot = MshrQueueSnapshot::new(
                    config.clone(),
                    vec![MshrEntry::from_parts(
                        current_entry.handle(),
                        current_entry.line(),
                        current_entry.ready_tick(),
                        current_entry.order(),
                        current_entry.in_service(),
                        current_entry.pending_modified(),
                        targets,
                    )],
                    current_mshr.next_handle(),
                    current_mshr.next_order() + 1,
                );
                let restored_snapshot = <$snapshot>::new_with_mshr(
                    snapshot.agent(),
                    snapshot.layout(),
                    snapshot.next_sequence(),
                    snapshot.lines().to_vec(),
                    mshr_snapshot,
                );
                let mut restored = match write_queue_entries {
                    Some(entries) => <$bank>::new_with_mshr_and_write_queue(
                        cache_agent,
                        layout(),
                        config,
                        CacheWriteQueueConfig::new(entries, 0).unwrap(),
                    ),
                    None => <$bank>::new_with_mshr(cache_agent, layout(), config),
                };
                match write_queue_entries {
                    Some(_) => restored
                        .restore(
                            &restored_snapshot
                                .with_write_queue(snapshot.write_queue().unwrap().clone()),
                        )
                        .unwrap(),
                    None => restored.restore(&restored_snapshot).unwrap(),
                };
                (restored, first, first_downstream)
            }

            #[test]
            fn enqueues_supported_post_fill_writeback_before_returning_fill_result() {
                let cache_agent = agent(7);
                let clean = clean_writeback(cache_agent, 101, 0x1000, 0xcc);
                let (mut restored, first, first_downstream) =
                    restored_bank_with_target(clean.clone(), Some(2));

                let fill_result = restored
                    .accept_fill(fill(&first_downstream, 0x44), $fill_event)
                    .unwrap();

                assert_eq!(fill_result.kind(), <$result_kind>::Fill);
                assert_eq!(
                    fill_result
                        .target_outcomes()
                        .iter()
                        .map(response_id)
                        .collect::<Vec<_>>(),
                    vec![first.id()]
                );
                assert_eq!(
                    fill_result.post_fill_downstream_requests(),
                    std::slice::from_ref(&clean)
                );
                assert_eq!(restored.write_queue_allocated_count(), 1);
                let issue = restored.issue_write_queue(0).unwrap().unwrap();
                assert_eq!(issue.kind(), CacheWriteQueueEntryKind::WritebackClean);
                assert_eq!(issue.request(), &clean);
                assert_eq!(restored.mshr_allocated_count(), 0);
            }

            #[test]
            fn rejects_post_fill_writeback_targets_without_write_queue() {
                let cache_agent = agent(7);
                let clean = clean_writeback(cache_agent, 102, 0x1000, 0xdd);
                let (mut restored, _, first_downstream) = restored_bank_with_target(clean, None);

                assert_eq!(
                    restored.accept_fill(fill(&first_downstream, 0x44), $fill_event),
                    Err(<$error>::WriteQueueDisabled)
                );
                assert_eq!(restored.pending_fill_count(), 1);
                assert_eq!(restored.mshr_allocated_count(), 1);
                assert_eq!(restored.state(Address::new(0x1000)), Some($transient_state));
            }

            #[test]
            fn rejects_post_fill_writeback_targets_when_write_queue_full() {
                let cache_agent = agent(7);
                let clean = clean_writeback(cache_agent, 103, 0x1000, 0xee);
                let (mut restored, _, first_downstream) = restored_bank_with_target(clean, Some(1));
                restored
                    .enqueue_writeback(clean_evict(cache_agent, 104, 0x2000), false, 0)
                    .unwrap();

                assert_eq!(
                    restored.accept_fill(fill(&first_downstream, 0x44), $fill_event),
                    Err(<$error>::WriteQueue(CacheWriteQueueError::EntrySlotsFull {
                        entries: 1,
                        reserve: 0,
                    },))
                );
                assert_eq!(restored.pending_fill_count(), 1);
                assert_eq!(restored.mshr_allocated_count(), 1);
                assert_eq!(restored.write_queue_allocated_count(), 1);
                assert_eq!(restored.state(Address::new(0x1000)), Some($transient_state));
            }

            #[test]
            fn forwards_post_fill_invalidate_targets_without_write_queue() {
                let cache_agent = agent(7);
                let invalidation = invalidate(cache_agent, 105, 0x1000);
                let (mut restored, _, first_downstream) =
                    restored_bank_with_target(invalidation.clone(), None);

                let fill_result = restored
                    .accept_fill(fill(&first_downstream, 0x44), $fill_event)
                    .unwrap();

                assert_eq!(
                    fill_result.post_fill_downstream_requests(),
                    std::slice::from_ref(&invalidation)
                );
                assert_eq!(restored.pending_fill_count(), 0);
                assert_eq!(restored.mshr_allocated_count(), 0);
            }

            #[test]
            fn forwards_post_fill_invalidate_targets_when_write_queue_full() {
                let cache_agent = agent(7);
                let invalidation = invalidate(cache_agent, 106, 0x1000);
                let (mut restored, _, first_downstream) =
                    restored_bank_with_target(invalidation.clone(), Some(1));
                restored
                    .enqueue_writeback(clean_evict(cache_agent, 107, 0x2000), false, 0)
                    .unwrap();

                let fill_result = restored
                    .accept_fill(fill(&first_downstream, 0x44), $fill_event)
                    .unwrap();

                assert_eq!(
                    fill_result.post_fill_downstream_requests(),
                    std::slice::from_ref(&invalidation)
                );
                assert_eq!(restored.pending_fill_count(), 0);
                assert_eq!(restored.mshr_allocated_count(), 0);
                assert_eq!(restored.write_queue_allocated_count(), 1);
            }
        }
    };
}

protocol_post_fill_bank_tests!(
    mesi,
    MesiCacheBank,
    MesiCacheBankSnapshot,
    MesiCacheBankError,
    MesiCacheControllerResultKind,
    MesiEvent::DataShared,
    MesiState::InvalidToExclusive
);

protocol_post_fill_bank_tests!(
    moesi,
    MoesiCacheBank,
    MoesiCacheBankSnapshot,
    MoesiCacheBankError,
    MoesiCacheControllerResultKind,
    MoesiEvent::DataShared,
    MoesiState::InvalidToExclusive
);

protocol_post_fill_bank_tests!(
    chi,
    ChiCacheBank,
    ChiCacheBankSnapshot,
    ChiCacheBankError,
    ChiCacheControllerResultKind,
    ChiEvent::CompDataSharedClean,
    ChiState::InvalidToSharedClean
);
