use rem6_isa_riscv::{
    MemoryWidth, RiscvVectorCompressPlan, RiscvVectorElements, RiscvVectorTailPolicy,
};

#[test]
fn vcompress_tail_undisturbed_preserves_elements_after_compressed_count() {
    let destination = RiscvVectorElements::new(
        MemoryWidth::Byte,
        vec![
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x70, 0xe8, 0x41, 0x1b, 0x1c, 0x1d,
            0x1e, 0x1f,
        ],
    )
    .unwrap();
    let source = RiscvVectorElements::new(
        MemoryWidth::Byte,
        vec![
            0xf0, 0xe8, 0x41, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ],
    )
    .unwrap();
    let mask = vec![
        false, false, false, false, false, false, false, false, true, true, true, true, true, true,
        true, true,
    ];

    let result = RiscvVectorCompressPlan::new(16, RiscvVectorTailPolicy::Undisturbed)
        .execute(&destination, &source, &mask)
        .unwrap();

    assert_eq!(result.compressed_count(), 8);
    assert_eq!(
        result.elements().as_slice(),
        &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x70, 0xe8, 0x41, 0x1b, 0x1c, 0x1d,
            0x1e, 0x1f
        ]
    );
}

#[test]
fn vcompress_tail_agnostic_uses_deterministic_ones_after_compressed_count() {
    let destination =
        RiscvVectorElements::new(MemoryWidth::Byte, vec![0xaa, 0xbb, 0xcc, 0xdd]).unwrap();
    let source = RiscvVectorElements::new(MemoryWidth::Byte, vec![0x10, 0x20, 0x30, 0x40]).unwrap();
    let mask = vec![true, false, true, false];

    let result = RiscvVectorCompressPlan::new(4, RiscvVectorTailPolicy::AgnosticAllOnes)
        .execute(&destination, &source, &mask)
        .unwrap();

    assert_eq!(result.compressed_count(), 2);
    assert_eq!(result.elements().as_slice(), &[0x10, 0x30, 0xff, 0xff]);
}
