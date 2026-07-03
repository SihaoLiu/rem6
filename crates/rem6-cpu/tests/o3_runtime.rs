use rem6_cpu::{
    CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState,
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueId, O3LoadStoreQueueEntry,
    O3PendingStateCheckpointPayload, O3PendingStateSnapshot, O3PhysicalRegisterId, O3PipelineStage,
    O3RegisterClass, O3RenameMapEntry, O3ReorderBufferEntry, O3RuntimeCheckpointPayload,
    O3ScopedReadyInstruction, O3WritebackCompletion, O3WritebackTransferPolicy,
    O3WritebackTransferSnapshot, RiscvCore, RiscvCpuExecutionEvent,
};
use rem6_isa_riscv::{
    AtomicMemoryOp, Immediate, MemoryAccessKind, MemoryWidth, Register, RegisterWrite,
    RiscvExecutionRecord, RiscvInstruction, RiscvVectorMaskMode, RiscvVectorMemoryInstruction,
    VectorRegister,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address, AgentId, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

const O3_RUNTIME_CHECKPOINT_MAGIC_BYTES: usize = 4;
const O3_RUNTIME_CHECKPOINT_VERSION_BYTES: usize = 1;
const O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET: usize =
    O3_RUNTIME_CHECKPOINT_MAGIC_BYTES + O3_RUNTIME_CHECKPOINT_VERSION_BYTES;
const O3_RUNTIME_CHECKPOINT_HEADER_BYTES: usize =
    O3_RUNTIME_CHECKPOINT_MAGIC_BYTES + O3_RUNTIME_CHECKPOINT_VERSION_BYTES + 4 * 4;
const O3_RUNTIME_ROB_DESTINATION_PRESENT_OFFSET: usize = 8 + 8;
const O3_RUNTIME_ROB_READY_OFFSET: usize = O3_RUNTIME_ROB_DESTINATION_PRESENT_OFFSET + 1 + 4;

#[test]
fn o3_runtime_checkpoint_round_trips_rob_lsq_rename_and_pending_state() {
    let pending_scope = O3DependencyScopeId::new(0x44);
    let produced_scope = O3DependencyScopeId::new(0x55);
    let snapshot = rem6_cpu::O3RuntimeSnapshot::new(
        [
            O3ReorderBufferEntry::new(
                10,
                Address::new(0x8000),
                Some(O3PhysicalRegisterId::new(40)),
            )
            .with_ready(true),
            O3ReorderBufferEntry::new(11, Address::new(0x8004), None),
        ],
        [
            O3LoadStoreQueueEntry::load(10, Some(Address::new(0x9000)), 8).with_completed(true),
            O3LoadStoreQueueEntry::store(11, None, 4),
        ],
        [
            O3RenameMapEntry::new(O3RegisterClass::Integer, 1, O3PhysicalRegisterId::new(40)),
            O3RenameMapEntry::new(
                O3RegisterClass::FloatingPoint,
                2,
                O3PhysicalRegisterId::new(80),
            ),
        ],
        O3PendingStateSnapshot::new(
            [pending_scope],
            [
                O3ScopedReadyInstruction::new(12, O3IssueQueueId::new(0), O3IssueOpClass::IntAlu)
                    .with_waits_on([pending_scope])
                    .with_produces([produced_scope]),
            ],
            O3WritebackTransferSnapshot::new(
                O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 2, 0).unwrap(),
                [O3WritebackCompletion::new(13)],
            ),
        )
        .unwrap(),
    )
    .unwrap();
    let payload = O3RuntimeCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();
    let decoded = O3RuntimeCheckpointPayload::decode(payload.encode().as_slice()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(decoded.snapshot().reorder_buffer()[0].sequence(), 10);
    assert_eq!(
        decoded.snapshot().load_store_queue()[0].address(),
        Some(Address::new(0x9000))
    );
    assert_eq!(
        decoded.snapshot().rename_map()[0].physical(),
        O3PhysicalRegisterId::new(40)
    );
    let pending_payload =
        O3PendingStateCheckpointPayload::from_snapshot(decoded.snapshot().pending_state().clone())
            .unwrap();
    assert_eq!(
        pending_payload.snapshot().resolved_dependency_scopes(),
        &[pending_scope]
    );
}

#[test]
fn o3_runtime_checkpoint_rejects_invalid_bool_bytes() {
    let payload = O3RuntimeCheckpointPayload::from_snapshot(
        rem6_cpu::O3RuntimeSnapshot::new(
            [O3ReorderBufferEntry::new(
                1,
                Address::new(0x8000),
                Some(O3PhysicalRegisterId::new(40)),
            )
            .with_ready(true)],
            [],
            [],
            O3PendingStateSnapshot::new(
                [],
                [],
                O3WritebackTransferSnapshot::new(
                    O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0).unwrap(),
                    [],
                ),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
    .encode();
    let rob_offset = o3_runtime_rob_payload_offset(&payload);

    let mut invalid_destination_present = payload.clone();
    invalid_destination_present[rob_offset + O3_RUNTIME_ROB_DESTINATION_PRESENT_OFFSET] = 2;
    assert!(matches!(
        O3RuntimeCheckpointPayload::decode(&invalid_destination_present),
        Err(rem6_cpu::O3RuntimeError::InvalidCheckpointBool {
            field: "ROB destination-present",
            value: 2
        })
    ));

    let mut invalid_ready = payload;
    invalid_ready[rob_offset + O3_RUNTIME_ROB_READY_OFFSET] = 2;
    assert!(matches!(
        O3RuntimeCheckpointPayload::decode(&invalid_ready),
        Err(rem6_cpu::O3RuntimeError::InvalidCheckpointBool {
            field: "ROB ready",
            value: 2
        })
    ));
}

#[test]
fn o3_runtime_snapshot_rejects_duplicate_rob_lsq_and_rename_entries() {
    let pending = O3PendingStateSnapshot::new(
        [],
        [],
        O3WritebackTransferSnapshot::new(
            O3WritebackTransferPolicy::new(O3PipelineStage::Iew, 1, 0).unwrap(),
            [],
        ),
    )
    .unwrap();

    assert!(rem6_cpu::O3RuntimeSnapshot::new(
        [
            O3ReorderBufferEntry::new(1, Address::new(0x8000), None),
            O3ReorderBufferEntry::new(1, Address::new(0x8004), None),
        ],
        [],
        [],
        pending.clone(),
    )
    .is_err());

    assert!(rem6_cpu::O3RuntimeSnapshot::new(
        [],
        [
            O3LoadStoreQueueEntry::load(2, Some(Address::new(0x9000)), 4),
            O3LoadStoreQueueEntry::store(2, Some(Address::new(0x9008)), 8),
        ],
        [],
        pending.clone(),
    )
    .is_err());

    assert!(rem6_cpu::O3RuntimeSnapshot::new(
        [],
        [],
        [
            O3RenameMapEntry::new(O3RegisterClass::Integer, 1, O3PhysicalRegisterId::new(10)),
            O3RenameMapEntry::new(O3RegisterClass::Integer, 1, O3PhysicalRegisterId::new(11)),
        ],
        pending,
    )
    .is_err());
}

#[test]
fn o3_runtime_stats_count_grouped_vector_segment_load_rename_destinations() {
    let core = RiscvCore::new(core(0x8000));
    let instruction =
        RiscvInstruction::VectorMemory(RiscvVectorMemoryInstruction::LoadSegmentUnitStride {
            vd: vreg(8),
            rs1: reg(10),
            width: MemoryWidth::Word,
            fields: 3,
            mask: RiscvVectorMaskMode::Unmasked,
        });
    let access = MemoryAccessKind::VectorLoadSegmentUnitStride {
        vd: vreg(8),
        address: 0x9000,
        width: MemoryWidth::Word,
        fields: 3,
        element_count: 2,
        byte_len: 48,
        byte_mask: None,
        group_registers: 2,
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 1),
        instruction,
        RiscvExecutionRecord::new(instruction, 0x8000, 0x8004, Vec::new(), Some(access)),
    ));

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.instructions(), 1);
    assert_eq!(stats.rob_allocations(), 1);
    assert_eq!(stats.rob_commits(), 1);
    assert_eq!(stats.rename_writes(), 6);
    assert_eq!(stats.lsq_loads(), 1);
    assert_eq!(stats.lsq_stores(), 0);
}

#[test]
fn o3_runtime_stats_ignore_x0_memory_destinations_for_rename_writes() {
    let core = RiscvCore::new(core(0x8000));
    let cases = [
        (
            RiscvInstruction::Load {
                rd: reg(0),
                rs1: reg(10),
                offset: Immediate::new(0),
                width: MemoryWidth::Word,
                signed: false,
            },
            MemoryAccessKind::Load {
                rd: reg(0),
                address: 0x9000,
                width: MemoryWidth::Word,
                signed: false,
            },
        ),
        (
            RiscvInstruction::LoadReserved {
                rd: reg(0),
                rs1: reg(10),
                width: MemoryWidth::Word,
                acquire: false,
                release: false,
            },
            MemoryAccessKind::LoadReserved {
                rd: reg(0),
                address: 0x9004,
                width: MemoryWidth::Word,
                acquire: false,
                release: false,
            },
        ),
        (
            RiscvInstruction::StoreConditional {
                rd: reg(0),
                rs1: reg(10),
                rs2: reg(11),
                width: MemoryWidth::Word,
                acquire: false,
                release: false,
            },
            MemoryAccessKind::StoreConditional {
                rd: reg(0),
                address: 0x9008,
                width: MemoryWidth::Word,
                value: 0x1234,
                acquire: false,
                release: false,
            },
        ),
        (
            RiscvInstruction::AtomicMemory {
                rd: reg(0),
                rs1: reg(10),
                rs2: reg(11),
                width: MemoryWidth::Word,
                op: AtomicMemoryOp::Add,
                acquire: false,
                release: false,
            },
            MemoryAccessKind::AtomicMemory {
                rd: reg(0),
                address: 0x900c,
                width: MemoryWidth::Word,
                op: AtomicMemoryOp::Add,
                value: 0x5678,
                acquire: false,
                release: false,
            },
        ),
    ];

    for (index, (instruction, access)) in cases.into_iter().enumerate() {
        let pc = 0x8000 + u64::try_from(index).unwrap() * 4;
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, 10 + u64::try_from(index).unwrap()),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
        ));
    }

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.instructions(), 4);
    assert_eq!(stats.rob_allocations(), 4);
    assert_eq!(stats.rob_commits(), 4);
    assert_eq!(stats.rename_writes(), 0);
    assert_eq!(stats.lsq_loads(), 3);
    assert_eq!(stats.lsq_stores(), 2);
}

#[test]
fn o3_runtime_stats_count_same_address_store_load_forwarding_match() {
    let core = RiscvCore::new(core(0x8000));
    let store = RiscvInstruction::Store {
        rs1: reg(10),
        rs2: reg(11),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    };
    let load = RiscvInstruction::Load {
        rd: reg(12),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 20),
        store,
        RiscvExecutionRecord::new(
            store,
            0x8000,
            0x8004,
            Vec::new(),
            Some(MemoryAccessKind::Store {
                address: 0x9000,
                width: MemoryWidth::Word,
                value: 0x5a,
            }),
        ),
    ));
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 21),
        load,
        RiscvExecutionRecord::new(
            load,
            0x8004,
            0x8008,
            Vec::new(),
            Some(MemoryAccessKind::Load {
                rd: reg(12),
                address: 0x9000,
                width: MemoryWidth::Word,
                signed: false,
            }),
        ),
    ));
    core.record_o3_completed_load_data(
        memory_request(21),
        &MemoryAccessKind::Load {
            rd: reg(12),
            address: 0x9000,
            width: MemoryWidth::Word,
            signed: false,
        },
        &[0x5a, 0, 0, 0],
    );

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.lsq_loads(), 1);
    assert_eq!(stats.lsq_stores(), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 1);
}

#[test]
fn o3_runtime_reset_stats_clears_store_forwarding_window() {
    let core = RiscvCore::new(core(0x8000));
    let store = scalar_store_instruction();
    let load = scalar_load_instruction();
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 20),
        store,
        RiscvExecutionRecord::new(
            store,
            0x8000,
            0x8004,
            Vec::new(),
            Some(scalar_store_access(0x5a)),
        ),
    ));

    core.reset_o3_runtime_stats();
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 21),
        load,
        RiscvExecutionRecord::new(load, 0x8004, 0x8008, Vec::new(), Some(scalar_load_access())),
    ));
    core.record_o3_completed_load_data(memory_request(21), &scalar_load_access(), &[0x5a, 0, 0, 0]);

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.lsq_loads(), 1);
    assert_eq!(stats.lsq_stores(), 0);
    assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 0);
    assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 0);
}

#[test]
fn o3_runtime_reset_stats_clears_pending_store_load_match() {
    let core = RiscvCore::new(core(0x8000));
    let store = scalar_store_instruction();
    let load = scalar_load_instruction();
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 20),
        store,
        RiscvExecutionRecord::new(
            store,
            0x8000,
            0x8004,
            Vec::new(),
            Some(scalar_store_access(0x5a)),
        ),
    ));
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 21),
        load,
        RiscvExecutionRecord::new(load, 0x8004, 0x8008, Vec::new(), Some(scalar_load_access())),
    ));

    core.reset_o3_runtime_stats();
    core.record_o3_completed_load_data(memory_request(21), &scalar_load_access(), &[0x5a, 0, 0, 0]);

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 0);
    assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 0);
}

#[test]
fn o3_runtime_completed_load_match_uses_fetch_request_identity() {
    let core = RiscvCore::new(core(0x8000));
    let store = scalar_store_instruction();
    let load = scalar_load_instruction();
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 20),
        store,
        RiscvExecutionRecord::new(
            store,
            0x8000,
            0x8004,
            Vec::new(),
            Some(scalar_store_access(0x5a)),
        ),
    ));
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 21),
        load,
        RiscvExecutionRecord::new(load, 0x8004, 0x8008, Vec::new(), Some(scalar_load_access())),
    ));

    core.record_o3_completed_load_data(memory_request(99), &scalar_load_access(), &[0x5a, 0, 0, 0]);
    assert_eq!(
        core.o3_runtime_stats()
            .lsq_store_to_load_forwarding_matches(),
        0
    );

    core.record_o3_completed_load_data(memory_request(21), &scalar_load_access(), &[0x5a, 0, 0, 0]);

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 1);
}

#[test]
fn o3_runtime_completed_load_mismatch_keeps_candidate_without_match() {
    let core = RiscvCore::new(core(0x8000));
    let store = scalar_store_instruction();
    let load = scalar_load_instruction();
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8000, 20),
        store,
        RiscvExecutionRecord::new(
            store,
            0x8000,
            0x8004,
            Vec::new(),
            Some(scalar_store_access(0x5a)),
        ),
    ));
    core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
        fetch_event(0x8004, 21),
        load,
        RiscvExecutionRecord::new(load, 0x8004, 0x8008, Vec::new(), Some(scalar_load_access())),
    ));
    core.record_o3_completed_load_data(memory_request(21), &scalar_load_access(), &[0xa5, 0, 0, 0]);

    let stats = core.o3_runtime_stats();
    assert_eq!(stats.lsq_store_to_load_forwarding_candidates(), 1);
    assert_eq!(stats.lsq_store_to_load_forwarding_matches(), 0);
}

#[test]
fn o3_runtime_checkpoint_encoding_is_stable_after_out_of_order_rename_retire() {
    let core = RiscvCore::new(core(0x8000));
    for (index, register) in [reg(11), reg(5)].into_iter().enumerate() {
        let pc = 0x8000 + u64::try_from(index).unwrap() * 4;
        let instruction = RiscvInstruction::Addi {
            rd: register,
            rs1: reg(0),
            imm: Immediate::new(i64::try_from(index).unwrap()),
        };
        core.record_o3_retired_instruction(&RiscvCpuExecutionEvent::new(
            fetch_event(pc, 20 + u64::try_from(index).unwrap()),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                pc,
                pc + 4,
                vec![RegisterWrite::new(register, u64::try_from(index).unwrap())],
                None,
            ),
        ));
    }

    let encoded = core.o3_runtime_checkpoint_payload().encode();
    let decoded = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

    assert_eq!(decoded.encode(), encoded);
    assert_eq!(decoded.snapshot().rename_map()[0].architectural(), 5);
    assert_eq!(decoded.snapshot().rename_map()[1].architectural(), 11);
}

fn o3_runtime_rob_payload_offset(payload: &[u8]) -> usize {
    let pending_len_bytes = payload
        [O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET..O3_RUNTIME_CHECKPOINT_PENDING_LEN_OFFSET + 4]
        .try_into()
        .unwrap();
    O3_RUNTIME_CHECKPOINT_HEADER_BYTES + u32::from_le_bytes(pending_len_bytes) as usize
}

fn core(entry: u64) -> CpuCore {
    CpuCore::new(
        CpuResetState::new(
            CpuId::new(0),
            PartitionId::new(0),
            AgentId::new(7),
            Address::new(entry),
        ),
        CpuFetchConfig::new(
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            MemoryRouteId::new(0),
            rem6_memory::CacheLineLayout::new(64).unwrap(),
            AccessSize::new(4).unwrap(),
        ),
    )
    .unwrap()
}

fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
    CpuFetchEvent::completed(
        CpuFetchRecord::new(
            10 + sequence,
            PartitionId::new(0),
            MemoryRouteId::new(0),
            TransportEndpointId::new("cpu0.ifetch").unwrap(),
            memory_request(sequence),
            Address::new(pc),
            AccessSize::new(4).unwrap(),
        ),
        0x0000_0073u32.to_le_bytes().to_vec(),
    )
}

fn memory_request(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn scalar_store_instruction() -> RiscvInstruction {
    RiscvInstruction::Store {
        rs1: reg(10),
        rs2: reg(11),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    }
}

fn scalar_load_instruction() -> RiscvInstruction {
    RiscvInstruction::Load {
        rd: reg(12),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    }
}

fn scalar_store_access(value: u64) -> MemoryAccessKind {
    MemoryAccessKind::Store {
        address: 0x9000,
        width: MemoryWidth::Word,
        value,
    }
}

fn scalar_load_access() -> MemoryAccessKind {
    MemoryAccessKind::Load {
        rd: reg(12),
        address: 0x9000,
        width: MemoryWidth::Word,
        signed: false,
    }
}

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn vreg(index: u8) -> VectorRegister {
    VectorRegister::new(index).unwrap()
}
