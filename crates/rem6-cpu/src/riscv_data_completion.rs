use std::borrow::Cow;

use rem6_isa_riscv::{
    MemoryAccessKind, MemoryResponseWritebackTarget, RiscvHartState, RiscvVectorConfig,
    VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};
use rem6_kernel::Tick;
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::{CpuId, RiscvCoreState, RiscvDataAccessEventKind, RiscvLoadReservation};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvDataCompletionOutcome {
    Completed,
    StoreConditionalFailed { tick: Tick },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RiscvDataCompletion {
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    physical_address: Address,
    size: AccessSize,
    request_byte_offset: usize,
    bytes: Option<Vec<u8>>,
    outcome: RiscvDataCompletionOutcome,
}

impl RiscvDataCompletion {
    pub(crate) fn from_issued_response(
        fetch_request: MemoryRequestId,
        access: MemoryAccessKind,
        physical_address: Address,
        size: AccessSize,
        request_byte_offset: usize,
        bytes: Option<Vec<u8>>,
    ) -> Self {
        Self {
            fetch_request,
            access,
            physical_address,
            size,
            request_byte_offset,
            bytes,
            outcome: RiscvDataCompletionOutcome::Completed,
        }
    }

    pub(crate) fn store_conditional_failed(
        fetch_request: MemoryRequestId,
        access: MemoryAccessKind,
        physical_address: Address,
        size: AccessSize,
        request_byte_offset: usize,
        tick: Tick,
    ) -> Self {
        assert!(
            matches!(&access, MemoryAccessKind::StoreConditional { .. }),
            "store-conditional failure completion requires SC access"
        );
        Self {
            fetch_request,
            access,
            physical_address,
            size,
            request_byte_offset,
            bytes: None,
            outcome: RiscvDataCompletionOutcome::StoreConditionalFailed { tick },
        }
    }

    pub(crate) const fn fetch_request(&self) -> MemoryRequestId {
        self.fetch_request
    }

    pub(crate) const fn access(&self) -> &MemoryAccessKind {
        &self.access
    }

    pub(crate) const fn physical_address(&self) -> Address {
        self.physical_address
    }

    pub(crate) const fn size(&self) -> AccessSize {
        self.size
    }

    pub(crate) const fn request_byte_offset(&self) -> usize {
        self.request_byte_offset
    }

    pub(crate) fn matches_issued_request(
        &self,
        fetch_request: MemoryRequestId,
        access: &MemoryAccessKind,
        physical_address: Address,
        size: AccessSize,
        request_byte_offset: usize,
    ) -> bool {
        self.fetch_request == fetch_request
            && &self.access == access
            && self.physical_address == physical_address
            && self.size == size
            && self.request_byte_offset == request_byte_offset
    }

    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }

    pub(crate) const fn data_event_kind(&self) -> RiscvDataAccessEventKind {
        match self.outcome {
            RiscvDataCompletionOutcome::Completed => RiscvDataAccessEventKind::Completed,
            RiscvDataCompletionOutcome::StoreConditionalFailed { .. } => {
                RiscvDataAccessEventKind::ConditionalFailed
            }
        }
    }

    fn required_bytes(&self, missing_data: &'static str) -> &[u8] {
        self.bytes.as_deref().expect(missing_data)
    }
}

pub(crate) fn apply_data_completion(
    state: &mut RiscvCoreState,
    cpu: CpuId,
    completion: &RiscvDataCompletion,
    missing_data: &'static str,
) {
    match completion.access() {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::AtomicMemory { .. } => {
            let writeback = completion
                .access()
                .read_response_writeback(completion.required_bytes(missing_data))
                .expect("read response payload width")
                .expect("read response writeback");
            match writeback.target() {
                MemoryResponseWritebackTarget::Integer(register) => {
                    state.hart.write(register, writeback.value());
                }
                MemoryResponseWritebackTarget::Float(register) => {
                    state.hart.write_float(register, writeback.value());
                }
            }
        }
        MemoryAccessKind::LoadReserved { .. } => {
            let writeback = completion
                .access()
                .read_response_writeback(completion.required_bytes(missing_data))
                .expect("read response payload width")
                .expect("read response writeback");
            state
                .hart
                .write(writeback.expect_integer_register(), writeback.value());
            state.reservation = Some(RiscvLoadReservation::new(
                completion.physical_address(),
                completion.size(),
            ));
        }
        MemoryAccessKind::StoreConditional { rd, .. } => match completion.outcome {
            RiscvDataCompletionOutcome::Completed => {
                state.hart.write(*rd, 0);
                state.reservation = None;
                state.sc_progress.record_success(cpu);
            }
            RiscvDataCompletionOutcome::StoreConditionalFailed { tick } => {
                state.hart.write(*rd, 1);
                state.reservation = None;
                state.sc_progress.record_failure(
                    cpu,
                    tick,
                    completion.physical_address(),
                    completion.size(),
                );
            }
        },
        MemoryAccessKind::VectorLoadUnitStride {
            vd,
            width,
            byte_len,
            byte_mask,
            group_registers,
            fault_only_first,
            ..
        } => {
            let data = completion.required_bytes(missing_data);
            let data = normalized_masked_load_data(
                *byte_len,
                byte_mask.as_deref(),
                completion.request_byte_offset(),
                data,
            );
            assert_eq!(*byte_len, data.len(), "vector load response payload width");
            let mut destination = read_vector_register_group(&state.hart, *vd, *group_registers);
            if let Some(byte_mask) = byte_mask {
                assert_eq!(
                    *byte_len,
                    byte_mask.len(),
                    "vector load byte mask payload width"
                );
                for (index, active) in byte_mask.iter().copied().enumerate() {
                    if active {
                        destination[index] = data[index];
                    }
                }
            } else {
                destination[..*byte_len].copy_from_slice(&data);
            }
            write_vector_register_group(&mut state.hart, *vd, *group_registers, &destination);
            if *fault_only_first {
                let completed_vl = (*byte_len / width.bytes()) as u32;
                let vector_config = state.hart.vector_config();
                state
                    .hart
                    .set_vector_config(RiscvVectorConfig::new(completed_vl, vector_config.vtype()));
            }
        }
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            vd,
            fields,
            element_count,
            byte_len,
            byte_mask,
            group_registers,
            ..
        } => {
            let data = completion.required_bytes(missing_data);
            let data = normalized_masked_load_data(
                *byte_len,
                byte_mask.as_deref(),
                completion.request_byte_offset(),
                data,
            );
            assert_eq!(*byte_len, data.len(), "segment vector load response width");
            if let Some(byte_mask) = byte_mask {
                assert_eq!(
                    *byte_len,
                    byte_mask.len(),
                    "segment vector load byte mask width"
                );
            }
            scatter_segment_load(
                &data,
                &mut state.hart,
                *vd,
                *fields,
                *element_count,
                byte_mask.as_deref(),
                *group_registers,
            );
        }
        MemoryAccessKind::VectorLoadStrided {
            vd,
            width,
            stride,
            element_count,
            span_len,
            byte_mask,
            group_registers,
            ..
        } => {
            let data = completion.required_bytes(missing_data);
            let data = normalized_masked_strided_load_data(
                *span_len,
                byte_mask.as_deref(),
                *stride,
                *element_count,
                width.bytes(),
                data,
            );
            assert_eq!(*span_len, data.len(), "strided vector load response width");
            let mut destination = read_vector_register_group(&state.hart, *vd, *group_registers);
            scatter_strided_load(
                &data,
                &mut destination,
                width.bytes(),
                *stride,
                *element_count,
                byte_mask.as_deref(),
            );
            write_vector_register_group(&mut state.hart, *vd, *group_registers, &destination);
        }
        MemoryAccessKind::VectorLoadIndexed {
            vd,
            width,
            offsets,
            span_len,
            byte_mask,
            group_registers,
            ..
        } => {
            let data = completion.required_bytes(missing_data);
            let data = normalized_masked_indexed_load_data(
                *span_len,
                byte_mask.as_deref(),
                offsets,
                width.bytes(),
                data,
            );
            assert_eq!(*span_len, data.len(), "indexed vector load response width");
            let mut destination = read_vector_register_group(&state.hart, *vd, *group_registers);
            scatter_indexed_load(
                &data,
                &mut destination,
                width.bytes(),
                offsets,
                byte_mask.as_deref(),
            );
            write_vector_register_group(&mut state.hart, *vd, *group_registers, &destination);
        }
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. } => {}
    }
    if let Some(data) = completion.bytes() {
        state.o3_runtime.record_completed_load_data(
            completion.fetch_request(),
            completion.access(),
            data,
        );
    }
}

fn normalized_masked_load_data<'a>(
    expected_len: usize,
    byte_mask: Option<&[bool]>,
    request_byte_offset: usize,
    data: &'a [u8],
) -> Cow<'a, [u8]> {
    if data.len() == expected_len {
        return Cow::Borrowed(data);
    }
    let Some(byte_mask) = byte_mask else {
        return Cow::Borrowed(data);
    };
    if byte_mask.len() != expected_len {
        return Cow::Borrowed(data);
    };
    let Some(request_end) = request_byte_offset.checked_add(data.len()) else {
        return Cow::Borrowed(data);
    };
    if request_end > expected_len {
        return Cow::Borrowed(data);
    }

    let mut expanded = vec![0; expected_len];
    expanded[request_byte_offset..request_end].copy_from_slice(data);
    Cow::Owned(expanded)
}

fn normalized_masked_indexed_load_data<'a>(
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

fn normalized_masked_strided_load_data<'a>(
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

fn scatter_segment_load(
    data: &[u8],
    hart: &mut RiscvHartState,
    register: VectorRegister,
    fields: usize,
    element_count: usize,
    byte_mask: Option<&[bool]>,
    group_registers: usize,
) {
    debug_assert!(element_count > 0);
    let element_bytes = data.len() / fields / element_count;
    for field in 0..fields {
        let field_register = vector_register_at(register, field * group_registers);
        let mut destination = read_vector_register_group(hart, field_register, group_registers);
        for element_index in 0..element_count {
            let source_offset = (element_index * fields + field) * element_bytes;
            let active = byte_mask.map(|mask| mask[source_offset]).unwrap_or(true);
            if !active {
                continue;
            }
            let destination_offset = element_index * element_bytes;
            destination[destination_offset..destination_offset + element_bytes]
                .copy_from_slice(&data[source_offset..source_offset + element_bytes]);
        }
        write_vector_register_group(hart, field_register, group_registers, &destination);
    }
}

fn scatter_strided_load(
    data: &[u8],
    destination: &mut [u8],
    element_bytes: usize,
    stride: usize,
    element_count: usize,
    byte_mask: Option<&[bool]>,
) {
    for element_index in 0..element_count {
        let memory_offset = element_index * stride;
        let destination_offset = element_index * element_bytes;
        let active = byte_mask
            .map(|mask| mask[destination_offset])
            .unwrap_or(true);
        if !active {
            continue;
        }
        destination[destination_offset..destination_offset + element_bytes]
            .copy_from_slice(&data[memory_offset..memory_offset + element_bytes]);
    }
}

fn scatter_indexed_load(
    data: &[u8],
    destination: &mut [u8],
    element_bytes: usize,
    offsets: &[usize],
    byte_mask: Option<&[bool]>,
) {
    for (element_index, memory_offset) in offsets.iter().copied().enumerate() {
        let destination_offset = element_index * element_bytes;
        let active = byte_mask
            .map(|mask| mask[destination_offset])
            .unwrap_or(true);
        if !active {
            continue;
        }
        destination[destination_offset..destination_offset + element_bytes]
            .copy_from_slice(&data[memory_offset..memory_offset + element_bytes]);
    }
}

fn read_vector_register_group(
    hart: &RiscvHartState,
    register: VectorRegister,
    group_registers: usize,
) -> Vec<u8> {
    let group_bytes = group_registers * RISCV_VECTOR_REGISTER_BYTES;
    let mut bytes = vec![0; group_bytes];
    for group_index in 0..group_registers {
        let vector = hart.read_vector(vector_register_at(register, group_index));
        let offset = group_index * RISCV_VECTOR_REGISTER_BYTES;
        bytes[offset..offset + RISCV_VECTOR_REGISTER_BYTES].copy_from_slice(&vector);
    }
    bytes
}

fn write_vector_register_group(
    hart: &mut RiscvHartState,
    register: VectorRegister,
    group_registers: usize,
    bytes: &[u8],
) {
    assert_eq!(
        bytes.len(),
        group_registers * RISCV_VECTOR_REGISTER_BYTES,
        "vector register group payload width"
    );
    for group_index in 0..group_registers {
        let offset = group_index * RISCV_VECTOR_REGISTER_BYTES;
        let mut vector = [0; RISCV_VECTOR_REGISTER_BYTES];
        vector.copy_from_slice(&bytes[offset..offset + RISCV_VECTOR_REGISTER_BYTES]);
        hart.write_vector(vector_register_at(register, group_index), vector);
    }
}

fn vector_register_at(register: VectorRegister, group_index: usize) -> VectorRegister {
    let index = usize::from(register.index()) + group_index;
    VectorRegister::new(index as u8).expect("validated vector register group")
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{MemoryWidth, Register};
    use rem6_memory::AgentId;

    use super::*;

    fn request_id(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(0), sequence)
    }

    fn store_conditional_access(rd: Register) -> MemoryAccessKind {
        MemoryAccessKind::StoreConditional {
            rd,
            address: 0x8000,
            width: MemoryWidth::Doubleword,
            value: 0x1122_3344_5566_7788,
            acquire: false,
            release: false,
        }
    }

    fn lane_mask(active_lanes: &[bool], lane_bytes: usize) -> Vec<bool> {
        active_lanes
            .iter()
            .flat_map(|active| std::iter::repeat(*active).take(lane_bytes))
            .collect()
    }

    fn vector_byte_mask(access: &MemoryAccessKind) -> Option<&[bool]> {
        let MemoryAccessKind::VectorLoadUnitStride { byte_mask, .. } = access else {
            panic!("expected vector unit-stride load");
        };
        byte_mask.as_deref()
    }

    #[test]
    fn masked_load_normalization_expands_nonzero_request_offset() {
        let mask = lane_mask(&[false, true, false], MemoryWidth::Word.bytes());
        let data = [
            0x11, 0x12, 0x13, 0x14, // active lane 1
            0x21, 0x22, 0x23, 0x24, // inactive lane 2 included in the issued prefix
        ];
        let completion = RiscvDataCompletion::from_issued_response(
            request_id(7),
            MemoryAccessKind::VectorLoadUnitStride {
                vd: VectorRegister::new(2).unwrap(),
                address: 0x4000,
                width: MemoryWidth::Word,
                byte_len: 12,
                byte_mask: Some(mask),
                group_registers: 1,
                fault_only_first: false,
            },
            Address::new(0x5004),
            AccessSize::new(8).unwrap(),
            4,
            Some(data.to_vec()),
        );

        assert_eq!(completion.request_byte_offset(), 4);
        let normalized = normalized_masked_load_data(
            12,
            vector_byte_mask(completion.access()),
            completion.request_byte_offset(),
            completion.bytes().unwrap(),
        );

        assert_eq!(normalized.len(), 12);
        assert_eq!(&normalized[0..4], &[0, 0, 0, 0]);
        assert_eq!(&normalized[4..12], &data);
    }

    #[test]
    fn load_reserved_completion_records_payload_reservation_range() {
        let mut state = RiscvCoreState::new(0x1000, 0);
        let completion = RiscvDataCompletion::from_issued_response(
            request_id(11),
            MemoryAccessKind::LoadReserved {
                rd: Register::new(5).unwrap(),
                address: 0x8000,
                width: MemoryWidth::Doubleword,
                acquire: false,
                release: false,
            },
            Address::new(0x9008),
            AccessSize::new(8).unwrap(),
            0,
            Some(0x1122_3344_5566_7788u64.to_le_bytes().to_vec()),
        );

        assert_eq!(completion.fetch_request(), request_id(11));
        assert_eq!(completion.physical_address(), Address::new(0x9008));
        assert_eq!(completion.size(), AccessSize::new(8).unwrap());
        assert_eq!(
            completion.bytes(),
            Some(&0x1122_3344_5566_7788u64.to_le_bytes()[..])
        );
        apply_data_completion(&mut state, CpuId::new(0), &completion, "LR data");

        assert_eq!(
            state.hart.read(Register::new(5).unwrap()),
            0x1122_3344_5566_7788
        );
        assert_eq!(
            state.reservation,
            Some(RiscvLoadReservation::new(
                completion.physical_address(),
                completion.size()
            ))
        );
    }

    #[test]
    fn successful_store_conditional_completion_writes_zero_and_records_success() {
        let cpu = CpuId::new(0);
        let rd = Register::new(7).unwrap();
        let physical_address = Address::new(0x9008);
        let size = AccessSize::new(8).unwrap();
        let mut state = RiscvCoreState::new(0x1000, 0);
        state.hart.write(rd, 9);
        state.reservation = Some(RiscvLoadReservation::new(physical_address, size));
        state
            .sc_progress
            .record_failure(cpu, 31, physical_address, size);
        let completion = RiscvDataCompletion::from_issued_response(
            request_id(12),
            store_conditional_access(rd),
            physical_address,
            size,
            0,
            None,
        );

        assert_eq!(
            completion.data_event_kind(),
            crate::RiscvDataAccessEventKind::Completed
        );
        apply_data_completion(&mut state, cpu, &completion, "SC response data");

        assert_eq!(state.hart.read(rd), 0);
        assert_eq!(state.reservation, None);
        assert_eq!(state.sc_progress.streak(cpu), None);
    }

    #[test]
    fn failed_store_conditional_completion_writes_one_and_preserves_failure_tick() {
        let cpu = CpuId::new(0);
        let rd = Register::new(7).unwrap();
        let physical_address = Address::new(0x9008);
        let size = AccessSize::new(8).unwrap();
        let mut state = RiscvCoreState::new(0x1000, 0);
        state.hart.write(rd, 9);
        state.reservation = Some(RiscvLoadReservation::new(physical_address, size));
        let completion = RiscvDataCompletion::store_conditional_failed(
            request_id(13),
            store_conditional_access(rd),
            physical_address,
            size,
            0,
            37,
        );

        assert_eq!(
            completion.data_event_kind(),
            crate::RiscvDataAccessEventKind::ConditionalFailed
        );
        apply_data_completion(&mut state, cpu, &completion, "SC response data");

        assert_eq!(state.hart.read(rd), 1);
        assert_eq!(state.reservation, None);
        let streak = state.sc_progress.streak(cpu).copied().unwrap();
        assert_eq!(streak.address(), physical_address);
        assert_eq!(streak.size(), size);
        assert_eq!(streak.first_failure_tick(), 37);
        assert_eq!(streak.last_failure_tick(), 37);
        assert_eq!(streak.failure_count(), 1);
    }

    #[test]
    #[should_panic(expected = "store-conditional failure completion requires SC access")]
    fn failed_store_conditional_completion_rejects_non_sc_access() {
        RiscvDataCompletion::store_conditional_failed(
            request_id(14),
            MemoryAccessKind::Load {
                rd: Register::new(5).unwrap(),
                address: 0x8000,
                width: MemoryWidth::Doubleword,
                signed: true,
            },
            Address::new(0x9008),
            AccessSize::new(8).unwrap(),
            0,
            41,
        );
    }
}
