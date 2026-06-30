use super::{
    iovec::{read_iovecs, RiscvIovec, RISCV_LINUX_IOV_MAX},
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ESRCH,
};

pub(super) const RISCV_LINUX_PROCESS_VM_READV: u64 = 270;
pub(super) const RISCV_LINUX_PROCESS_VM_WRITEV: u64 = 271;

pub(super) fn syscall_process_vm_readv(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    syscall_process_vm_iovecs(
        request,
        state,
        guest_memory_reader,
        guest_memory_writer,
        ProcessVmDirection::RemoteToLocal,
    )
}

pub(super) fn syscall_process_vm_writev(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    syscall_process_vm_iovecs(
        request,
        state,
        guest_memory_reader,
        guest_memory_writer,
        ProcessVmDirection::LocalToRemote,
    )
}

#[derive(Clone, Copy)]
enum ProcessVmDirection {
    RemoteToLocal,
    LocalToRemote,
}

fn syscall_process_vm_iovecs(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
    direction: ProcessVmDirection,
) -> u64 {
    if request.argument(5) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let local_iov_count = request.argument(2);
    let remote_iov_count = request.argument(4);
    if local_iov_count > RISCV_LINUX_IOV_MAX || remote_iov_count > RISCV_LINUX_IOV_MAX {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if local_iov_count == 0 {
        return 0;
    }

    let (local_iovecs, local_total) =
        match read_iovecs(guest_memory_reader, request.argument(1), local_iov_count) {
            Ok(iovecs) => iovecs,
            Err(errno) => return linux_error(errno),
        };
    if local_total == 0 || remote_iov_count == 0 {
        return 0;
    }
    let (remote_iovecs, remote_total) =
        match read_iovecs(guest_memory_reader, request.argument(3), remote_iov_count) {
            Ok(iovecs) => iovecs,
            Err(errno) => return linux_error(errno),
        };
    let transfer = local_total.min(remote_total);
    if transfer == 0 {
        return 0;
    }
    if let Err(errno) = process_vm_target(request.argument(0), state) {
        return linux_error(errno);
    }

    let (source_iovecs, destination_iovecs) = match direction {
        ProcessVmDirection::RemoteToLocal => (&remote_iovecs, &local_iovecs),
        ProcessVmDirection::LocalToRemote => (&local_iovecs, &remote_iovecs),
    };

    match copy_process_vm_iovecs(
        guest_memory_reader,
        guest_memory_writer,
        source_iovecs,
        destination_iovecs,
        transfer,
    ) {
        Ok(copied) => copied,
        Err(errno) => linux_error(errno),
    }
}

fn process_vm_target(pid_argument: u64, state: &RiscvSyscallState) -> Result<(), u64> {
    let requested_pid = pid_argument as u32 as i32;
    if requested_pid <= 0 {
        return Err(RISCV_LINUX_ESRCH);
    }
    let requested_pid = requested_pid as u64;
    if requested_pid == state.identity().thread_group_id()
        || requested_pid == state.identity().thread_id()
    {
        Ok(())
    } else {
        Err(RISCV_LINUX_ESRCH)
    }
}

fn copy_process_vm_iovecs(
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
    source_iovecs: &[RiscvIovec],
    destination_iovecs: &[RiscvIovec],
    transfer: u64,
) -> Result<u64, u64> {
    let mut copied = 0;
    let mut source = IovecCursor::new(source_iovecs);
    let mut destination = IovecCursor::new(destination_iovecs);

    while copied < transfer {
        let remaining = transfer - copied;
        let Some((source_address, source_remaining)) = source.current() else {
            return finish_or_fault(copied);
        };
        let Some((destination_address, destination_remaining)) = destination.current() else {
            return finish_or_fault(copied);
        };
        let chunk = source_remaining.min(destination_remaining).min(remaining);
        let Ok(chunk_len) = usize::try_from(chunk) else {
            return finish_or_fault(copied);
        };
        if !guest_memory_writer.can_write(destination_address, chunk_len) {
            return finish_or_fault(copied);
        }
        let Some(bytes) = guest_memory_reader
            .read(source_address, chunk_len)
            .filter(|bytes| bytes.len() == chunk_len)
        else {
            return finish_or_fault(copied);
        };
        if !guest_memory_writer.write(destination_address, &bytes) {
            return finish_or_fault(copied);
        }

        copied += chunk;
        source.advance(chunk);
        destination.advance(chunk);
    }

    Ok(copied)
}

struct IovecCursor<'a> {
    iovecs: &'a [RiscvIovec],
    index: usize,
    offset: u64,
}

impl<'a> IovecCursor<'a> {
    const fn new(iovecs: &'a [RiscvIovec]) -> Self {
        Self {
            iovecs,
            index: 0,
            offset: 0,
        }
    }

    fn current(&mut self) -> Option<(u64, u64)> {
        self.skip_empty();
        let iovec = self.iovecs.get(self.index)?;
        let address = iovec.address.checked_add(self.offset)?;
        Some((address, iovec.len - self.offset))
    }

    fn advance(&mut self, bytes: u64) {
        self.offset += bytes;
    }

    fn skip_empty(&mut self) {
        while self.index < self.iovecs.len() && self.offset >= self.iovecs[self.index].len {
            self.index += 1;
            self.offset = 0;
        }
    }
}

fn finish_or_fault(copied: u64) -> Result<u64, u64> {
    if copied == 0 {
        Err(RISCV_LINUX_EFAULT)
    } else {
        Ok(copied)
    }
}
