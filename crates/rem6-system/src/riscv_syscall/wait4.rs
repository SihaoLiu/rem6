use crate::{
    GuestProcessGroupId, GuestWaitOptions, GuestWaitOutcome, GuestWaitQueue, GuestWaitSelector,
};

use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallEmulation, RiscvSyscallIdentity,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_GETRUSAGE: u64 = 165;
pub(super) const RISCV_LINUX_WAIT4: u64 = 260;

const RISCV_LINUX_ECHILD: u64 = 10;
const RISCV_LINUX_RUSAGE_CHILDREN: u64 = (-1_i64) as u64;
const RISCV_LINUX_RUSAGE_SELF: u64 = 0;
const RISCV_LINUX_RUSAGE_THREAD: u64 = 1;
const RISCV_LINUX_WNOHANG: u64 = 0x0000_0001;
const RISCV64_LINUX_RUSAGE_BYTES: usize = 144;

impl RiscvSyscallState {
    pub const fn guest_wait_queue(&self) -> &GuestWaitQueue {
        &self.guest_wait
    }

    pub fn push_wait_child(&mut self, child: crate::GuestChildStatus) {
        self.guest_wait.push(child);
    }
}

impl RiscvSyscallEmulation {
    pub fn push_wait_child(&self, child: crate::GuestChildStatus) {
        self.with_state_mut(|state| state.push_wait_child(child));
    }
}

pub(super) fn syscall_process_group_id(identity: RiscvSyscallIdentity) -> GuestProcessGroupId {
    let process_group =
        u32::try_from(identity.thread_group_id()).expect("RISC-V Linux process id fits u32");
    GuestProcessGroupId::new(process_group).expect("RISC-V Linux process id is nonzero")
}

pub(super) fn syscall_wait4(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    let Ok(selector) = GuestWaitSelector::from_wait4_pid(request.argument(0) as i64 as i32) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let requested_options = request.argument(2);
    if requested_options & !RISCV_LINUX_WNOHANG != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let options = if requested_options & RISCV_LINUX_WNOHANG != 0 {
        GuestWaitOptions::nonblocking()
    } else {
        GuestWaitOptions::blocking()
    };

    match state.guest_wait.wait(selector, options) {
        GuestWaitOutcome::Ready(child) => {
            let status_address = request.argument(1);
            let rusage_address = request.argument(3);
            let writer_required = status_address != 0 || rusage_address != 0;
            let writer = if writer_required {
                let Some(writer) = guest_memory_writer else {
                    return linux_error(RISCV_LINUX_EFAULT);
                };
                Some(writer)
            } else {
                None
            };
            if let Some(writer) = writer {
                if status_address != 0 {
                    let status = child.status().raw_wait_status().to_le_bytes();
                    if !writer.write(status_address, &status) {
                        return linux_error(RISCV_LINUX_EFAULT);
                    }
                }
                if rusage_address != 0 && write_zero_rusage(rusage_address, writer) != 0 {
                    return linux_error(RISCV_LINUX_EFAULT);
                }
            }
            u64::from(child.pid().get())
        }
        GuestWaitOutcome::NoReady | GuestWaitOutcome::Retry => linux_error(RISCV_LINUX_ECHILD),
    }
}

pub(super) fn syscall_getrusage(
    request: RiscvSyscallRequest,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> u64 {
    if !valid_rusage_selector(request.argument(0)) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let usage_address = request.argument(1);
    if usage_address == 0 {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    let Some(writer) = guest_memory_writer else {
        return linux_error(RISCV_LINUX_EFAULT);
    };
    write_zero_rusage(usage_address, writer)
}

fn write_zero_rusage(address: u64, writer: &RiscvGuestMemoryWriter) -> u64 {
    let rusage = [0; RISCV64_LINUX_RUSAGE_BYTES];
    if writer.write(address, &rusage) {
        0
    } else {
        linux_error(RISCV_LINUX_EFAULT)
    }
}

const fn valid_rusage_selector(selector: u64) -> bool {
    matches!(
        selector,
        RISCV_LINUX_RUSAGE_SELF | RISCV_LINUX_RUSAGE_CHILDREN | RISCV_LINUX_RUSAGE_THREAD
    )
}
