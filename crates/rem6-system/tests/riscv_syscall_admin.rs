#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryReader, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_CHROOT: u64 = 51;
const RISCV_LINUX_EPERM: u64 = 1;
const RISCV_LINUX_EFAULT: u64 = 14;

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

fn admin_store(path: &'static [u8]) -> Arc<Mutex<rem6_memory::PartitionedMemoryStore>> {
    loaded_program_store_with_data(&[(0x8000, 0)], &[(0x9000, path)])
}

fn handle_chroot(
    state: &mut RiscvSyscallState,
    reader: &RiscvGuestMemoryReader,
    path: u64,
) -> Option<RiscvSyscallOutcome> {
    RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CHROOT, [path, 0, 0, 0, 0, 0]),
        state,
        0,
        Some(reader),
        None,
    )
}

#[test]
fn linux_table_chroot_reads_guest_path_before_deterministic_permission_error() {
    let store = admin_store(b"/\0");
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_chroot(&mut state, &reader, 0x9000),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_chroot_faults_null_guest_path_without_unknown_record() {
    let store = admin_store(b"/\0");
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_chroot(&mut state, &reader, 0),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}
