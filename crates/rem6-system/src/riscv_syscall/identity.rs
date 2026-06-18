const RISCV_LINUX_SINGLE_PROCESS_ID: u64 = 100;

use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RISCV_LINUX_EFAULT,
    RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
};

pub(super) const RISCV_LINUX_SETGID: u64 = 144;
pub(super) const RISCV_LINUX_SETREGID: u64 = 143;
pub(super) const RISCV_LINUX_SETUID: u64 = 146;
pub(super) const RISCV_LINUX_SETREUID: u64 = 145;
pub(super) const RISCV_LINUX_SETRESUID: u64 = 147;
pub(super) const RISCV_LINUX_GETRESUID: u64 = 148;
pub(super) const RISCV_LINUX_SETRESGID: u64 = 149;
pub(super) const RISCV_LINUX_GETRESGID: u64 = 150;
pub(super) const RISCV_LINUX_GETGROUPS: u64 = 158;
pub(super) const RISCV_LINUX_SETGROUPS: u64 = 159;
pub(super) const RISCV_LINUX_GETPID: u64 = 172;
pub(super) const RISCV_LINUX_GETPPID: u64 = 173;
pub(super) const RISCV_LINUX_GETUID: u64 = 174;
pub(super) const RISCV_LINUX_GETEUID: u64 = 175;
pub(super) const RISCV_LINUX_GETGID: u64 = 176;
pub(super) const RISCV_LINUX_GETEGID: u64 = 177;
pub(super) const RISCV_LINUX_GETTID: u64 = 178;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvSyscallIdentity {
    thread_group_id: u64,
    thread_id: u64,
    parent_process_id: u64,
    user_id: u64,
    effective_user_id: u64,
    saved_user_id: u64,
    group_id: u64,
    effective_group_id: u64,
    saved_group_id: u64,
}

impl RiscvSyscallIdentity {
    pub(crate) const fn new(
        thread_group_id: u64,
        thread_id: u64,
        parent_process_id: u64,
        user_id: u64,
        effective_user_id: u64,
        group_id: u64,
        effective_group_id: u64,
    ) -> Self {
        Self {
            thread_group_id,
            thread_id,
            parent_process_id,
            user_id,
            effective_user_id,
            saved_user_id: effective_user_id,
            group_id,
            effective_group_id,
            saved_group_id: effective_group_id,
        }
    }

    pub(crate) const fn linux_single_process() -> Self {
        Self::new(
            RISCV_LINUX_SINGLE_PROCESS_ID,
            RISCV_LINUX_SINGLE_PROCESS_ID,
            0,
            RISCV_LINUX_SINGLE_PROCESS_ID,
            RISCV_LINUX_SINGLE_PROCESS_ID,
            RISCV_LINUX_SINGLE_PROCESS_ID,
            RISCV_LINUX_SINGLE_PROCESS_ID,
        )
    }

    pub(crate) const fn thread_group_id(self) -> u64 {
        self.thread_group_id
    }

    pub(crate) const fn thread_id(self) -> u64 {
        self.thread_id
    }

    pub(crate) const fn parent_process_id(self) -> u64 {
        self.parent_process_id
    }

    pub(crate) const fn user_id(self) -> u64 {
        self.user_id
    }

    pub(crate) const fn effective_user_id(self) -> u64 {
        self.effective_user_id
    }

    pub(crate) const fn saved_user_id(self) -> u64 {
        self.saved_user_id
    }

    pub(crate) const fn group_id(self) -> u64 {
        self.group_id
    }

    pub(crate) const fn effective_group_id(self) -> u64 {
        self.effective_group_id
    }

    pub(crate) const fn saved_group_id(self) -> u64 {
        self.saved_group_id
    }
}

pub(super) fn syscall_identity(number: u64, identity: RiscvSyscallIdentity) -> Option<u64> {
    match number {
        RISCV_LINUX_GETPID => Some(identity.thread_group_id()),
        RISCV_LINUX_GETPPID => Some(identity.parent_process_id()),
        RISCV_LINUX_GETTID => Some(identity.thread_id()),
        RISCV_LINUX_GETUID => Some(identity.user_id()),
        RISCV_LINUX_GETEUID => Some(identity.effective_user_id()),
        RISCV_LINUX_GETGID => Some(identity.group_id()),
        RISCV_LINUX_GETEGID => Some(identity.effective_group_id()),
        _ => None,
    }
}

pub(super) fn syscall_res_identity(
    request: RiscvSyscallRequest,
    identity: RiscvSyscallIdentity,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let (real_id, effective_id, saved_id) = match request.number() {
        RISCV_LINUX_GETRESUID => (
            identity.user_id(),
            identity.effective_user_id(),
            identity.saved_user_id(),
        ),
        RISCV_LINUX_GETRESGID => (
            identity.group_id(),
            identity.effective_group_id(),
            identity.saved_group_id(),
        ),
        _ => unreachable!("RISC-V Linux resolved identity syscall is handled by caller"),
    };
    for (address, value) in [
        (request.argument(0), real_id),
        (request.argument(1), effective_id),
        (request.argument(2), saved_id),
    ] {
        if !write_uid_gid(address, value, guest_memory) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    0
}

pub(super) fn syscall_setres_identity(
    request: RiscvSyscallRequest,
    identity: &mut RiscvSyscallIdentity,
) -> u64 {
    match request.number() {
        RISCV_LINUX_SETRESUID => setres_user_identity(request, identity),
        RISCV_LINUX_SETRESGID => setres_group_identity(request, identity),
        _ => unreachable!("RISC-V Linux resolved identity update syscall is handled by caller"),
    }
}

pub(super) fn syscall_setre_identity(
    request: RiscvSyscallRequest,
    identity: &mut RiscvSyscallIdentity,
) -> u64 {
    match request.number() {
        RISCV_LINUX_SETREUID => setre_user_identity(request, identity),
        RISCV_LINUX_SETREGID => setre_group_identity(request, identity),
        _ => unreachable!("RISC-V Linux real/effective identity syscall is handled by caller"),
    }
}

pub(super) fn syscall_set_identity(
    request: RiscvSyscallRequest,
    identity: &mut RiscvSyscallIdentity,
) -> u64 {
    match request.number() {
        RISCV_LINUX_SETUID => set_user_identity(request.argument(0), identity),
        RISCV_LINUX_SETGID => set_group_identity(request.argument(0), identity),
        _ => unreachable!("RISC-V Linux identity update syscall is handled by caller"),
    }
}

pub(super) fn syscall_getgroups(
    request: RiscvSyscallRequest,
    _identity: RiscvSyscallIdentity,
    _guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    if getgroups_count_is_negative(request.argument(0)) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    0
}

pub(super) fn syscall_setgroups() -> u64 {
    linux_error(RISCV_LINUX_EPERM)
}

fn set_user_identity(requested: u64, identity: &mut RiscvSyscallIdentity) -> u64 {
    if identity.effective_user_id == 0 {
        identity.user_id = requested;
        identity.effective_user_id = requested;
        identity.saved_user_id = requested;
        return 0;
    }
    if requested == identity.user_id || requested == identity.saved_user_id {
        identity.effective_user_id = requested;
        0
    } else {
        linux_error(RISCV_LINUX_EPERM)
    }
}

fn set_group_identity(requested: u64, identity: &mut RiscvSyscallIdentity) -> u64 {
    if identity.effective_user_id == 0 {
        identity.group_id = requested;
        identity.effective_group_id = requested;
        identity.saved_group_id = requested;
        return 0;
    }
    if requested == identity.group_id || requested == identity.saved_group_id {
        identity.effective_group_id = requested;
        0
    } else {
        linux_error(RISCV_LINUX_EPERM)
    }
}

fn setres_user_identity(request: RiscvSyscallRequest, identity: &mut RiscvSyscallIdentity) -> u64 {
    let current = [
        identity.user_id,
        identity.effective_user_id,
        identity.saved_user_id,
    ];
    let privileged = identity.effective_user_id == 0;
    let Ok(real_id) = next_identity_id(request.argument(0), identity.user_id, &current, privileged)
    else {
        return linux_error(RISCV_LINUX_EPERM);
    };
    let Ok(effective_id) = next_identity_id(
        request.argument(1),
        identity.effective_user_id,
        &current,
        privileged,
    ) else {
        return linux_error(RISCV_LINUX_EPERM);
    };
    let Ok(saved_id) = next_identity_id(
        request.argument(2),
        identity.saved_user_id,
        &current,
        privileged,
    ) else {
        return linux_error(RISCV_LINUX_EPERM);
    };
    identity.user_id = real_id;
    identity.effective_user_id = effective_id;
    identity.saved_user_id = saved_id;
    0
}

fn setre_user_identity(request: RiscvSyscallRequest, identity: &mut RiscvSyscallIdentity) -> u64 {
    let Ok((user_id, effective_user_id, saved_user_id)) = next_setre_identity(
        identity.user_id,
        identity.effective_user_id,
        identity.saved_user_id,
        identity.effective_user_id == 0,
        request.argument(0),
        request.argument(1),
    ) else {
        return linux_error(RISCV_LINUX_EPERM);
    };
    identity.user_id = user_id;
    identity.effective_user_id = effective_user_id;
    identity.saved_user_id = saved_user_id;
    0
}

fn setres_group_identity(request: RiscvSyscallRequest, identity: &mut RiscvSyscallIdentity) -> u64 {
    let current = [
        identity.group_id,
        identity.effective_group_id,
        identity.saved_group_id,
    ];
    let privileged = identity.effective_user_id == 0;
    let Ok(real_id) =
        next_identity_id(request.argument(0), identity.group_id, &current, privileged)
    else {
        return linux_error(RISCV_LINUX_EPERM);
    };
    let Ok(effective_id) = next_identity_id(
        request.argument(1),
        identity.effective_group_id,
        &current,
        privileged,
    ) else {
        return linux_error(RISCV_LINUX_EPERM);
    };
    let Ok(saved_id) = next_identity_id(
        request.argument(2),
        identity.saved_group_id,
        &current,
        privileged,
    ) else {
        return linux_error(RISCV_LINUX_EPERM);
    };
    identity.group_id = real_id;
    identity.effective_group_id = effective_id;
    identity.saved_group_id = saved_id;
    0
}

fn setre_group_identity(request: RiscvSyscallRequest, identity: &mut RiscvSyscallIdentity) -> u64 {
    let Ok((group_id, effective_group_id, saved_group_id)) = next_setre_identity(
        identity.group_id,
        identity.effective_group_id,
        identity.saved_group_id,
        identity.effective_user_id == 0,
        request.argument(0),
        request.argument(1),
    ) else {
        return linux_error(RISCV_LINUX_EPERM);
    };
    identity.group_id = group_id;
    identity.effective_group_id = effective_group_id;
    identity.saved_group_id = saved_group_id;
    0
}

fn next_setre_identity(
    real_id: u64,
    effective_id: u64,
    saved_id: u64,
    privileged: bool,
    requested_real_id: u64,
    requested_effective_id: u64,
) -> Result<(u64, u64, u64), ()> {
    let real_id_choices = [real_id, effective_id];
    let effective_id_choices = [real_id, effective_id, saved_id];
    let next_real_id = next_identity_id(requested_real_id, real_id, &real_id_choices, privileged)?;
    let next_effective_id = next_identity_id(
        requested_effective_id,
        effective_id,
        &effective_id_choices,
        privileged,
    )?;
    let next_saved_id = if setre_updates_saved_id(
        requested_real_id,
        requested_effective_id,
        next_effective_id,
        real_id,
    ) {
        next_effective_id
    } else {
        saved_id
    };
    Ok((next_real_id, next_effective_id, next_saved_id))
}

fn setre_updates_saved_id(
    requested_real_id: u64,
    requested_effective_id: u64,
    effective_id: u64,
    previous_real_id: u64,
) -> bool {
    !identity_no_change(requested_real_id)
        || (!identity_no_change(requested_effective_id) && effective_id != previous_real_id)
}

fn next_identity_id(
    requested: u64,
    current: u64,
    allowed: &[u64],
    privileged: bool,
) -> Result<u64, ()> {
    if identity_no_change(requested) {
        return Ok(current);
    }
    if privileged || allowed.contains(&requested) {
        Ok(requested)
    } else {
        Err(())
    }
}

const fn identity_no_change(requested: u64) -> bool {
    requested == u64::MAX || requested == u32::MAX as u64
}

const fn getgroups_count_is_negative(count: u64) -> bool {
    (count as u32) & (1 << 31) != 0
}

fn write_uid_gid(address: u64, value: u64, guest_memory: &RiscvGuestMemoryWriter) -> bool {
    let Ok(value) = u32::try_from(value) else {
        return false;
    };
    guest_memory.write(address, &value.to_le_bytes())
}
