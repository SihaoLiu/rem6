use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, VectorRegister};
use rem6_memory::{AccessSize, Address, CacheLineLayout};

use super::supports_translated_cross_line_data_access;

#[test]
fn translated_cross_line_vector_access_accepts_same_base_page() {
    let layout = CacheLineLayout::new(16).unwrap();
    let size = AccessSize::new(32).unwrap();
    let access = vector_load_unit_stride(0x1000);

    assert!(supports_translated_cross_line_data_access(
        &access,
        Address::new(0x8000),
        size,
        layout
    ));
}

#[test]
fn translated_cross_line_vector_access_rejects_base_page_crossing() {
    let layout = CacheLineLayout::new(16).unwrap();
    let size = AccessSize::new(32).unwrap();
    let access = vector_load_unit_stride(0x1ff0);

    assert!(!supports_translated_cross_line_data_access(
        &access,
        Address::new(0x8000),
        size,
        layout
    ));
}

fn vector_load_unit_stride(address: u64) -> MemoryAccessKind {
    MemoryAccessKind::VectorLoadUnitStride {
        vd: VectorRegister::new(2).unwrap(),
        address,
        width: MemoryWidth::Word,
        byte_len: 32,
        byte_mask: None,
        group_registers: 2,
    }
}
