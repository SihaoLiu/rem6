use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryOperation, ResponseStatus};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerEvent, TrafficControllerState,
    TrafficStateGenerator, TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTrace,
    TrafficTraceConfig, TrafficTraceErrorKind, TrafficTraceReplayAction,
    TrafficTraceReplayActionQueue, TrafficTraceReplayCompletion, TrafficTraceReplayFailure,
    TrafficTraceReplayOutcome, TrafficTraceReplaySource, TrafficTraceResponseKind,
    TrafficTransition, TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_READ_RESP: u32 = 2;
const GEM5_READ_RESP_WITH_INVALIDATE: u32 = 3;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_WRITE_RESP: u32 = 5;
const GEM5_WRITE_COMPLETE_RESP: u32 = 6;
const GEM5_SOFT_PF_REQ: u32 = 11;
const GEM5_SOFT_PF_RESP: u32 = 14;
const GEM5_SWAP_REQ: u32 = 34;
const GEM5_SWAP_RESP: u32 = 35;
const GEM5_SC_UPGRADE_REQ: u32 = 18;
const GEM5_SC_UPGRADE_FAIL_REQ: u32 = 20;
const GEM5_UPGRADE_FAIL_RESP: u32 = 21;
const GEM5_STORE_COND_FAIL_REQ: u32 = 28;
const GEM5_STORE_COND_RESP: u32 = 29;
const GEM5_MEM_FENCE_REQ: u32 = 38;
const GEM5_MEM_FENCE_RESP: u32 = 41;
const GEM5_INVALID_DEST_ERROR: u32 = 46;
const GEM5_WRITE_ERROR: u32 = 49;
const GEM5_HTM_REQ: u32 = 56;
const GEM5_HTM_REQ_RESP: u32 = 57;
const GEM5_FLAG_PHYSICAL: u32 = 0x0000_0200;
const GEM5_FLAG_ATOMIC_NO_RETURN_OP: u32 = 0x8000_0000;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: Option<u64>,
    size: Option<u32>,
    packet_id: Option<u64>,
}

#[derive(Clone, Copy)]
struct FlaggedPacketFields {
    tick: u64,
    command: u32,
    address: Option<u64>,
    size: Option<u32>,
    flags: Option<u32>,
    packet_id: Option<u64>,
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn controller_for_packets(packets: &[PacketFields]) -> TrafficController {
    controller_for_packets_with_offset(packets, 0)
}

fn controller_for_packets_with_offset(
    packets: &[PacketFields],
    addr_offset: u64,
) -> TrafficController {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(TICK_FREQUENCY, packets),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace)
        .unwrap()
        .with_addr_offset(addr_offset)
        .unwrap();
    let controller_config = TrafficControllerConfig::new(
        graph(vec![state(0, u64::MAX)], vec![transition(0, 0)]),
        vec![TrafficControllerState::new(
            TrafficStateId::new(0),
            TrafficStateGenerator::Trace(rem6_traffic::TrafficTraceGenerator::new(config)),
        )],
    )
    .unwrap();
    TrafficController::new(controller_config)
}

fn controller_for_flagged_packets(packets: &[FlaggedPacketFields]) -> TrafficController {
    controller_for_flagged_packets_with_offset(packets, 0)
}

fn controller_for_flagged_packets_with_offset(
    packets: &[FlaggedPacketFields],
    addr_offset: u64,
) -> TrafficController {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &flagged_gem5_packet_trace(TICK_FREQUENCY, packets),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace)
        .unwrap()
        .with_addr_offset(addr_offset)
        .unwrap();
    let controller_config = TrafficControllerConfig::new(
        graph(vec![state(0, u64::MAX)], vec![transition(0, 0)]),
        vec![TrafficControllerState::new(
            TrafficStateId::new(0),
            TrafficStateGenerator::Trace(rem6_traffic::TrafficTraceGenerator::new(config)),
        )],
    )
    .unwrap();
    TrafficController::new(controller_config)
}

#[test]
fn traffic_controller_matches_trace_response_to_pending_memory_request() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x4000),
            size: Some(8),
            packet_id: Some(3),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x4000),
            size: Some(8),
            packet_id: Some(3),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.request().operation(), MemoryOperation::ReadShared);
    assert!(request.request().requires_response());
    assert!(request_batch.trace_response_match().is_none());

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response = response_batch.trace_response().unwrap();
    assert_eq!(
        response.kind(),
        TrafficTraceResponseKind::ReadWithInvalidate
    );
    assert!(response.invalidates_line());

    let matched = response_batch.trace_response_match().unwrap();
    assert_eq!(matched.response(), response);
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
            assert_eq!(source.trace_packet_id(), Some(3));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_response) => {
            assert_eq!(memory_response.request_id(), request.request().id());
            assert_eq!(memory_response.status(), ResponseStatus::Completed);
            assert_eq!(memory_response.data().unwrap().len(), 8);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
}

#[test]
fn traffic_controller_records_write_complete_response_after_write_resp() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x4080),
            size: Some(8),
            packet_id: Some(30),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_RESP,
            address: Some(0x4080),
            size: Some(8),
            packet_id: Some(30),
        },
        PacketFields {
            tick: 9,
            command: GEM5_WRITE_COMPLETE_RESP,
            address: Some(0x4080),
            size: Some(8),
            packet_id: Some(30),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.request().operation(), MemoryOperation::Write);

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response = response_batch.trace_response().unwrap();
    assert_eq!(response.kind(), TrafficTraceResponseKind::Write);
    match response_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::MemoryResponse {
            tick,
            response,
            trace_data,
        } => {
            assert_eq!(*tick, response_batch.trace_response().unwrap().tick());
            assert_eq!(response.request_id(), request.request().id());
            assert_eq!(response.status(), ResponseStatus::Completed);
            assert_eq!(trace_data, &None);
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);
    assert_eq!(controller.trace_replay_summary().write_completions(), 0);

    let write_complete_batch = controller.next_event(response.tick(), 0).unwrap().unwrap();
    let write_complete = write_complete_batch.trace_response().unwrap();
    assert_eq!(
        write_complete.kind(),
        TrafficTraceResponseKind::WriteComplete
    );
    let matched = write_complete_batch.trace_response_match().unwrap();
    assert_eq!(matched.response(), write_complete);
    match matched.completion() {
        TrafficTraceReplayCompletion::WriteCompletion(completion) => {
            assert_eq!(completion.request_id(), request.request().id());
            assert_eq!(completion.request_line(), Address::new(0x4080));
            assert_eq!(completion.response(), write_complete);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
    match write_complete_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::MemoryWriteCompletion {
            tick,
            request,
            request_line,
            response,
        } => {
            assert_eq!(*tick, write_complete.tick());
            assert_eq!(*request, request_batch.request().unwrap().request().id());
            assert_eq!(*request_line, Address::new(0x4080));
            assert_eq!(*response, write_complete);
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);
    assert_eq!(controller.trace_replay_summary().write_completions(), 1);

    let mut action_queue = TrafficTraceReplayActionQueue::default();
    action_queue.record_batch(&response_batch).unwrap();
    action_queue.record_batch(&write_complete_batch).unwrap();
    assert!(action_queue.pop_memory_response().is_some());
    let write_completion = action_queue.pop_memory_write_completion().unwrap();
    assert_eq!(write_completion.tick(), write_complete.tick());
    assert_eq!(write_completion.request_id(), request.request().id());
    assert_eq!(write_completion.request_line(), Address::new(0x4080));
    assert_eq!(write_completion.response(), write_complete);
    assert_eq!(action_queue.summary().memory_completions(), 1);
    assert_eq!(action_queue.summary().write_completions(), 1);
}

#[test]
fn traffic_controller_matches_physical_write_complete_after_addr_offset() {
    let mut controller = controller_for_flagged_packets_with_offset(
        &[
            FlaggedPacketFields {
                tick: 5,
                command: GEM5_WRITE_REQ,
                address: Some(0x4080),
                size: Some(8),
                flags: None,
                packet_id: Some(31),
            },
            FlaggedPacketFields {
                tick: 7,
                command: GEM5_WRITE_RESP,
                address: Some(0x4080),
                size: Some(8),
                flags: None,
                packet_id: Some(31),
            },
            FlaggedPacketFields {
                tick: 9,
                command: GEM5_WRITE_COMPLETE_RESP,
                address: Some(0x4080),
                size: Some(8),
                flags: Some(GEM5_FLAG_PHYSICAL),
                packet_id: Some(31),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x40c0);
    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert!(matches!(
        response_batch.trace_replay_action(),
        Some(TrafficTraceReplayAction::MemoryResponse { .. })
    ));

    let write_complete_batch = controller
        .next_event(response_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let write_complete = write_complete_batch.trace_response().unwrap();
    assert!(write_complete.trace_address_is_physical());
    assert_eq!(write_complete.address().unwrap().get(), 0x4080);
    let matched = write_complete_batch.trace_response_match().unwrap();
    match matched.completion() {
        TrafficTraceReplayCompletion::WriteCompletion(completion) => {
            assert_eq!(completion.request_id(), request.request().id());
            assert_eq!(completion.request_line(), Address::new(0x40c0));
            assert_eq!(completion.response(), write_complete);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
    assert!(matches!(
        write_complete_batch.trace_replay_action(),
        Some(TrafficTraceReplayAction::MemoryWriteCompletion { .. })
    ));
}

#[test]
fn traffic_controller_requires_size_for_physical_write_complete_after_addr_offset() {
    let mut controller = controller_for_flagged_packets_with_offset(
        &[
            FlaggedPacketFields {
                tick: 5,
                command: GEM5_WRITE_REQ,
                address: Some(0x4100),
                size: Some(8),
                flags: None,
                packet_id: Some(32),
            },
            FlaggedPacketFields {
                tick: 7,
                command: GEM5_WRITE_RESP,
                address: Some(0x4100),
                size: Some(8),
                flags: None,
                packet_id: Some(32),
            },
            FlaggedPacketFields {
                tick: 9,
                command: GEM5_WRITE_COMPLETE_RESP,
                address: Some(0x4100),
                size: None,
                flags: Some(GEM5_FLAG_PHYSICAL),
                packet_id: Some(32),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x4140);
    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert!(matches!(
        response_batch.trace_replay_action(),
        Some(TrafficTraceReplayAction::MemoryResponse { .. })
    ));

    let write_complete_batch = controller
        .next_event(response_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let write_complete = write_complete_batch.trace_response().unwrap();
    assert!(write_complete.trace_address_is_physical());
    assert_eq!(write_complete.size_bytes(), None);
    assert!(write_complete_batch.trace_response_match().is_none());
    assert!(write_complete_batch.trace_replay_action().is_none());
}

#[test]
fn traffic_controller_requires_packet_id_for_write_complete_response() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x40c0),
            size: Some(8),
            packet_id: Some(31),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_RESP,
            address: Some(0x40c0),
            size: Some(8),
            packet_id: Some(31),
        },
        PacketFields {
            tick: 9,
            command: GEM5_WRITE_COMPLETE_RESP,
            address: None,
            size: None,
            packet_id: None,
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert_eq!(
        response_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::Write
    );
    assert!(matches!(
        response_batch.trace_replay_action().unwrap(),
        TrafficTraceReplayAction::MemoryResponse { .. }
    ));

    let write_complete_batch = controller
        .next_event(response_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();

    assert_eq!(
        write_complete_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::WriteComplete
    );
    assert!(write_complete_batch.trace_response_match().is_none());
    assert!(write_complete_batch.trace_replay_action().is_none());
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);
    assert_eq!(controller.trace_replay_summary().write_completions(), 0);
}

#[test]
fn traffic_controller_does_not_match_weak_write_complete_to_stale_pending_write() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x40c0),
            size: Some(8),
            packet_id: Some(31),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_RESP,
            address: Some(0x40c0),
            size: Some(8),
            packet_id: Some(31),
        },
        PacketFields {
            tick: 8,
            command: GEM5_WRITE_REQ,
            address: Some(0x41c0),
            size: Some(8),
            packet_id: Some(32),
        },
        PacketFields {
            tick: 9,
            command: GEM5_WRITE_COMPLETE_RESP,
            address: None,
            size: None,
            packet_id: None,
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let first_request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let first_response_batch = controller
        .next_event(first_request_batch.request().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    assert_eq!(
        first_response_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::Write
    );
    assert!(matches!(
        first_response_batch.trace_replay_action().unwrap(),
        TrafficTraceReplayAction::MemoryResponse { .. }
    ));

    let second_request_batch = controller
        .next_event(first_response_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let second_request = second_request_batch.request().unwrap().clone();
    assert_eq!(second_request.request().operation(), MemoryOperation::Write);

    let weak_write_complete_batch = controller
        .next_event(second_request.tick(), 0)
        .unwrap()
        .unwrap();

    assert_eq!(
        weak_write_complete_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::WriteComplete
    );
    assert!(weak_write_complete_batch.trace_response_match().is_none());
    assert!(weak_write_complete_batch.trace_replay_action().is_none());
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);
    assert_eq!(controller.trace_replay_summary().write_completions(), 0);
}

#[test]
fn traffic_controller_matches_swap_response_to_atomic_no_return_request() {
    let mut controller = controller_for_flagged_packets(&[
        FlaggedPacketFields {
            tick: 5,
            command: GEM5_SWAP_REQ,
            address: Some(0x4100),
            size: Some(8),
            flags: Some(GEM5_FLAG_ATOMIC_NO_RETURN_OP),
            packet_id: Some(31),
        },
        FlaggedPacketFields {
            tick: 7,
            command: GEM5_SWAP_RESP,
            address: Some(0x4100),
            size: Some(8),
            flags: None,
            packet_id: Some(31),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(
        request.request().operation(),
        MemoryOperation::AtomicNoReturn
    );
    assert!(request.request().requires_response());
    assert!(!request.request().returns_data());

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response = response_batch.trace_response().unwrap();
    assert_eq!(response.kind(), TrafficTraceResponseKind::Swap);
    assert!(response.returns_data());

    let matched = response_batch.trace_response_match().unwrap();
    assert_eq!(matched.response(), response);
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_response) => {
            assert_eq!(memory_response.request_id(), request.request().id());
            assert_eq!(memory_response.status(), ResponseStatus::Completed);
            assert_eq!(memory_response.data(), None);
            assert_eq!(memory_response.trace_data().unwrap().len(), 8);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
    match response_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::MemoryResponse {
            tick,
            response,
            trace_data,
        } => {
            assert_eq!(*tick, matched.response().tick());
            assert_eq!(response.request_id(), request.request().id());
            assert_eq!(response.data(), None);
            assert_eq!(trace_data.as_deref().unwrap().len(), 8);
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
}

#[test]
fn traffic_controller_exposes_prefetch_response_trace_data_separately_from_memory_response() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_SOFT_PF_REQ,
            address: Some(0x4300),
            size: Some(16),
            packet_id: Some(33),
        },
        PacketFields {
            tick: 7,
            command: GEM5_SOFT_PF_RESP,
            address: Some(0x4300),
            size: Some(16),
            packet_id: Some(33),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.request().operation(), MemoryOperation::PrefetchRead);
    assert!(request.request().requires_response());
    assert!(!request.request().returns_data());

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response = response_batch.trace_response().unwrap();
    assert_eq!(response.kind(), TrafficTraceResponseKind::SoftPrefetch);
    assert!(response.returns_data());

    let matched = response_batch.trace_response_match().unwrap();
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_completion) => {
            assert_eq!(
                memory_completion.response().request_id(),
                request.request().id()
            );
            assert_eq!(memory_completion.response().data(), None);
            assert_eq!(memory_completion.trace_data().unwrap().len(), 16);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
    match response_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::MemoryResponse {
            tick,
            response,
            trace_data,
        } => {
            assert_eq!(*tick, matched.response().tick());
            assert_eq!(response.request_id(), request.request().id());
            assert_eq!(response.data(), None);
            assert_eq!(trace_data.as_deref().unwrap().len(), 16);
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
    let mut action_queue = TrafficTraceReplayActionQueue::default();
    action_queue.record_batch(&response_batch).unwrap();
    let response_record = action_queue.pop_memory_response().unwrap();
    assert_eq!(
        response_record.response().request_id(),
        request.request().id()
    );
    assert_eq!(response_record.response().data(), None);
    assert_eq!(response_record.trace_data().unwrap().len(), 16);
    assert!(action_queue.pop_memory_response().is_none());
}

#[test]
fn traffic_controller_matches_trace_error_to_atomic_no_return_request() {
    let mut controller = controller_for_flagged_packets(&[
        FlaggedPacketFields {
            tick: 5,
            command: GEM5_SWAP_REQ,
            address: Some(0x4200),
            size: Some(8),
            flags: Some(GEM5_FLAG_ATOMIC_NO_RETURN_OP),
            packet_id: Some(32),
        },
        FlaggedPacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: Some(0x4200),
            size: Some(8),
            flags: None,
            packet_id: Some(32),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(
        request.request().operation(),
        MemoryOperation::AtomicNoReturn
    );

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = error_batch.trace_error_match().unwrap();
    assert!(matched.error().is_write());
    match matched.failure() {
        TrafficTraceReplayFailure::Memory(failure) => {
            assert_eq!(failure.request_id(), request.request().id());
            assert_eq!(failure.error(), TrafficTraceErrorKind::Write);
        }
        failure => panic!("unexpected trace replay failure: {failure:?}"),
    }
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
            assert_eq!(source.trace_packet_id(), Some(32));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_trace_response_to_pending_sync_event() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(4),
        },
        PacketFields {
            tick: 7,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(4),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let sync_batch = controller.next_event(20, 0).unwrap().unwrap();
    let sync = sync_batch.trace_sync().unwrap();
    assert!(sync.requires_response());
    assert!(sync_batch.trace_response_match().is_none());

    let response_batch = controller.next_event(sync.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Sync(source) => {
            assert_eq!(source.sequence(), sync.sequence());
            assert_eq!(source.trace_packet_id(), Some(4));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
    assert_eq!(matched.completion(), &TrafficTraceReplayCompletion::Ack);
}

#[test]
fn traffic_controller_matches_trace_error_to_pending_write_request() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(5),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(5),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.request().operation(), MemoryOperation::Write);

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = error_batch.trace_error_match().unwrap();
    assert!(matched.error().is_write());
    match matched.failure() {
        TrafficTraceReplayFailure::Memory(failure) => {
            assert_eq!(failure.request_id(), request.request().id());
            assert_eq!(failure.error(), TrafficTraceErrorKind::Write);
        }
        failure => panic!("unexpected trace replay failure: {failure:?}"),
    }
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_trace_error_to_pending_htm_request() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_HTM_REQ,
            address: Some(0x5400),
            size: Some(16),
            packet_id: Some(14),
        },
        PacketFields {
            tick: 7,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0x5400),
            size: Some(16),
            packet_id: Some(14),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert!(htm.requires_response());

    let error_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    let matched = error_batch.trace_error_match().unwrap();
    match matched.failure() {
        TrafficTraceReplayFailure::Control(failure) => {
            assert_eq!(failure.error(), TrafficTraceErrorKind::InvalidDestination);
        }
        failure => panic!("unexpected trace replay failure: {failure:?}"),
    }
    match matched.source() {
        TrafficTraceReplaySource::Htm(source) => {
            assert_eq!(source.sequence(), htm.sequence());
            assert_eq!(source.trace_packet_id(), Some(14));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_records_trace_replay_outcomes() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x5800),
            size: Some(8),
            packet_id: Some(15),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x5800),
            size: Some(8),
            packet_id: Some(15),
        },
        PacketFields {
            tick: 9,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(16),
        },
        PacketFields {
            tick: 11,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(16),
        },
        PacketFields {
            tick: 13,
            command: GEM5_WRITE_REQ,
            address: Some(0x5a00),
            size: Some(8),
            packet_id: Some(17),
        },
        PacketFields {
            tick: 15,
            command: GEM5_WRITE_ERROR,
            address: Some(0x5a00),
            size: Some(8),
            packet_id: Some(17),
        },
        PacketFields {
            tick: 17,
            command: GEM5_HTM_REQ,
            address: Some(0x5c00),
            size: Some(16),
            packet_id: Some(18),
        },
        PacketFields {
            tick: 19,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0x5c00),
            size: Some(16),
            packet_id: Some(18),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    assert_eq!(controller.trace_replay_summary().memory_completions(), 0);
    assert_eq!(controller.trace_replay_summary().control_completions(), 0);
    assert_eq!(controller.trace_replay_summary().memory_failures(), 0);
    assert_eq!(controller.trace_replay_summary().control_failures(), 0);

    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert!(request_batch.trace_replay_outcome().is_none());

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    match response_batch.trace_replay_outcome().unwrap() {
        TrafficTraceReplayOutcome::Completion(match_) => {
            assert_eq!(match_.response().trace_packet_id(), Some(15));
        }
        outcome => panic!("unexpected trace replay outcome: {outcome:?}"),
    }
    match response_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::MemoryResponse { tick, response, .. } => {
            assert_eq!(*tick, response_batch.trace_response().unwrap().tick());
            assert_eq!(response.request_id(), request.request().id());
            assert_eq!(response.status(), ResponseStatus::Completed);
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);
    assert_eq!(controller.trace_replay_summary().control_completions(), 0);
    assert_eq!(controller.trace_replay_summary().memory_failures(), 0);
    assert_eq!(controller.trace_replay_summary().control_failures(), 0);

    let sync_batch = controller
        .next_event(response_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let sync = sync_batch.trace_sync().unwrap();
    assert!(sync_batch.trace_replay_outcome().is_none());

    let sync_response_batch = controller.next_event(sync.tick(), 0).unwrap().unwrap();
    match sync_response_batch.trace_replay_outcome().unwrap() {
        TrafficTraceReplayOutcome::Completion(match_) => {
            assert_eq!(match_.completion(), &TrafficTraceReplayCompletion::Ack);
        }
        outcome => panic!("unexpected trace replay outcome: {outcome:?}"),
    }
    match sync_response_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::ControlAck { tick } => {
            assert_eq!(*tick, sync_response_batch.trace_response().unwrap().tick());
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);
    assert_eq!(controller.trace_replay_summary().control_completions(), 1);
    assert_eq!(controller.trace_replay_summary().memory_failures(), 0);
    assert_eq!(controller.trace_replay_summary().control_failures(), 0);

    let write_batch = controller
        .next_event(sync_response_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let write = write_batch.request().unwrap().clone();
    assert!(write_batch.trace_replay_outcome().is_none());

    let write_error_batch = controller.next_event(write.tick(), 0).unwrap().unwrap();
    match write_error_batch.trace_replay_outcome().unwrap() {
        TrafficTraceReplayOutcome::Failure(match_) => match match_.failure() {
            TrafficTraceReplayFailure::Memory(failure) => {
                assert_eq!(failure.request_id(), write.request().id());
            }
            failure => panic!("unexpected trace replay failure: {failure:?}"),
        },
        outcome => panic!("unexpected trace replay outcome: {outcome:?}"),
    }
    match write_error_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::MemoryFailure { tick, failure } => {
            assert_eq!(*tick, write_error_batch.trace_error().unwrap().tick());
            assert_eq!(failure.request_id(), write.request().id());
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);
    assert_eq!(controller.trace_replay_summary().control_completions(), 1);
    assert_eq!(controller.trace_replay_summary().memory_failures(), 1);
    assert_eq!(controller.trace_replay_summary().control_failures(), 0);

    let htm_batch = controller
        .next_event(write_error_batch.trace_error().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert!(htm_batch.trace_replay_outcome().is_none());

    let error_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    match error_batch.trace_replay_outcome().unwrap() {
        TrafficTraceReplayOutcome::Failure(match_) => {
            assert_eq!(
                match_.error().kind(),
                TrafficTraceErrorKind::InvalidDestination
            );
        }
        outcome => panic!("unexpected trace replay outcome: {outcome:?}"),
    }
    match error_batch.trace_replay_action().unwrap() {
        TrafficTraceReplayAction::ControlFailure { tick, failure } => {
            assert_eq!(*tick, error_batch.trace_error().unwrap().tick());
            assert_eq!(failure.error(), TrafficTraceErrorKind::InvalidDestination);
        }
        action => panic!("unexpected trace replay action: {action:?}"),
    }
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);
    assert_eq!(controller.trace_replay_summary().control_completions(), 1);
    assert_eq!(controller.trace_replay_summary().memory_failures(), 1);
    assert_eq!(controller.trace_replay_summary().control_failures(), 1);
}

#[test]
fn traffic_trace_replay_action_queue_drains_executable_results() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x5e00),
            size: Some(8),
            packet_id: Some(19),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x5e00),
            size: Some(8),
            packet_id: Some(19),
        },
        PacketFields {
            tick: 9,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(20),
        },
        PacketFields {
            tick: 11,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(20),
        },
        PacketFields {
            tick: 13,
            command: GEM5_WRITE_REQ,
            address: Some(0x5f00),
            size: Some(8),
            packet_id: Some(21),
        },
        PacketFields {
            tick: 15,
            command: GEM5_WRITE_ERROR,
            address: Some(0x5f00),
            size: Some(8),
            packet_id: Some(21),
        },
        PacketFields {
            tick: 17,
            command: GEM5_HTM_REQ,
            address: Some(0x6000),
            size: Some(16),
            packet_id: Some(22),
        },
        PacketFields {
            tick: 19,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0x6000),
            size: Some(16),
            packet_id: Some(22),
        },
    ]);
    let mut action_queue = TrafficTraceReplayActionQueue::default();

    assert!(controller.start(20).unwrap().is_empty());

    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    action_queue.record_batch(&request_batch).unwrap();
    assert!(action_queue.is_empty());

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert!(matches!(
        response_batch.events(),
        [
            TrafficControllerEvent::TraceResponse(_),
            TrafficControllerEvent::TraceResponseMatch(_),
            TrafficControllerEvent::TraceReplayAction(
                TrafficTraceReplayAction::MemoryResponse { .. }
            ),
        ]
    ));
    action_queue.record_batch(&response_batch).unwrap();
    let response_record = action_queue.pop_memory_response().unwrap();
    assert_eq!(
        response_record.tick(),
        response_batch.trace_response().unwrap().tick()
    );
    let response = response_record.response();
    assert_eq!(response.request_id(), request.request().id());
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data().unwrap().len(), 8);
    assert!(action_queue.pop_memory_response().is_none());
    assert_eq!(action_queue.summary().memory_completions(), 1);
    assert_eq!(action_queue.summary().control_completions(), 0);
    assert_eq!(action_queue.summary().memory_failures(), 0);
    assert_eq!(action_queue.summary().control_failures(), 0);

    let sync_batch = controller
        .next_event(response_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let sync = sync_batch.trace_sync().unwrap();
    action_queue.record_batch(&sync_batch).unwrap();
    assert!(action_queue.is_empty());

    let sync_response_batch = controller.next_event(sync.tick(), 0).unwrap().unwrap();
    action_queue.record_batch(&sync_response_batch).unwrap();
    assert_eq!(
        action_queue.pop_control_ack_tick().unwrap(),
        sync_response_batch.trace_response().unwrap().tick()
    );
    assert!(action_queue.pop_control_ack_tick().is_none());
    assert_eq!(action_queue.summary().memory_completions(), 1);
    assert_eq!(action_queue.summary().control_completions(), 1);
    assert_eq!(action_queue.summary().memory_failures(), 0);
    assert_eq!(action_queue.summary().control_failures(), 0);

    let write_batch = controller
        .next_event(sync_response_batch.trace_response().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let write = write_batch.request().unwrap().clone();
    action_queue.record_batch(&write_batch).unwrap();
    assert!(action_queue.is_empty());

    let write_error_batch = controller.next_event(write.tick(), 0).unwrap().unwrap();
    action_queue.record_batch(&write_error_batch).unwrap();
    let memory_failure = action_queue.pop_memory_failure().unwrap();
    assert_eq!(
        memory_failure.tick(),
        write_error_batch.trace_error().unwrap().tick()
    );
    assert_eq!(memory_failure.failure().request_id(), write.request().id());
    assert_eq!(
        memory_failure.failure().error(),
        TrafficTraceErrorKind::Write
    );
    assert!(action_queue.pop_memory_failure().is_none());
    assert_eq!(action_queue.summary().memory_completions(), 1);
    assert_eq!(action_queue.summary().control_completions(), 1);
    assert_eq!(action_queue.summary().memory_failures(), 1);
    assert_eq!(action_queue.summary().control_failures(), 0);

    let htm_batch = controller
        .next_event(write_error_batch.trace_error().unwrap().tick(), 0)
        .unwrap()
        .unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    action_queue.record_batch(&htm_batch).unwrap();
    assert!(action_queue.is_empty());

    let error_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    action_queue.record_batch(&error_batch).unwrap();
    let control_failure = action_queue.pop_control_failure().unwrap();
    assert_eq!(
        control_failure.tick(),
        error_batch.trace_error().unwrap().tick()
    );
    assert_eq!(
        control_failure.failure().error(),
        TrafficTraceErrorKind::InvalidDestination
    );
    assert!(action_queue.pop_control_failure().is_none());
    assert_eq!(action_queue.summary().memory_completions(), 1);
    assert_eq!(action_queue.summary().control_completions(), 1);
    assert_eq!(action_queue.summary().memory_failures(), 1);
    assert_eq!(action_queue.summary().control_failures(), 1);
    assert!(action_queue.is_empty());
}

#[test]
fn traffic_trace_replay_action_queue_preserves_recorded_action_order() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x6100),
            size: Some(8),
            packet_id: Some(23),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x6100),
            size: Some(8),
            packet_id: Some(23),
        },
        PacketFields {
            tick: 9,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(24),
        },
        PacketFields {
            tick: 11,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(24),
        },
        PacketFields {
            tick: 13,
            command: GEM5_WRITE_REQ,
            address: Some(0x6200),
            size: Some(8),
            packet_id: Some(25),
        },
        PacketFields {
            tick: 15,
            command: GEM5_WRITE_ERROR,
            address: Some(0x6200),
            size: Some(8),
            packet_id: Some(25),
        },
        PacketFields {
            tick: 17,
            command: GEM5_HTM_REQ,
            address: Some(0x6300),
            size: Some(16),
            packet_id: Some(26),
        },
        PacketFields {
            tick: 19,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0x6300),
            size: Some(16),
            packet_id: Some(26),
        },
    ]);
    let mut action_queue = TrafficTraceReplayActionQueue::default();

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    action_queue.record_batch(&request_batch).unwrap();

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response_tick = response_batch.trace_response().unwrap().tick();
    action_queue.record_batch(&response_batch).unwrap();
    let sync_batch = controller.next_event(response_tick, 0).unwrap().unwrap();
    let sync = sync_batch.trace_sync().unwrap();
    action_queue.record_batch(&sync_batch).unwrap();

    let sync_response_batch = controller.next_event(sync.tick(), 0).unwrap().unwrap();
    let sync_response_tick = sync_response_batch.trace_response().unwrap().tick();
    action_queue.record_batch(&sync_response_batch).unwrap();
    let write_batch = controller
        .next_event(sync_response_tick, 0)
        .unwrap()
        .unwrap();
    let write = write_batch.request().unwrap().clone();
    action_queue.record_batch(&write_batch).unwrap();

    let write_error_batch = controller.next_event(write.tick(), 0).unwrap().unwrap();
    let write_error_tick = write_error_batch.trace_error().unwrap().tick();
    action_queue.record_batch(&write_error_batch).unwrap();
    let htm_batch = controller.next_event(write_error_tick, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    action_queue.record_batch(&htm_batch).unwrap();

    let error_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    let error_tick = error_batch.trace_error().unwrap().tick();
    action_queue.record_batch(&error_batch).unwrap();

    let first = action_queue.pop_action().unwrap();
    match first {
        TrafficTraceReplayAction::MemoryResponse { tick, .. } => {
            assert_eq!(tick, response_tick);
        }
        action => panic!("unexpected first replay action: {action:?}"),
    }
    let second = action_queue.pop_action().unwrap();
    assert_eq!(
        second,
        TrafficTraceReplayAction::ControlAck {
            tick: sync_response_tick
        }
    );
    let third = action_queue.pop_action().unwrap();
    match third {
        TrafficTraceReplayAction::MemoryFailure { tick, .. } => {
            assert_eq!(tick, write_error_tick);
        }
        action => panic!("unexpected third replay action: {action:?}"),
    }
    let fourth = action_queue.pop_action().unwrap();
    match fourth {
        TrafficTraceReplayAction::ControlFailure { tick, .. } => {
            assert_eq!(tick, error_tick);
        }
        action => panic!("unexpected fourth replay action: {action:?}"),
    }
    assert!(action_queue.pop_action().is_none());
    assert_eq!(action_queue.summary().memory_completions(), 1);
    assert_eq!(action_queue.summary().control_completions(), 1);
    assert_eq!(action_queue.summary().memory_failures(), 1);
    assert_eq!(action_queue.summary().control_failures(), 1);
    assert!(action_queue.is_empty());
}

#[test]
fn traffic_controller_snapshot_restores_trace_replay_summary() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x5d00),
            size: Some(8),
            packet_id: Some(17),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x5d00),
            size: Some(8),
            packet_id: Some(17),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert!(response_batch.trace_replay_outcome().is_some());
    assert_eq!(controller.trace_replay_summary().memory_completions(), 1);

    let restored = TrafficController::restore(controller.snapshot()).unwrap();
    assert_eq!(restored.trace_replay_summary().memory_completions(), 1);
    assert_eq!(restored.trace_replay_summary().control_completions(), 0);
    assert_eq!(restored.trace_replay_summary().memory_failures(), 0);
    assert_eq!(restored.trace_replay_summary().control_failures(), 0);
}

#[test]
fn traffic_controller_matches_upgrade_fail_response_to_failed_sc_upgrade() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_SC_UPGRADE_FAIL_REQ,
            address: Some(0x8000),
            size: Some(64),
            packet_id: Some(8),
        },
        PacketFields {
            tick: 7,
            command: GEM5_UPGRADE_FAIL_RESP,
            address: Some(0x8000),
            size: Some(64),
            packet_id: Some(8),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(
        request.request().operation(),
        MemoryOperation::StoreConditionalUpgradeFail
    );

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_response) => {
            assert_eq!(memory_response.request_id(), request.request().id());
            assert_eq!(memory_response.status(), ResponseStatus::Completed);
            assert_eq!(memory_response.data().unwrap().len(), 64);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
}

#[test]
fn traffic_controller_maps_failed_sc_upgrade_response_to_store_conditional_failure() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_SC_UPGRADE_REQ,
            address: Some(0x8100),
            size: Some(64),
            packet_id: Some(28),
        },
        PacketFields {
            tick: 7,
            command: GEM5_UPGRADE_FAIL_RESP,
            address: Some(0x8100),
            size: Some(64),
            packet_id: Some(28),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(
        request.request().operation(),
        MemoryOperation::StoreConditionalUpgrade
    );

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_response) => {
            assert_eq!(memory_response.request_id(), request.request().id());
            assert_eq!(
                memory_response.status(),
                ResponseStatus::StoreConditionalFailed
            );
            assert_eq!(memory_response.data(), None);
            assert_eq!(memory_response.trace_data().unwrap().len(), 64);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
}

#[test]
fn traffic_controller_preserves_forced_store_conditional_failure_status() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_STORE_COND_FAIL_REQ,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(9),
        },
        PacketFields {
            tick: 7,
            command: GEM5_STORE_COND_RESP,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(9),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(
        request.request().operation(),
        MemoryOperation::StoreConditionalFail
    );

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.completion() {
        TrafficTraceReplayCompletion::Memory(memory_response) => {
            assert_eq!(memory_response.request_id(), request.request().id());
            assert_eq!(
                memory_response.status(),
                ResponseStatus::StoreConditionalFailed
            );
            assert_eq!(memory_response.data(), None);
        }
        completion => panic!("unexpected trace replay completion: {completion:?}"),
    }
}

#[test]
fn traffic_controller_matches_trace_response_after_addr_offset() {
    let mut controller = controller_for_packets_with_offset(
        &[
            PacketFields {
                tick: 5,
                command: GEM5_READ_REQ,
                address: Some(0x9000),
                size: Some(8),
                packet_id: Some(10),
            },
            PacketFields {
                tick: 7,
                command: GEM5_READ_RESP,
                address: Some(0x9000),
                size: Some(8),
                packet_id: Some(10),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x9040);

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert_eq!(
        response_batch
            .trace_response()
            .unwrap()
            .address()
            .unwrap()
            .get(),
        0x9040
    );
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_physical_trace_response_after_addr_offset() {
    let mut controller = controller_for_flagged_packets_with_offset(
        &[
            FlaggedPacketFields {
                tick: 5,
                command: GEM5_READ_REQ,
                address: Some(0x9000),
                size: Some(8),
                flags: None,
                packet_id: Some(13),
            },
            FlaggedPacketFields {
                tick: 7,
                command: GEM5_READ_RESP,
                address: Some(0x9000),
                size: Some(8),
                flags: Some(GEM5_FLAG_PHYSICAL),
                packet_id: Some(13),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x9040);

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response = response_batch.trace_response().unwrap();
    assert!(response.trace_address_is_physical());
    assert_eq!(response.address().unwrap().get(), 0x9000);
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
    assert!(matches!(
        response_batch.trace_replay_action(),
        Some(TrafficTraceReplayAction::MemoryResponse { .. })
    ));
}

#[test]
fn traffic_controller_requires_size_for_physical_trace_response_after_addr_offset() {
    let mut controller = controller_for_flagged_packets_with_offset(
        &[
            FlaggedPacketFields {
                tick: 5,
                command: GEM5_READ_REQ,
                address: Some(0x9080),
                size: Some(8),
                flags: None,
                packet_id: Some(15),
            },
            FlaggedPacketFields {
                tick: 7,
                command: GEM5_READ_RESP,
                address: Some(0x9080),
                size: None,
                flags: Some(GEM5_FLAG_PHYSICAL),
                packet_id: Some(15),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x90c0);

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response = response_batch.trace_response().unwrap();
    assert!(response.trace_address_is_physical());
    assert_eq!(response.size_bytes(), None);
    assert!(response_batch.trace_response_match().is_none());
    assert!(response_batch.trace_replay_action().is_none());
}

#[test]
fn traffic_controller_requires_source_packet_id_for_physical_trace_response() {
    let mut controller = controller_for_flagged_packets(&[
        FlaggedPacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x9100),
            size: Some(8),
            flags: None,
            packet_id: None,
        },
        FlaggedPacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x9100),
            size: Some(8),
            flags: Some(GEM5_FLAG_PHYSICAL),
            packet_id: Some(17),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x9100);
    assert_eq!(request.trace_packet_id(), None);

    let response_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let response = response_batch.trace_response().unwrap();
    assert!(response.trace_address_is_physical());
    assert_eq!(response.trace_packet_id(), Some(17));
    assert_eq!(response.size_bytes(), Some(8));
    assert!(response_batch.trace_response_match().is_none());
    assert!(response_batch.trace_replay_action().is_none());
}

#[test]
fn traffic_controller_matches_trace_error_after_addr_offset() {
    let mut controller = controller_for_packets_with_offset(
        &[
            PacketFields {
                tick: 5,
                command: GEM5_WRITE_REQ,
                address: Some(0x9400),
                size: Some(8),
                packet_id: Some(11),
            },
            PacketFields {
                tick: 7,
                command: GEM5_WRITE_ERROR,
                address: Some(0x9400),
                size: Some(8),
                packet_id: Some(11),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x9440);

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert_eq!(
        error_batch.trace_error().unwrap().address().unwrap().get(),
        0x9440
    );
    let matched = error_batch.trace_error_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_matches_physical_trace_error_after_addr_offset() {
    let mut controller = controller_for_flagged_packets_with_offset(
        &[
            FlaggedPacketFields {
                tick: 5,
                command: GEM5_WRITE_REQ,
                address: Some(0x9400),
                size: Some(8),
                flags: None,
                packet_id: Some(14),
            },
            FlaggedPacketFields {
                tick: 7,
                command: GEM5_WRITE_ERROR,
                address: Some(0x9400),
                size: Some(8),
                flags: Some(GEM5_FLAG_PHYSICAL),
                packet_id: Some(14),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x9440);

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let error = error_batch.trace_error().unwrap();
    assert!(error.trace_address_is_physical());
    assert_eq!(error.address().unwrap().get(), 0x9400);
    let matched = error_batch.trace_error_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
    assert!(matches!(
        error_batch.trace_replay_action(),
        Some(TrafficTraceReplayAction::MemoryFailure { .. })
    ));
}

#[test]
fn traffic_controller_requires_size_for_physical_trace_error_after_addr_offset() {
    let mut controller = controller_for_flagged_packets_with_offset(
        &[
            FlaggedPacketFields {
                tick: 5,
                command: GEM5_WRITE_REQ,
                address: Some(0x9480),
                size: Some(8),
                flags: None,
                packet_id: Some(16),
            },
            FlaggedPacketFields {
                tick: 7,
                command: GEM5_WRITE_ERROR,
                address: Some(0x9480),
                size: None,
                flags: Some(GEM5_FLAG_PHYSICAL),
                packet_id: Some(16),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x94c0);

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let error = error_batch.trace_error().unwrap();
    assert!(error.trace_address_is_physical());
    assert_eq!(error.size_bytes(), None);
    assert!(error_batch.trace_error_match().is_none());
    assert!(error_batch.trace_replay_action().is_none());
}

#[test]
fn traffic_controller_requires_source_packet_id_for_physical_trace_error() {
    let mut controller = controller_for_flagged_packets(&[
        FlaggedPacketFields {
            tick: 5,
            command: GEM5_WRITE_REQ,
            address: Some(0x9500),
            size: Some(8),
            flags: None,
            packet_id: None,
        },
        FlaggedPacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: Some(0x9500),
            size: Some(8),
            flags: Some(GEM5_FLAG_PHYSICAL),
            packet_id: Some(18),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();
    assert_eq!(request.address().get(), 0x9500);
    assert_eq!(request.trace_packet_id(), None);

    let error_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    let error = error_batch.trace_error().unwrap();
    assert!(error.trace_address_is_physical());
    assert_eq!(error.trace_packet_id(), Some(18));
    assert_eq!(error.size_bytes(), Some(8));
    assert!(error_batch.trace_error_match().is_none());
    assert!(error_batch.trace_replay_action().is_none());
}

#[test]
fn traffic_controller_matches_htm_response_after_addr_offset() {
    let mut controller = controller_for_packets_with_offset(
        &[
            PacketFields {
                tick: 5,
                command: GEM5_HTM_REQ,
                address: Some(0x9800),
                size: Some(16),
                packet_id: Some(12),
            },
            PacketFields {
                tick: 7,
                command: GEM5_HTM_REQ_RESP,
                address: Some(0x9800),
                size: Some(16),
                packet_id: Some(12),
            },
        ],
        0x40,
    );

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert_eq!(htm.address().unwrap().get(), 0x9840);

    let response_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    assert_eq!(
        response_batch
            .trace_response()
            .unwrap()
            .address()
            .unwrap()
            .get(),
        0x9840
    );
    assert_eq!(
        response_batch.trace_response_match().unwrap().completion(),
        &TrafficTraceReplayCompletion::Ack
    );
}

#[test]
fn traffic_controller_keeps_pending_htm_after_metadata_mismatch() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_HTM_REQ,
            address: Some(0xa000),
            size: Some(16),
            packet_id: None,
        },
        PacketFields {
            tick: 7,
            command: GEM5_HTM_REQ_RESP,
            address: Some(0xb000),
            size: Some(16),
            packet_id: None,
        },
        PacketFields {
            tick: 9,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0xa000),
            size: Some(8),
            packet_id: None,
        },
        PacketFields {
            tick: 11,
            command: GEM5_HTM_REQ_RESP,
            address: Some(0xa000),
            size: Some(16),
            packet_id: None,
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert!(htm.requires_response());

    let wrong_address_batch = controller.next_event(htm.tick(), 0).unwrap().unwrap();
    let wrong_address = wrong_address_batch.trace_response().unwrap();
    assert!(wrong_address_batch.trace_response_match().is_none());

    let wrong_size_batch = controller
        .next_event(wrong_address.tick(), 0)
        .unwrap()
        .unwrap();
    let wrong_size = wrong_size_batch.trace_error().unwrap();
    assert!(wrong_size_batch.trace_error_match().is_none());

    let response_batch = controller
        .next_event(wrong_size.tick(), 0)
        .unwrap()
        .unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    assert_eq!(matched.completion(), &TrafficTraceReplayCompletion::Ack);
    match matched.source() {
        TrafficTraceReplaySource::Htm(source) => {
            assert_eq!(source.sequence(), htm.sequence());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_keeps_pending_request_after_policy_mismatch() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(6),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_RESP,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(6),
        },
        PacketFields {
            tick: 9,
            command: GEM5_READ_RESP,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(6),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();

    let mismatch_batch = controller.next_event(request.tick(), 0).unwrap().unwrap();
    assert_eq!(
        mismatch_batch.trace_response().unwrap().kind(),
        TrafficTraceResponseKind::Write
    );
    assert!(mismatch_batch.trace_response_match().is_none());

    let response_batch = controller.next_event(27, 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();
    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_snapshot_restores_pending_trace_response_match() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_READ_REQ,
            address: Some(0x7000),
            size: Some(8),
            packet_id: Some(7),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP,
            address: Some(0x7000),
            size: Some(8),
            packet_id: Some(7),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let request_batch = controller.next_event(20, 0).unwrap().unwrap();
    let request = request_batch.request().unwrap().clone();

    let snapshot = controller.snapshot();
    let mut restored = TrafficController::restore(snapshot).unwrap();
    let response_batch = restored.next_event(request.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();

    match matched.source() {
        TrafficTraceReplaySource::Memory(source) => {
            assert_eq!(source.request().id(), request.request().id());
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

#[test]
fn traffic_controller_snapshot_restores_pending_htm_trace_response_match() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_HTM_REQ,
            address: Some(0xa400),
            size: Some(16),
            packet_id: Some(13),
        },
        PacketFields {
            tick: 7,
            command: GEM5_HTM_REQ_RESP,
            address: Some(0xa400),
            size: Some(16),
            packet_id: Some(13),
        },
    ]);

    assert!(controller.start(20).unwrap().is_empty());
    let htm_batch = controller.next_event(20, 0).unwrap().unwrap();
    let htm = htm_batch.trace_htm().unwrap();
    assert!(htm.requires_response());

    let snapshot = controller.snapshot();
    let mut restored = TrafficController::restore(snapshot).unwrap();
    let response_batch = restored.next_event(htm.tick(), 0).unwrap().unwrap();
    let matched = response_batch.trace_response_match().unwrap();

    assert_eq!(matched.completion(), &TrafficTraceReplayCompletion::Ack);
    match matched.source() {
        TrafficTraceReplaySource::Htm(source) => {
            assert_eq!(source.sequence(), htm.sequence());
            assert_eq!(source.trace_packet_id(), Some(13));
        }
        source => panic!("unexpected trace replay source: {source:?}"),
    }
}

fn state(id: u32, duration: u64) -> TrafficStateSpec {
    TrafficStateSpec::new(TrafficStateId::new(id), duration)
}

fn transition(from: u32, to: u32) -> TrafficTransition {
    TrafficTransition::new(
        TrafficStateId::new(from),
        TrafficStateId::new(to),
        TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE).unwrap(),
    )
}

fn graph(
    states: Vec<TrafficStateSpec>,
    transitions: Vec<TrafficTransition>,
) -> TrafficStateGraphConfig {
    TrafficStateGraphConfig::new(states, TrafficStateId::new(0), transitions).unwrap()
}

fn gem5_packet_trace(tick_frequency: u64, packets: &[PacketFields]) -> Vec<u8> {
    let mut bytes = GEM5_MAGIC.to_vec();
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, tick_frequency);
    append_record(&mut bytes, &header);

    for packet in packets {
        let mut message = Vec::new();
        append_key(&mut message, 1, 0);
        append_varint(&mut message, packet.tick);
        append_key(&mut message, 2, 0);
        append_varint(&mut message, u64::from(packet.command));
        if let Some(address) = packet.address {
            append_key(&mut message, 3, 0);
            append_varint(&mut message, address);
        }
        if let Some(size) = packet.size {
            append_key(&mut message, 4, 0);
            append_varint(&mut message, u64::from(size));
        }
        if let Some(packet_id) = packet.packet_id {
            append_key(&mut message, 6, 0);
            append_varint(&mut message, packet_id);
        }
        append_record(&mut bytes, &message);
    }

    bytes
}

fn flagged_gem5_packet_trace(tick_frequency: u64, packets: &[FlaggedPacketFields]) -> Vec<u8> {
    let mut bytes = GEM5_MAGIC.to_vec();
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, tick_frequency);
    append_record(&mut bytes, &header);

    for packet in packets {
        let mut message = Vec::new();
        append_key(&mut message, 1, 0);
        append_varint(&mut message, packet.tick);
        append_key(&mut message, 2, 0);
        append_varint(&mut message, u64::from(packet.command));
        if let Some(address) = packet.address {
            append_key(&mut message, 3, 0);
            append_varint(&mut message, address);
        }
        if let Some(size) = packet.size {
            append_key(&mut message, 4, 0);
            append_varint(&mut message, u64::from(size));
        }
        if let Some(flags) = packet.flags {
            append_key(&mut message, 5, 0);
            append_varint(&mut message, u64::from(flags));
        }
        if let Some(packet_id) = packet.packet_id {
            append_key(&mut message, 6, 0);
            append_varint(&mut message, packet_id);
        }
        append_record(&mut bytes, &message);
    }

    bytes
}

fn append_record(bytes: &mut Vec<u8>, message: &[u8]) {
    append_varint(
        bytes,
        u64::try_from(message.len()).expect("test message length fits u64"),
    );
    bytes.extend_from_slice(message);
}

fn append_key(bytes: &mut Vec<u8>, field: u32, wire_type: u8) {
    append_varint(bytes, (u64::from(field) << 3) | u64::from(wire_type));
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push((value as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}
