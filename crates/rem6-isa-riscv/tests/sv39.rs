use rem6_isa_riscv::{
    RiscvSv39AccessKind, RiscvSv39PageFault, RiscvSv39PageTableLevel, RiscvSv39Pte,
    RiscvSv39VirtualAddress,
};

const V: u64 = 1 << 0;
const R: u64 = 1 << 1;
const W: u64 = 1 << 2;
const X: u64 = 1 << 3;
const U: u64 = 1 << 4;
const G: u64 = 1 << 5;
const A: u64 = 1 << 6;
const D: u64 = 1 << 7;

#[test]
fn sv39_pte_decodes_flags_and_physical_page_number() {
    let raw = (0x12345_u64 << 10) | V | R | X | U | G | A | D;
    let pte = RiscvSv39Pte::new(raw);

    assert_eq!(pte.raw(), raw);
    assert!(pte.valid());
    assert!(pte.readable());
    assert!(!pte.writable());
    assert!(pte.executable());
    assert!(pte.user());
    assert!(pte.global());
    assert!(pte.accessed());
    assert!(pte.dirty());
    assert!(pte.is_leaf());
    assert_eq!(pte.physical_page_number(), 0x12345);
    assert_eq!(pte.physical_address_base(), 0x1234_5000);
    assert_eq!(pte.validate(), Ok(()));
    assert_eq!(
        pte.validate_leaf_access(RiscvSv39AccessKind::InstructionFetch),
        Ok(())
    );
    assert_eq!(pte.validate_leaf_access(RiscvSv39AccessKind::Load), Ok(()));
}

#[test]
fn sv39_pte_rejects_invalid_reserved_and_reserved_permission_encodings() {
    assert_eq!(
        RiscvSv39Pte::new(R).validate(),
        Err(RiscvSv39PageFault::InvalidEntry)
    );
    assert_eq!(
        RiscvSv39Pte::new(V | W | A | D).validate(),
        Err(RiscvSv39PageFault::ReservedPermissionEncoding)
    );
    assert_eq!(
        RiscvSv39Pte::new((1 << 54) | V | R | A).validate(),
        Err(RiscvSv39PageFault::ReservedBitsSet { bits: 1 << 54 })
    );

    let pointer = RiscvSv39Pte::new(V);
    assert!(!pointer.is_leaf());
    assert_eq!(pointer.validate(), Ok(()));
    assert_eq!(
        pointer.validate_leaf_access(RiscvSv39AccessKind::Load),
        Err(RiscvSv39PageFault::NonLeaf)
    );
}

#[test]
fn sv39_pte_checks_leaf_access_permissions_and_ad_bits() {
    let read_write = RiscvSv39Pte::new((0x20_u64 << 10) | V | R | W | A | D);
    assert_eq!(
        read_write.validate_leaf_access(RiscvSv39AccessKind::Load),
        Ok(())
    );
    assert_eq!(
        read_write.validate_leaf_access(RiscvSv39AccessKind::Store),
        Ok(())
    );
    assert_eq!(
        read_write.validate_leaf_access(RiscvSv39AccessKind::Atomic),
        Ok(())
    );
    assert_eq!(
        read_write.validate_leaf_access(RiscvSv39AccessKind::InstructionFetch),
        Err(RiscvSv39PageFault::PermissionDenied {
            access: RiscvSv39AccessKind::InstructionFetch
        })
    );

    let execute_only = RiscvSv39Pte::new((0x21_u64 << 10) | V | X | A);
    assert_eq!(
        execute_only.validate_leaf_access(RiscvSv39AccessKind::InstructionFetch),
        Ok(())
    );
    assert_eq!(
        execute_only.validate_leaf_access(RiscvSv39AccessKind::Load),
        Err(RiscvSv39PageFault::PermissionDenied {
            access: RiscvSv39AccessKind::Load
        })
    );
    assert_eq!(
        execute_only.validate_leaf_access(RiscvSv39AccessKind::Store),
        Err(RiscvSv39PageFault::PermissionDenied {
            access: RiscvSv39AccessKind::Store
        })
    );

    let accessed_clear = RiscvSv39Pte::new((0x22_u64 << 10) | V | R | D);
    assert_eq!(
        accessed_clear.validate_leaf_access(RiscvSv39AccessKind::Load),
        Err(RiscvSv39PageFault::AccessedBitClear)
    );

    let dirty_clear = RiscvSv39Pte::new((0x23_u64 << 10) | V | R | W | A);
    assert_eq!(
        dirty_clear.validate_leaf_access(RiscvSv39AccessKind::Store),
        Err(RiscvSv39PageFault::DirtyBitClear)
    );
    assert_eq!(
        dirty_clear.validate_leaf_access(RiscvSv39AccessKind::Atomic),
        Err(RiscvSv39PageFault::DirtyBitClear)
    );
}

#[test]
fn sv39_virtual_address_decodes_canonical_low_and_high_halves() {
    let low_raw = (0x0aa_u64 << 30) | (0x155_u64 << 21) | (0x1fe_u64 << 12) | 0xabc;
    let low = RiscvSv39VirtualAddress::new(low_raw).unwrap();

    assert_eq!(low.raw(), low_raw);
    assert_eq!(low.page_offset(), 0xabc);
    assert_eq!(
        low.virtual_page_number(),
        (0x0aa_u32 << 18) | (0x155_u32 << 9) | 0x1fe
    );
    assert_eq!(low.vpn(RiscvSv39PageTableLevel::Level0), 0x1fe);
    assert_eq!(low.vpn(RiscvSv39PageTableLevel::Level1), 0x155);
    assert_eq!(low.vpn(RiscvSv39PageTableLevel::Level2), 0x0aa);

    let high_low_bits = (0x1ab_u64 << 30) | (0x101_u64 << 21) | (0x017_u64 << 12) | 0x678;
    let high_raw = high_low_bits | (u64::MAX << 39);
    let high = RiscvSv39VirtualAddress::new(high_raw).unwrap();

    assert_eq!(high.raw(), high_raw);
    assert_eq!(high.page_offset(), 0x678);
    assert_eq!(
        high.virtual_page_number(),
        (0x1ab_u32 << 18) | (0x101_u32 << 9) | 0x017
    );
    assert_eq!(high.vpn(RiscvSv39PageTableLevel::Level0), 0x017);
    assert_eq!(high.vpn(RiscvSv39PageTableLevel::Level1), 0x101);
    assert_eq!(high.vpn(RiscvSv39PageTableLevel::Level2), 0x1ab);
}

#[test]
fn sv39_virtual_address_rejects_noncanonical_hole_addresses() {
    assert_eq!(
        RiscvSv39VirtualAddress::new(1 << 39),
        Err(RiscvSv39PageFault::NonCanonicalVirtualAddress { address: 1 << 39 })
    );
    assert_eq!(
        RiscvSv39VirtualAddress::new(1 << 38),
        Err(RiscvSv39PageFault::NonCanonicalVirtualAddress { address: 1 << 38 })
    );
}

#[test]
fn sv39_virtual_address_computes_page_table_entry_addresses() {
    let address =
        RiscvSv39VirtualAddress::new((0x012_u64 << 30) | (0x034_u64 << 21) | (0x056_u64 << 12))
            .unwrap();

    assert_eq!(
        address.page_table_entry_address(0x12345, RiscvSv39PageTableLevel::Level2),
        Ok((0x12345_u64 << 12) + (0x012 * 8))
    );
    assert_eq!(
        address.page_table_entry_address(0x22001, RiscvSv39PageTableLevel::Level1),
        Ok((0x22001_u64 << 12) + (0x034 * 8))
    );
    assert_eq!(
        address.page_table_entry_address(0x33002, RiscvSv39PageTableLevel::Level0),
        Ok((0x33002_u64 << 12) + (0x056 * 8))
    );
    assert_eq!(
        address.page_table_entry_address(1 << 44, RiscvSv39PageTableLevel::Level2),
        Err(RiscvSv39PageFault::PageTablePointerOutOfRange { ppn: 1 << 44 })
    );
}

#[test]
fn sv39_leaf_physical_address_uses_level_specific_page_fragments() {
    let address = RiscvSv39VirtualAddress::new(
        (0x012_u64 << 30) | (0x034_u64 << 21) | (0x056_u64 << 12) | 0x789,
    )
    .unwrap();

    let page = RiscvSv39Pte::new((0x12345_u64 << 10) | V | R | A);
    assert_eq!(
        page.leaf_physical_address(
            address,
            RiscvSv39PageTableLevel::Level0,
            RiscvSv39AccessKind::Load
        ),
        Ok((0x12345_u64 << 12) | 0x789)
    );

    let megapage_ppn = (0x1234_u64 << 18) | (0x1ab_u64 << 9);
    let megapage = RiscvSv39Pte::new((megapage_ppn << 10) | V | R | W | A | D);
    assert_eq!(
        megapage.leaf_physical_address(
            address,
            RiscvSv39PageTableLevel::Level1,
            RiscvSv39AccessKind::Store,
        ),
        Ok((0x1234_u64 << 30) | (0x1ab_u64 << 21) | (0x056_u64 << 12) | 0x789)
    );

    let gigapage_ppn = 0x2345_u64 << 18;
    let gigapage = RiscvSv39Pte::new((gigapage_ppn << 10) | V | X | A);
    assert_eq!(
        gigapage.leaf_physical_address(
            address,
            RiscvSv39PageTableLevel::Level2,
            RiscvSv39AccessKind::InstructionFetch,
        ),
        Ok((0x2345_u64 << 30) | (0x034_u64 << 21) | (0x056_u64 << 12) | 0x789)
    );

    let misaligned_megapage = RiscvSv39Pte::new(((megapage_ppn | 1) << 10) | V | R | A);
    assert_eq!(
        misaligned_megapage.leaf_physical_address(
            address,
            RiscvSv39PageTableLevel::Level1,
            RiscvSv39AccessKind::Load,
        ),
        Err(RiscvSv39PageFault::MisalignedSuperpage {
            level: RiscvSv39PageTableLevel::Level1,
            ppn: megapage_ppn | 1,
        })
    );

    let misaligned_gigapage = RiscvSv39Pte::new(((gigapage_ppn | (1 << 9)) << 10) | V | X | A);
    assert_eq!(
        misaligned_gigapage.leaf_physical_address(
            address,
            RiscvSv39PageTableLevel::Level2,
            RiscvSv39AccessKind::InstructionFetch,
        ),
        Err(RiscvSv39PageFault::MisalignedSuperpage {
            level: RiscvSv39PageTableLevel::Level2,
            ppn: gigapage_ppn | (1 << 9),
        })
    );
}
