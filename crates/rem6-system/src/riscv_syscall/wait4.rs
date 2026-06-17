use crate::{
    GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestWaitOptions, GuestWaitOutcome,
    GuestWaitQueue, GuestWaitSelector, GuestWaitStatus,
};

use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallEmulation, RiscvSyscallIdentity,
    RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EFAULT,
    RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_GETRUSAGE: u64 = 165;
pub(super) const RISCV_LINUX_WAITID: u64 = 95;
pub(super) const RISCV_LINUX_WAIT4: u64 = 260;

const RISCV_LINUX_ECHILD: u64 = 10;
const RISCV_LINUX_RUSAGE_CHILDREN: u64 = (-1_i64) as u64;
const RISCV_LINUX_RUSAGE_SELF: u64 = 0;
const RISCV_LINUX_RUSAGE_THREAD: u64 = 1;
const RISCV_LINUX_P_ALL: u64 = 0;
const RISCV_LINUX_P_PID: u64 = 1;
const RISCV_LINUX_P_PGID: u64 = 2;
const RISCV_LINUX_WNOHANG: u64 = 0x0000_0001;
const RISCV_LINUX_WSTOPPED: u64 = 0x0000_0002;
const RISCV_LINUX_WEXITED: u64 = 0x0000_0004;
const RISCV_LINUX_WCONTINUED: u64 = 0x0000_0008;
const RISCV_LINUX_WNOWAIT: u64 = 0x0100_0000;
const RISCV64_LINUX_RUSAGE_BYTES: usize = 144;
const RISCV64_LINUX_SIGINFO_BYTES: usize = 128;
const RISCV64_LINUX_SIGINFO_SIGCHLD: i32 = 17;
const RISCV64_LINUX_CLD_EXITED: i32 = 1;
const RISCV64_LINUX_CLD_KILLED: i32 = 2;
const RISCV64_LINUX_CLD_DUMPED: i32 = 3;
const RISCV64_LINUX_CLD_STOPPED: i32 = 5;
const RISCV64_LINUX_CLD_CONTINUED: i32 = 6;
const RISCV64_LINUX_SIGCONT: i32 = 18;

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

pub(super) fn syscall_waitid(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> RiscvSyscallOutcome {
    let Some(selector) = waitid_selector(request.argument(0), request.argument(1)) else {
        return waitid_return(linux_error(RISCV_LINUX_EINVAL));
    };
    let requested_options = request.argument(3);
    if !valid_waitid_options(requested_options) {
        return waitid_return(linux_error(RISCV_LINUX_EINVAL));
    }
    let wait_options = if requested_options & RISCV_LINUX_WNOHANG != 0 {
        GuestWaitOptions::nonblocking()
    } else {
        GuestWaitOptions::blocking()
    };

    match waitid_child(selector, requested_options, state) {
        WaitidChildOutcome::Ready(child) => {
            let siginfo_address = request.argument(2);
            let rusage_address = request.argument(4);
            let writer_required = siginfo_address != 0 || rusage_address != 0;
            let writer = if writer_required {
                let Some(writer) = guest_memory_writer else {
                    return waitid_return(linux_error(RISCV_LINUX_EFAULT));
                };
                Some(writer)
            } else {
                None
            };
            if let Some(writer) = writer {
                if siginfo_address != 0
                    && !write_waitid_siginfo(siginfo_address, child, state.identity(), writer)
                {
                    return waitid_return(linux_error(RISCV_LINUX_EFAULT));
                }
                if rusage_address != 0 && write_zero_rusage(rusage_address, writer) != 0 {
                    return waitid_return(linux_error(RISCV_LINUX_EFAULT));
                }
            }
            waitid_return(0)
        }
        WaitidChildOutcome::NoWaitableChild if wait_options.is_nonblocking() => {
            let siginfo_address = request.argument(2);
            if siginfo_address != 0 {
                let Some(writer) = guest_memory_writer else {
                    return waitid_return(linux_error(RISCV_LINUX_EFAULT));
                };
                if !write_zero_siginfo(siginfo_address, writer) {
                    return waitid_return(linux_error(RISCV_LINUX_EFAULT));
                }
            }
            waitid_return(0)
        }
        WaitidChildOutcome::NoWaitableChild => RiscvSyscallOutcome::Blocked,
        WaitidChildOutcome::NoChild => waitid_return(linux_error(RISCV_LINUX_ECHILD)),
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

fn waitid_selector(id_type: u64, id: u64) -> Option<GuestWaitSelector> {
    match id_type {
        RISCV_LINUX_P_ALL => Some(GuestWaitSelector::AnyChild),
        RISCV_LINUX_P_PID => {
            let pid = u32::try_from(id).ok()?;
            GuestProcessId::new(pid)
                .ok()
                .map(GuestWaitSelector::Process)
        }
        RISCV_LINUX_P_PGID => {
            if id == 0 {
                Some(GuestWaitSelector::CurrentProcessGroup)
            } else {
                let process_group = u32::try_from(id).ok()?;
                GuestProcessGroupId::new(process_group)
                    .ok()
                    .map(GuestWaitSelector::ProcessGroup)
            }
        }
        _ => None,
    }
}

const fn valid_waitid_options(options: u64) -> bool {
    const STATUS_MASK: u64 = RISCV_LINUX_WEXITED | RISCV_LINUX_WSTOPPED | RISCV_LINUX_WCONTINUED;
    const VALID_MASK: u64 = STATUS_MASK | RISCV_LINUX_WNOHANG | RISCV_LINUX_WNOWAIT;

    options & !VALID_MASK == 0 && options & STATUS_MASK != 0
}

const fn waitid_return(value: u64) -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return { value }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WaitidChildOutcome {
    Ready(GuestChildStatus),
    NoChild,
    NoWaitableChild,
}

fn waitid_child(
    selector: GuestWaitSelector,
    requested_options: u64,
    state: &mut RiscvSyscallState,
) -> WaitidChildOutcome {
    let snapshot = state.guest_wait.snapshot();
    let current_process_group = snapshot.current_process_group();
    let mut selected_child_exists = false;
    let waitable_child = snapshot.pending().iter().copied().find(|child| {
        if !waitid_selector_matches(selector, *child, current_process_group) {
            return false;
        }
        selected_child_exists = true;
        waitid_status_matches(child.status(), requested_options)
    });
    let Some(child) = waitable_child else {
        return if selected_child_exists {
            WaitidChildOutcome::NoWaitableChild
        } else {
            WaitidChildOutcome::NoChild
        };
    };

    if requested_options & RISCV_LINUX_WNOWAIT != 0 {
        return WaitidChildOutcome::Ready(child);
    }

    state
        .guest_wait
        .take_matching(selector, |candidate| candidate == child)
        .map_or(
            WaitidChildOutcome::NoWaitableChild,
            WaitidChildOutcome::Ready,
        )
}

fn waitid_selector_matches(
    selector: GuestWaitSelector,
    child: GuestChildStatus,
    current_process_group: GuestProcessGroupId,
) -> bool {
    match selector {
        GuestWaitSelector::AnyChild => true,
        GuestWaitSelector::CurrentProcessGroup => child.process_group() == current_process_group,
        GuestWaitSelector::Process(pid) => child.pid() == pid,
        GuestWaitSelector::ProcessGroup(process_group) => child.process_group() == process_group,
    }
}

const fn waitid_status_matches(status: GuestWaitStatus, requested_options: u64) -> bool {
    (status.is_exited() || status.is_signaled()) && requested_options & RISCV_LINUX_WEXITED != 0
        || status.is_stopped() && requested_options & RISCV_LINUX_WSTOPPED != 0
        || status.is_continued() && requested_options & RISCV_LINUX_WCONTINUED != 0
}

fn write_waitid_siginfo(
    address: u64,
    child: GuestChildStatus,
    identity: RiscvSyscallIdentity,
    writer: &RiscvGuestMemoryWriter,
) -> bool {
    let mut siginfo = [0; RISCV64_LINUX_SIGINFO_BYTES];
    write_le_i32(&mut siginfo, 0, RISCV64_LINUX_SIGINFO_SIGCHLD);
    write_le_i32(&mut siginfo, 8, waitid_siginfo_code(child.status()));
    write_le_i32(&mut siginfo, 16, child.pid().get() as i32);
    write_le_u32(&mut siginfo, 20, linux_siginfo_user_id(identity.user_id()));
    write_le_i32(&mut siginfo, 24, waitid_siginfo_status(child.status()));
    writer.write(address, &siginfo)
}

fn write_zero_siginfo(address: u64, writer: &RiscvGuestMemoryWriter) -> bool {
    writer.write(address, &[0; RISCV64_LINUX_SIGINFO_BYTES])
}

const fn waitid_siginfo_code(status: GuestWaitStatus) -> i32 {
    if status.is_exited() {
        RISCV64_LINUX_CLD_EXITED
    } else if status.is_signaled() && status.core_dumped() {
        RISCV64_LINUX_CLD_DUMPED
    } else if status.is_signaled() {
        RISCV64_LINUX_CLD_KILLED
    } else if status.is_stopped() {
        RISCV64_LINUX_CLD_STOPPED
    } else {
        RISCV64_LINUX_CLD_CONTINUED
    }
}

const fn waitid_siginfo_status(status: GuestWaitStatus) -> i32 {
    if let Some(code) = status.exit_code() {
        code as i32
    } else if let Some(signal) = status.terminating_signal() {
        signal.number() as i32
    } else if let Some(signal) = status.stop_signal() {
        signal.number() as i32
    } else {
        RISCV64_LINUX_SIGCONT
    }
}

fn linux_siginfo_user_id(value: u64) -> u32 {
    value.min(u32::MAX as u64) as u32
}

fn write_le_i32(output: &mut [u8], offset: usize, value: i32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_le_u32(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
