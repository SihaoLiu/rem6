use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, RISCV_VECTOR_REGISTER_BYTES};
use rem6_memory::{AccessSize, Address, CacheLineLayout};

pub(crate) fn supports_cross_line_data_access(
    access: &MemoryAccessKind,
    address: Address,
    size: AccessSize,
    line_layout: CacheLineLayout,
) -> bool {
    let line_bytes = line_layout.bytes();
    if line_layout.line_offset(address) != 0
        || size.bytes() <= line_bytes
        || !size.bytes().is_multiple_of(line_bytes)
    {
        return false;
    }

    match access {
        MemoryAccessKind::VectorLoadUnitStride {
            width,
            byte_len,
            group_registers,
            ..
        } => supported_full_register_group_vector_access(*width, *byte_len, *group_registers, size),
        MemoryAccessKind::VectorStoreUnitStride {
            width,
            data,
            group_registers,
            ..
        } => {
            supported_full_register_group_vector_access(*width, data.len(), *group_registers, size)
        }
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            width,
            fields,
            element_count,
            byte_len,
            byte_mask,
            group_registers,
            ..
        } => supported_two_line_segment_m1_access(
            *width,
            *fields,
            *element_count,
            *byte_len,
            byte_mask.as_deref(),
            *group_registers,
            size,
            line_bytes,
        ),
        MemoryAccessKind::VectorStoreSegmentUnitStride {
            width,
            fields,
            element_count,
            data,
            byte_mask,
            group_registers,
            ..
        } => supported_two_line_segment_m1_access(
            *width,
            *fields,
            *element_count,
            data.len(),
            byte_mask.as_deref(),
            *group_registers,
            size,
            line_bytes,
        ),
        MemoryAccessKind::VectorLoadStrided {
            width,
            stride,
            element_count,
            span_len,
            group_registers,
            ..
        } => sparse_e64_strided_m1_vector_access(
            *width,
            *stride,
            *element_count,
            *span_len,
            *group_registers,
            size,
        ),
        MemoryAccessKind::VectorStoreStrided {
            width,
            stride,
            element_count,
            data,
            group_registers,
            ..
        } => sparse_e64_strided_m1_vector_access(
            *width,
            *stride,
            *element_count,
            data.len(),
            *group_registers,
            size,
        ),
        MemoryAccessKind::VectorLoadIndexed {
            width,
            index_width,
            offsets,
            span_len,
            group_registers,
            ..
        } => sparse_e64_indexed_m1_vector_access(
            *width,
            *index_width,
            offsets,
            *span_len,
            *group_registers,
            size,
        ),
        MemoryAccessKind::VectorStoreIndexed {
            width,
            index_width,
            offsets,
            data,
            group_registers,
            ..
        } => sparse_e64_indexed_m1_vector_access(
            *width,
            *index_width,
            offsets,
            data.len(),
            *group_registers,
            size,
        ),
        _ => false,
    }
}

fn supported_full_register_group_vector_access(
    width: MemoryWidth,
    byte_len: usize,
    group_registers: usize,
    size: AccessSize,
) -> bool {
    let Ok(size_bytes) = usize::try_from(size.bytes()) else {
        return false;
    };
    let full_group_bytes = group_registers * RISCV_VECTOR_REGISTER_BYTES;
    let supported_shape = (width == MemoryWidth::Halfword && group_registers == 2)
        || (width == MemoryWidth::Word && matches!(group_registers, 2 | 4 | 8));
    supported_shape && byte_len == full_group_bytes && size_bytes == byte_len
}

fn supported_two_line_segment_m1_access(
    width: MemoryWidth,
    fields: usize,
    element_count: usize,
    byte_len: usize,
    byte_mask: Option<&[bool]>,
    group_registers: usize,
    size: AccessSize,
    line_bytes: u64,
) -> bool {
    let Ok(size_bytes) = usize::try_from(size.bytes()) else {
        return false;
    };
    group_registers == 1
        && byte_mask.is_none()
        && size.bytes() == line_bytes * 2
        && ((width == MemoryWidth::Doubleword && fields == 2 && element_count == 2)
            || (width == MemoryWidth::Word && fields == 4 && element_count == 2))
        && byte_len == 32
        && size_bytes == byte_len
}

fn sparse_e64_strided_m1_vector_access(
    width: MemoryWidth,
    stride: usize,
    element_count: usize,
    byte_len: usize,
    group_registers: usize,
    size: AccessSize,
) -> bool {
    let Ok(size_bytes) = usize::try_from(size.bytes()) else {
        return false;
    };
    group_registers == 1
        && width == MemoryWidth::Doubleword
        && stride == 24
        && element_count == 2
        && byte_len == 32
        && size_bytes == byte_len
}

fn sparse_e64_indexed_m1_vector_access(
    width: MemoryWidth,
    index_width: MemoryWidth,
    offsets: &[usize],
    byte_len: usize,
    group_registers: usize,
    size: AccessSize,
) -> bool {
    let Ok(size_bytes) = usize::try_from(size.bytes()) else {
        return false;
    };
    group_registers == 1
        && width == MemoryWidth::Doubleword
        && matches!(
            index_width,
            MemoryWidth::Byte | MemoryWidth::Halfword | MemoryWidth::Word | MemoryWidth::Doubleword
        )
        && offsets == [0, 24]
        && byte_len == 32
        && size_bytes == byte_len
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, VectorRegister};
    use rem6_memory::{AccessSize, Address, CacheLineLayout};

    use super::supports_cross_line_data_access;

    #[test]
    fn cross_line_vector_access_accepts_aligned_two_line_full_lmul2_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 32, 2),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_e16_two_line_full_lmul2_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_unit_stride_with_width(0x8000, MemoryWidth::Halfword, 32, 2),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_unit_stride_with_width(0x8000, MemoryWidth::Halfword, 32, 2),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_four_line_full_lmul4_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(64).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 64, 4),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_unit_stride(0x8000, 64, 4),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_eight_line_full_lmul8_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(128).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 128, 8),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_unit_stride(0x8000, 128, 8),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_e64_m1_segment() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_segment_e64_m1(0x8000),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_segment_e64_m1(0x8000),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_e32_m1_four_field_segment() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_segment_e32_m1_four_field(0x8000),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_segment_e32_m1_four_field(0x8000),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_e32_four_field_segment_outside_exact_shape() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size_32 = AccessSize::new(32).unwrap();
        let size_48 = AccessSize::new(48).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_segment_e32_four_field(0x8000, 2, 32, Some(vec![true; 32]), 1),
            Address::new(0x8000),
            size_32,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_store_segment_e32_four_field(0x8000, 2, 32, Some(vec![true; 32]), 1),
            Address::new(0x8000),
            size_32,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_load_segment_e32_four_field(0x8000, 3, 48, None, 1),
            Address::new(0x8000),
            size_48,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_store_segment_e32_four_field(0x8000, 3, 48, None, 1),
            Address::new(0x8000),
            size_48,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_load_segment_e32_m1_four_field(0x8004),
            Address::new(0x8004),
            size_32,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_store_segment_e32_m1_four_field(0x8004),
            Address::new(0x8004),
            size_32,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_load_segment_e32_four_field(0x8000, 2, 32, None, 2),
            Address::new(0x8000),
            size_32,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_store_segment_e32_four_field(0x8000, 2, 32, None, 2),
            Address::new(0x8000),
            size_32,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_four_line_e64_m1_segment() {
        let layout = CacheLineLayout::new(8).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_segment_e64_m1(0x8000),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_store_segment_e64_m1(0x8000),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_masked_e64_m1_segment() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_segment_e64_m1_with_mask(0x8000),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(!supports_cross_line_data_access(
            &vector_store_segment_e64_m1_with_mask(0x8000),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_indexed_e64_m1() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_indexed_e64_m1(0x8000, MemoryWidth::Doubleword, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_indexed_e64_m1(0x8000, MemoryWidth::Doubleword, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_indexed_e64_m1_with_e32_indices() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_indexed_e64_m1(0x8000, MemoryWidth::Word, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_indexed_e64_m1(0x8000, MemoryWidth::Word, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_indexed_e64_m1_with_e16_indices() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_indexed_e64_m1(0x8000, MemoryWidth::Halfword, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_indexed_e64_m1(0x8000, MemoryWidth::Halfword, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_indexed_e64_m1_with_e8_indices() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_indexed_e64_m1(0x8000, MemoryWidth::Byte, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_indexed_e64_m1(0x8000, MemoryWidth::Byte, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_strided_e64_m1() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_strided_e64_m1(0x8000, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_strided_e64_m1(0x8000, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_unaligned_full_lmul2_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_unit_stride(0x8004, 32, 2),
            Address::new(0x8004),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_partial_lmul8_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(64).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 64, 8),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_partial_lmul4_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 32, 4),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    fn vector_load_unit_stride(
        address: u64,
        byte_len: usize,
        group_registers: usize,
    ) -> MemoryAccessKind {
        vector_load_unit_stride_with_width(address, MemoryWidth::Word, byte_len, group_registers)
    }

    fn vector_load_unit_stride_with_width(
        address: u64,
        width: MemoryWidth,
        byte_len: usize,
        group_registers: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadUnitStride {
            vd: VectorRegister::new(2).unwrap(),
            address,
            width,
            byte_len,
            byte_mask: None,
            group_registers,
            fault_only_first: false,
        }
    }

    fn vector_store_unit_stride(
        address: u64,
        byte_len: usize,
        group_registers: usize,
    ) -> MemoryAccessKind {
        vector_store_unit_stride_with_width(address, MemoryWidth::Word, byte_len, group_registers)
    }

    fn vector_store_unit_stride_with_width(
        address: u64,
        width: MemoryWidth,
        byte_len: usize,
        group_registers: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreUnitStride {
            address,
            width,
            data: vec![0; byte_len],
            byte_mask: None,
            group_registers,
        }
    }

    fn vector_load_segment_e64_m1(address: u64) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            vd: VectorRegister::new(2).unwrap(),
            address,
            width: MemoryWidth::Doubleword,
            fields: 2,
            element_count: 2,
            byte_len: 32,
            byte_mask: None,
            group_registers: 1,
        }
    }

    fn vector_load_segment_e32_m1_four_field(address: u64) -> MemoryAccessKind {
        vector_load_segment_e32_four_field(address, 2, 32, None, 1)
    }

    fn vector_load_segment_e32_four_field(
        address: u64,
        element_count: usize,
        byte_len: usize,
        byte_mask: Option<Vec<bool>>,
        group_registers: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            vd: VectorRegister::new(2).unwrap(),
            address,
            width: MemoryWidth::Word,
            fields: 4,
            element_count,
            byte_len,
            byte_mask,
            group_registers,
        }
    }

    fn vector_load_segment_e64_m1_with_mask(address: u64) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            vd: VectorRegister::new(2).unwrap(),
            address,
            width: MemoryWidth::Doubleword,
            fields: 2,
            element_count: 2,
            byte_len: 32,
            byte_mask: Some(vec![true; 32]),
            group_registers: 1,
        }
    }

    fn vector_store_segment_e64_m1(address: u64) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreSegmentUnitStride {
            address,
            width: MemoryWidth::Doubleword,
            fields: 2,
            element_count: 2,
            data: vec![0; 32],
            byte_mask: None,
            group_registers: 1,
        }
    }

    fn vector_store_segment_e32_m1_four_field(address: u64) -> MemoryAccessKind {
        vector_store_segment_e32_four_field(address, 2, 32, None, 1)
    }

    fn vector_store_segment_e32_four_field(
        address: u64,
        element_count: usize,
        byte_len: usize,
        byte_mask: Option<Vec<bool>>,
        group_registers: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreSegmentUnitStride {
            address,
            width: MemoryWidth::Word,
            fields: 4,
            element_count,
            data: vec![0; byte_len],
            byte_mask,
            group_registers,
        }
    }

    fn vector_store_segment_e64_m1_with_mask(address: u64) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreSegmentUnitStride {
            address,
            width: MemoryWidth::Doubleword,
            fields: 2,
            element_count: 2,
            data: vec![0; 32],
            byte_mask: Some(vec![true; 32]),
            group_registers: 1,
        }
    }

    fn vector_load_strided_e64_m1(address: u64, span_len: usize) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadStrided {
            vd: VectorRegister::new(1).unwrap(),
            address,
            width: MemoryWidth::Doubleword,
            stride: 24,
            element_count: 2,
            span_len,
            byte_mask: None,
            group_registers: 1,
        }
    }

    fn vector_store_strided_e64_m1(address: u64, data_len: usize) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreStrided {
            address,
            width: MemoryWidth::Doubleword,
            stride: 24,
            element_count: 2,
            data: vec![0; data_len],
            byte_mask: vec![true; data_len],
            group_registers: 1,
        }
    }

    fn vector_load_indexed_e64_m1(
        address: u64,
        index_width: MemoryWidth,
        span_len: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadIndexed {
            vd: VectorRegister::new(1).unwrap(),
            address,
            width: MemoryWidth::Doubleword,
            index_width,
            offsets: vec![0, 24],
            span_len,
            byte_mask: None,
            group_registers: 1,
        }
    }

    fn vector_store_indexed_e64_m1(
        address: u64,
        index_width: MemoryWidth,
        data_len: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreIndexed {
            address,
            width: MemoryWidth::Doubleword,
            index_width,
            offsets: vec![0, 24],
            data: vec![0; data_len],
            byte_mask: vec![true; data_len],
            group_registers: 1,
        }
    }
}
