#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{GuestTrap, GuestTrapKind};
use support::*;

const SBI_BASE_EXTENSION: u64 = 0x10;
const SBI_BASE_GET_SPEC_VERSION: i32 = 0;
const SBI_BASE_PROBE_EXTENSION: i32 = 3;
const SBI_DEBUG_CONSOLE_EXTENSION: u64 = 0x4442_434e;
const SBI_DEBUG_CONSOLE_WRITE_BYTE: i32 = 2;
const SBI_SPEC_VERSION_2_0: u64 = 2 << 24;

fn lui_addi_parts(value: u64) -> (u32, i32) {
    let hi = ((value + 0x800) >> 12) as u32;
    let lo = (value as i64 - ((u64::from(hi) << 12) as i64)) as i32;
    (hi, lo)
}

#[test]
fn supervisor_sbi_debug_console_write_byte_records_output() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(71);
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let (dbcn_hi, dbcn_lo) = lui_addi_parts(SBI_DEBUG_CONSOLE_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, SBI_BASE_EXTENSION as i32)),
        (0x8004, addi(16, 0, SBI_BASE_GET_SPEC_VERSION)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(6, 11, 0)),
        (0x8014, lui(17, dbcn_hi)),
        (0x8018, addi(17, 17, dbcn_lo)),
        (0x801c, addi(16, 0, SBI_DEBUG_CONSOLE_WRITE_BYTE)),
        (0x8020, addi(10, 0, i32::from(b'A'))),
        (0x8024, 0x0000_0073),
        (0x8028, addi(7, 10, 0)),
        (0x802c, addi(8, 11, 0)),
        (0x8030, addi(17, 0, SBI_BASE_EXTENSION as i32)),
        (0x8034, addi(16, 0, SBI_BASE_PROBE_EXTENSION)),
        (0x8038, lui(10, dbcn_hi)),
        (0x803c, addi(10, 10, dbcn_lo)),
        (0x8040, 0x0000_0073),
        (0x8044, addi(9, 11, 0)),
        (0x8048, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 2, Arc::clone(&controller)).unwrap(),
        source,
    );
    let driver = RiscvSystemRunDriver::new(trap_port)
        .with_riscv_sbi_firmware()
        .with_riscv_syscall_emulation();

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            120,
            |cpu| GuestEventId::new(710 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(710), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8048)]
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), SBI_SPEC_VERSION_2_0);
    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.read_register(reg(8)), 0);
    assert_eq!(core.read_register(reg(9)), 0);
    assert_eq!(
        driver.riscv_sbi_firmware().unwrap().debug_console_bytes(),
        b"A"
    );
    assert_eq!(run.riscv_debug_console_bytes(), b"A");
}
