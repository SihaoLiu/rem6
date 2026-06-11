#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    GuestFd, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome,
    RiscvSyscallRequest, RiscvSyscallState, RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_LSEEK: u64 = 62;
const RISCV_LINUX_OPENAT: u64 = 56;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_DUP: u64 = 23;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_AT_FDCWD: u64 = (-100_i64) as u64;
const RISCV_LINUX_SEEK_SET: u64 = 0;
const RISCV_LINUX_SEEK_CUR: u64 = 1;
const RISCV_LINUX_SEEK_END: u64 = 2;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EINVAL: u64 = 22;
const RISCV_LINUX_ESPIPE: u64 = 29;

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

fn memory_for_path() -> std::sync::Arc<std::sync::Mutex<rem6_memory::PartitionedMemoryStore>> {
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[(0x9000, b"/input.txt\0"), (0x9100, b"\0\0\0")],
    )
}

fn open_registered_input(state: &mut RiscvSyscallState, reader: &RiscvGuestMemoryReader) -> u64 {
    state.register_guest_file(b"/input.txt", b"abcdef");
    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_OPENAT,
            [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
        ),
        state,
        0,
        Some(reader),
        None,
    );
    match outcome {
        Some(RiscvSyscallOutcome::Return { value }) => value,
        other => panic!("unexpected openat outcome: {other:?}"),
    }
}

#[test]
fn linux_table_lseek_seek_set_changes_registered_file_read_offset() {
    let store = memory_for_path();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(std::sync::Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(std::sync::Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let fd = open_registered_input(&mut state, &reader);

    let seek = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_LSEEK,
            [fd, 2, RISCV_LINUX_SEEK_SET, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        None,
    );
    assert_eq!(seek, Some(RiscvSyscallOutcome::Return { value: 2 }));

    let read = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_READ, [fd, 0x9100, 3, 0, 0, 0]),
        &mut state,
        0,
        None,
        Some(&writer),
    );
    assert_eq!(read, Some(RiscvSyscallOutcome::Return { value: 3 }));
    assert_eq!(
        guest_memory_reader(std::sync::Arc::clone(&store))(0x9100, 3),
        Some(b"cde".to_vec())
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(fd as i32).unwrap())
            .unwrap()
            .get(),
        5
    );
}

#[test]
fn linux_table_lseek_seek_cur_updates_shared_description_after_dup() {
    let store = memory_for_path();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(std::sync::Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let fd = open_registered_input(&mut state, &reader);

    let dup = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(0x8000, RISCV_LINUX_DUP, [fd, 0, 0, 0, 0, 0]),
        &mut state,
        0,
        None,
        None,
    );
    let dup_fd = match dup {
        Some(RiscvSyscallOutcome::Return { value }) => value,
        other => panic!("unexpected dup outcome: {other:?}"),
    };

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LSEEK,
                [fd, 2, RISCV_LINUX_SEEK_SET, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LSEEK,
                [dup_fd, 3, RISCV_LINUX_SEEK_CUR, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(fd as i32).unwrap())
            .unwrap()
            .get(),
        5
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(dup_fd as i32).unwrap())
            .unwrap()
            .get(),
        5
    );
}

#[test]
fn linux_table_lseek_seek_end_uses_registered_file_size() {
    let store = memory_for_path();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(std::sync::Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let fd = open_registered_input(&mut state, &reader);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_LSEEK,
            [fd, (-2_i64) as u64, RISCV_LINUX_SEEK_END, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        None,
    );

    assert_eq!(outcome, Some(RiscvSyscallOutcome::Return { value: 4 }));
}

#[test]
fn linux_table_lseek_rejects_bad_fd_and_invalid_whence() {
    let store = memory_for_path();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(std::sync::Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let fd = open_registered_input(&mut state, &reader);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LSEEK,
                [99, 0, RISCV_LINUX_SEEK_SET, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_LSEEK, [fd, 0, 99, 0, 0, 0]),
            &mut state,
            0,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_lseek_rejects_negative_resulting_offset() {
    let store = memory_for_path();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(std::sync::Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let fd = open_registered_input(&mut state, &reader);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_LSEEK,
            [fd, (-1_i64) as u64, RISCV_LINUX_SEEK_SET, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        None,
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(fd as i32).unwrap())
            .unwrap()
            .get(),
        0
    );
}

#[test]
fn linux_table_lseek_standard_fd_reports_illegal_seek() {
    let mut state = RiscvSyscallState::new(0);

    let outcome = RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_LSEEK,
            [0, 0, RISCV_LINUX_SEEK_SET, 0, 0, 0],
        ),
        &mut state,
        0,
        None,
        None,
    );

    assert_eq!(
        outcome,
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESPIPE)
        })
    );
}

#[test]
fn user_ecall_lseek_seek_set_changes_read_offset_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(79);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let fetch_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu0.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core = riscv_core(0, 0, 7, "cpu0.ifetch", fetch_route, 0x8000);
    core.set_privilege_mode(RiscvPrivilegeMode::User);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, RISCV_LINUX_OPENAT as i32)),
            (0x8004, addi(10, 0, -100)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 0, 0)),
            (0x8010, addi(13, 0, 0)),
            (0x8014, 0x0000_0073),
            (0x8018, addi(5, 10, 0)),
            (0x801c, addi(17, 0, RISCV_LINUX_LSEEK as i32)),
            (0x8020, addi(10, 5, 0)),
            (0x8024, addi(11, 0, 2)),
            (0x8028, addi(12, 0, RISCV_LINUX_SEEK_SET as i32)),
            (0x802c, 0x0000_0073),
            (0x8030, addi(6, 10, 0)),
            (0x8034, addi(17, 0, RISCV_LINUX_READ as i32)),
            (0x8038, addi(10, 5, 0)),
            (0x803c, lui(11, 9)),
            (0x8040, addi(11, 11, 0x100)),
            (0x8044, addi(12, 0, 3)),
            (0x8048, 0x0000_0073),
            (0x804c, addi(7, 10, 0)),
            (0x8050, addi(17, 0, RISCV_LINUX_EXIT as i32)),
            (0x8054, addi(10, 7, 0)),
            (0x8058, 0x0000_0073),
        ],
        &[(0x9000, b"/input.txt\0"), (0x9100, b"\0\0\0")],
    );
    let controller = std::sync::Arc::new(std::sync::Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, std::sync::Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_and_guest_memory_io(
            guest_memory_reader(std::sync::Arc::clone(&store)),
            guest_memory_writer(std::sync::Arc::clone(&store)),
        );
    driver
        .riscv_syscall_emulation()
        .unwrap()
        .register_guest_file(b"/input.txt", b"abcdef");

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(std::sync::Arc::clone(&store)),
            |_cpu| responder(std::sync::Arc::clone(&store)),
            180,
            |cpu| GuestEventId::new(620 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(620), source, 3);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 3);
    assert_eq!(core.read_register(reg(6)), 2);
    assert_eq!(core.read_register(reg(7)), 3);
    assert_eq!(
        guest_memory_reader(std::sync::Arc::clone(&store))(0x9100, 3),
        Some(b"cde".to_vec())
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
