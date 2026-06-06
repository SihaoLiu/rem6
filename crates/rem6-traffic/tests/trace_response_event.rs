use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficGeneratorError,
    TrafficRequestKind, TrafficStateGenerator, TrafficStateGraphConfig, TrafficStateId,
    TrafficStateSpec, TrafficTrace, TrafficTraceConfig, TrafficTraceEvent, TrafficTraceGenerator,
    TrafficTraceResponseKind, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_READ_RESP: u32 = 2;
const GEM5_READ_RESP_WITH_INVALIDATE: u32 = 3;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_WRITE_RESP: u32 = 5;
const GEM5_WRITE_COMPLETE_RESP: u32 = 6;
const GEM5_SOFT_PF_RESP: u32 = 14;
const GEM5_HARD_PF_RESP: u32 = 15;
const GEM5_UPGRADE_RESP: u32 = 19;
const GEM5_UPGRADE_FAIL_RESP: u32 = 21;
const GEM5_READ_EX_RESP: u32 = 23;
const GEM5_STORE_COND_RESP: u32 = 29;
const GEM5_LOCKED_RMW_READ_RESP: u32 = 31;
const GEM5_LOCKED_RMW_WRITE_RESP: u32 = 33;
const GEM5_SWAP_RESP: u32 = 35;
const GEM5_MEM_SYNC_RESP: u32 = 40;
const GEM5_MEM_FENCE_RESP: u32 = 41;
const GEM5_CLEAN_SHARED_RESP: u32 = 43;
const GEM5_CLEAN_INVALID_RESP: u32 = 45;
const GEM5_INVALIDATE_RESP: u32 = 55;
const GEM5_HTM_REQ_RESP: u32 = 57;
const GEM5_FLAG_PHYSICAL: u32 = 0x0000_0200;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: Option<u64>,
    size: Option<u32>,
    flags: Option<u32>,
    packet_id: Option<u64>,
    pc: Option<u64>,
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn trace_config(trace: TrafficTrace) -> TrafficTraceConfig {
    TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap()
}

#[test]
fn trace_generator_emits_response_events() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 5,
                    command: GEM5_READ_REQ,
                    address: Some(0x20),
                    size: Some(8),
                    flags: None,
                    packet_id: Some(1),
                    pc: Some(0x1000),
                },
                PacketFields {
                    tick: 7,
                    command: GEM5_READ_RESP_WITH_INVALIDATE,
                    address: Some(0x4000),
                    size: Some(8),
                    flags: None,
                    packet_id: Some(2),
                    pc: Some(0x1004),
                },
                PacketFields {
                    tick: 9,
                    command: GEM5_MEM_FENCE_RESP,
                    address: None,
                    size: None,
                    flags: None,
                    packet_id: Some(3),
                    pc: Some(0x1008),
                },
                PacketFields {
                    tick: 13,
                    command: GEM5_WRITE_REQ,
                    address: Some(0x30),
                    size: Some(4),
                    flags: None,
                    packet_id: Some(4),
                    pc: Some(0x100c),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(100);

    let read = match generator.next_event(100, 0).unwrap().unwrap() {
        TrafficTraceEvent::Request(event) => event,
        _ => panic!("read packet should emit a request event"),
    };
    assert_eq!(read.tick(), 105);
    assert_eq!(read.sequence(), 0);
    assert_eq!(read.kind(), TrafficRequestKind::Read);
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);

    let read_response = match generator.next_event(read.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Response(event) => event,
        _ => panic!("ReadRespWithInvalidate should emit a response event"),
    };
    assert_eq!(read_response.tick(), 107);
    assert_eq!(read_response.sequence(), 1);
    assert_eq!(
        read_response.kind(),
        TrafficTraceResponseKind::ReadWithInvalidate
    );
    assert_eq!(read_response.address(), Some(Address::new(0x4000)));
    assert_eq!(read_response.size_bytes(), Some(8));
    assert_eq!(read_response.trace_packet_id(), Some(2));
    assert_eq!(read_response.trace_pc(), Some(Address::new(0x1004)));

    let fence_response = match generator
        .next_event(read_response.tick(), 0)
        .unwrap()
        .unwrap()
    {
        TrafficTraceEvent::Response(event) => event,
        _ => panic!("MemFenceResp should emit a response event"),
    };
    assert_eq!(fence_response.tick(), 109);
    assert_eq!(fence_response.sequence(), 2);
    assert_eq!(fence_response.kind(), TrafficTraceResponseKind::MemFence);
    assert_eq!(fence_response.address(), None);
    assert_eq!(fence_response.size_bytes(), None);
    assert_eq!(fence_response.trace_packet_id(), Some(3));
    assert_eq!(fence_response.trace_pc(), Some(Address::new(0x1008)));

    let write = match generator
        .next_event(fence_response.tick(), 0)
        .unwrap()
        .unwrap()
    {
        TrafficTraceEvent::Request(event) => event,
        _ => panic!("write packet should emit a request event"),
    };
    assert_eq!(write.tick(), 113);
    assert_eq!(write.sequence(), 3);
    assert_eq!(write.kind(), TrafficRequestKind::Write);
    assert_eq!(write.request().operation(), MemoryOperation::Write);

    assert_eq!(generator.summary().packet_count(), 4);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
    assert_eq!(generator.summary().bytes_written(), 4);
    assert_eq!(generator.summary().first_tick(), Some(105));
    assert_eq!(generator.summary().last_tick(), Some(113));
}

#[test]
fn trace_generator_next_request_reports_response_event_boundary() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_READ_RESP,
                address: Some(0x4000),
                size: Some(8),
                flags: None,
                packet_id: None,
                pc: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(0);

    assert_eq!(
        generator.next_request(0, 0).unwrap_err(),
        TrafficGeneratorError::TraceResponseEventRequiresNextEvent {
            command: "ReadResp",
        }
    );

    let response = match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Response(event) => event,
        _ => panic!("ReadResp should remain pending"),
    };
    assert_eq!(response.tick(), 5);
    assert_eq!(response.sequence(), 0);
    assert_eq!(response.kind(), TrafficTraceResponseKind::Read);
    assert_eq!(response.address(), Some(Address::new(0x4000)));
    assert_eq!(response.size_bytes(), Some(8));
}

#[test]
fn trace_parser_rejects_response_flags() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 5,
                    command: GEM5_WRITE_RESP,
                    address: Some(0x4000),
                    size: Some(8),
                    flags: Some(GEM5_FLAG_PHYSICAL),
                    packet_id: None,
                    pc: None,
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags {
            flags: GEM5_FLAG_PHYSICAL,
        }
    );
}

#[test]
fn traffic_controller_emits_trace_response_event() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_WRITE_COMPLETE_RESP,
                address: Some(0x4000),
                size: Some(8),
                flags: None,
                packet_id: Some(9),
                pc: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap();
    let controller_config = TrafficControllerConfig::new(
        graph(vec![state(0, u64::MAX)], vec![transition(0, 0)]),
        vec![TrafficControllerState::new(
            TrafficStateId::new(0),
            TrafficStateGenerator::Trace(TrafficTraceGenerator::new(config)),
        )],
    )
    .unwrap();
    let mut controller = TrafficController::new(controller_config);

    assert!(controller.start(20).unwrap().is_empty());
    let batch = controller.next_event(20, 0).unwrap().unwrap();
    assert!(batch.request().is_none());
    assert!(batch.trace_sync().is_none());
    assert!(batch.trace_tlb().is_none());
    assert!(batch.trace_cache().is_none());
    assert!(batch.trace_htm().is_none());
    assert!(batch.trace_diagnostic().is_none());
    assert!(batch.trace_error().is_none());
    let response = batch.trace_response().unwrap();
    assert_eq!(response.tick(), 25);
    assert_eq!(response.sequence(), 0);
    assert_eq!(response.kind(), TrafficTraceResponseKind::WriteComplete);
    assert_eq!(response.address(), Some(Address::new(0x4000)));
    assert_eq!(response.size_bytes(), Some(8));
    assert_eq!(response.trace_packet_id(), Some(9));
}

#[test]
fn trace_generator_maps_all_gem5_response_kinds() {
    let commands = [
        GEM5_READ_RESP,
        GEM5_READ_RESP_WITH_INVALIDATE,
        GEM5_WRITE_RESP,
        GEM5_WRITE_COMPLETE_RESP,
        GEM5_SOFT_PF_RESP,
        GEM5_HARD_PF_RESP,
        GEM5_UPGRADE_RESP,
        GEM5_UPGRADE_FAIL_RESP,
        GEM5_READ_EX_RESP,
        GEM5_STORE_COND_RESP,
        GEM5_LOCKED_RMW_READ_RESP,
        GEM5_LOCKED_RMW_WRITE_RESP,
        GEM5_SWAP_RESP,
        GEM5_MEM_SYNC_RESP,
        GEM5_MEM_FENCE_RESP,
        GEM5_CLEAN_SHARED_RESP,
        GEM5_CLEAN_INVALID_RESP,
        GEM5_INVALIDATE_RESP,
        GEM5_HTM_REQ_RESP,
    ];
    let packets = commands
        .into_iter()
        .enumerate()
        .map(|(index, command)| PacketFields {
            tick: u64::try_from(index).unwrap() + 1,
            command,
            address: None,
            size: None,
            flags: None,
            packet_id: None,
            pc: None,
        })
        .collect::<Vec<_>>();
    let trace =
        TrafficTrace::from_gem5_packet_trace(&gem5_packet_trace(TICK_FREQUENCY, &packets), 1_000)
            .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(0);

    let kinds = (0..commands.len())
        .map(|_| match generator.next_event(0, 0).unwrap().unwrap() {
            TrafficTraceEvent::Response(event) => event.kind(),
            _ => panic!("gem5 response command should emit a response event"),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        vec![
            TrafficTraceResponseKind::Read,
            TrafficTraceResponseKind::ReadWithInvalidate,
            TrafficTraceResponseKind::Write,
            TrafficTraceResponseKind::WriteComplete,
            TrafficTraceResponseKind::SoftPrefetch,
            TrafficTraceResponseKind::HardPrefetch,
            TrafficTraceResponseKind::Upgrade,
            TrafficTraceResponseKind::UpgradeFail,
            TrafficTraceResponseKind::ReadExclusive,
            TrafficTraceResponseKind::StoreConditional,
            TrafficTraceResponseKind::LockedRmwRead,
            TrafficTraceResponseKind::LockedRmwWrite,
            TrafficTraceResponseKind::Swap,
            TrafficTraceResponseKind::MemSync,
            TrafficTraceResponseKind::MemFence,
            TrafficTraceResponseKind::CleanShared,
            TrafficTraceResponseKind::CleanInvalid,
            TrafficTraceResponseKind::Invalidate,
            TrafficTraceResponseKind::HtmRequest,
        ]
    );
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
        if let Some(flags) = packet.flags {
            append_key(&mut message, 5, 0);
            append_varint(&mut message, u64::from(flags));
        }
        if let Some(packet_id) = packet.packet_id {
            append_key(&mut message, 6, 0);
            append_varint(&mut message, packet_id);
        }
        if let Some(pc) = packet.pc {
            append_key(&mut message, 7, 0);
            append_varint(&mut message, pc);
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
