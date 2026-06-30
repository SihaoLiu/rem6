use super::*;

const RISCV_LINUX_UMOUNT2_FOR_TEST: u64 = 39;
const RISCV_LINUX_MOUNT_FOR_TEST: u64 = 40;
const RISCV_LINUX_PIVOT_ROOT_FOR_TEST: u64 = 41;
const RISCV_LINUX_VHANGUP_FOR_TEST: u64 = 58;
const RISCV_LINUX_ACCT_FOR_TEST: u64 = 89;
const RISCV_LINUX_KEXEC_LOAD_FOR_TEST: u64 = 104;
const RISCV_LINUX_INIT_MODULE_FOR_TEST: u64 = 105;
const RISCV_LINUX_DELETE_MODULE_FOR_TEST: u64 = 106;
const RISCV_LINUX_REBOOT_FOR_TEST: u64 = 142;
const RISCV_LINUX_SETHOSTNAME_FOR_TEST: u64 = 161;
const RISCV_LINUX_SETDOMAINNAME_FOR_TEST: u64 = 162;
const RISCV_LINUX_SWAPON_FOR_TEST: u64 = 224;
const RISCV_LINUX_SWAPOFF_FOR_TEST: u64 = 225;
const RISCV_LINUX_FINIT_MODULE_FOR_TEST: u64 = 273;
const RISCV_LINUX_KEXEC_FILE_LOAD_FOR_TEST: u64 = 294;
const RISCV_LINUX_EINVAL_FOR_TEST: u64 = 22;

#[test]
fn linux_table_admin_syscalls_report_deterministic_errors_without_unknown_records() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory = admin_guest_memory_reader();

    for (pc, number, arguments, errno) in [
        (
            0x8000,
            RISCV_LINUX_UMOUNT2_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x8004,
            RISCV_LINUX_UMOUNT2_FOR_TEST,
            [0x9000, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8006,
            RISCV_LINUX_UMOUNT2_FOR_TEST,
            [0x9000, 0x8000_0000, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL_FOR_TEST,
        ),
        (
            0x8008,
            RISCV_LINUX_MOUNT_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x800c,
            RISCV_LINUX_MOUNT_FOR_TEST,
            [0x9000, 0x9000, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x800e,
            RISCV_LINUX_MOUNT_FOR_TEST,
            [0x9000, 0x9000, 1, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x8010,
            RISCV_LINUX_PIVOT_ROOT_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x8014,
            RISCV_LINUX_PIVOT_ROOT_FOR_TEST,
            [0x9000, 0x9000, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8018,
            RISCV_LINUX_ACCT_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x801c,
            RISCV_LINUX_REBOOT_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8020,
            RISCV_LINUX_SETHOSTNAME_FOR_TEST,
            [0, 4, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x8022,
            RISCV_LINUX_SETHOSTNAME_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x8024,
            RISCV_LINUX_SETHOSTNAME_FOR_TEST,
            [0x9010, 4, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8028,
            RISCV_LINUX_SETDOMAINNAME_FOR_TEST,
            [0, 4, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x802a,
            RISCV_LINUX_SETDOMAINNAME_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x802c,
            RISCV_LINUX_SETDOMAINNAME_FOR_TEST,
            [0x9010, 4, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8030,
            RISCV_LINUX_SWAPON_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x8034,
            RISCV_LINUX_SWAPON_FOR_TEST,
            [0x9000, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8036,
            RISCV_LINUX_SWAPON_FOR_TEST,
            [0x9000, 0x8000_0000, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL_FOR_TEST,
        ),
        (
            0x8038,
            RISCV_LINUX_SWAPOFF_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EFAULT,
        ),
        (
            0x803c,
            RISCV_LINUX_SWAPOFF_FOR_TEST,
            [0x9000, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8040,
            RISCV_LINUX_VHANGUP_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8044,
            RISCV_LINUX_KEXEC_LOAD_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8048,
            RISCV_LINUX_INIT_MODULE_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x804c,
            RISCV_LINUX_DELETE_MODULE_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8050,
            RISCV_LINUX_FINIT_MODULE_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8054,
            RISCV_LINUX_KEXEC_FILE_LOAD_FOR_TEST,
            [0, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, number, arguments),
                &mut state,
                29,
                Some(&guest_memory),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

fn admin_guest_memory_reader() -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(|address, len| {
        read_from_region(address, len, 0x9000, b"/\0")
            .or_else(|| read_from_region(address, len, 0x9010, b"rem6\0"))
    })
}

fn read_from_region(address: u64, len: usize, base: u64, bytes: &[u8]) -> Option<Vec<u8>> {
    let offset = usize::try_from(address.checked_sub(base)?).ok()?;
    let end = offset.checked_add(len)?;
    bytes.get(offset..end).map(<[u8]>::to_vec)
}
