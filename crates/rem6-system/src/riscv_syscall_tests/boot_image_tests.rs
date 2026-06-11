use super::*;

use rem6_memory::Address;

#[test]
fn riscv_se_boot_image_program_break_uses_page_rounded_loaded_end() {
    let empty = BootImage::new(Address::new(0x8000));
    assert_eq!(
        RiscvSyscallEmulation::try_linux_user_for_boot_image(&empty)
            .unwrap()
            .state()
            .program_break(),
        0
    );

    let aligned = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x9000), vec![0; 0x1000])
        .unwrap();
    assert_eq!(
        RiscvSyscallEmulation::try_linux_user_for_boot_image(&aligned)
            .unwrap()
            .state()
            .program_break(),
        0xa000
    );

    let unaligned = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x9002), vec![0xaa, 0xbb, 0xcc])
        .unwrap();
    assert_eq!(
        RiscvSyscallEmulation::try_linux_user_for_boot_image(&unaligned)
            .unwrap()
            .state()
            .program_break(),
        0xa000
    );
}

#[test]
fn riscv_se_boot_image_program_break_rejects_unrepresentable_page_rounding() {
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(u64::MAX - 2), vec![0xaa, 0xbb])
        .unwrap();

    let Err(error) = RiscvSyscallEmulation::try_linux_user_for_boot_image(&image) else {
        panic!("expected unrepresentable program break error");
    };
    assert_eq!(
        error,
        RiscvSyscallImageLayoutError::UnrepresentableProgramBreak {
            loaded_segment_end: u64::MAX,
            page_bytes: RISCV_PAGE_BYTES,
        }
    );
}
