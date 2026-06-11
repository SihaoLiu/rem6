use crate::{
    GuestProcessGroupId, GuestWaitOptions, GuestWaitOutcome, GuestWaitQueue, GuestWaitSelector,
};

use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallEmulation, RiscvSyscallIdentity,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_WAIT4: u64 = 260;

const RISCV_LINUX_ECHILD: u64 = 10;
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
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .push_wait_child(child);
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
                if rusage_address != 0 {
                    let rusage = [0; RISCV64_LINUX_RUSAGE_BYTES];
                    if !writer.write(rusage_address, &rusage) {
                        return linux_error(RISCV_LINUX_EFAULT);
                    }
                }
            }
            u64::from(child.pid().get())
        }
        GuestWaitOutcome::NoReady | GuestWaitOutcome::Retry => linux_error(RISCV_LINUX_ECHILD),
    }
}
