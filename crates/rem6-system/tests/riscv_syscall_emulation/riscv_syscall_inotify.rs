use super::riscv_syscall_emulation_support::*;
use rem6_system::{
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RiscvSyscallTable,
};

const RISCV_LINUX_INOTIFY_INIT1: u64 = 26;
const RISCV_LINUX_INOTIFY_ADD_WATCH: u64 = 27;
const RISCV_LINUX_INOTIFY_RM_WATCH: u64 = 28;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_OPENAT: u64 = 56;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_PPOLL: u64 = 73;
const RISCV_LINUX_EAGAIN: u64 = 11;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EEXIST: u64 = 17;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_AT_FDCWD: u64 = u64::MAX - 99;
const RISCV_LINUX_O_WRONLY: u64 = 1;
const RISCV_LINUX_O_CREAT: u64 = 0o100;
const RISCV_LINUX_O_NONBLOCK: u64 = 0x800;
const RISCV_LINUX_POLLIN: i16 = 0x0001;
const RISCV_LINUX_IN_CREATE: u32 = 0x0000_0100;
const RISCV_LINUX_IN_DELETE: u32 = 0x0000_0200;
const RISCV_LINUX_IN_IGNORED: u32 = 0x0000_8000;
const RISCV_LINUX_IN_MASK_CREATE: u32 = 0x1000_0000;
const RISCV_LINUX_IN_MASK_ADD: u32 = 0x2000_0000;
const RISCV_LINUX_IN_ISDIR: u32 = 0x4000_0000;
const RISCV_LINUX_IN_ONESHOT: u32 = 0x8000_0000;
const RISCV_LINUX_INOTIFY_EVENT_BYTES: u64 = 16;
const INOTIFY_NAME_BYTES: u64 = 16;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn inotify_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let watched = b"watched\0";
    let created = b"watched/new.txt\0";
    let created_again = b"watched/again.txt\0";
    let pollfd = pollfd_bytes(3, RISCV_LINUX_POLLIN);
    let zero_timeout = [0_u8; 16];
    let read_buffer = [0_u8; 128];
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x9000, watched),
            (0x9020, created),
            (0x9080, created_again),
            (0x9040, &pollfd),
            (0x9050, &zero_timeout),
            (0x9100, &read_buffer),
        ],
    )
}

fn pollfd_bytes(fd: i32, events: i16) -> [u8; 8] {
    let mut bytes = [0_u8; 8];
    bytes[..4].copy_from_slice(&fd.to_le_bytes());
    bytes[4..6].copy_from_slice(&events.to_le_bytes());
    bytes
}

fn memory_i32(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> i32 {
    let bytes = guest_memory_reader(Arc::clone(store))(address, 4).unwrap();
    i32::from_le_bytes(bytes.try_into().unwrap())
}

fn memory_u32(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> u32 {
    let bytes = guest_memory_reader(Arc::clone(store))(address, 4).unwrap();
    u32::from_le_bytes(bytes.try_into().unwrap())
}

fn memory_bytes(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64, bytes: usize) -> Vec<u8> {
    guest_memory_reader(Arc::clone(store))(address, bytes).unwrap()
}

fn pollfd_revents(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> i16 {
    let bytes = guest_memory_reader(Arc::clone(store))(address + 6, 2).unwrap();
    i16::from_le_bytes(bytes.try_into().unwrap())
}

fn write_pollfd_revents(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64, revents: i16) {
    assert!(guest_memory_writer(Arc::clone(store))(
        address + 6,
        &revents.to_le_bytes()
    ));
}

fn handle_with_memory(
    state: &mut RiscvSyscallState,
    number: u64,
    arguments: [u64; 6],
    reader: Option<&RiscvGuestMemoryReader>,
    writer: Option<&RiscvGuestMemoryWriter>,
) -> RiscvSyscallOutcome {
    RiscvSyscallTable::new()
        .handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, number, arguments),
            state,
            7,
            reader,
            writer,
        )
        .expect("syscall must be handled")
}

fn return_value(outcome: RiscvSyscallOutcome) -> u64 {
    match outcome {
        RiscvSyscallOutcome::Return { value } => value,
        outcome => panic!("unexpected syscall outcome: {outcome:?}"),
    }
}

#[test]
fn linux_table_inotify_directory_create_event_is_readable_and_consumed() {
    let store = inotify_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_directory("watched");

    let fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_INOTIFY_INIT1,
        [RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0, 0],
        None,
        None,
    ));
    assert_eq!(fd, 3);

    let wd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_INOTIFY_ADD_WATCH,
        [
            fd,
            0x9000,
            u64::from(RISCV_LINUX_IN_CREATE | RISCV_LINUX_IN_DELETE),
            0,
            0,
            0,
        ],
        Some(&reader),
        None,
    ));
    assert_eq!(wd, 1);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9100, RISCV_LINUX_INOTIFY_EVENT_BYTES, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EAGAIN)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9040, 1, 0x9050, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    assert_eq!(pollfd_revents(&store, 0x9040), 0);

    let created_fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_OPENAT,
        [
            RISCV_LINUX_AT_FDCWD,
            0x9020,
            RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_CREAT,
            0o644,
            0,
            0,
        ],
        Some(&reader),
        None,
    ));
    assert_eq!(created_fd, 4);

    write_pollfd_revents(&store, 0x9040, 0);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9040, 1, 0x9050, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        1
    );
    assert_eq!(
        pollfd_revents(&store, 0x9040) & RISCV_LINUX_POLLIN,
        RISCV_LINUX_POLLIN
    );

    let record_bytes = RISCV_LINUX_INOTIFY_EVENT_BYTES + INOTIFY_NAME_BYTES;
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9100, record_bytes, 0, 0, 0],
            None,
            Some(&writer),
        )),
        record_bytes
    );
    assert_eq!(memory_i32(&store, 0x9100), wd as i32);
    assert_eq!(memory_u32(&store, 0x9104), RISCV_LINUX_IN_CREATE);
    assert_eq!(memory_u32(&store, 0x9108), 0);
    assert_eq!(memory_u32(&store, 0x910c), INOTIFY_NAME_BYTES as u32);
    assert_eq!(
        &memory_bytes(&store, 0x9110, INOTIFY_NAME_BYTES as usize)[..8],
        b"new.txt\0"
    );

    write_pollfd_revents(&store, 0x9040, 0);
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_PPOLL,
            [0x9040, 1, 0x9050, 0, 0, 0],
            Some(&reader),
            Some(&writer),
        )),
        0
    );
    assert_eq!(pollfd_revents(&store, 0x9040), 0);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_INOTIFY_RM_WATCH,
            [fd, wd, 0, 0, 0, 0],
            None,
            None,
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9100, RISCV_LINUX_INOTIFY_EVENT_BYTES, 0, 0, 0],
            None,
            Some(&writer),
        )),
        RISCV_LINUX_INOTIFY_EVENT_BYTES
    );
    assert_eq!(memory_i32(&store, 0x9100), wd as i32);
    assert_eq!(memory_u32(&store, 0x9104), RISCV_LINUX_IN_IGNORED);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_WRITE,
            [fd, 0x9100, 8, 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_CLOSE,
            [fd, 0, 0, 0, 0, 0],
            None,
            None,
        )),
        0
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9100, record_bytes, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EBADF)
    );
}

#[test]
fn linux_table_inotify_mask_create_rejects_duplicate_and_mask_add_combination() {
    let store = inotify_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_directory("watched");

    let fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_INOTIFY_INIT1,
        [0, 0, 0, 0, 0, 0],
        None,
        None,
    ));
    assert_eq!(fd, 3);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_INOTIFY_ADD_WATCH,
            [
                fd,
                0x9000,
                u64::from(
                    RISCV_LINUX_IN_CREATE | RISCV_LINUX_IN_MASK_CREATE | RISCV_LINUX_IN_MASK_ADD,
                ),
                0,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );

    let wd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_INOTIFY_ADD_WATCH,
        [
            fd,
            0x9000,
            u64::from(RISCV_LINUX_IN_CREATE | RISCV_LINUX_IN_MASK_CREATE),
            0,
            0,
            0,
        ],
        Some(&reader),
        None,
    ));
    assert_eq!(wd, 1);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_INOTIFY_ADD_WATCH,
            [
                fd,
                0x9000,
                u64::from(RISCV_LINUX_IN_DELETE | RISCV_LINUX_IN_MASK_CREATE),
                0,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EEXIST)
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_INOTIFY_ADD_WATCH,
            [
                fd,
                0x9000,
                u64::from(RISCV_LINUX_IN_DELETE | RISCV_LINUX_IN_MASK_ADD),
                0,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        wd
    );
}

#[test]
fn linux_table_inotify_add_watch_validates_fd_and_mask_before_path_resolution() {
    let store = inotify_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_directory("watched");

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_INOTIFY_ADD_WATCH,
            [99, 0xffff, u64::from(RISCV_LINUX_IN_CREATE), 0, 0, 0],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EBADF)
    );

    let fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_INOTIFY_INIT1,
        [0, 0, 0, 0, 0, 0],
        None,
        None,
    ));
    assert_eq!(fd, 3);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_INOTIFY_ADD_WATCH,
            [
                fd,
                0xffff,
                u64::from(
                    RISCV_LINUX_IN_CREATE | RISCV_LINUX_IN_MASK_CREATE | RISCV_LINUX_IN_MASK_ADD,
                ),
                0,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_INOTIFY_ADD_WATCH,
            [
                fd,
                0x9000,
                u64::from(RISCV_LINUX_IN_CREATE | RISCV_LINUX_IN_ISDIR),
                0,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        1
    );
}

#[test]
fn linux_table_inotify_oneshot_queues_ignored_and_removes_watch_after_first_event() {
    let store = inotify_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_directory("watched");

    let fd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_INOTIFY_INIT1,
        [RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0, 0],
        None,
        None,
    ));
    assert_eq!(fd, 3);
    let wd = return_value(handle_with_memory(
        &mut state,
        RISCV_LINUX_INOTIFY_ADD_WATCH,
        [
            fd,
            0x9000,
            u64::from(RISCV_LINUX_IN_CREATE | RISCV_LINUX_IN_ONESHOT),
            0,
            0,
            0,
        ],
        Some(&reader),
        None,
    ));
    assert_eq!(wd, 1);

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_OPENAT,
            [
                RISCV_LINUX_AT_FDCWD,
                0x9020,
                RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_CREAT,
                0o644,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        4
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9100, 128, 0, 0, 0],
            None,
            Some(&writer),
        )),
        RISCV_LINUX_INOTIFY_EVENT_BYTES + INOTIFY_NAME_BYTES + RISCV_LINUX_INOTIFY_EVENT_BYTES
    );
    assert_eq!(memory_i32(&store, 0x9100), wd as i32);
    assert_eq!(memory_u32(&store, 0x9104), RISCV_LINUX_IN_CREATE);
    assert_eq!(
        memory_i32(
            &store,
            0x9100 + RISCV_LINUX_INOTIFY_EVENT_BYTES + INOTIFY_NAME_BYTES
        ),
        wd as i32
    );
    assert_eq!(
        memory_u32(
            &store,
            0x9104 + RISCV_LINUX_INOTIFY_EVENT_BYTES + INOTIFY_NAME_BYTES
        ),
        RISCV_LINUX_IN_IGNORED
    );

    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_OPENAT,
            [
                RISCV_LINUX_AT_FDCWD,
                0x9080,
                RISCV_LINUX_O_WRONLY | RISCV_LINUX_O_CREAT,
                0o644,
                0,
                0,
            ],
            Some(&reader),
            None,
        )),
        5
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_READ,
            [fd, 0x9100, 128, 0, 0, 0],
            None,
            Some(&writer),
        )),
        linux_error(RISCV_LINUX_EAGAIN)
    );
    assert_eq!(
        return_value(handle_with_memory(
            &mut state,
            RISCV_LINUX_INOTIFY_RM_WATCH,
            [fd, wd, 0, 0, 0, 0],
            None,
            None,
        )),
        linux_error(RISCV_LINUX_EINVAL)
    );
}
