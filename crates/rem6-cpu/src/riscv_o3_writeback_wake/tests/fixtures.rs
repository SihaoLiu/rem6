use super::*;

pub(super) fn scalar_load_event(
    pc: u64,
    sequence: u64,
    destination: u8,
    address: u64,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::Load {
        rd: register(destination),
        rs1: register(2),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
        signed: false,
    };
    let access = MemoryAccessKind::Load {
        rd: register(destination),
        address,
        width: MemoryWidth::Word,
        signed: false,
    };
    RiscvCpuExecutionEvent::new(
        CpuFetchEvent::completed(
            CpuFetchRecord::new(
                sequence,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                memory_request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            0x0000_0073_u32.to_le_bytes().to_vec(),
        ),
        instruction,
        RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
    )
}
