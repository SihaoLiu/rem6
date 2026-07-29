use super::*;

pub(super) fn project_pending_live_fetch(
    state: &crate::RiscvCoreState,
    owner_rows: &[(u64, MemoryRequestId)],
    expected_requests: &[MemoryRequestId],
    pending: &[super::super::RiscvO3LiveCheckpointPendingDataAddress],
    completed_fetches: &[&crate::CpuFetchEvent],
) -> Result<LiveFetchProjection, RiscvO3LiveCheckpointError> {
    let expected_owner = pending
        .iter()
        .map(|row| (row.sequence, row.fetch.request_id()))
        .collect::<Vec<_>>();
    let expected_pending_requests = pending
        .iter()
        .map(|row| row.fetch.request_id())
        .collect::<Vec<_>>();
    let Some(first) = pending.first() else {
        return Err(invalid("pending-address profile lacks its pending fetch"));
    };
    let Some(last) = pending.last() else {
        return Err(invalid("pending-address profile lacks its pending fetch"));
    };
    let first_pc = first.fetch.pc().get();
    let next_pc = last
        .fetch
        .pc()
        .get()
        .checked_add(4)
        .ok_or(invalid("pending-address PC overflows"))?;
    if owner_rows != expected_owner
        || expected_requests != expected_pending_requests
        || completed_fetches.len() != pending.len()
        || !matches!(state.hart.pc(), pc if pc == first_pc || pc == next_pc)
    {
        return Err(invalid("pending-address fetch ownership is inconsistent"));
    }
    for (index, (pending, completed_fetch)) in pending.iter().zip(completed_fetches).enumerate() {
        let decoded = decode_pending_instruction(pending)?;
        let request = pending.fetch.request_id();
        let expected_pc = first_pc
            .checked_add(4 * index as u64)
            .ok_or(invalid("pending-address PC overflows"))?;
        if **completed_fetch != pending.fetch
            || pending.fetch.pc().get() != expected_pc
            || !state
                .o3_runtime
                .checkpoint_live_instruction_matches(pending.sequence, decoded.instruction())
            || state
                .events
                .iter()
                .any(|event| event.fetch().request_id() == request)
            || state.executed_fetches.contains(&request)
            || state.issued_data_for_fetches.contains(&request)
        {
            return Err(invalid("pending-address fetch ownership is inconsistent"));
        }
    }
    let mut projected_hart = state.hart.clone();
    projected_hart.set_pc(first_pc);
    Ok(LiveFetchProjection {
        events: Vec::new(),
        executed_fetch_requests: Vec::new(),
        issued_fetch_requests: Vec::new(),
        projected_hart: Some(projected_hart),
    })
}

fn decode_pending_instruction(
    pending: &super::super::RiscvO3LiveCheckpointPendingDataAddress,
) -> Result<rem6_isa_riscv::RiscvDecodedInstruction, RiscvO3LiveCheckpointError> {
    let raw: [u8; 4] = pending
        .fetch
        .data()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(invalid(
            "pending-address fetch is not an uncompressed instruction",
        ))?;
    let decoded = rem6_isa_riscv::RiscvInstruction::decode_with_length(u32::from_le_bytes(raw))
        .map_err(|_| invalid("pending-address instruction does not decode"))?;
    let valid = match pending.destination {
        Some(destination) => {
            let rd = rem6_isa_riscv::Register::new(destination.architectural() as u8)
                .map_err(|_| invalid("pending load destination is not a scalar register"))?;
            destination.register_class() == crate::O3RegisterClass::Integer
                && !destination.physical().is_invalid()
                && matches!(
                    decoded.instruction(),
                    rem6_isa_riscv::RiscvInstruction::Load {
                        rd: load_rd,
                        rs1,
                        width: rem6_isa_riscv::MemoryWidth::Doubleword,
                        ..
                    } if load_rd == rd && rs1 == pending.producer_register && !load_rd.is_zero()
                )
        }
        None => matches!(
            decoded.instruction(),
            rem6_isa_riscv::RiscvInstruction::Store {
                rs1,
                rs2,
                width: rem6_isa_riscv::MemoryWidth::Doubleword,
                ..
            } if rs1 == pending.producer_register && !rs2.is_zero() && rs1 != rs2
        ),
    };
    if decoded.bytes() != 4 || !valid {
        return Err(invalid("pending-address instruction shape is inconsistent"));
    }
    Ok(decoded)
}
