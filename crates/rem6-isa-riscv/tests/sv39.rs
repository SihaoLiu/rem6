use rem6_isa_riscv::{RiscvSv39AccessKind, RiscvSv39PageFault, RiscvSv39Pte};

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
