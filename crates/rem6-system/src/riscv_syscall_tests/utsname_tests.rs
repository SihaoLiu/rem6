use super::*;

const RISCV_LINUX_UNAME_FOR_TEST: u64 = 160;
const RISCV_LINUX_UTS_FIELD_BYTES_FOR_TEST: usize = 65;
const RISCV_LINUX_NEW_UTS_FIELD_COUNT_FOR_TEST: usize = 6;
const RISCV_LINUX_NEW_UTS_BYTES_FOR_TEST: usize =
    RISCV_LINUX_UTS_FIELD_BYTES_FOR_TEST * RISCV_LINUX_NEW_UTS_FIELD_COUNT_FOR_TEST;

#[test]
fn linux_table_uname_writes_full_new_utsname_with_cleared_domainname() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_UNAME_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), RISCV_LINUX_NEW_UTS_BYTES_FOR_TEST);
    let mut utsname = [b'?'; RISCV_LINUX_NEW_UTS_BYTES_FOR_TEST];
    for (address, bytes) in writes.iter() {
        let offset = usize::try_from(address.checked_sub(0x9000).unwrap()).unwrap();
        utsname[offset..offset + bytes.len()].copy_from_slice(bytes);
    }

    assert_eq!(uts_field(&utsname, 0), "Linux");
    assert_eq!(uts_field(&utsname, 1), "sim.gem5.org");
    assert_eq!(uts_field(&utsname, 2), "5.1.0");
    assert_eq!(uts_field(&utsname, 3), "#1 Mon Aug 18 11:32:15 EDT 2003");
    assert_eq!(uts_field(&utsname, 4), "riscv64");
    assert!(utsname[325..390].iter().all(|byte| *byte == 0));
}

#[test]
fn linux_table_uname_returns_efault_after_partial_guest_write_failure() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        if address == 0x9008 {
            return false;
        }
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_UNAME_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(writes.lock().unwrap().len(), 8);
}

fn uts_field(bytes: &[u8], index: usize) -> &str {
    let start = index * RISCV_LINUX_UTS_FIELD_BYTES_FOR_TEST;
    let field = &bytes[start..start + RISCV_LINUX_UTS_FIELD_BYTES_FOR_TEST];
    let len = field
        .iter()
        .position(|byte| *byte == 0)
        .expect("utsname field should be NUL terminated");
    std::str::from_utf8(&field[..len]).unwrap()
}
