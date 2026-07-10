use super::riscv_syscall_emulation_support::*;
use rem6_system::{
    GuestFd, RiscvGuestMemoryReader, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RiscvSyscallTable,
};

const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_F_GETFD: u64 = 1;
const RISCV_LINUX_EXIT: u64 = 93;
const RISCV_LINUX_OPENAT2: u64 = 437;
const RISCV_LINUX_AT_FDCWD: u64 = (-100_i64) as u64;
const RISCV_LINUX_O_CREAT: u64 = 0o100;
const RISCV_LINUX_O_CLOEXEC: u64 = 0o2_000_000;
const RISCV_PAGE_BYTES: u64 = 4096;
const RISCV_LINUX_E2BIG: u64 = 7;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;

fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}

fn open_how(flags: u64, mode: u64, resolve: u64) -> [u8; 24] {
    let mut bytes = [0; 24];
    bytes[..8].copy_from_slice(&flags.to_le_bytes());
    bytes[8..16].copy_from_slice(&mode.to_le_bytes());
    bytes[16..].copy_from_slice(&resolve.to_le_bytes());
    bytes
}

fn memory_with_openat2_path_and_how(how: &[u8]) -> Arc<Mutex<rem6_memory::PartitionedMemoryStore>> {
    loaded_program_store_with_data(&[(0x8000, 0)], &[(0x9000, b"/input.txt\0"), (0x9100, how)])
}

fn handle_openat2(
    state: &mut RiscvSyscallState,
    reader: &RiscvGuestMemoryReader,
    how_size: u64,
) -> Option<RiscvSyscallOutcome> {
    handle_openat2_at(state, reader, 0x9100, how_size)
}

fn handle_openat2_at(
    state: &mut RiscvSyscallState,
    reader: &RiscvGuestMemoryReader,
    how_address: u64,
    how_size: u64,
) -> Option<RiscvSyscallOutcome> {
    RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8000,
            RISCV_LINUX_OPENAT2,
            [RISCV_LINUX_AT_FDCWD, 0x9000, how_address, how_size, 0, 0],
        ),
        state,
        0,
        Some(reader),
        None,
    )
}

#[test]
fn linux_table_openat2_reads_guest_open_how_and_records_open() {
    let how = open_how(RISCV_LINUX_O_CLOEXEC, 0, 0);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");

    assert_eq!(
        handle_openat2(&mut state, &reader, how.len() as u64),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );

    let fd = GuestFd::new(3).unwrap();
    assert!(state.guest_fds().close_on_exec(fd).unwrap());
    assert_eq!(state.guest_opens().len(), 1);
    assert_eq!(state.guest_opens()[0].path(), b"/input.txt");
    assert_eq!(state.guest_opens()[0].flags(), RISCV_LINUX_O_CLOEXEC);
}

#[test]
fn linux_table_openat2_rejects_short_open_how_without_opening() {
    let how = open_how(RISCV_LINUX_O_CLOEXEC, 0, 0);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");

    assert_eq!(
        handle_openat2(&mut state, &reader, 16),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.guest_opens().is_empty());
}

#[test]
fn linux_table_openat2_rejects_oversized_open_how_without_opening() {
    let how = open_how(RISCV_LINUX_O_CLOEXEC, 0, 0);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");

    assert_eq!(
        handle_openat2(&mut state, &reader, RISCV_PAGE_BYTES + 1),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_E2BIG)
        })
    );
    assert!(state.guest_opens().is_empty());
}

#[test]
fn linux_table_openat2_faults_when_open_how_is_unreadable() {
    let store = loaded_program_store_with_data(&[(0x8000, 0)], &[(0x9000, b"/input.txt\0")]);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");

    assert_eq!(
        handle_openat2_at(&mut state, &reader, 0xa000, 24),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.guest_opens().is_empty());
}

#[test]
fn linux_table_openat2_accepts_zero_extended_open_how() {
    let mut how = open_how(RISCV_LINUX_O_CLOEXEC, 0, 0).to_vec();
    how.extend_from_slice(&[0; 8]);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");

    assert_eq!(
        handle_openat2(&mut state, &reader, how.len() as u64),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(state.guest_opens().len(), 1);
}

#[test]
fn linux_table_openat2_rejects_nonzero_extended_open_how_without_opening() {
    let mut how = open_how(RISCV_LINUX_O_CLOEXEC, 0, 0).to_vec();
    how.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 0]);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");

    assert_eq!(
        handle_openat2(&mut state, &reader, how.len() as u64),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_E2BIG)
        })
    );
    assert!(state.guest_opens().is_empty());
}

#[test]
fn linux_table_openat2_rejects_resolve_policy_without_opening() {
    let how = open_how(RISCV_LINUX_O_CLOEXEC, 0, 1);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");

    assert_eq!(
        handle_openat2(&mut state, &reader, how.len() as u64),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.guest_opens().is_empty());
}

#[test]
fn linux_table_openat2_rejects_mode_without_create_flag() {
    let how = open_how(RISCV_LINUX_O_CLOEXEC, 0o644, 0);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"abcdef");

    assert_eq!(
        handle_openat2(&mut state, &reader, how.len() as u64),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.guest_opens().is_empty());
}

#[test]
fn linux_table_openat2_allows_mode_with_create_flag() {
    let how = open_how(RISCV_LINUX_O_CREAT | RISCV_LINUX_O_CLOEXEC, 0o644, 0);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_openat2(&mut state, &reader, how.len() as u64),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(state.guest_opens().len(), 1);
    assert_eq!(state.guest_opens()[0].mode(), 0o644);
}

#[test]
fn linux_table_openat2_rejects_invalid_mode_bits() {
    let how = open_how(RISCV_LINUX_O_CREAT | RISCV_LINUX_O_CLOEXEC, 0o100000, 0);
    let store = memory_with_openat2_path_and_how(&how);
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        handle_openat2(&mut state, &reader, how.len() as u64),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.guest_opens().is_empty());
}

#[test]
fn user_ecall_openat2_sets_close_on_exec_before_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(82);
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
    let how = open_how(RISCV_LINUX_O_CLOEXEC, 0, 0);
    let store = loaded_program_store_with_data(
        &[
            (0x8000, addi(17, 0, RISCV_LINUX_OPENAT2 as i32)),
            (0x8004, addi(10, 0, -100)),
            (0x8008, lui(11, 9)),
            (0x800c, addi(12, 11, 0x100)),
            (0x8010, addi(13, 0, how.len() as i32)),
            (0x8014, 0x0000_0073),
            (0x8018, addi(5, 10, 0)),
            (0x801c, addi(17, 0, RISCV_LINUX_FCNTL as i32)),
            (0x8020, addi(10, 5, 0)),
            (0x8024, addi(11, 0, RISCV_LINUX_F_GETFD as i32)),
            (0x8028, addi(12, 0, 0)),
            (0x802c, 0x0000_0073),
            (0x8030, addi(6, 10, 0)),
            (0x8034, addi(17, 0, RISCV_LINUX_EXIT as i32)),
            (0x8038, addi(10, 0, 0)),
            (0x803c, 0x0000_0073),
        ],
        &[(0x9000, b"/input.txt\0"), (0x9100, &how)],
    );
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_syscall_emulation_and_guest_memory_reader(guest_memory_reader(Arc::clone(
            &store,
        )));
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
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            150,
            |cpu| GuestEventId::new(540 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(540), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 3);
    assert_eq!(core.read_register(reg(6)), 1);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}
