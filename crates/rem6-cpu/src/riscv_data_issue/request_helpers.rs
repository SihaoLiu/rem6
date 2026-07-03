use std::borrow::Cow;

use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, RiscvPmaAccessKind, RiscvPmpAccessKind};
use rem6_memory::{AccessSize, Address, MemoryError};

use crate::RiscvCpuError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MaskedVectorRequestSpan {
    pub(crate) address: Address,
    pub(crate) size: AccessSize,
    pub(crate) byte_offset: usize,
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
    let Some((start, end)) = contiguous_masked_vector_memory_span(access) else {
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

fn contiguous_masked_vector_memory_span(access: &MemoryAccessKind) -> Option<(usize, usize)> {
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
        } if byte_mask.len() == *byte_len => contiguous_active_byte_span(byte_mask),
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

fn contiguous_active_byte_span(byte_mask: &[bool]) -> Option<(usize, usize)> {
    let first = byte_mask.iter().position(|active| *active)?;
    let last = byte_mask.iter().rposition(|active| *active)? + 1;
    if byte_mask[first..last].iter().all(|active| *active) {
        Some((first, last))
    } else {
        None
    }
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
    let Some((start, end)) = contiguous_active_byte_span(byte_mask) else {
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
