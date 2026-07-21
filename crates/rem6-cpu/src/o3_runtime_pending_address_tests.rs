use super::*;

use crate::{
    riscv_fetch_ahead::O3MemoryResultWindowRole,
    riscv_live_retire_window::stage_o3_data_access_younger_window, CpuCore, CpuFetchConfig,
    CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState, RiscvCore,
};
use rem6_isa_riscv::{
    Immediate, MemoryAccessKind, MemoryWidth, Register, RiscvDecodedInstruction,
    RiscvExecutionRecord, RiscvInstruction,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

#[path = "o3_runtime_pending_address_tests/staging.rs"]
mod staging;

const HEAD_PC: u64 = 0x8000;
const PENDING_PC: u64 = 0x8004;
const FIRST_SUFFIX_PC: u64 = 0x8008;
const SECOND_SUFFIX_PC: u64 = 0x800c;
const EXTRA_SUFFIX_PC: u64 = 0x8010;

#[derive(Clone)]
struct PendingAddressFixture {
    runtime: O3RuntimeState,
    head_fetch: MemoryRequestId,
    pending: O3PendingDataAddressRequest,
    suffix: Vec<(Address, RiscvInstruction)>,
}

impl PendingAddressFixture {
    fn new(scalar_memory_depth: usize, scalar_live_depth: usize) -> Self {
        let mut runtime = O3RuntimeState::default();
        assert!(runtime.set_window_depths(scalar_memory_depth, scalar_live_depth));
        let head = load_event(HEAD_PC, 10, 5, 2, 0x9000);
        assert!(runtime.stage_live_data_access_issue(
            &head,
            request(20),
            31,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
        Self {
            runtime,
            head_fetch: head.fetch().request_id(),
            pending: pending_request(11, PENDING_PC, ld(6, 5, 0), reg(5)),
            suffix: vec![
                staged_instruction(FIRST_SUFFIX_PC, addi(7, 5, 8)),
                staged_instruction(SECOND_SUFFIX_PC, add(8, 6, 7)),
            ],
        }
    }

    fn stage_default(&mut self) -> usize {
        self.runtime.stage_pending_data_address_window(
            self.head_fetch,
            self.pending.clone(),
            self.suffix.clone(),
        )
    }
}

fn pending_address_core_fixture(
    authorization_dependent: u32,
    staged_dependent: u32,
) -> (RiscvCore, RiscvCpuExecutionEvent, Vec<CpuFetchEvent>) {
    pending_address_core_fixture_with_live_depth(authorization_dependent, staged_dependent, 4)
}

fn pending_address_core_fixture_with_live_depth(
    authorization_dependent: u32,
    staged_dependent: u32,
    scalar_live_depth: usize,
) -> (RiscvCore, RiscvCpuExecutionEvent, Vec<CpuFetchEvent>) {
    let core = core_with_completed_fetches([
        (10, HEAD_PC, ld(5, 2, 0)),
        (11, PENDING_PC, authorization_dependent),
        (12, FIRST_SUFFIX_PC, addi(7, 5, 8)),
        (13, SECOND_SUFFIX_PC, add(8, 6, 7)),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_window_depths(4, scalar_live_depth);
    core.write_register(reg(2), 0x9000);
    assert_eq!(core.next_fetch_ahead_before_retire(), None);

    let mut fetch_events = core
        .core
        .state
        .lock()
        .expect("cpu core lock")
        .events
        .clone();
    if authorization_dependent != staged_dependent {
        let dependent_fetch = fetch_events
            .iter_mut()
            .find(|event| event.request_id() == request(11))
            .expect("dependent fetch event");
        *dependent_fetch = fetch_event_with_raw(PENDING_PC, 11, staged_dependent);
    }
    let head = load_event(HEAD_PC, 10, 5, 2, 0x9000);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state
                .memory_result_window_authorizations
                .get(&request(11))
                .copied()
                .map(|authorization| authorization.role()),
            Some(O3MemoryResultWindowRole::YoungerDependentRead)
        );
        assert!(state.o3_runtime.stage_live_data_access_issue(
            &head,
            request(20),
            31,
            O3DataAccessWindowPolicy::MemoryResultWindow,
        ));
    }
    (core, head, fetch_events)
}

fn core_with_completed_fetches(fetches: impl IntoIterator<Item = (u64, u64, u32)>) -> RiscvCore {
    let core = RiscvCore::new(
        CpuCore::new(
            CpuResetState::new(
                CpuId::new(0),
                PartitionId::new(0),
                AgentId::new(7),
                Address::new(HEAD_PC),
            ),
            CpuFetchConfig::new(
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRouteId::new(0),
                CacheLineLayout::new(16).unwrap(),
                AccessSize::new(4).unwrap(),
            ),
        )
        .unwrap(),
    );
    let mut core_state = core.core.state.lock().expect("cpu core lock");
    for (sequence, pc, raw) in fetches {
        core_state
            .events
            .push(fetch_event_with_raw(pc, sequence, raw));
    }
    drop(core_state);
    core
}

fn load_event(pc: u64, sequence: u64, rd: u8, rs1: u8, address: u64) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: reg(rd),
        rs1: reg(rs1),
        offset: Immediate::new(0),
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: reg(rd),
        address,
        width: MemoryWidth::Doubleword,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event_with_raw(pc, sequence, ld(rd, rs1, 0)),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}

fn pending_request(
    sequence: u64,
    pc: u64,
    raw: u32,
    producer_register: Register,
) -> O3PendingDataAddressRequest {
    O3PendingDataAddressRequest::new(
        fetch_event_with_raw(pc, sequence, raw),
        vec![request(sequence)],
        decoded(raw),
        producer_register,
    )
}

fn staged_instruction(pc: u64, raw: u32) -> (Address, RiscvInstruction) {
    (Address::new(pc), decoded(raw).instruction())
}

fn decoded(raw: u32) -> RiscvDecodedInstruction {
    RiscvInstruction::decode_with_length(raw).expect("test instruction decodes")
}

fn fetch_event_with_raw(pc: u64, sequence: u64, raw: u32) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        raw.to_le_bytes().to_vec(),
    )
}

fn ld(rd: u8, rs1: u8, offset: i32) -> u32 {
    i_type(i64::from(offset), rs1, 0b011, rd, 0x03)
}

fn addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(i64::from(imm), rs1, 0b000, rd, 0x13)
}

fn add(rd: u8, rs1: u8, rs2: u8) -> u32 {
    r_type(0, rs2, rs1, 0b000, rd, 0x33)
}

fn i_type(imm: i64, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    ((imm as u32 & 0xfff) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn integer_mapping(runtime: &O3RuntimeState, architectural: u32) -> Option<O3PhysicalRegisterId> {
    runtime
        .snapshot()
        .rename_map()
        .iter()
        .find(|entry| {
            entry.register_class() == O3RegisterClass::Integer
                && entry.architectural() == architectural
        })
        .map(|entry| entry.physical())
}

fn pc_rows(runtime: &O3RuntimeState) -> Vec<Address> {
    runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .map(|entry| entry.pc())
        .collect()
}
