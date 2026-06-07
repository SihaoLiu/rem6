#![allow(dead_code)]

use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficStateGenerator,
    TrafficStateGeneratorSnapshot, TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec,
    TrafficTrace, TrafficTraceConfig, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};
use rem6_transport::TransportEndpointId;

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;

pub const GEM5_READ_REQ: u32 = 1;
pub const GEM5_READ_RESP: u32 = 2;
pub const GEM5_READ_RESP_WITH_INVALIDATE: u32 = 3;
pub const GEM5_WRITE_REQ: u32 = 4;
pub const GEM5_WRITE_RESP: u32 = 5;
pub const GEM5_WRITEBACK_DIRTY: u32 = 7;
pub const GEM5_SOFT_PF_REQ: u32 = 11;
pub const GEM5_SOFT_PF_RESP: u32 = 14;
pub const GEM5_STORE_COND_FAIL_REQ: u32 = 28;
pub const GEM5_STORE_COND_RESP: u32 = 29;
pub const GEM5_MEM_FENCE_REQ: u32 = 38;
pub const GEM5_MEM_FENCE_RESP: u32 = 41;
pub const GEM5_CLEAN_SHARED_REQ: u32 = 42;
pub const GEM5_CLEAN_SHARED_RESP: u32 = 43;
pub const GEM5_CLEAN_INVALID_REQ: u32 = 44;
pub const GEM5_CLEAN_INVALID_RESP: u32 = 45;
pub const GEM5_INVALID_DEST_ERROR: u32 = 46;
pub const GEM5_WRITE_ERROR: u32 = 49;
pub const GEM5_PRINT_REQ: u32 = 52;
pub const GEM5_FLUSH_REQ: u32 = 53;
pub const GEM5_HTM_REQ: u32 = 56;
pub const GEM5_HTM_REQ_RESP: u32 = 57;
pub const GEM5_HTM_ABORT: u32 = 58;
pub const GEM5_TLBI_EXT_SYNC: u32 = 59;

#[derive(Clone, Copy)]
pub struct PacketFields {
    pub tick: u64,
    pub command: u32,
    pub address: Option<u64>,
    pub size: Option<u32>,
    pub packet_id: Option<u64>,
}

pub fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

pub fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

pub fn request(sequence: u64) -> MemoryRequest {
    request_from(1, sequence)
}

pub fn request_from(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(agent), sequence),
        Address::new(0x4000 + sequence * 0x40),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap()
}

pub fn completed_response(request: &MemoryRequest, data: &[u8]) -> MemoryResponse {
    MemoryResponse::completed(request, Some(data.to_vec())).unwrap()
}

pub fn controller_for_packets(packets: &[PacketFields]) -> TrafficController {
    controller_for_packets_with_state_duration(packets, u64::MAX)
}

pub fn controller_for_packets_with_state_duration(
    packets: &[PacketFields],
    duration: u64,
) -> TrafficController {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(TICK_FREQUENCY, packets),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap();
    let controller_config = TrafficControllerConfig::new(
        graph(vec![state(0, duration)], vec![transition(0, 0)]),
        vec![TrafficControllerState::new(
            TrafficStateId::new(0),
            TrafficStateGenerator::Trace(rem6_traffic::TrafficTraceGenerator::new(config)),
        )],
    )
    .unwrap();
    TrafficController::new(controller_config)
}

pub fn trace_cursor(controller: &TrafficController) -> usize {
    match controller.snapshot().generators()[0].generator() {
        TrafficStateGeneratorSnapshot::Trace(snapshot) => snapshot.cursor(),
        _ => panic!("traffic replay test controller must use trace generator"),
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
