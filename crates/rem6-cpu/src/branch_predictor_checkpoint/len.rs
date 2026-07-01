use super::{
    checked_offset, checked_product, checked_sum, read_u32, read_u64, BranchPredictorError,
    ReturnAddressStackSnapshot, CHECKPOINT_ACTIVE_SPECULATION_BYTES,
    CHECKPOINT_ACTIVE_SPECULATION_V3_BYTES, CHECKPOINT_ACTIVE_SPECULATION_V4_BYTES,
    CHECKPOINT_ACTIVE_SPECULATION_V5_BYTES, CHECKPOINT_BTB_ENTRY_BYTES,
    CHECKPOINT_BTB_HEADER_BYTES, CHECKPOINT_BTB_LEGACY_HEADER_BYTES, CHECKPOINT_COUNTER_BYTES,
    CHECKPOINT_HEADER_BYTES, CHECKPOINT_PENDING_SPECULATION_BYTES, CHECKPOINT_RAS_HEADER_BYTES,
    CHECKPOINT_RAS_OPERATION_FIXED_BYTES, CHECKPOINT_TARGET_BYTES, CHECKPOINT_VERSION, U64_BYTES,
    V2_CHECKPOINT_VERSION, V3_CHECKPOINT_VERSION, V4_CHECKPOINT_VERSION, V5_CHECKPOINT_VERSION,
};

pub(super) fn checkpoint_payload_len(
    table_entries: usize,
    branch_target_buffer_entries: usize,
    return_address_stack: &ReturnAddressStackSnapshot,
    pending_count: usize,
    active_count: usize,
    version: u8,
) -> Result<usize, BranchPredictorError> {
    let len = legacy_checkpoint_payload_len(table_entries, pending_count, active_count)?;
    let branch_target_buffer_bytes =
        branch_target_buffer_checkpoint_len(branch_target_buffer_entries, version)?;
    let len = checked_sum("payload-size", len, branch_target_buffer_bytes)?;
    if version == V3_CHECKPOINT_VERSION {
        let active_delta = checked_product(
            "active-speculations",
            active_count,
            CHECKPOINT_ACTIVE_SPECULATION_V3_BYTES - CHECKPOINT_ACTIVE_SPECULATION_BYTES,
        )?;
        checked_sum("payload-size", len, active_delta)
    } else if version == V4_CHECKPOINT_VERSION
        || version == V5_CHECKPOINT_VERSION
        || version == CHECKPOINT_VERSION
    {
        let return_address_stack_bytes = return_address_stack_checkpoint_len(return_address_stack)?;
        let len = checked_sum("payload-size", len, return_address_stack_bytes)?;
        let active_speculation_bytes = if version == V4_CHECKPOINT_VERSION {
            CHECKPOINT_ACTIVE_SPECULATION_V4_BYTES
        } else {
            CHECKPOINT_ACTIVE_SPECULATION_V5_BYTES
        };
        let active_delta = checked_product(
            "active-speculations",
            active_count,
            active_speculation_bytes - CHECKPOINT_ACTIVE_SPECULATION_BYTES,
        )?;
        checked_sum("payload-size", len, active_delta)
    } else {
        Ok(len)
    }
}

pub(super) fn legacy_checkpoint_payload_len(
    table_entries: usize,
    pending_count: usize,
    active_count: usize,
) -> Result<usize, BranchPredictorError> {
    let counter_bytes = checked_product("counter-table", table_entries, CHECKPOINT_COUNTER_BYTES)?;
    let target_bytes = checked_product("target-table", table_entries, CHECKPOINT_TARGET_BYTES)?;
    let pending_bytes = checked_product(
        "pending-speculations",
        pending_count,
        CHECKPOINT_PENDING_SPECULATION_BYTES,
    )?;
    let active_bytes = checked_product(
        "active-speculations",
        active_count,
        CHECKPOINT_ACTIVE_SPECULATION_BYTES,
    )?;
    let len = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, counter_bytes)?;
    let len = checked_sum("payload-size", len, target_bytes)?;
    let len = checked_sum("payload-size", len, pending_bytes)?;
    checked_sum("payload-size", len, active_bytes)
}

pub(super) fn v2_checkpoint_payload_len(
    payload: &[u8],
    table_entries: usize,
    pending_count: usize,
    active_count: usize,
) -> Result<usize, BranchPredictorError> {
    let counter_bytes = checked_product("counter-table", table_entries, CHECKPOINT_COUNTER_BYTES)?;
    let target_bytes = checked_product("target-table", table_entries, CHECKPOINT_TARGET_BYTES)?;
    let pending_bytes = checked_product(
        "pending-speculations",
        pending_count,
        CHECKPOINT_PENDING_SPECULATION_BYTES,
    )?;
    let active_bytes = checked_product(
        "active-speculations",
        active_count,
        CHECKPOINT_ACTIVE_SPECULATION_BYTES,
    )?;
    let btb_offset = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, counter_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, target_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, pending_bytes)?;
    let btb_header_end = checked_offset(btb_offset, CHECKPOINT_BTB_LEGACY_HEADER_BYTES)?;
    if payload.len() < btb_header_end {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: btb_header_end,
            actual: payload.len(),
        });
    }
    let mut btb_header = btb_offset;
    let branch_target_buffer_entries = read_u32(payload, &mut btb_header)? as usize;
    let branch_target_buffer_bytes =
        branch_target_buffer_checkpoint_len(branch_target_buffer_entries, V2_CHECKPOINT_VERSION)?;
    let len = checked_sum("payload-size", btb_offset, branch_target_buffer_bytes)?;
    checked_sum("payload-size", len, active_bytes)
}

pub(super) fn v3_checkpoint_payload_len(
    payload: &[u8],
    table_entries: usize,
    pending_count: usize,
    active_count: usize,
) -> Result<usize, BranchPredictorError> {
    let counter_bytes = checked_product("counter-table", table_entries, CHECKPOINT_COUNTER_BYTES)?;
    let target_bytes = checked_product("target-table", table_entries, CHECKPOINT_TARGET_BYTES)?;
    let pending_bytes = checked_product(
        "pending-speculations",
        pending_count,
        CHECKPOINT_PENDING_SPECULATION_BYTES,
    )?;
    let active_bytes = checked_product(
        "active-speculations",
        active_count,
        CHECKPOINT_ACTIVE_SPECULATION_V3_BYTES,
    )?;
    let btb_offset = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, counter_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, target_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, pending_bytes)?;
    let btb_header_end = checked_offset(btb_offset, CHECKPOINT_BTB_LEGACY_HEADER_BYTES)?;
    if payload.len() < btb_header_end {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: btb_header_end,
            actual: payload.len(),
        });
    }
    let mut btb_header = btb_offset;
    let branch_target_buffer_entries = read_u32(payload, &mut btb_header)? as usize;
    let branch_target_buffer_bytes =
        branch_target_buffer_checkpoint_len(branch_target_buffer_entries, V3_CHECKPOINT_VERSION)?;
    let len = checked_sum("payload-size", btb_offset, branch_target_buffer_bytes)?;
    checked_sum("payload-size", len, active_bytes)
}

pub(super) fn v4_checkpoint_payload_len(
    payload: &[u8],
    table_entries: usize,
    pending_count: usize,
    active_count: usize,
) -> Result<usize, BranchPredictorError> {
    checkpoint_payload_len_with_return_address_stack(
        payload,
        table_entries,
        pending_count,
        active_count,
        CHECKPOINT_ACTIVE_SPECULATION_V4_BYTES,
        V4_CHECKPOINT_VERSION,
    )
}

pub(super) fn v5_checkpoint_payload_len(
    payload: &[u8],
    table_entries: usize,
    pending_count: usize,
    active_count: usize,
) -> Result<usize, BranchPredictorError> {
    checkpoint_payload_len_with_return_address_stack(
        payload,
        table_entries,
        pending_count,
        active_count,
        CHECKPOINT_ACTIVE_SPECULATION_V5_BYTES,
        V5_CHECKPOINT_VERSION,
    )
}

pub(super) fn v6_checkpoint_payload_len(
    payload: &[u8],
    table_entries: usize,
    pending_count: usize,
    active_count: usize,
) -> Result<usize, BranchPredictorError> {
    checkpoint_payload_len_with_return_address_stack(
        payload,
        table_entries,
        pending_count,
        active_count,
        CHECKPOINT_ACTIVE_SPECULATION_V5_BYTES,
        CHECKPOINT_VERSION,
    )
}

fn checkpoint_payload_len_with_return_address_stack(
    payload: &[u8],
    table_entries: usize,
    pending_count: usize,
    active_count: usize,
    active_speculation_bytes: usize,
    version: u8,
) -> Result<usize, BranchPredictorError> {
    let counter_bytes = checked_product("counter-table", table_entries, CHECKPOINT_COUNTER_BYTES)?;
    let target_bytes = checked_product("target-table", table_entries, CHECKPOINT_TARGET_BYTES)?;
    let pending_bytes = checked_product(
        "pending-speculations",
        pending_count,
        CHECKPOINT_PENDING_SPECULATION_BYTES,
    )?;
    let active_bytes = checked_product(
        "active-speculations",
        active_count,
        active_speculation_bytes,
    )?;
    let btb_offset = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, counter_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, target_bytes)?;
    let btb_offset = checked_sum("payload-size", btb_offset, pending_bytes)?;
    let btb_header_end = checked_offset(btb_offset, btb_header_bytes(version))?;
    if payload.len() < btb_header_end {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: btb_header_end,
            actual: payload.len(),
        });
    }
    let mut btb_header = btb_offset;
    let branch_target_buffer_entries = read_u32(payload, &mut btb_header)? as usize;
    let branch_target_buffer_bytes =
        branch_target_buffer_checkpoint_len(branch_target_buffer_entries, version)?;
    let ras_offset = checked_sum("payload-size", btb_offset, branch_target_buffer_bytes)?;
    let return_address_stack_bytes =
        return_address_stack_checkpoint_len_from_payload(payload, ras_offset)?;
    let len = checked_sum("payload-size", ras_offset, return_address_stack_bytes)?;
    checked_sum("payload-size", len, active_bytes)
}

pub(super) fn return_address_stack_checkpoint_len(
    snapshot: &ReturnAddressStackSnapshot,
) -> Result<usize, BranchPredictorError> {
    let stack_bytes = checked_product(
        "return-address-stack-entries",
        snapshot.stack_entries().len(),
        U64_BYTES,
    )?;
    let mut len = checked_sum(
        "return-address-stack-size",
        CHECKPOINT_RAS_HEADER_BYTES,
        stack_bytes,
    )?;
    for operation in snapshot.pending_operations() {
        let before_bytes = checked_product(
            "return-address-stack-operation-before",
            operation.stack_before().len(),
            U64_BYTES,
        )?;
        let after_bytes = checked_product(
            "return-address-stack-operation-after",
            operation.stack_after().len(),
            U64_BYTES,
        )?;
        let operation_bytes = checked_sum(
            "return-address-stack-operation-size",
            CHECKPOINT_RAS_OPERATION_FIXED_BYTES,
            before_bytes,
        )?;
        let operation_bytes = checked_sum(
            "return-address-stack-operation-size",
            operation_bytes,
            after_bytes,
        )?;
        len = checked_sum("return-address-stack-size", len, operation_bytes)?;
    }
    Ok(len)
}

fn return_address_stack_checkpoint_len_from_payload(
    payload: &[u8],
    offset: usize,
) -> Result<usize, BranchPredictorError> {
    let header_end = checked_offset(offset, CHECKPOINT_RAS_HEADER_BYTES)?;
    if payload.len() < header_end {
        return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
            expected: header_end,
            actual: payload.len(),
        });
    }
    let mut header = offset;
    let _entries = read_u32(payload, &mut header)? as usize;
    let stack_count = read_u32(payload, &mut header)? as usize;
    let pending_count = read_u32(payload, &mut header)? as usize;
    let _next_operation = read_u64(payload, &mut header)?;
    let stack_bytes = checked_product("return-address-stack-entries", stack_count, U64_BYTES)?;
    let mut ras_len = checked_sum(
        "return-address-stack-size",
        CHECKPOINT_RAS_HEADER_BYTES,
        stack_bytes,
    )?;
    let mut operation_offset = checked_sum("payload-size", offset, ras_len)?;
    for _ in 0..pending_count {
        let operation_header_end =
            checked_offset(operation_offset, CHECKPOINT_RAS_OPERATION_FIXED_BYTES)?;
        if payload.len() < operation_header_end {
            return Err(BranchPredictorError::InvalidCheckpointPayloadSize {
                expected: operation_header_end,
                actual: payload.len(),
            });
        }
        let mut operation_header = checked_offset(
            operation_offset,
            U64_BYTES + 1 + CHECKPOINT_TARGET_BYTES * 2,
        )?;
        let stack_before_count = read_u32(payload, &mut operation_header)? as usize;
        let stack_after_count = read_u32(payload, &mut operation_header)? as usize;
        let before_bytes = checked_product(
            "return-address-stack-operation-before",
            stack_before_count,
            U64_BYTES,
        )?;
        let after_bytes = checked_product(
            "return-address-stack-operation-after",
            stack_after_count,
            U64_BYTES,
        )?;
        let operation_len = checked_sum(
            "return-address-stack-operation-size",
            CHECKPOINT_RAS_OPERATION_FIXED_BYTES,
            before_bytes,
        )?;
        let operation_len = checked_sum(
            "return-address-stack-operation-size",
            operation_len,
            after_bytes,
        )?;
        ras_len = checked_sum("return-address-stack-size", ras_len, operation_len)?;
        operation_offset = checked_sum("payload-size", operation_offset, operation_len)?;
    }
    Ok(ras_len)
}

pub(super) fn branch_target_buffer_checkpoint_len(
    entries: usize,
    version: u8,
) -> Result<usize, BranchPredictorError> {
    let entry_bytes = checked_product(
        "branch-target-buffer-entries",
        entries,
        CHECKPOINT_BTB_ENTRY_BYTES,
    )?;
    checked_sum(
        "branch-target-buffer-size",
        btb_header_bytes(version),
        entry_bytes,
    )
}

const fn btb_header_bytes(version: u8) -> usize {
    if version == CHECKPOINT_VERSION {
        CHECKPOINT_BTB_HEADER_BYTES
    } else {
        CHECKPOINT_BTB_LEGACY_HEADER_BYTES
    }
}
