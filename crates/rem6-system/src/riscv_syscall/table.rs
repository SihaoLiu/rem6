use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvSyscallTable;

impl RiscvSyscallTable {
    pub const fn new() -> Self {
        Self
    }

    pub fn handle(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
    ) -> Option<RiscvSyscallOutcome> {
        self.handle_at_tick(request, state, 0)
    }

    pub fn handle_at_tick(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
        tick: Tick,
    ) -> Option<RiscvSyscallOutcome> {
        self.handle_with_guest_memory_at_tick(request, state, tick, None)
    }

    pub fn handle_with_guest_memory_at_tick(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
        tick: Tick,
        guest_memory: Option<&RiscvGuestMemoryReader>,
    ) -> Option<RiscvSyscallOutcome> {
        self.handle_with_guest_memory_io_at_tick(request, state, tick, guest_memory, None)
    }

    pub fn handle_with_guest_memory_io_at_tick(
        self,
        request: RiscvSyscallRequest,
        state: &mut RiscvSyscallState,
        tick: Tick,
        guest_memory_reader: Option<&RiscvGuestMemoryReader>,
        guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
    ) -> Option<RiscvSyscallOutcome> {
        state.refresh_guest_timerfds(tick);
        match request.number() {
            5..=16 => {
                xattr::syscall_xattr(request, state, guest_memory_reader, guest_memory_writer)
            }
            RISCV_LINUX_LOOKUP_DCOOKIE | RISCV_LINUX_NFSSERVCTL => Some(known_ni_syscall_outcome()),
            RISCV_LINUX_UMOUNT2 => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_umount2(request, guest_memory),
                })
            }
            RISCV_LINUX_MOUNT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_mount(request, guest_memory),
                })
            }
            RISCV_LINUX_PIVOT_ROOT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_pivot_root(request, guest_memory),
                })
            }
            RISCV_LINUX_GETCWD => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_getcwd(
                        request.argument(0),
                        request.argument(1),
                        state,
                        guest_memory,
                    ),
                })
            }
            RISCV_LINUX_CHDIR => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_chdir(request, state, guest_memory),
                })
            }
            RISCV_LINUX_FCHDIR => Some(RiscvSyscallOutcome::Return {
                value: syscall_fchdir(request, state),
            }),
            RISCV_LINUX_CAPGET => Some(RiscvSyscallOutcome::Return {
                value: syscall_capget(request, state, guest_memory_reader, guest_memory_writer),
            }),
            RISCV_LINUX_CAPSET => Some(RiscvSyscallOutcome::Return {
                value: syscall_capset(request, state, guest_memory_reader, guest_memory_writer),
            }),
            RISCV_LINUX_DUP => Some(RiscvSyscallOutcome::Return {
                value: syscall_dup(request.argument(0), state),
            }),
            RISCV_LINUX_DUP3 => Some(RiscvSyscallOutcome::Return {
                value: syscall_dup3(
                    request.argument(0),
                    request.argument(1),
                    request.argument(2),
                    state,
                ),
            }),
            RISCV_LINUX_INOTIFY_INIT1 => Some(RiscvSyscallOutcome::Return {
                value: syscall_inotify_init1(request, state),
            }),
            RISCV_LINUX_INOTIFY_ADD_WATCH => {
                syscall_inotify_add_watch(request, state, guest_memory_reader)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_INOTIFY_RM_WATCH => Some(RiscvSyscallOutcome::Return {
                value: syscall_inotify_rm_watch(request, state),
            }),
            RISCV_LINUX_FCNTL => {
                syscall_fcntl(request, state, guest_memory_reader, guest_memory_writer)
            }
            RISCV_LINUX_EVENTFD2 => Some(RiscvSyscallOutcome::Return {
                value: syscall_eventfd2(request, state),
            }),
            RISCV_LINUX_TIMERFD_CREATE => Some(RiscvSyscallOutcome::Return {
                value: syscall_timerfd_create(request, state),
            }),
            RISCV_LINUX_TIMERFD_SETTIME => syscall_timerfd_settime(
                request,
                state,
                tick,
                guest_memory_reader,
                guest_memory_writer,
            )
            .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_TIMERFD_GETTIME => {
                syscall_timerfd_gettime(request, state, tick, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_SIGNALFD4 => syscall_signalfd4(request, state, guest_memory_reader)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_MEMFD_CREATE => {
                guest_memory_reader.map(|reader| RiscvSyscallOutcome::Return {
                    value: syscall_memfd_create(request, state, reader),
                })
            }
            RISCV_LINUX_EPOLL_CREATE1 => Some(RiscvSyscallOutcome::Return {
                value: syscall_epoll_create1(request, state),
            }),
            RISCV_LINUX_EPOLL_CTL => syscall_epoll_ctl(request, state, guest_memory_reader)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_EPOLL_PWAIT => {
                syscall_epoll_pwait(request, state, guest_memory_reader, guest_memory_writer)
            }
            RISCV_LINUX_EPOLL_PWAIT2 => {
                syscall_epoll_pwait2(request, state, guest_memory_reader, guest_memory_writer)
            }
            RISCV_LINUX_FLOCK => Some(RiscvSyscallOutcome::Return {
                value: syscall_flock(request, state),
            }),
            RISCV_LINUX_FADVISE64 => Some(RiscvSyscallOutcome::Return {
                value: syscall_fadvise64(request, state),
            }),
            RISCV_LINUX_FTRUNCATE => Some(RiscvSyscallOutcome::Return {
                value: syscall_ftruncate(request, state),
            }),
            RISCV_LINUX_FALLOCATE => Some(RiscvSyscallOutcome::Return {
                value: syscall_fallocate(request, state),
            }),
            RISCV_LINUX_TRUNCATE => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_truncate(request, state, guest_memory),
                })
            }
            RISCV_LINUX_IOCTL => Some(RiscvSyscallOutcome::Return {
                value: syscall_ioctl(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_OPENAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_openat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_OPEN => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_open(request, state, guest_memory),
                })
            }
            RISCV_LINUX_OPENAT2 => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_openat2(request, state, guest_memory),
                })
            }
            RISCV_LINUX_GETDENTS64 => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_getdents64(request, state, guest_memory),
                })
            }
            RISCV_LINUX_MKDIRAT | RISCV_NEWLIB_LEGACY_MKDIR => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_mkdir(request, state, guest_memory),
                })
            }
            RISCV_LINUX_MKNODAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_mknodat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_LINK | RISCV_LINUX_LINKAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_link_operation(request, state, guest_memory),
                })
            }
            RISCV_LINUX_SYMLINKAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_symlinkat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_UNLINK | RISCV_LINUX_UNLINKAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_unlink_operation(request, state, guest_memory),
                })
            }
            RISCV_LINUX_RENAMEAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_renameat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_RENAMEAT2 => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_renameat2(request, state, guest_memory),
                })
            }
            RISCV_LINUX_CLOSE => Some(RiscvSyscallOutcome::Return {
                value: syscall_close(request.argument(0), state),
            }),
            RISCV_LINUX_CLOSE_RANGE => Some(RiscvSyscallOutcome::Return {
                value: syscall_close_range(
                    request.argument(0),
                    request.argument(1),
                    request.argument(2),
                    state,
                ),
            }),
            RISCV_LINUX_LSEEK => Some(RiscvSyscallOutcome::Return {
                value: syscall_lseek(request, state),
            }),
            RISCV_LINUX_READ => guest_memory_writer.map(|guest_memory| {
                match syscall_read(request, state, guest_memory) {
                    Some(value) => RiscvSyscallOutcome::Return { value },
                    None => RiscvSyscallOutcome::Blocked,
                }
            }),
            RISCV_LINUX_PREAD64 => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_pread64(request, state, guest_memory),
                })
            }
            RISCV_LINUX_PREADV => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_preadv(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_WRITE => guest_memory_reader.map(|guest_memory| {
                match syscall_write(request, state, tick, guest_memory) {
                    Some(value) => RiscvSyscallOutcome::Return { value },
                    None => RiscvSyscallOutcome::Blocked,
                }
            }),
            RISCV_LINUX_PWRITE64 => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_pwrite64(request, state, tick, guest_memory),
                })
            }
            RISCV_LINUX_PWRITEV => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_pwritev(request, state, tick, guest_memory),
                })
            }
            RISCV_LINUX_SENDFILE => {
                if request.argument(2) == 0 {
                    Some(RiscvSyscallOutcome::Return {
                        value: syscall_sendfile(request, state, None, None),
                    })
                } else {
                    match (guest_memory_reader, guest_memory_writer) {
                        (Some(reader), Some(writer)) => Some(RiscvSyscallOutcome::Return {
                            value: syscall_sendfile(request, state, Some(reader), Some(writer)),
                        }),
                        _ => None,
                    }
                }
            }
            RISCV_LINUX_SPLICE => Some(syscall_splice(
                request,
                state,
                guest_memory_reader,
                guest_memory_writer,
            )),
            RISCV_LINUX_VMSPLICE => guest_memory_reader
                .map(|guest_memory| syscall_vmsplice(request, state, guest_memory)),
            RISCV_LINUX_TEE => Some(syscall_tee(request, state)),
            RISCV_LINUX_COPY_FILE_RANGE => Some(RiscvSyscallOutcome::Return {
                value: syscall_copy_file_range(
                    request,
                    state,
                    guest_memory_reader,
                    guest_memory_writer,
                ),
            }),
            RISCV_LINUX_WRITEV => guest_memory_reader.map(|guest_memory| {
                match syscall_writev(request, state, tick, guest_memory) {
                    Some(value) => RiscvSyscallOutcome::Return { value },
                    None => RiscvSyscallOutcome::Blocked,
                }
            }),
            RISCV_LINUX_READV => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| {
                    match syscall_readv(request, state, reader, writer) {
                        Some(value) => RiscvSyscallOutcome::Return { value },
                        None => RiscvSyscallOutcome::Blocked,
                    }
                })
            }),
            RISCV_LINUX_PIPE2 => guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                value: syscall_pipe2(request, state, writer),
            }),
            RISCV_LINUX_SOCKET => Some(RiscvSyscallOutcome::Return {
                value: syscall_socket(request, state),
            }),
            RISCV_LINUX_SOCKETPAIR => {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_socketpair(request, state, writer),
                })
            }
            RISCV_LINUX_BIND => guest_memory_reader.map(|reader| RiscvSyscallOutcome::Return {
                value: syscall_socket_bind(request, state, reader),
            }),
            RISCV_LINUX_LISTEN => Some(RiscvSyscallOutcome::Return {
                value: syscall_socket_listen(request, state),
            }),
            RISCV_LINUX_ACCEPT => match syscall_socket_accept(request, state, 0) {
                Some(value) => Some(RiscvSyscallOutcome::Return { value }),
                None => Some(RiscvSyscallOutcome::Blocked),
            },
            RISCV_LINUX_CONNECT => {
                let reader = guest_memory_reader?;
                match syscall_socket_connect(request, state, reader) {
                    Some(value) => Some(RiscvSyscallOutcome::Return { value }),
                    None => Some(RiscvSyscallOutcome::Blocked),
                }
            }
            RISCV_LINUX_ACCEPT4 => {
                match syscall_socket_accept(request, state, request.argument(3)) {
                    Some(value) => Some(RiscvSyscallOutcome::Return { value }),
                    None => Some(RiscvSyscallOutcome::Blocked),
                }
            }
            RISCV_LINUX_GETSOCKNAME => {
                let reader = guest_memory_reader?;
                let writer = guest_memory_writer?;
                Some(RiscvSyscallOutcome::Return {
                    value: syscall_getsockname(request, state, reader, writer),
                })
            }
            RISCV_LINUX_GETPEERNAME => {
                let reader = guest_memory_reader?;
                let writer = guest_memory_writer?;
                Some(RiscvSyscallOutcome::Return {
                    value: syscall_getpeername(request, state, reader, writer),
                })
            }
            RISCV_LINUX_SENDTO => {
                guest_memory_reader.map(|reader| match syscall_sendto(request, state, reader) {
                    Some(value) => RiscvSyscallOutcome::Return { value },
                    None => RiscvSyscallOutcome::Blocked,
                })
            }
            RISCV_LINUX_RECVFROM => {
                guest_memory_writer.map(|writer| match syscall_recvfrom(request, state, writer) {
                    Some(value) => RiscvSyscallOutcome::Return { value },
                    None => RiscvSyscallOutcome::Blocked,
                })
            }
            RISCV_LINUX_SENDMSG => {
                guest_memory_reader.map(|reader| match syscall_sendmsg(request, state, reader) {
                    Some(value) => RiscvSyscallOutcome::Return { value },
                    None => RiscvSyscallOutcome::Blocked,
                })
            }
            RISCV_LINUX_RECVMSG => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| {
                    match syscall_recvmsg(request, state, reader, writer) {
                        Some(value) => RiscvSyscallOutcome::Return { value },
                        None => RiscvSyscallOutcome::Blocked,
                    }
                })
            }),
            RISCV_LINUX_SETSOCKOPT => {
                guest_memory_reader.map(|reader| RiscvSyscallOutcome::Return {
                    value: syscall_setsockopt(request, state, reader),
                })
            }
            RISCV_LINUX_GETSOCKOPT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_getsockopt(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_SHUTDOWN => Some(RiscvSyscallOutcome::Return {
                value: syscall_shutdown(request, state),
            }),
            RISCV_LINUX_PPOLL => {
                syscall_ppoll(request, state, guest_memory_reader, guest_memory_writer)
            }
            RISCV_LINUX_PSELECT6 => {
                syscall_pselect6(request, state, guest_memory_reader, guest_memory_writer)
            }
            RISCV_LINUX_READLINKAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_readlinkat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_FCHMOD => Some(RiscvSyscallOutcome::Return {
                value: syscall_fchmod(request, state),
            }),
            RISCV_LINUX_FCHMODAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_fchmodat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_FCHMODAT2 => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_fchmodat2(request, state, guest_memory),
                })
            }
            RISCV_LINUX_FCHOWNAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_fchownat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_FCHOWN => Some(RiscvSyscallOutcome::Return {
                value: syscall_fchown(request, state),
            }),
            RISCV_NEWLIB_LEGACY_CHMOD => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_chmod(request, state, guest_memory),
                })
            }
            RISCV_LINUX_FACCESSAT => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_faccessat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_FACCESSAT2 => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_faccessat2(request, state, guest_memory),
                })
            }
            RISCV_LINUX_NEWFSTATAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_newfstatat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_STAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_stat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_LSTAT => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_lstat(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_STATX => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_statx(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_UTIMENSAT => {
                guest_memory_reader.map(|reader| RiscvSyscallOutcome::Return {
                    value: syscall_utimensat(request, state, reader),
                })
            }
            RISCV_LINUX_ACCESS => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_access(request, state, guest_memory),
                })
            }
            RISCV_LINUX_FSTAT => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_fstat(request, state, guest_memory),
                })
            }
            RISCV_LINUX_STATFS => guest_memory_reader.and_then(|reader| {
                guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                    value: syscall_statfs(request, state, reader, writer),
                })
            }),
            RISCV_LINUX_FSTATFS => guest_memory_writer.map(|writer| RiscvSyscallOutcome::Return {
                value: syscall_fstatfs(request, state, writer),
            }),
            RISCV_LINUX_SYNC => Some(RiscvSyscallOutcome::Return {
                value: syscall_sync(),
            }),
            RISCV_LINUX_FSYNC | RISCV_LINUX_FDATASYNC | RISCV_LINUX_SYNCFS => {
                Some(RiscvSyscallOutcome::Return {
                    value: syscall_fd_sync(request.argument(0), state),
                })
            }
            RISCV_LINUX_SYNC_FILE_RANGE => Some(RiscvSyscallOutcome::Return {
                value: syscall_sync_file_range(request, state),
            }),
            RISCV_LINUX_READAHEAD => Some(RiscvSyscallOutcome::Return {
                value: syscall_readahead(request, state),
            }),
            RISCV_LINUX_SETNS => Some(RiscvSyscallOutcome::Return {
                value: syscall_setns(request, state),
            }),
            RISCV_LINUX_GETRANDOM => {
                let flags = request.argument(2);
                if invalid_getrandom_flags(flags) {
                    Some(RiscvSyscallOutcome::Return {
                        value: linux_error(RISCV_LINUX_EINVAL),
                    })
                } else if request.argument(1) == 0 {
                    Some(RiscvSyscallOutcome::Return { value: 0 })
                } else {
                    guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                        value: syscall_getrandom(request, state, guest_memory),
                    })
                }
            }
            thread::RISCV_LINUX_SET_TID_ADDRESS
            | thread::RISCV_LINUX_MEMBARRIER
            | thread::RISCV_LINUX_RSEQ => {
                thread::syscall_thread(request, state, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_TIMES
            | RISCV_LINUX_GETTIMEOFDAY
            | RISCV_LINUX_CLOCK_GETTIME
            | RISCV_LINUX_CLOCK_GETRES
            | clock::RISCV_NEWLIB_CLOCK_GETTIME64
            | clock::RISCV_NEWLIB_LEGACY_TIME => syscall_clock(request, tick, guest_memory_writer),
            RISCV_LINUX_GETITIMER => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_getitimer(request, state, guest_memory),
                })
            }
            RISCV_LINUX_SETITIMER => guest_memory_reader.and_then(|reader| {
                syscall_setitimer(request, state, reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }),
            RISCV_LINUX_SETPGID => Some(RiscvSyscallOutcome::Return {
                value: syscall_setpgid(request, state),
            }),
            RISCV_LINUX_GETPGID => Some(RiscvSyscallOutcome::Return {
                value: syscall_getpgid(request, state),
            }),
            RISCV_LINUX_GETSID => Some(RiscvSyscallOutcome::Return {
                value: syscall_getsid(request, state),
            }),
            RISCV_LINUX_SETSID => Some(RiscvSyscallOutcome::Return {
                value: syscall_setsid(state),
            }),
            RISCV_LINUX_ACCT => syscall_acct(request, guest_memory_reader)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_UNSHARE => Some(RiscvSyscallOutcome::Return {
                value: syscall_unshare(request),
            }),
            RISCV_LINUX_PERSONALITY => Some(RiscvSyscallOutcome::Return {
                value: syscall_personality(request, state),
            }),
            RISCV_LINUX_PRCTL => {
                syscall_prctl(request, state, guest_memory_reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_PIDFD_OPEN => Some(RiscvSyscallOutcome::Return {
                value: syscall_pidfd_open(request, state),
            }),
            RISCV_LINUX_PIDFD_GETFD => Some(RiscvSyscallOutcome::Return {
                value: syscall_pidfd_getfd(request, state),
            }),
            RISCV_LINUX_PIDFD_SEND_SIGNAL => Some(RiscvSyscallOutcome::Return {
                value: syscall_pidfd_send_signal(request, state, tick),
            }),
            RISCV_LINUX_EXECVE => guest_memory_reader.map(|guest_memory| {
                match syscall_execve_error_path(request, state, guest_memory) {
                    Some(value) => RiscvSyscallOutcome::Return { value },
                    None => unsupported_syscall_outcome(request, state, tick),
                }
            }),
            RISCV_LINUX_UNAME => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: write_riscv_linux_utsname(request.argument(0), guest_memory),
                })
            }
            RISCV_LINUX_REBOOT => Some(RiscvSyscallOutcome::Return {
                value: syscall_reboot(),
            }),
            RISCV_LINUX_SETHOSTNAME => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_sethostname(request, guest_memory),
                })
            }
            RISCV_LINUX_SETDOMAINNAME => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_setdomainname(request, guest_memory),
                })
            }
            RISCV_LINUX_SYSINFO => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_sysinfo(
                        request.argument(0),
                        tick,
                        state.linux_se_memory_capacity(),
                        guest_memory,
                    ),
                })
            }
            RISCV_LINUX_SYSLOG => Some(RiscvSyscallOutcome::Return {
                value: syscall_syslog(request),
            }),
            RISCV_LINUX_FUTEX => syscall_futex(
                request,
                state,
                tick,
                guest_memory_reader,
                guest_memory_writer,
            ),
            RISCV_LINUX_WAIT4 => Some(syscall_wait4(request, state, guest_memory_writer)),
            RISCV_LINUX_WAITID => Some(syscall_waitid(request, state, guest_memory_writer)),
            RISCV_LINUX_GETRUSAGE => Some(RiscvSyscallOutcome::Return {
                value: syscall_getrusage(request, guest_memory_writer),
            }),
            RISCV_LINUX_GETRESUID | RISCV_LINUX_GETRESGID => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_res_identity(request, state.identity(), guest_memory),
                })
            }
            RISCV_LINUX_SETRESUID | RISCV_LINUX_SETRESGID => Some(RiscvSyscallOutcome::Return {
                value: syscall_setres_identity(request, &mut state.identity),
            }),
            RISCV_LINUX_SETREUID | RISCV_LINUX_SETREGID => Some(RiscvSyscallOutcome::Return {
                value: syscall_setre_identity(request, &mut state.identity),
            }),
            RISCV_LINUX_SETUID | RISCV_LINUX_SETGID => Some(RiscvSyscallOutcome::Return {
                value: syscall_set_identity(request, &mut state.identity),
            }),
            RISCV_LINUX_SETFSUID | RISCV_LINUX_SETFSGID => Some(RiscvSyscallOutcome::Return {
                value: syscall_set_file_system_identity(request, &mut state.identity),
            }),
            RISCV_LINUX_GETGROUPS => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_getgroups(request, state.identity(), guest_memory),
                })
            }
            RISCV_LINUX_SETGROUPS => Some(RiscvSyscallOutcome::Return {
                value: syscall_setgroups(),
            }),
            RISCV_LINUX_GETRLIMIT => syscall_getrlimit(request, state, guest_memory_writer)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_PRLIMIT64 => {
                syscall_prlimit64(request, state, guest_memory_reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_SET_ROBUST_LIST => Some(RiscvSyscallOutcome::Return {
                value: syscall_set_robust_list(request, state),
            }),
            RISCV_LINUX_GET_ROBUST_LIST => {
                guest_memory_writer.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_get_robust_list(request, state, guest_memory),
                })
            }
            RISCV_LINUX_GETCPU => syscall_getcpu(request, guest_memory_writer)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_RISCV_HWPROBE => Some(RiscvSyscallOutcome::Return {
                value: syscall_riscv_hwprobe(request, guest_memory_reader, guest_memory_writer),
            }),
            RISCV_LINUX_RISCV_FLUSH_ICACHE => Some(RiscvSyscallOutcome::Return {
                value: syscall_riscv_flush_icache(request),
            }),
            RISCV_LINUX_NANOSLEEP => syscall_nanosleep(request, guest_memory_reader)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_CLOCK_NANOSLEEP => {
                syscall_clock_nanosleep(request, tick, guest_memory_reader)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_UMASK => Some(RiscvSyscallOutcome::Return {
                value: syscall_umask(request.argument(0), state),
            }),
            scheduler::RISCV_LINUX_SCHED_SETPARAM => {
                scheduler::syscall_sched_setparam(request, state, guest_memory_reader)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_IOPRIO_GET => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_ioprio_get(request, state),
            }),
            scheduler::RISCV_LINUX_IOPRIO_SET => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_ioprio_set(request, state),
            }),
            scheduler::RISCV_LINUX_SCHED_SETSCHEDULER => {
                scheduler::syscall_sched_setscheduler(request, state, guest_memory_reader)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_SCHED_SETAFFINITY => {
                scheduler::syscall_sched_setaffinity(request, state, guest_memory_reader)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_SCHED_GETSCHEDULER => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_sched_getscheduler(request, state),
            }),
            scheduler::RISCV_LINUX_SCHED_GETPARAM => {
                scheduler::syscall_sched_getparam(request, state, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_SCHED_GETAFFINITY => {
                scheduler::syscall_sched_getaffinity(request, state, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_SCHED_GET_PRIORITY_MAX => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_sched_get_priority_max(request),
            }),
            scheduler::RISCV_LINUX_SCHED_GET_PRIORITY_MIN => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_sched_get_priority_min(request),
            }),
            scheduler::RISCV_LINUX_SCHED_RR_GET_INTERVAL => {
                scheduler::syscall_sched_rr_get_interval(request, state, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            scheduler::RISCV_LINUX_SETPRIORITY => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_setpriority(request, state),
            }),
            scheduler::RISCV_LINUX_GETPRIORITY => Some(RiscvSyscallOutcome::Return {
                value: scheduler::syscall_getpriority(request, state),
            }),
            RISCV_LINUX_KILL => Some(RiscvSyscallOutcome::Return {
                value: syscall_kill(request, state, tick),
            }),
            RISCV_LINUX_TKILL => Some(RiscvSyscallOutcome::Return {
                value: syscall_tkill(request, state, tick),
            }),
            RISCV_LINUX_TGKILL => Some(RiscvSyscallOutcome::Return {
                value: syscall_tgkill(request, state, tick),
            }),
            RISCV_LINUX_SIGALTSTACK => {
                syscall_sigaltstack(request, state, guest_memory_reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_SCHED_YIELD => Some(RiscvSyscallOutcome::Return { value: 0 }),
            RISCV_LINUX_RT_SIGRETURN => Some(unsupported_syscall_outcome(request, state, tick)),
            RISCV_LINUX_RT_SIGQUEUEINFO => {
                syscall_rt_sigqueueinfo(request, state, tick, guest_memory_reader)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_RT_SIGSUSPEND => {
                syscall_rt_sigsuspend(request, state, tick, guest_memory_reader)
            }
            RISCV_LINUX_RT_SIGACTION => {
                syscall_rt_sigaction(request, state, guest_memory_reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_RT_SIGPROCMASK => syscall_rt_sigprocmask(
                request,
                state,
                tick,
                guest_memory_reader,
                guest_memory_writer,
            )
            .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_RT_SIGPENDING => syscall_rt_sigpending(request, state, guest_memory_writer)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_RT_SIGTIMEDWAIT => {
                syscall_rt_sigtimedwait(request, state, guest_memory_reader, guest_memory_writer)
                    .map(|value| RiscvSyscallOutcome::Return { value })
            }
            RISCV_LINUX_MPROTECT => Some(RiscvSyscallOutcome::Return {
                value: syscall_mprotect(request, state),
            }),
            RISCV_LINUX_MSYNC => Some(RiscvSyscallOutcome::Return {
                value: syscall_msync(request, state),
            }),
            RISCV_LINUX_MLOCK | RISCV_LINUX_MUNLOCK => Some(RiscvSyscallOutcome::Return {
                value: syscall_memory_lock_range(request, state),
            }),
            RISCV_LINUX_MLOCK2 => Some(RiscvSyscallOutcome::Return {
                value: syscall_mlock2(request, state),
            }),
            RISCV_LINUX_MLOCKALL => Some(RiscvSyscallOutcome::Return {
                value: syscall_mlockall(request.argument(0)),
            }),
            RISCV_LINUX_MUNLOCKALL => Some(RiscvSyscallOutcome::Return {
                value: syscall_munlockall(),
            }),
            RISCV_LINUX_MBIND => Some(RiscvSyscallOutcome::Return {
                value: syscall_mbind(request, state, guest_memory_reader),
            }),
            RISCV_LINUX_GET_MEMPOLICY => syscall_get_mempolicy(request, state, guest_memory_writer)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_SET_MEMPOLICY => Some(RiscvSyscallOutcome::Return {
                value: syscall_set_mempolicy(request, state, guest_memory_reader),
            }),
            RISCV_LINUX_SETRLIMIT => syscall_setrlimit(request, state, guest_memory_reader)
                .map(|value| RiscvSyscallOutcome::Return { value }),
            RISCV_LINUX_EXIT | RISCV_LINUX_EXIT_GROUP => Some(syscall_exit(
                request.argument(0),
                state,
                tick,
                guest_memory_writer,
            )),
            RISCV_LINUX_GETPID | RISCV_LINUX_GETPPID | RISCV_LINUX_GETTID | RISCV_LINUX_GETUID
            | RISCV_LINUX_GETEUID | RISCV_LINUX_GETGID | RISCV_LINUX_GETEGID => {
                Some(RiscvSyscallOutcome::Return {
                    value: syscall_identity(request.number(), state.identity())
                        .expect("RISC-V Linux identity syscall is handled"),
                })
            }
            RISCV_LINUX_BRK => Some(RiscvSyscallOutcome::Return {
                value: syscall_brk(request.argument(0), state, guest_memory_writer),
            }),
            RISCV_LINUX_SWAPON => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_swapon(request, guest_memory),
                })
            }
            RISCV_LINUX_SWAPOFF => {
                guest_memory_reader.map(|guest_memory| RiscvSyscallOutcome::Return {
                    value: syscall_swapoff(request, guest_memory),
                })
            }
            RISCV_LINUX_MMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_mmap(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_MUNMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_munmap(request.argument(0), request.argument(1), state),
            }),
            RISCV_LINUX_MREMAP => Some(RiscvSyscallOutcome::Return {
                value: syscall_mremap(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_MINCORE => Some(RiscvSyscallOutcome::Return {
                value: syscall_mincore(request, state, guest_memory_writer),
            }),
            RISCV_LINUX_MADVISE => Some(RiscvSyscallOutcome::Return {
                value: syscall_madvise(request, state, guest_memory_writer),
            }),
            _ => Some(unsupported_syscall_outcome(request, state, tick)),
        }
    }
}

fn known_ni_syscall_outcome() -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return {
        value: linux_error(RISCV_LINUX_ENOSYS),
    }
}

fn unsupported_syscall_outcome(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
) -> RiscvSyscallOutcome {
    state.push_unknown_syscall(RiscvUnknownSyscallRecord::new(
        request.pc(),
        request.number(),
        request.arguments(),
        tick,
    ));
    RiscvSyscallOutcome::Return {
        value: linux_error(RISCV_LINUX_ENOSYS),
    }
}
