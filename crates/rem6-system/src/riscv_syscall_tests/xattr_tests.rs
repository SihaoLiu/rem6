use super::*;
use std::sync::{Arc, Mutex};

const RISCV_LINUX_SETXATTR_FOR_TEST: u64 = 5;
const RISCV_LINUX_LSETXATTR_FOR_TEST: u64 = 6;
const RISCV_LINUX_FSETXATTR_FOR_TEST: u64 = 7;
const RISCV_LINUX_GETXATTR_FOR_TEST: u64 = 8;
const RISCV_LINUX_LGETXATTR_FOR_TEST: u64 = 9;
const RISCV_LINUX_FGETXATTR_FOR_TEST: u64 = 10;
const RISCV_LINUX_LISTXATTR_FOR_TEST: u64 = 11;
const RISCV_LINUX_LLISTXATTR_FOR_TEST: u64 = 12;
const RISCV_LINUX_FLISTXATTR_FOR_TEST: u64 = 13;
const RISCV_LINUX_REMOVEXATTR_FOR_TEST: u64 = 14;
const RISCV_LINUX_LREMOVEXATTR_FOR_TEST: u64 = 15;
const RISCV_LINUX_FREMOVEXATTR_FOR_TEST: u64 = 16;
const RISCV_LINUX_MKDIRAT_FOR_TEST: u64 = 34;
const RISCV_LINUX_UNLINKAT_FOR_TEST: u64 = 35;
const RISCV_LINUX_RENAMEAT2_FOR_TEST: u64 = 276;
const RISCV_LINUX_DUP3_FOR_TEST: u64 = 24;
const RISCV_LINUX_OPENAT_FOR_TEST: u64 = 56;
const RISCV_LINUX_CLOSE_FOR_TEST: u64 = 57;
const RISCV_LINUX_AT_FDCWD_FOR_TEST: u64 = (-100_i64) as u64;
const RISCV_LINUX_AT_REMOVEDIR_FOR_TEST: u64 = 0x200;
const RISCV_LINUX_O_WRONLY_FOR_TEST: u64 = 1;
const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;
const RISCV_LINUX_O_DIRECTORY_FOR_TEST: u64 = 0o200000;
const RISCV_LINUX_XATTR_CREATE_FOR_TEST: u64 = 1;
const RISCV_LINUX_XATTR_REPLACE_FOR_TEST: u64 = 2;

type RecordedWrites = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_path_xattr_roundtrips_and_removes_guest_file_attribute() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/guest.txt", b"guest");
    let reader = memory_reader(vec![
        (0x9000, b"/guest.txt\0".to_vec()),
        (0x9100, b"user.rem6\0".to_vec()),
        (0x9200, b"value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9200, 5, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9300, 8, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_LISTXATTR_FOR_TEST,
                [0x9000, 0x9400, 16, 0, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_REMOVEXATTR_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9500, 8, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(61)
        })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_writes_in_range(&writes, 0x9300, 5), b"value");
    assert_eq!(collect_writes_in_range(&writes, 0x9400, 10), b"user.rem6\0");
}

#[test]
fn linux_table_fd_xattr_roundtrips_through_open_guest_file() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/guest.txt", b"guest");
    let reader = memory_reader(vec![
        (0x9000, b"/guest.txt\0".to_vec()),
        (0x9100, b"user.fd\0".to_vec()),
        (0x9200, b"fd-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FSETXATTR_FOR_TEST,
                [3, 0x9100, 0x9200, 8, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FGETXATTR_FOR_TEST,
                [3, 0x9100, 0x9300, 16, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FLISTXATTR_FOR_TEST,
                [3, 0x9400, 16, 0, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FREMOVEXATTR_FOR_TEST,
                [3, 0x9100, 0, 0, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_writes_in_range(&writes, 0x9300, 8), b"fd-value");
    assert_eq!(collect_writes_in_range(&writes, 0x9400, 8), b"user.fd\0");
}

#[test]
fn linux_table_lpath_xattr_operates_on_final_symlink_identity() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"target.txt", b"target");
    state.register_guest_symlink(b"link.txt", b"target.txt");
    let reader = memory_reader(vec![
        (0x9000, b"link.txt\0".to_vec()),
        (0x9100, b"user.link\0".to_vec()),
        (0x9200, b"link-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LSETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9200, 10, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9300, 16, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(61)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_LGETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9400, 16, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_LLISTXATTR_FOR_TEST,
                [0x9000, 0x9500, 16, 0, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_LREMOVEXATTR_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_LGETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9600, 16, 0, 0]
            ),
            &mut state,
            6,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(61)
        })
    );
    let writes = writes.lock().unwrap();
    assert_eq!(collect_writes_in_range(&writes, 0x9400, 10), b"link-value");
    assert_eq!(collect_writes_in_range(&writes, 0x9500, 10), b"user.link\0");
}

#[test]
fn linux_table_xattr_flags_match_linux_create_replace_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/guest.txt", b"guest");
    let reader = memory_reader(vec![
        (0x9000, b"/guest.txt\0".to_vec()),
        (0x9100, b"user.flag\0".to_vec()),
        (0x9200, b"user.missing\0".to_vec()),
        (0x9300, b"first".to_vec()),
        (0x9400, b"second".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [
                    0x9000,
                    0x9100,
                    0x9300,
                    5,
                    RISCV_LINUX_XATTR_CREATE_FOR_TEST,
                    0,
                ]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [
                    0x9000,
                    0x9100,
                    0x9300,
                    5,
                    RISCV_LINUX_XATTR_CREATE_FOR_TEST,
                    0,
                ]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(17)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [
                    0x9000,
                    0x9200,
                    0x9300,
                    5,
                    RISCV_LINUX_XATTR_REPLACE_FOR_TEST,
                    0,
                ]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(61)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [
                    0x9000,
                    0x9100,
                    0x9300,
                    5,
                    RISCV_LINUX_XATTR_CREATE_FOR_TEST | RISCV_LINUX_XATTR_REPLACE_FOR_TEST,
                    0,
                ]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(22)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [
                    0x9000,
                    0x9100,
                    0x9400,
                    6,
                    RISCV_LINUX_XATTR_REPLACE_FOR_TEST,
                    0,
                ]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9500, 8, 0, 0]
            ),
            &mut state,
            6,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 6 })
    );
    let writes = writes.lock().unwrap();
    assert_eq!(collect_writes_in_range(&writes, 0x9500, 6), b"second");
}

#[test]
fn linux_table_xattr_get_and_list_probe_sizes_and_report_erange() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/guest.txt", b"guest");
    let reader = memory_reader(vec![
        (0x9000, b"/guest.txt\0".to_vec()),
        (0x9100, b"user.size\0".to_vec()),
        (0x9200, b"value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9200, 5, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9300, 4, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(34)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_LISTXATTR_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_LISTXATTR_FOR_TEST,
                [0x9000, 0x9400, 9, 0, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(34)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_fd_xattr_survives_unlink_until_file_description_closes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"guest");
    let reader = memory_reader(vec![
        (0x9000, b"guest.txt\0".to_vec()),
        (0x9100, b"user.fd\0".to_vec()),
        (0x9200, b"fd-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FSETXATTR_FOR_TEST,
                [3, 0x9100, 0x9200, 8, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FGETXATTR_FOR_TEST,
                [3, 0x9100, 0x9300, 16, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_CLOSE_FOR_TEST, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_FGETXATTR_FOR_TEST,
                [3, 0x9100, 0x9400, 16, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(9)
        })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_writes_in_range(&writes, 0x9300, 8), b"fd-value");
}

#[test]
fn linux_table_unlinked_open_file_xattr_does_not_leak_to_recreated_path() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![
        (0x9000, b"guest.txt\0".to_vec()),
        (0x9100, b"user.fd\0".to_vec()),
        (0x9200, b"fd-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9000,
                    RISCV_LINUX_O_WRONLY_FOR_TEST | RISCV_LINUX_O_CREAT_FOR_TEST,
                    0o600,
                    0,
                    0,
                ]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FSETXATTR_FOR_TEST,
                [3, 0x9100, 0x9200, 8, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_OPENAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9000,
                    RISCV_LINUX_O_WRONLY_FOR_TEST | RISCV_LINUX_O_CREAT_FOR_TEST,
                    0o600,
                    0,
                    0,
                ]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9300, 16, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(61)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_FGETXATTR_FOR_TEST,
                [3, 0x9100, 0x9400, 16, 0, 0]
            ),
            &mut state,
            6,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_writes_in_range(&writes, 0x9400, 8), b"fd-value");
}

#[test]
fn linux_table_dup3_replacement_releases_unlinked_file_xattr() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![
        (0x9000, b"old.txt\0".to_vec()),
        (0x9100, b"new.txt\0".to_vec()),
        (0x9200, b"user.fd\0".to_vec()),
        (0x9300, b"fd-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9000,
                    RISCV_LINUX_O_WRONLY_FOR_TEST | RISCV_LINUX_O_CREAT_FOR_TEST,
                    0o600,
                    0,
                    0,
                ]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_OPENAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9100,
                    RISCV_LINUX_O_WRONLY_FOR_TEST | RISCV_LINUX_O_CREAT_FOR_TEST,
                    0o600,
                    0,
                    0,
                ]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FSETXATTR_FOR_TEST,
                [3, 0x9200, 0x9300, 8, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.guest_xattrs.len(), 1);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.guest_xattrs.len(), 1);
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_DUP3_FOR_TEST, [4, 3, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert!(state.guest_xattrs.is_empty());
}

#[test]
fn linux_table_rename_replacement_drops_replaced_file_xattr() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"source.txt", b"new");
    state.register_guest_file(b"target.txt", b"old");
    let reader = memory_reader(vec![
        (0x9000, b"source.txt\0".to_vec()),
        (0x9100, b"target.txt\0".to_vec()),
        (0x9200, b"user.old\0".to_vec()),
        (0x9300, b"old-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [0x9100, 0x9200, 0x9300, 9, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9100,
                    0,
                    0,
                ]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9100, 0, 0, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_OPENAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9100,
                    RISCV_LINUX_O_WRONLY_FOR_TEST | RISCV_LINUX_O_CREAT_FOR_TEST,
                    0o600,
                    0,
                    0,
                ]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9100, 0x9200, 0x9400, 16, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(61)
        })
    );
}

#[test]
fn linux_table_rmdir_drops_removed_directory_xattr() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![
        (0x9000, b"xdir\0".to_vec()),
        (0x9100, b"user.dir\0".to_vec()),
        (0x9200, b"dir-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0o755, 0, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9200, 9, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9000,
                    RISCV_LINUX_AT_REMOVEDIR_FOR_TEST,
                    0,
                    0,
                    0,
                ]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0o755, 0, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9300, 16, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(61)
        })
    );
}

#[test]
fn linux_table_rejects_xattr_on_implicit_parent_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"implicit/child.txt", b"child");
    let reader = memory_reader(vec![
        (0x9000, b"implicit\0".to_vec()),
        (0x9100, b"user.dir\0".to_vec()),
        (0x9200, b"dir-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [0x9000, 0x9100, 0x9200, 9, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(2)
        })
    );
    assert!(state.guest_xattrs.is_empty());
}

#[test]
fn linux_table_directory_rename_preserves_directory_xattr() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![
        (0x9000, b"old-dir\0".to_vec()),
        (0x9100, b"new-dir\0".to_vec()),
        (0x9200, b"user.dir\0".to_vec()),
        (0x9300, b"dir-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0o755, 0, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [0x9000, 0x9200, 0x9300, 9, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9100,
                    0,
                    0,
                ]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETXATTR_FOR_TEST,
                [0x9100, 0x9200, 0x9400, 16, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_writes_in_range(&writes, 0x9400, 9), b"dir-value");
}

#[test]
fn linux_table_open_directory_fd_xattr_survives_directory_rename() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![
        (0x9000, b"old-dir\0".to_vec()),
        (0x9100, b"new-dir\0".to_vec()),
        (0x9200, b"user.dir\0".to_vec()),
        (0x9300, b"dir-value".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0o755, 0, 0, 0]
            ),
            &mut state,
            1,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_OPENAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_TEST,
                    0,
                    0,
                    0,
                ]
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SETXATTR_FOR_TEST,
                [0x9000, 0x9200, 0x9300, 9, 0, 0]
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD_FOR_TEST,
                    0x9100,
                    0,
                    0,
                ]
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FGETXATTR_FOR_TEST,
                [3, 0x9200, 0x9400, 16, 0, 0]
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_writes_in_range(&writes, 0x9400, 9), b"dir-value");
}

fn memory_reader(regions: Vec<(u64, Vec<u8>)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, len| {
        regions.iter().find_map(|(base, bytes)| {
            let offset = usize::try_from(address.checked_sub(*base)?).ok()?;
            let end = offset.checked_add(len)?;
            bytes.get(offset..end).map(Vec::from)
        })
    })
}

fn recording_writer(writes: RecordedWrites) -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes.lock().unwrap().push((address, bytes.to_vec()));
        true
    })
}

fn collect_writes_in_range(writes: &[(u64, Vec<u8>)], base: u64, len: usize) -> Vec<u8> {
    let mut result = vec![0; len];
    let end = base + len as u64;
    for (address, chunk) in writes {
        if *address < base || *address + chunk.len() as u64 > end {
            continue;
        }
        let offset = usize::try_from(address - base).unwrap();
        result[offset..offset + chunk.len()].copy_from_slice(chunk);
    }
    result
}
