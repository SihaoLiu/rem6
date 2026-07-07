use std::borrow::Cow;

use rem6_isa_riscv::{
    MemoryAccessKind, MemoryWidth, RiscvPmaAccessKind, RiscvPmpAccessKind,
    RISCV_VECTOR_REGISTER_BYTES,
};
use rem6_memory::{AccessSize, Address, CacheLineLayout, MemoryError};

use crate::RiscvCpuError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MaskedVectorRequestSpan {
    pub(crate) address: Address,
    pub(crate) size: AccessSize,
    pub(crate) byte_offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FaultOnlyFirstLinePrefix {
    pub(crate) access: MemoryAccessKind,
    pub(crate) size: AccessSize,
    pub(crate) byte_offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PmaAlignmentCheck {
    pub(crate) address: Address,
    pub(crate) size: AccessSize,
}

fn access_width(access: &MemoryAccessKind) -> MemoryWidth {
    match access {
        MemoryAccessKind::Load { width, .. }
        | MemoryAccessKind::FloatLoad { width, .. }
        | MemoryAccessKind::VectorLoadUnitStride { width, .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { width, .. }
        | MemoryAccessKind::VectorLoadStrided { width, .. }
        | MemoryAccessKind::VectorLoadIndexed { width, .. }
        | MemoryAccessKind::LoadReserved { width, .. }
        | MemoryAccessKind::StoreConditional { width, .. }
        | MemoryAccessKind::AtomicMemory { width, .. }
        | MemoryAccessKind::Store { width, .. }
        | MemoryAccessKind::FloatStore { width, .. }
        | MemoryAccessKind::VectorStoreUnitStride { width, .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { width, .. }
        | MemoryAccessKind::VectorStoreStrided { width, .. }
        | MemoryAccessKind::VectorStoreIndexed { width, .. } => *width,
    }
}

pub(crate) fn access_size(access: &MemoryAccessKind) -> Result<AccessSize, RiscvCpuError> {
    match access {
        MemoryAccessKind::VectorLoadUnitStride { byte_len, .. } => {
            AccessSize::new(*byte_len as u64).map_err(RiscvCpuError::Memory)
        }
        MemoryAccessKind::VectorLoadSegmentUnitStride { byte_len, .. } => {
            AccessSize::new(*byte_len as u64).map_err(RiscvCpuError::Memory)
        }
        MemoryAccessKind::VectorStoreUnitStride { data, .. } => {
            AccessSize::new(data.len() as u64).map_err(RiscvCpuError::Memory)
        }
        MemoryAccessKind::VectorStoreSegmentUnitStride { data, .. } => {
            AccessSize::new(data.len() as u64).map_err(RiscvCpuError::Memory)
        }
        MemoryAccessKind::VectorLoadStrided { span_len, .. } => {
            AccessSize::new(*span_len as u64).map_err(RiscvCpuError::Memory)
        }
        MemoryAccessKind::VectorStoreStrided { data, .. } => {
            AccessSize::new(data.len() as u64).map_err(RiscvCpuError::Memory)
        }
        MemoryAccessKind::VectorLoadIndexed { span_len, .. } => {
            AccessSize::new(*span_len as u64).map_err(RiscvCpuError::Memory)
        }
        MemoryAccessKind::VectorStoreIndexed { data, .. } => {
            AccessSize::new(data.len() as u64).map_err(RiscvCpuError::Memory)
        }
        _ => memory_width_size(access_width(access)),
    }
}

pub(crate) fn masked_vector_memory_request_span(
    access: &MemoryAccessKind,
    base_address: Address,
    base_size: AccessSize,
) -> Result<MaskedVectorRequestSpan, RiscvCpuError> {
    if preserves_full_register_group_request_span(access) {
        return Ok(MaskedVectorRequestSpan {
            address: base_address,
            size: base_size,
            byte_offset: 0,
        });
    }

    let Some((start, end)) = masked_vector_memory_active_span(access) else {
        return Ok(MaskedVectorRequestSpan {
            address: base_address,
            size: base_size,
            byte_offset: 0,
        });
    };
    if start == 0 && end as u64 == base_size.bytes() {
        return Ok(MaskedVectorRequestSpan {
            address: base_address,
            size: base_size,
            byte_offset: 0,
        });
    }

    let size = AccessSize::new((end - start) as u64).map_err(RiscvCpuError::Memory)?;
    let address = base_address
        .get()
        .checked_add(start as u64)
        .map(Address::new)
        .ok_or(RiscvCpuError::Memory(MemoryError::AddressOverflow {
            start: base_address,
            size: base_size,
        }))?;
    Ok(MaskedVectorRequestSpan {
        address,
        size,
        byte_offset: start,
    })
}

pub(crate) fn fault_only_first_line_prefix(
    access: &MemoryAccessKind,
    address: Address,
    size: AccessSize,
    byte_offset: usize,
    line_layout: CacheLineLayout,
) -> Result<Option<FaultOnlyFirstLinePrefix>, RiscvCpuError> {
    let MemoryAccessKind::VectorLoadUnitStride {
        width,
        byte_len: _,
        byte_mask: None,
        fault_only_first: true,
        ..
    } = access
    else {
        return Ok(None);
    };

    let line_offset = line_layout.line_offset(address);
    let line_remaining = line_layout.bytes() - line_offset;
    if size.bytes() <= line_remaining {
        return Ok(None);
    }

    let element_bytes = width.bytes() as u64;
    let prefix_bytes = line_remaining - (line_remaining % element_bytes);
    if prefix_bytes == 0 || prefix_bytes >= size.bytes() {
        return Ok(None);
    }

    let prefix_len = usize::try_from(prefix_bytes).map_err(|_| {
        RiscvCpuError::Memory(MemoryError::AccessSizeTooLarge {
            size: AccessSize::new(prefix_bytes).expect("nonzero prefix size"),
        })
    })?;
    let mut access = access.clone();
    let MemoryAccessKind::VectorLoadUnitStride { byte_len, .. } = &mut access else {
        unreachable!("matched vector unit-stride load above");
    };
    *byte_len = (*byte_len).min(prefix_len);

    Ok(Some(FaultOnlyFirstLinePrefix {
        access,
        size: AccessSize::new(prefix_bytes).map_err(RiscvCpuError::Memory)?,
        byte_offset,
    }))
}

fn preserves_full_register_group_request_span(access: &MemoryAccessKind) -> bool {
    match access {
        MemoryAccessKind::VectorLoadUnitStride {
            width,
            byte_len,
            byte_mask: Some(_),
            group_registers,
            ..
        } => full_register_group_request_shape(*width, *byte_len, *group_registers),
        MemoryAccessKind::VectorStoreUnitStride {
            width,
            data,
            byte_mask: Some(_),
            group_registers,
            ..
        } => full_register_group_request_shape(*width, data.len(), *group_registers),
        _ => false,
    }
}

fn full_register_group_request_shape(
    width: MemoryWidth,
    byte_len: usize,
    group_registers: usize,
) -> bool {
    let full_group_bytes = group_registers * RISCV_VECTOR_REGISTER_BYTES;
    let supported_shape = (width == MemoryWidth::Halfword && group_registers == 2)
        || (width == MemoryWidth::Word && matches!(group_registers, 2 | 4 | 8));
    supported_shape && byte_len == full_group_bytes
}

fn masked_vector_memory_active_span(access: &MemoryAccessKind) -> Option<(usize, usize)> {
    match access {
        MemoryAccessKind::VectorLoadUnitStride {
            byte_len,
            byte_mask: Some(byte_mask),
            ..
        }
        | MemoryAccessKind::VectorLoadSegmentUnitStride {
            byte_len,
            byte_mask: Some(byte_mask),
            ..
        } if byte_mask.len() == *byte_len => active_byte_span(byte_mask),
        MemoryAccessKind::VectorLoadIndexed {
            span_len,
            byte_mask: Some(byte_mask),
            offsets,
            width,
            ..
        } => indexed_active_byte_span(*span_len, byte_mask, offsets, width.bytes()),
        MemoryAccessKind::VectorLoadStrided {
            span_len,
            byte_mask: Some(byte_mask),
            stride,
            element_count,
            width,
            ..
        } => strided_active_byte_span(*span_len, byte_mask, *stride, *element_count, width.bytes()),
        MemoryAccessKind::VectorStoreUnitStride {
            data,
            byte_mask: Some(byte_mask),
            ..
        }
        | MemoryAccessKind::VectorStoreSegmentUnitStride {
            data,
            byte_mask: Some(byte_mask),
            ..
        } if byte_mask.len() == data.len() => active_byte_span(byte_mask),
        MemoryAccessKind::VectorStoreStrided {
            data, byte_mask, ..
        } if byte_mask.len() == data.len() => active_byte_span(byte_mask),
        MemoryAccessKind::VectorStoreIndexed {
            data, byte_mask, ..
        } if byte_mask.len() == data.len() => active_byte_span(byte_mask),
        _ => None,
    }
}

fn indexed_active_byte_span(
    span_len: usize,
    byte_mask: &[bool],
    offsets: &[usize],
    element_bytes: usize,
) -> Option<(usize, usize)> {
    if byte_mask.len() != offsets.len().checked_mul(element_bytes)? {
        return None;
    }

    let mut first = span_len;
    let mut last = 0usize;
    for (element_index, memory_offset) in offsets.iter().copied().enumerate() {
        let source_offset = element_index.checked_mul(element_bytes)?;
        for element_byte in 0..element_bytes {
            if !byte_mask[source_offset + element_byte] {
                continue;
            }
            let memory_byte = memory_offset.checked_add(element_byte)?;
            if memory_byte >= span_len {
                return None;
            }
            first = first.min(memory_byte);
            last = last.max(memory_byte + 1);
        }
    }
    (last != 0).then_some((first, last))
}

fn strided_active_byte_span(
    span_len: usize,
    byte_mask: &[bool],
    stride: usize,
    element_count: usize,
    element_bytes: usize,
) -> Option<(usize, usize)> {
    if byte_mask.len() != element_count.checked_mul(element_bytes)? {
        return None;
    }

    let mut first = span_len;
    let mut last = 0usize;
    for element_index in 0..element_count {
        let source_offset = element_index.checked_mul(element_bytes)?;
        let memory_offset = element_index.checked_mul(stride)?;
        for element_byte in 0..element_bytes {
            if !byte_mask[source_offset + element_byte] {
                continue;
            }
            let memory_byte = memory_offset.checked_add(element_byte)?;
            if memory_byte >= span_len {
                return None;
            }
            first = first.min(memory_byte);
            last = last.max(memory_byte + 1);
        }
    }
    (last != 0).then_some((first, last))
}

fn active_byte_span(byte_mask: &[bool]) -> Option<(usize, usize)> {
    let first = byte_mask.iter().position(|active| *active)?;
    let last = byte_mask.iter().rposition(|active| *active)? + 1;
    Some((first, last))
}

pub(crate) fn normalized_masked_load_data<'a>(
    expected_len: usize,
    byte_mask: Option<&[bool]>,
    data: &'a [u8],
) -> Cow<'a, [u8]> {
    if data.len() == expected_len {
        return Cow::Borrowed(data);
    }
    let Some(byte_mask) = byte_mask else {
        return Cow::Borrowed(data);
    };
    let Some((start, end)) = active_byte_span(byte_mask) else {
        return Cow::Borrowed(data);
    };
    if byte_mask.len() != expected_len || end - start != data.len() {
        return Cow::Borrowed(data);
    }

    let mut expanded = vec![0; expected_len];
    expanded[start..end].copy_from_slice(data);
    Cow::Owned(expanded)
}

pub(crate) fn normalized_masked_indexed_load_data<'a>(
    expected_len: usize,
    byte_mask: Option<&[bool]>,
    offsets: &[usize],
    element_bytes: usize,
    data: &'a [u8],
) -> Cow<'a, [u8]> {
    if data.len() == expected_len {
        return Cow::Borrowed(data);
    }
    let Some(byte_mask) = byte_mask else {
        return Cow::Borrowed(data);
    };
    let Some((start, end)) =
        indexed_active_byte_span(expected_len, byte_mask, offsets, element_bytes)
    else {
        return Cow::Borrowed(data);
    };
    if end - start != data.len() {
        return Cow::Borrowed(data);
    }

    let mut expanded = vec![0; expected_len];
    expanded[start..end].copy_from_slice(data);
    Cow::Owned(expanded)
}

pub(crate) fn normalized_masked_strided_load_data<'a>(
    expected_len: usize,
    byte_mask: Option<&[bool]>,
    stride: usize,
    element_count: usize,
    element_bytes: usize,
    data: &'a [u8],
) -> Cow<'a, [u8]> {
    if data.len() == expected_len {
        return Cow::Borrowed(data);
    }
    let Some(byte_mask) = byte_mask else {
        return Cow::Borrowed(data);
    };
    let Some((start, end)) = strided_active_byte_span(
        expected_len,
        byte_mask,
        stride,
        element_count,
        element_bytes,
    ) else {
        return Cow::Borrowed(data);
    };
    if end - start != data.len() {
        return Cow::Borrowed(data);
    }

    let mut expanded = vec![0; expected_len];
    expanded[start..end].copy_from_slice(data);
    Cow::Owned(expanded)
}

pub(crate) fn vector_store_request_payload(
    size: AccessSize,
    request_byte_offset: usize,
    data: &[u8],
    byte_mask: Option<&[bool]>,
) -> Result<(Vec<u8>, Option<Vec<bool>>), RiscvCpuError> {
    if let Some(byte_mask) = byte_mask {
        if byte_mask.len() != data.len() {
            let expected = AccessSize::new(data.len() as u64).map_err(RiscvCpuError::Memory)?;
            return Err(RiscvCpuError::Memory(MemoryError::ByteMaskSizeMismatch {
                expected,
                actual: byte_mask.len() as u64,
            }));
        }
    }
    let request_len: usize = size
        .bytes()
        .try_into()
        .map_err(|_| RiscvCpuError::Memory(MemoryError::AccessSizeTooLarge { size }))?;
    let request_end = request_byte_offset
        .checked_add(request_len)
        .ok_or(RiscvCpuError::Memory(MemoryError::PayloadSizeMismatch {
            expected: size,
            actual: data.len() as u64,
        }))?;
    let data = data
        .get(request_byte_offset..request_end)
        .ok_or(RiscvCpuError::Memory(MemoryError::PayloadSizeMismatch {
            expected: size,
            actual: data.len() as u64,
        }))?
        .to_vec();
    let byte_mask = byte_mask
        .map(|byte_mask| {
            byte_mask
                .get(request_byte_offset..request_end)
                .ok_or(RiscvCpuError::Memory(MemoryError::ByteMaskSizeMismatch {
                    expected: size,
                    actual: byte_mask.len() as u64,
                }))
                .map(<[bool]>::to_vec)
        })
        .transpose()?;

    Ok((data, byte_mask))
}

pub(crate) fn pma_alignment_checks(
    access: &MemoryAccessKind,
    request_address: Address,
    request_size: AccessSize,
    request_byte_offset: usize,
) -> Result<Vec<PmaAlignmentCheck>, RiscvCpuError> {
    if let Some(checks) =
        vector_pma_alignment_checks(access, request_address, request_size, request_byte_offset)?
    {
        return Ok(checks);
    }

    Ok(vec![PmaAlignmentCheck {
        address: request_address,
        size: request_size,
    }])
}

fn vector_pma_alignment_checks(
    access: &MemoryAccessKind,
    request_address: Address,
    request_size: AccessSize,
    request_byte_offset: usize,
) -> Result<Option<Vec<PmaAlignmentCheck>>, RiscvCpuError> {
    let Some(active_offsets) = vector_pma_element_offsets(access) else {
        return Ok(None);
    };

    let element_size = memory_width_size(access_width(access))?;
    let base_address = request_base_address(request_address, request_size, request_byte_offset)?;
    let mut checks = Vec::with_capacity(active_offsets.len());
    for offset in active_offsets {
        checks.push(PmaAlignmentCheck {
            address: address_with_offset(base_address, request_size, offset)?,
            size: element_size,
        });
    }
    Ok(Some(checks))
}

fn vector_pma_element_offsets(access: &MemoryAccessKind) -> Option<Vec<usize>> {
    // Strided vector memory keeps the existing span-level PMA model because
    // current translated/top-level evidence accepts unaligned strided lanes.
    match access {
        MemoryAccessKind::VectorLoadUnitStride {
            byte_len,
            byte_mask: Some(byte_mask),
            width,
            ..
        } => active_suppressed_span_element_offsets(byte_mask, *byte_len, width.bytes()),
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            byte_len,
            byte_mask: Some(byte_mask),
            width,
            ..
        } => active_suppressed_span_element_offsets(byte_mask, *byte_len, width.bytes()),
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            byte_len,
            byte_mask: None,
            width,
            fields,
            element_count,
            ..
        } => segment_element_offsets(*byte_len, *fields, *element_count, width.bytes()),
        MemoryAccessKind::VectorLoadIndexed {
            byte_mask,
            offsets,
            width,
            ..
        } => active_indexed_load_offsets(byte_mask.as_deref(), offsets, width.bytes()),
        MemoryAccessKind::VectorStoreUnitStride {
            data,
            byte_mask: Some(byte_mask),
            width,
            ..
        } => active_suppressed_span_element_offsets(byte_mask, data.len(), width.bytes()),
        MemoryAccessKind::VectorStoreSegmentUnitStride {
            data,
            byte_mask: Some(byte_mask),
            width,
            ..
        } => active_suppressed_span_element_offsets(byte_mask, data.len(), width.bytes()),
        MemoryAccessKind::VectorStoreSegmentUnitStride {
            data,
            byte_mask: None,
            width,
            fields,
            element_count,
            ..
        } => segment_element_offsets(data.len(), *fields, *element_count, width.bytes()),
        MemoryAccessKind::VectorStoreIndexed {
            data,
            byte_mask,
            offsets,
            width,
            ..
        } if byte_mask.len() == data.len() => {
            active_indexed_store_offsets(byte_mask, offsets, width.bytes())
        }
        _ => None,
    }
}

fn segment_element_offsets(
    byte_len: usize,
    fields: usize,
    element_count: usize,
    element_bytes: usize,
) -> Option<Vec<usize>> {
    let expected_len = fields
        .checked_mul(element_count)?
        .checked_mul(element_bytes)?;
    (expected_len == byte_len).then(|| contiguous_element_offsets(byte_len, element_bytes))?
}

fn contiguous_element_offsets(byte_len: usize, element_bytes: usize) -> Option<Vec<usize>> {
    if element_bytes == 0 || !byte_len.is_multiple_of(element_bytes) {
        return None;
    }

    Some((0..byte_len).step_by(element_bytes).collect())
}

fn active_suppressed_span_element_offsets(
    byte_mask: &[bool],
    byte_len: usize,
    element_bytes: usize,
) -> Option<Vec<usize>> {
    if byte_mask.len() != byte_len || element_bytes == 0 || !byte_len.is_multiple_of(element_bytes)
    {
        return None;
    }

    active_suppressed_element_offsets(
        byte_len / element_bytes,
        element_bytes,
        |element_index| element_index.checked_mul(element_bytes),
        |_, offset, byte| byte_mask[offset + byte],
    )
}

fn active_indexed_load_offsets(
    byte_mask: Option<&[bool]>,
    offsets: &[usize],
    element_bytes: usize,
) -> Option<Vec<usize>> {
    match byte_mask {
        Some(byte_mask) => active_compact_element_offsets(
            byte_mask,
            offsets.len(),
            element_bytes,
            |element_index| offsets.get(element_index).copied(),
        ),
        None if element_bytes != 0 => Some(offsets.to_vec()),
        None => None,
    }
}

fn active_compact_element_offsets(
    byte_mask: &[bool],
    element_count: usize,
    element_bytes: usize,
    memory_offset: impl Fn(usize) -> Option<usize>,
) -> Option<Vec<usize>> {
    if element_bytes == 0 || byte_mask.len() != element_count.checked_mul(element_bytes)? {
        return None;
    }

    active_element_offsets(
        element_count,
        element_bytes,
        memory_offset,
        |source_offset, _, byte| byte_mask[source_offset + byte],
    )
}

fn active_indexed_store_offsets(
    byte_mask: &[bool],
    offsets: &[usize],
    element_bytes: usize,
) -> Option<Vec<usize>> {
    active_store_offsets(byte_mask, offsets.len(), element_bytes, |element_index| {
        offsets.get(element_index).copied()
    })
}

fn active_store_offsets(
    byte_mask: &[bool],
    element_count: usize,
    element_bytes: usize,
    memory_offset: impl Fn(usize) -> Option<usize>,
) -> Option<Vec<usize>> {
    if element_bytes == 0 {
        return None;
    }

    active_element_offsets(
        element_count,
        element_bytes,
        memory_offset,
        |_, memory_offset, byte| {
            byte_mask
                .get(memory_offset + byte)
                .copied()
                .unwrap_or(false)
        },
    )
}

fn active_element_offsets(
    element_count: usize,
    element_bytes: usize,
    memory_offset: impl Fn(usize) -> Option<usize>,
    active_byte: impl Fn(usize, usize, usize) -> bool,
) -> Option<Vec<usize>> {
    let mut active_offsets = Vec::new();
    for element_index in 0..element_count {
        let source_offset = element_index.checked_mul(element_bytes)?;
        let offset = memory_offset(element_index)?;
        let active = (0..element_bytes).any(|byte| active_byte(source_offset, offset, byte));
        if active {
            active_offsets.push(offset);
        }
    }
    Some(active_offsets)
}

fn active_suppressed_element_offsets(
    element_count: usize,
    element_bytes: usize,
    memory_offset: impl Fn(usize) -> Option<usize>,
    active_byte: impl Fn(usize, usize, usize) -> bool,
) -> Option<Vec<usize>> {
    let mut active_offsets = Vec::new();
    let mut saw_suppressed = false;
    for element_index in 0..element_count {
        let source_offset = element_index.checked_mul(element_bytes)?;
        let offset = memory_offset(element_index)?;
        let active = (0..element_bytes).any(|byte| active_byte(source_offset, offset, byte));
        if active {
            active_offsets.push(offset);
        } else {
            saw_suppressed = true;
        }
    }

    saw_suppressed.then_some(active_offsets)
}

fn request_base_address(
    request_address: Address,
    request_size: AccessSize,
    request_byte_offset: usize,
) -> Result<Address, RiscvCpuError> {
    let offset = u64::try_from(request_byte_offset).map_err(|_| {
        RiscvCpuError::Memory(MemoryError::AccessSizeTooLarge { size: request_size })
    })?;
    request_address
        .get()
        .checked_sub(offset)
        .map(Address::new)
        .ok_or(RiscvCpuError::Memory(MemoryError::AddressOverflow {
            start: request_address,
            size: request_size,
        }))
}

fn address_with_offset(
    base_address: Address,
    request_size: AccessSize,
    byte_offset: usize,
) -> Result<Address, RiscvCpuError> {
    let offset = u64::try_from(byte_offset).map_err(|_| {
        RiscvCpuError::Memory(MemoryError::AccessSizeTooLarge { size: request_size })
    })?;
    base_address
        .get()
        .checked_add(offset)
        .map(Address::new)
        .ok_or(RiscvCpuError::Memory(MemoryError::AddressOverflow {
            start: base_address,
            size: request_size,
        }))
}

pub(crate) fn pmp_access_kind(access: &MemoryAccessKind) -> RiscvPmpAccessKind {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride { .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
        | MemoryAccessKind::VectorLoadStrided { .. }
        | MemoryAccessKind::VectorLoadIndexed { .. }
        | MemoryAccessKind::LoadReserved { .. } => RiscvPmpAccessKind::Read,
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. }
        | MemoryAccessKind::StoreConditional { .. }
        | MemoryAccessKind::AtomicMemory { .. } => RiscvPmpAccessKind::Write,
    }
}

pub(crate) fn pma_access_kind(access: &MemoryAccessKind) -> RiscvPmaAccessKind {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride { .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
        | MemoryAccessKind::VectorLoadStrided { .. }
        | MemoryAccessKind::VectorLoadIndexed { .. }
        | MemoryAccessKind::LoadReserved { .. } => RiscvPmaAccessKind::Read,
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. }
        | MemoryAccessKind::StoreConditional { .. }
        | MemoryAccessKind::AtomicMemory { .. } => RiscvPmaAccessKind::Write,
    }
}

pub(crate) fn access_address(access: &MemoryAccessKind) -> u64 {
    match access {
        MemoryAccessKind::Load { address, .. }
        | MemoryAccessKind::FloatLoad { address, .. }
        | MemoryAccessKind::VectorLoadUnitStride { address, .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { address, .. }
        | MemoryAccessKind::VectorLoadStrided { address, .. }
        | MemoryAccessKind::VectorLoadIndexed { address, .. }
        | MemoryAccessKind::LoadReserved { address, .. }
        | MemoryAccessKind::StoreConditional { address, .. }
        | MemoryAccessKind::AtomicMemory { address, .. }
        | MemoryAccessKind::Store { address, .. }
        | MemoryAccessKind::FloatStore { address, .. }
        | MemoryAccessKind::VectorStoreUnitStride { address, .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { address, .. }
        | MemoryAccessKind::VectorStoreStrided { address, .. }
        | MemoryAccessKind::VectorStoreIndexed { address, .. } => *address,
    }
}

fn memory_width_size(width: MemoryWidth) -> Result<AccessSize, RiscvCpuError> {
    AccessSize::new(width.bytes() as u64).map_err(RiscvCpuError::Memory)
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::VectorRegister;

    use super::*;

    #[test]
    fn pma_alignment_checks_use_active_unit_stride_masked_elements() {
        let access = vector_load_unit_stride_with_mask(0x8000, &[true, false, true, false]);
        let checks = pma_alignment_checks(
            &access,
            Address::new(0x8000),
            AccessSize::new(12).unwrap(),
            0,
        )
        .unwrap();

        assert_eq!(checks, vec![pma_check(0x8000, 4), pma_check(0x8008, 4),]);
    }

    #[test]
    fn pma_alignment_checks_use_unmasked_segment_elements() {
        let load = MemoryAccessKind::VectorLoadSegmentUnitStride {
            vd: VectorRegister::new(2).unwrap(),
            address: 0x8000,
            width: MemoryWidth::Word,
            fields: 3,
            element_count: 1,
            byte_len: 12,
            byte_mask: None,
            group_registers: 1,
        };
        let checks =
            pma_alignment_checks(&load, Address::new(0x8000), AccessSize::new(12).unwrap(), 0)
                .unwrap();
        assert_eq!(
            checks,
            vec![
                pma_check(0x8000, 4),
                pma_check(0x8004, 4),
                pma_check(0x8008, 4),
            ]
        );

        let store = MemoryAccessKind::VectorStoreSegmentUnitStride {
            address: 0x8020,
            width: MemoryWidth::Word,
            fields: 3,
            element_count: 1,
            data: vec![0; 12],
            byte_mask: None,
            group_registers: 1,
        };
        let checks = pma_alignment_checks(
            &store,
            Address::new(0x8020),
            AccessSize::new(12).unwrap(),
            0,
        )
        .unwrap();
        assert_eq!(
            checks,
            vec![
                pma_check(0x8020, 4),
                pma_check(0x8024, 4),
                pma_check(0x8028, 4),
            ]
        );
    }

    #[test]
    fn masked_vector_request_span_compacts_m1_unit_stride_gap() {
        let access = vector_load_unit_stride_with_mask(0x8000, &[true, false, true, false]);
        let span = masked_vector_memory_request_span(
            &access,
            Address::new(0x8000),
            AccessSize::new(16).unwrap(),
        )
        .unwrap();

        assert_eq!(
            span,
            MaskedVectorRequestSpan {
                address: Address::new(0x8000),
                size: AccessSize::new(12).unwrap(),
                byte_offset: 0,
            }
        );
    }

    #[test]
    fn masked_vector_request_span_preserves_full_register_group_shape() {
        let access = MemoryAccessKind::VectorLoadUnitStride {
            vd: VectorRegister::new(2).unwrap(),
            address: 0x8000,
            width: MemoryWidth::Word,
            byte_len: 2 * RISCV_VECTOR_REGISTER_BYTES,
            byte_mask: Some(lane_mask(
                &[true, false, true, false, true, false, true, false],
                MemoryWidth::Word.bytes(),
            )),
            group_registers: 2,
            fault_only_first: false,
        };
        let span = masked_vector_memory_request_span(
            &access,
            Address::new(0x8000),
            AccessSize::new(32).unwrap(),
        )
        .unwrap();

        assert_eq!(
            span,
            MaskedVectorRequestSpan {
                address: Address::new(0x8000),
                size: AccessSize::new(32).unwrap(),
                byte_offset: 0,
            }
        );
    }

    #[test]
    fn pma_alignment_checks_recover_base_from_leading_inactive_lanes() {
        let access = vector_load_unit_stride_with_mask(0x8000, &[false, true, false, true]);
        let checks = pma_alignment_checks(
            &access,
            Address::new(0x8004),
            AccessSize::new(12).unwrap(),
            4,
        )
        .unwrap();

        assert_eq!(checks, vec![pma_check(0x8004, 4), pma_check(0x800c, 4),]);
    }

    #[test]
    fn pma_alignment_checks_preserve_all_active_vector_span() {
        let access = vector_load_unit_stride_with_mask(0x8004, &[true, true, true, true]);
        let checks = pma_alignment_checks(
            &access,
            Address::new(0x8004),
            AccessSize::new(16).unwrap(),
            0,
        )
        .unwrap();

        assert_eq!(checks, vec![pma_check(0x8004, 16)]);
    }

    #[test]
    fn pma_alignment_checks_keep_masked_strided_span_semantics() {
        let access = MemoryAccessKind::VectorLoadStrided {
            vd: VectorRegister::new(2).unwrap(),
            address: 0x900a,
            width: MemoryWidth::Word,
            stride: 6,
            element_count: 3,
            span_len: 16,
            byte_mask: Some(lane_mask(&[false, true, true], MemoryWidth::Word.bytes())),
            group_registers: 1,
        };
        let checks = pma_alignment_checks(
            &access,
            Address::new(0x9010),
            AccessSize::new(10).unwrap(),
            6,
        )
        .unwrap();

        assert_eq!(checks, vec![pma_check(0x9010, 10)]);
    }

    #[test]
    fn pma_alignment_checks_use_active_indexed_store_elements() {
        let mut byte_mask = vec![false; 32];
        byte_mask[4..8].fill(true);
        byte_mask[28..32].fill(true);
        let access = MemoryAccessKind::VectorStoreIndexed {
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Byte,
            offsets: vec![4, 16, 28],
            data: vec![0; 32],
            byte_mask,
            group_registers: 1,
        };
        let checks = pma_alignment_checks(
            &access,
            Address::new(0x9004),
            AccessSize::new(28).unwrap(),
            4,
        )
        .unwrap();

        assert_eq!(checks, vec![pma_check(0x9004, 4), pma_check(0x901c, 4),]);
    }

    #[test]
    fn pma_alignment_checks_use_unmasked_indexed_load_elements() {
        let access = MemoryAccessKind::VectorLoadIndexed {
            vd: VectorRegister::new(2).unwrap(),
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Byte,
            offsets: vec![4, 12],
            span_len: 16,
            byte_mask: None,
            group_registers: 1,
        };
        let checks = pma_alignment_checks(
            &access,
            Address::new(0x9004),
            AccessSize::new(12).unwrap(),
            4,
        )
        .unwrap();

        assert_eq!(checks, vec![pma_check(0x9004, 4), pma_check(0x900c, 4),]);
    }

    #[test]
    fn pma_alignment_checks_use_unmasked_indexed_store_elements() {
        let mut byte_mask = vec![false; 16];
        byte_mask[4..8].fill(true);
        byte_mask[12..16].fill(true);
        let access = MemoryAccessKind::VectorStoreIndexed {
            address: 0x9000,
            width: MemoryWidth::Word,
            index_width: MemoryWidth::Byte,
            offsets: vec![4, 12],
            data: vec![0; 16],
            byte_mask,
            group_registers: 1,
        };
        let checks = pma_alignment_checks(
            &access,
            Address::new(0x9004),
            AccessSize::new(12).unwrap(),
            4,
        )
        .unwrap();

        assert_eq!(checks, vec![pma_check(0x9004, 4), pma_check(0x900c, 4),]);
    }

    fn vector_load_unit_stride_with_mask(address: u64, lanes: &[bool]) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadUnitStride {
            vd: VectorRegister::new(2).unwrap(),
            address,
            width: MemoryWidth::Word,
            byte_len: lanes.len() * MemoryWidth::Word.bytes(),
            byte_mask: Some(lane_mask(lanes, MemoryWidth::Word.bytes())),
            group_registers: 1,
            fault_only_first: false,
        }
    }

    fn lane_mask(lanes: &[bool], element_bytes: usize) -> Vec<bool> {
        let mut mask = Vec::with_capacity(lanes.len() * element_bytes);
        for lane in lanes {
            mask.extend(std::iter::repeat(*lane).take(element_bytes));
        }
        mask
    }

    fn pma_check(address: u64, bytes: u64) -> PmaAlignmentCheck {
        PmaAlignmentCheck {
            address: Address::new(address),
            size: AccessSize::new(bytes).unwrap(),
        }
    }
}
