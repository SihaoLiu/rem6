use rem6_isa_riscv::{RiscvPmaAccessKind, RiscvPmaError, RiscvPmaRange, RiscvPmaTable};

#[test]
fn pma_rejects_misaligned_data_access_without_supported_region() {
    let pma = RiscvPmaTable::new();

    assert_eq!(
        pma.check_data_alignment(0x1003, 8, RiscvPmaAccessKind::Read),
        Err(RiscvPmaError::MisalignedDataAccess {
            address: 0x1003,
            size: 8,
            kind: RiscvPmaAccessKind::Read,
        })
    );
}

#[test]
fn pma_allows_misaligned_data_access_inside_declared_region() {
    let mut pma = RiscvPmaTable::new();
    pma.add_misaligned_range(RiscvPmaRange::new(0x1000, 0x1100).unwrap())
        .unwrap();

    assert_eq!(
        pma.check_data_alignment(0x1003, 8, RiscvPmaAccessKind::Write),
        Ok(())
    );
    assert_eq!(
        pma.check_data_alignment(0x10fc, 8, RiscvPmaAccessKind::Write),
        Err(RiscvPmaError::MisalignedDataAccess {
            address: 0x10fc,
            size: 8,
            kind: RiscvPmaAccessKind::Write,
        })
    );
}

#[test]
fn pma_keeps_aligned_data_access_independent_of_regions() {
    let pma = RiscvPmaTable::new();

    assert_eq!(
        pma.check_data_alignment(0x2000, 8, RiscvPmaAccessKind::Read),
        Ok(())
    );
}

#[test]
fn pma_marks_accesses_uncacheable_only_inside_declared_ranges() {
    let mut pma = RiscvPmaTable::new();
    pma.add_uncacheable_range(RiscvPmaRange::new(0x2000, 0x2100).unwrap())
        .unwrap();

    assert_eq!(pma.is_uncacheable(0x2008, 8), Ok(true));
    assert_eq!(pma.is_uncacheable(0x20fc, 8), Ok(false));
    assert_eq!(pma.is_uncacheable(0x2100, 4), Ok(false));
    assert_eq!(
        pma.uncacheable_ranges(),
        &[RiscvPmaRange::new(0x2000, 0x2100).unwrap()]
    );
}
