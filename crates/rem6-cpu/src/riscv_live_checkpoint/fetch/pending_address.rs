use super::*;

pub(super) fn project_pending_live_fetch(
    state: &crate::RiscvCoreState,
    owner_rows: &[(u64, MemoryRequestId)],
    expected_requests: &[MemoryRequestId],
    pending: &super::super::RiscvO3LiveCheckpointPendingDataAddress,
    completed_fetches: &[&crate::CpuFetchEvent],
) -> Result<LiveFetchProjection, RiscvO3LiveCheckpointError> {
    let expected_owner = [(pending.sequence, pending.fetch.request_id())];
    let [completed_fetch] = completed_fetches else {
        return Err(invalid(
            "pending store does not own exactly one completed fetch",
        ));
    };
    let raw: [u8; 4] = completed_fetch
        .data()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or(invalid(
            "pending store fetch is not an uncompressed instruction",
        ))?;
    let decoded = rem6_isa_riscv::RiscvInstruction::decode_with_length(u32::from_le_bytes(raw))
        .map_err(|_| invalid("pending store instruction does not decode"))?;
    let pending_pc = pending.fetch.pc().get();
    let next_pc = pending_pc
        .checked_add(4)
        .ok_or(invalid("pending store PC overflows"))?;
    if owner_rows != expected_owner
        || expected_requests != [pending.fetch.request_id()]
        || **completed_fetch != pending.fetch
        || decoded.bytes() != 4
        || !matches!(
            decoded.instruction(),
            rem6_isa_riscv::RiscvInstruction::Store {
                rs1,
                rs2,
                width: rem6_isa_riscv::MemoryWidth::Doubleword,
                ..
            } if rs1 == pending.producer_register && !rs2.is_zero() && rs1 != rs2
        )
        || !state
            .o3_runtime
            .checkpoint_live_instruction_matches(pending.sequence, decoded.instruction())
        || state
            .events
            .iter()
            .any(|event| event.fetch().request_id() == pending.fetch.request_id())
        || state.executed_fetches.contains(&pending.fetch.request_id())
        || state
            .issued_data_for_fetches
            .contains(&pending.fetch.request_id())
        || !matches!(state.hart.pc(), pc if pc == pending_pc || pc == next_pc)
    {
        return Err(invalid("pending store fetch ownership is inconsistent"));
    }
    let mut projected_hart = state.hart.clone();
    projected_hart.set_pc(pending_pc);
    Ok(LiveFetchProjection {
        events: Vec::new(),
        executed_fetch_requests: Vec::new(),
        issued_fetch_requests: Vec::new(),
        projected_hart: Some(projected_hart),
    })
}
