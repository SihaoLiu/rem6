use super::*;

const RISCV_LINUX_PRCTL_FOR_TEST: u64 = 167;
const RISCV_LINUX_PR_SET_PDEATHSIG_FOR_TEST: u64 = 1;
const RISCV_LINUX_PR_GET_PDEATHSIG_FOR_TEST: u64 = 2;
const RISCV_LINUX_PR_GET_DUMPABLE_FOR_TEST: u64 = 3;
const RISCV_LINUX_PR_SET_DUMPABLE_FOR_TEST: u64 = 4;

#[test]
fn linux_table_prctl_pdeathsig_round_trips_state_and_validates_signal_range() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });
    let faulting_writer = RiscvGuestMemoryWriter::new(|_, _| false);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_PDEATHSIG_FOR_TEST, 0x9000, 1, 2, 3, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(0x9000, 0_i32.to_le_bytes().to_vec())]
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_SET_PDEATHSIG_FOR_TEST, 10, 1, 2, 3, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_PDEATHSIG_FOR_TEST, 0x9004, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(0x9004, 10_i32.to_le_bytes().to_vec())]
    );
    writes.lock().unwrap().clear();

    for (pc, signal) in [
        (0x800c, (-1_i64) as u64),
        (0x8010, 65),
        (0x8014, 0x1_0000_000a),
        (0x8018, 0x1_0000_0000),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_PRCTL_FOR_TEST,
                    [RISCV_LINUX_PR_SET_PDEATHSIG_FOR_TEST, signal, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x801c,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_PDEATHSIG_FOR_TEST, 0x9008, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(0x9008, 10_i32.to_le_bytes().to_vec())]
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8020,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_SET_PDEATHSIG_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8024,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_PDEATHSIG_FOR_TEST, 0x900c, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(0x900c, 0_i32.to_le_bytes().to_vec())]
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8028,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_PDEATHSIG_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_prctl_dumpable_round_trips_state_and_rejects_invalid_values() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8100,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_DUMPABLE_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8104,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_SET_DUMPABLE_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8108,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_DUMPABLE_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    for (pc, value) in [(0x810c, 2), (0x8110, (-1_i64) as u64)] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_PRCTL_FOR_TEST,
                    [RISCV_LINUX_PR_SET_DUMPABLE_FOR_TEST, value, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8114,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_DUMPABLE_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8118,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_SET_DUMPABLE_FOR_TEST, 1, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x811c,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_DUMPABLE_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert!(state.unknown_syscalls().is_empty());
}
