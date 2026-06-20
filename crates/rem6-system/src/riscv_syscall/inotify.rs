use std::collections::{BTreeMap, VecDeque};

use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EAGAIN,
    RISCV_LINUX_EBADF, RISCV_LINUX_EEXIST, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT, RISCV_LINUX_ENOTDIR,
    RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDONLY, RISCV_LINUX_PATH_MAX,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_INOTIFY_INIT1: u64 = 26;
pub(super) const RISCV_LINUX_INOTIFY_ADD_WATCH: u64 = 27;
pub(super) const RISCV_LINUX_INOTIFY_RM_WATCH: u64 = 28;

const RISCV_LINUX_IN_ACCESS: u32 = 0x0000_0001;
const RISCV_LINUX_IN_MODIFY: u32 = 0x0000_0002;
const RISCV_LINUX_IN_ATTRIB: u32 = 0x0000_0004;
const RISCV_LINUX_IN_CLOSE_WRITE: u32 = 0x0000_0008;
const RISCV_LINUX_IN_CLOSE_NOWRITE: u32 = 0x0000_0010;
const RISCV_LINUX_IN_OPEN: u32 = 0x0000_0020;
const RISCV_LINUX_IN_MOVED_FROM: u32 = 0x0000_0040;
const RISCV_LINUX_IN_MOVED_TO: u32 = 0x0000_0080;
const RISCV_LINUX_IN_CREATE: u32 = 0x0000_0100;
const RISCV_LINUX_IN_DELETE: u32 = 0x0000_0200;
const RISCV_LINUX_IN_DELETE_SELF: u32 = 0x0000_0400;
const RISCV_LINUX_IN_MOVE_SELF: u32 = 0x0000_0800;
const RISCV_LINUX_IN_UNMOUNT: u32 = 0x0000_2000;
const RISCV_LINUX_IN_Q_OVERFLOW: u32 = 0x0000_4000;
const RISCV_LINUX_IN_IGNORED: u32 = 0x0000_8000;
const RISCV_LINUX_IN_ONLYDIR: u32 = 0x0100_0000;
const RISCV_LINUX_IN_DONT_FOLLOW: u32 = 0x0200_0000;
const RISCV_LINUX_IN_EXCL_UNLINK: u32 = 0x0400_0000;
const RISCV_LINUX_IN_MASK_CREATE: u32 = 0x1000_0000;
const RISCV_LINUX_IN_MASK_ADD: u32 = 0x2000_0000;
const RISCV_LINUX_IN_ISDIR: u32 = 0x4000_0000;
const RISCV_LINUX_IN_ONESHOT: u32 = 0x8000_0000;
const RISCV_LINUX_IN_ALL_EVENTS: u32 = RISCV_LINUX_IN_ACCESS
    | RISCV_LINUX_IN_MODIFY
    | RISCV_LINUX_IN_ATTRIB
    | RISCV_LINUX_IN_CLOSE_WRITE
    | RISCV_LINUX_IN_CLOSE_NOWRITE
    | RISCV_LINUX_IN_OPEN
    | RISCV_LINUX_IN_MOVED_FROM
    | RISCV_LINUX_IN_MOVED_TO
    | RISCV_LINUX_IN_CREATE
    | RISCV_LINUX_IN_DELETE
    | RISCV_LINUX_IN_DELETE_SELF
    | RISCV_LINUX_IN_MOVE_SELF;
const RISCV_LINUX_IN_WATCH_FLAGS: u32 = RISCV_LINUX_IN_ONLYDIR
    | RISCV_LINUX_IN_DONT_FOLLOW
    | RISCV_LINUX_IN_EXCL_UNLINK
    | RISCV_LINUX_IN_MASK_CREATE
    | RISCV_LINUX_IN_MASK_ADD
    | RISCV_LINUX_IN_ONESHOT;
const RISCV_LINUX_IN_ALWAYS_QUEUED: u32 =
    RISCV_LINUX_IN_UNMOUNT | RISCV_LINUX_IN_Q_OVERFLOW | RISCV_LINUX_IN_IGNORED;
const RISCV_LINUX_IN_VALID_MASK: u32 = RISCV_LINUX_IN_ALL_EVENTS
    | RISCV_LINUX_IN_ALWAYS_QUEUED
    | RISCV_LINUX_IN_WATCH_FLAGS
    | RISCV_LINUX_IN_ISDIR;
const RISCV_LINUX_INOTIFY_VALID_FLAGS: u64 = RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;
const RISCV_LINUX_INOTIFY_EVENT_BYTES: usize = 16;
const RISCV_LINUX_INOTIFY_NAME_ALIGN: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestInotify {
    next_watch: i32,
    watches: BTreeMap<i32, RiscvGuestInotifyWatch>,
    events: VecDeque<RiscvGuestInotifyEvent>,
}

impl Default for RiscvGuestInotify {
    fn default() -> Self {
        Self {
            next_watch: 1,
            watches: BTreeMap::new(),
            events: VecDeque::new(),
        }
    }
}

impl RiscvGuestInotify {
    fn add_watch(&mut self, path: Vec<u8>, mask: u32) -> Result<i32, RiscvGuestInotifyError> {
        if mask & RISCV_LINUX_IN_MASK_CREATE != 0 && mask & RISCV_LINUX_IN_MASK_ADD != 0 {
            return Err(RiscvGuestInotifyError::InvalidMask);
        }
        if let Some((&wd, watch)) = self
            .watches
            .iter_mut()
            .find(|(_wd, watch)| watch.path == path)
        {
            if mask & RISCV_LINUX_IN_MASK_CREATE != 0 {
                return Err(RiscvGuestInotifyError::AlreadyWatched);
            }
            watch.mask = if mask & RISCV_LINUX_IN_MASK_ADD != 0 {
                watch.mask | mask
            } else {
                mask
            };
            return Ok(wd);
        }
        let wd = self.next_watch;
        self.next_watch = self
            .next_watch
            .checked_add(1)
            .ok_or(RiscvGuestInotifyError::WatchSpaceExhausted)?;
        self.watches
            .insert(wd, RiscvGuestInotifyWatch { path, mask });
        Ok(wd)
    }

    fn remove_watch(&mut self, wd: i32) -> Result<(), RiscvGuestInotifyError> {
        self.watches
            .remove(&wd)
            .ok_or(RiscvGuestInotifyError::MissingWatch)?;
        self.events.push_back(RiscvGuestInotifyEvent::without_name(
            wd,
            RISCV_LINUX_IN_IGNORED,
        ));
        Ok(())
    }

    fn enqueue_create(&mut self, parent: &[u8], name: &[u8]) {
        let mut remove_watches = Vec::new();
        for (wd, watch) in &self.watches {
            if watch.path == parent && watch.mask & RISCV_LINUX_IN_CREATE != 0 {
                self.events.push_back(RiscvGuestInotifyEvent::with_name(
                    *wd,
                    RISCV_LINUX_IN_CREATE,
                    name.to_vec(),
                ));
                if watch.mask & RISCV_LINUX_IN_ONESHOT != 0 {
                    self.events.push_back(RiscvGuestInotifyEvent::without_name(
                        *wd,
                        RISCV_LINUX_IN_IGNORED,
                    ));
                    remove_watches.push(*wd);
                }
            }
        }
        for wd in remove_watches {
            self.watches.remove(&wd);
        }
    }

    fn readable(&self) -> bool {
        !self.events.is_empty()
    }

    fn queued_bytes_for_count(&self, count: usize) -> Result<usize, RiscvGuestInotifyReadError> {
        let Some(first) = self.events.front() else {
            return Ok(0);
        };
        let first_bytes = first.record_len();
        if count < first_bytes {
            return Err(RiscvGuestInotifyReadError::InvalidSize);
        }

        let mut bytes = 0usize;
        for event in &self.events {
            let record_len = event.record_len();
            if bytes + record_len > count {
                break;
            }
            bytes += record_len;
        }
        Ok(bytes)
    }

    fn read_bytes_for_count(&self, count: usize) -> Result<Vec<u8>, RiscvGuestInotifyReadError> {
        let bytes = self.queued_bytes_for_count(count)?;
        let mut out = Vec::with_capacity(bytes);
        for event in &self.events {
            if out.len() + event.record_len() > bytes {
                break;
            }
            event.append_bytes(&mut out);
        }
        Ok(out)
    }

    fn consume_bytes(&mut self, bytes: usize) {
        let mut remaining = bytes;
        while remaining > 0 {
            let record_len = self
                .events
                .front()
                .expect("inotify read only consumes queued records")
                .record_len();
            remaining -= record_len;
            self.events.pop_front();
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvGuestInotifyWatch {
    path: Vec<u8>,
    mask: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvGuestInotifyEvent {
    wd: i32,
    mask: u32,
    name: Vec<u8>,
}

impl RiscvGuestInotifyEvent {
    fn with_name(wd: i32, mask: u32, name: Vec<u8>) -> Self {
        Self { wd, mask, name }
    }

    fn without_name(wd: i32, mask: u32) -> Self {
        Self {
            wd,
            mask,
            name: Vec::new(),
        }
    }

    fn name_len(&self) -> usize {
        if self.name.is_empty() {
            0
        } else {
            align_inotify_name_len(self.name.len().saturating_add(1))
        }
    }

    fn record_len(&self) -> usize {
        RISCV_LINUX_INOTIFY_EVENT_BYTES + self.name_len()
    }

    fn append_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.wd.to_le_bytes());
        out.extend_from_slice(&self.mask.to_le_bytes());
        out.extend_from_slice(&0_u32.to_le_bytes());
        out.extend_from_slice(&(self.name_len() as u32).to_le_bytes());
        if self.name_len() != 0 {
            out.extend_from_slice(&self.name);
            out.resize(out.len() + 1, 0);
            out.resize(out.len() + self.name_len() - self.name.len() - 1, 0);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestInotifyError {
    AlreadyWatched,
    InvalidMask,
    MissingWatch,
    WatchSpaceExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestInotifyRead {
    Bytes(Vec<u8>),
    Blocked,
    WouldBlock,
    InvalidSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestInotifyReady {
    readable: bool,
}

impl RiscvGuestInotifyReady {
    pub(super) const fn readable(self) -> bool {
        self.readable
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestInotifyReadError {
    InvalidSize,
}

impl RiscvSyscallState {
    pub(super) fn notify_guest_file_created(&mut self, path: &[u8]) {
        let Some((parent, name)) = inotify_parent_and_name(path) else {
            return;
        };
        for inotify in self.guest_inotifies.values_mut() {
            inotify.enqueue_create(&parent, &name);
        }
    }

    pub(super) fn guest_inotify_read(
        &self,
        fd: GuestFd,
        count: u64,
    ) -> Result<Option<RiscvGuestInotifyRead>, GuestFdError> {
        let Some(inotify) = self.inotify_for_fd(fd)? else {
            return Ok(None);
        };
        let count = usize::try_from(count).unwrap_or(usize::MAX);
        Ok(Some(match inotify.read_bytes_for_count(count) {
            Ok(bytes) if !bytes.is_empty() => RiscvGuestInotifyRead::Bytes(bytes),
            Ok(_) if self.inotify_nonblocking(fd)? => RiscvGuestInotifyRead::WouldBlock,
            Ok(_) => RiscvGuestInotifyRead::Blocked,
            Err(RiscvGuestInotifyReadError::InvalidSize) => RiscvGuestInotifyRead::InvalidSize,
        }))
    }

    pub(super) fn consume_guest_inotify_read(
        &mut self,
        fd: GuestFd,
        bytes: usize,
    ) -> Result<(), GuestFdError> {
        let Some(description) = self.inotify_description_for_fd(fd)? else {
            return Err(GuestFdError::BadFd { fd });
        };
        let inotify = self
            .guest_inotifies
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        inotify.consume_bytes(bytes);
        Ok(())
    }

    pub(super) fn guest_inotify_ready(
        &self,
        fd: GuestFd,
    ) -> Result<Option<RiscvGuestInotifyReady>, GuestFdError> {
        let Some(inotify) = self.inotify_for_fd(fd)? else {
            return Ok(None);
        };
        Ok(Some(RiscvGuestInotifyReady {
            readable: inotify.readable(),
        }))
    }

    pub(super) fn remove_guest_inotify_description(&mut self, description: GuestFileDescriptionId) {
        self.guest_inotifies.remove(&description);
    }

    fn open_guest_inotify(&mut self, flags: u64) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_guest_fd_excluding(&[])?;
        let description = self.next_open_description()?;
        let close_on_exec = flags & RISCV_LINUX_O_CLOEXEC != 0;
        let status_flags = RISCV_LINUX_O_RDONLY | (flags & RISCV_LINUX_O_NONBLOCK);
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(status_flags as u32),
            ))?;
        self.guest_fds.insert(
            fd,
            GuestFdEntry::new(description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_inotifies
            .insert(description, RiscvGuestInotify::default());
        Ok(fd)
    }

    fn add_guest_inotify_watch(
        &mut self,
        fd: GuestFd,
        path: Vec<u8>,
        mask: u32,
    ) -> Result<Option<Result<i32, RiscvGuestInotifyError>>, GuestFdError> {
        let Some(description) = self.inotify_description_for_fd(fd)? else {
            return Ok(None);
        };
        let inotify = self
            .guest_inotifies
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        Ok(Some(inotify.add_watch(path, mask)))
    }

    fn remove_guest_inotify_watch(
        &mut self,
        fd: GuestFd,
        wd: i32,
    ) -> Result<Option<()>, GuestFdError> {
        let Some(description) = self.inotify_description_for_fd(fd)? else {
            return Ok(None);
        };
        let inotify = self
            .guest_inotifies
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        match inotify.remove_watch(wd) {
            Ok(()) => Ok(Some(())),
            Err(RiscvGuestInotifyError::MissingWatch) => Ok(None),
            Err(RiscvGuestInotifyError::WatchSpaceExhausted) => Err(GuestFdError::FdSpaceExhausted),
            Err(RiscvGuestInotifyError::AlreadyWatched | RiscvGuestInotifyError::InvalidMask) => {
                unreachable!("remove_watch does not validate add-watch masks")
            }
        }
    }

    fn inotify_description_for_fd(
        &self,
        fd: GuestFd,
    ) -> Result<Option<GuestFileDescriptionId>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self
            .guest_inotifies
            .contains_key(&description)
            .then_some(description))
    }

    fn inotify_for_fd(&self, fd: GuestFd) -> Result<Option<&RiscvGuestInotify>, GuestFdError> {
        let Some(description) = self.inotify_description_for_fd(fd)? else {
            return Ok(None);
        };
        self.guest_inotifies
            .get(&description)
            .map(Some)
            .ok_or(GuestFdError::MissingFileDescription { description })
    }

    fn inotify_nonblocking(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        Ok(self.guest_fds.status_flags(fd)?.bits() & RISCV_LINUX_O_NONBLOCK as u32 != 0)
    }
}

pub(super) fn syscall_inotify_init1(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let flags = request.argument(0);
    if flags & !RISCV_LINUX_INOTIFY_VALID_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.open_guest_inotify(flags) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_) => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn syscall_inotify_add_watch(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let mask = request.argument(2) as u32;
    if mask == 0 || mask & !RISCV_LINUX_IN_VALID_MASK != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if mask & RISCV_LINUX_IN_MASK_CREATE != 0 && mask & RISCV_LINUX_IN_MASK_ADD != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    match state.inotify_description_for_fd(fd) {
        Ok(Some(_description)) => {}
        Ok(None) => return Some(linux_error(RISCV_LINUX_EINVAL)),
        Err(GuestFdError::BadFd { .. }) => return Some(linux_error(RISCV_LINUX_EBADF)),
        Err(_) => return Some(linux_error(RISCV_LINUX_EINVAL)),
    }
    let path = match read_guest_c_string(guest_memory?, request.argument(1), RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return Some(linux_error(RISCV_LINUX_EFAULT)),
        Err(RiscvGuestCStringError::TooLong) => {
            return Some(linux_error(RISCV_LINUX_ENAMETOOLONG));
        }
    };
    let path = match resolve_inotify_watch_path(state, &path, mask) {
        Ok(path) => path,
        Err(errno) => return Some(linux_error(errno)),
    };

    Some(match state.add_guest_inotify_watch(fd, path, mask) {
        Ok(Some(Ok(wd))) => wd as u32 as u64,
        Ok(Some(Err(RiscvGuestInotifyError::AlreadyWatched))) => linux_error(RISCV_LINUX_EEXIST),
        Ok(Some(Err(RiscvGuestInotifyError::InvalidMask))) => linux_error(RISCV_LINUX_EINVAL),
        Ok(Some(Err(RiscvGuestInotifyError::WatchSpaceExhausted))) => {
            linux_error(RISCV_LINUX_EMFILE)
        }
        Ok(Some(Err(RiscvGuestInotifyError::MissingWatch))) => linux_error(RISCV_LINUX_EINVAL),
        Ok(None) => linux_error(RISCV_LINUX_EINVAL),
        Err(GuestFdError::BadFd { .. }) => linux_error(RISCV_LINUX_EBADF),
        Err(_) => linux_error(RISCV_LINUX_EINVAL),
    })
}

pub(super) fn syscall_inotify_rm_watch(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let wd = request.argument(1) as u32 as i32;
    match state.remove_guest_inotify_watch(fd, wd) {
        Ok(Some(())) => 0,
        Ok(None) => linux_error(RISCV_LINUX_EINVAL),
        Err(GuestFdError::BadFd { .. }) => linux_error(RISCV_LINUX_EBADF),
        Err(_) => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn inotify_read_result(read: RiscvGuestInotifyRead) -> Option<u64> {
    match read {
        RiscvGuestInotifyRead::Bytes(bytes) => Some(bytes.len() as u64),
        RiscvGuestInotifyRead::Blocked => None,
        RiscvGuestInotifyRead::WouldBlock => Some(linux_error(RISCV_LINUX_EAGAIN)),
        RiscvGuestInotifyRead::InvalidSize => Some(linux_error(RISCV_LINUX_EINVAL)),
    }
}

fn resolve_inotify_watch_path(
    state: &RiscvSyscallState,
    path: &[u8],
    mask: u32,
) -> Result<Vec<u8>, u64> {
    if path.is_empty() {
        return Err(RISCV_LINUX_ENOENT);
    }
    let path = if mask & RISCV_LINUX_IN_DONT_FOLLOW != 0 {
        state
            .resolve_guest_path_following_intermediate_symlinks(path)
            .map_err(|error| error.linux_error_code())?
    } else {
        state
            .resolve_guest_path_following_symlinks(path)
            .map_err(|error| error.linux_error_code())?
    };
    let path = state.existing_guest_path_key(&path).unwrap_or(path);
    let is_directory = state.guest_directory_entries(&path).is_some();
    let exists =
        is_directory || state.guest_path_registered(&path) || state.guest_links.contains_key(&path);
    if !exists {
        return Err(RISCV_LINUX_ENOENT);
    }
    if mask & RISCV_LINUX_IN_ONLYDIR != 0 && !is_directory {
        return Err(RISCV_LINUX_ENOTDIR);
    }
    Ok(path)
}

fn inotify_parent_and_name(path: &[u8]) -> Option<(Vec<u8>, Vec<u8>)> {
    let name_start = path
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or(0, |index| index + 1);
    if name_start >= path.len() {
        return None;
    }
    let parent = if name_start == 0 {
        Vec::new()
    } else {
        path[..name_start - 1].to_vec()
    };
    Some((parent, path[name_start..].to_vec()))
}

fn align_inotify_name_len(len: usize) -> usize {
    len.saturating_add(RISCV_LINUX_INOTIFY_NAME_ALIGN - 1) / RISCV_LINUX_INOTIFY_NAME_ALIGN
        * RISCV_LINUX_INOTIFY_NAME_ALIGN
}
