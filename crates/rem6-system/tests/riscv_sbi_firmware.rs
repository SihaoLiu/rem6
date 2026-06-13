#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_cpu::RiscvHartRunState;
use rem6_system::RiscvCoreCheckpointPort;
use rem6_system::{
    GuestEvent, GuestEventDelivery, GuestEventKind, GuestTrap, GuestTrapKind,
    RiscvSystemRunStopReason,
};
use support::*;

const REM6_SBI_IMPL_ID: u64 = 0x7265_6d36;
const SBI_SPEC_VERSION_0_3: u64 = 3;
const SBI_HSM_EXTENSION: u64 = 0x0048_534d;
const SBI_IPI_EXTENSION: u64 = 0x0073_5049;
const SBI_RFENCE_EXTENSION: u64 = 0x5246_4e43;
const SBI_SRST_EXTENSION: u64 = 0x5352_5354;
const SBI_HSM_HART_STOP: i32 = 1;
const SBI_HSM_HART_STARTED: u64 = 0;
const SBI_HSM_HART_STOPPED: u64 = 1;
const SSIP: u64 = 1 << 1;
const STIP: u64 = 1 << 5;

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn lui_addi_parts(value: u64) -> (u32, i32) {
    let hi = ((value + 0x800) >> 12) as u32;
    let lo = (value as i64 - ((u64::from(hi) << 12) as i64)) as i32;
    (hi, lo)
}

fn bne(rs1: u8, rs2: u8, offset: i32) -> u32 {
    let imm = offset as u32;
    (((imm >> 12) & 0x1) << 31)
        | (((imm >> 5) & 0x3f) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0x1 << 12)
        | (((imm >> 1) & 0xf) << 8)
        | (((imm >> 11) & 0x1) << 7)
        | 0x63
}

#[test]
fn supervisor_sbi_base_get_spec_version_returns_before_user_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(61);
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
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x10)),
        (0x8004, addi(16, 0, 0)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(6, 11, 0)),
        (0x8014, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(360 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(360), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8014)]
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), SBI_SPEC_VERSION_0_3);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn supervisor_sbi_unknown_extension_returns_not_supported_before_user_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(62);
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
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x55)),
        (0x8004, addi(16, 0, 0)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(6, 11, 0)),
        (0x8014, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(380 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(380), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8014)]
    );
    assert_eq!(core.read_register(reg(5)), (-2_i64) as u64);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.read_register(reg(10)), (-2_i64) as u64);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn supervisor_sbi_base_identity_and_probe_calls_return_success() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(63);
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
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x10)),
        (0x8004, addi(16, 0, 1)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, addi(6, 11, 0)),
        (0x8014, addi(16, 0, 2)),
        (0x8018, 0x0000_0073),
        (0x801c, addi(7, 10, 0)),
        (0x8020, addi(8, 11, 0)),
        (0x8024, addi(10, 0, 0x10)),
        (0x8028, addi(16, 0, 3)),
        (0x802c, 0x0000_0073),
        (0x8030, addi(9, 10, 0)),
        (0x8034, addi(12, 11, 0)),
        (0x8038, lui(10, (SBI_SRST_EXTENSION >> 12) as u32)),
        (0x803c, addi(10, 10, (SBI_SRST_EXTENSION & 0x0fff) as i32)),
        (0x8040, addi(16, 0, 3)),
        (0x8044, 0x0000_0073),
        (0x8048, addi(13, 10, 0)),
        (0x804c, addi(14, 11, 0)),
        (0x8050, addi(10, 0, 0x11)),
        (0x8054, addi(16, 0, 3)),
        (0x8058, 0x0000_0073),
        (0x805c, addi(15, 10, 0)),
        (0x8060, addi(18, 11, 0)),
        (0x8064, addi(16, 0, 4)),
        (0x8068, 0x0000_0073),
        (0x806c, addi(19, 10, 0)),
        (0x8070, addi(20, 11, 0)),
        (0x8074, addi(16, 0, 5)),
        (0x8078, 0x0000_0073),
        (0x807c, addi(21, 10, 0)),
        (0x8080, addi(22, 11, 0)),
        (0x8084, addi(16, 0, 6)),
        (0x8088, 0x0000_0073),
        (0x808c, addi(23, 10, 0)),
        (0x8090, addi(24, 11, 0)),
        (0x8094, 0x0010_0073),
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
            200,
            |cpu| GuestEventId::new(400 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(400), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8094)]
    );
    for register in [5, 7, 9, 13, 15, 19, 21, 23] {
        assert_eq!(core.read_register(reg(register)), 0);
    }
    assert_eq!(core.read_register(reg(6)), REM6_SBI_IMPL_ID);
    assert_eq!(core.read_register(reg(8)), 0);
    assert_eq!(core.read_register(reg(12)), 1);
    assert_eq!(core.read_register(reg(14)), 1);
    assert_eq!(core.read_register(reg(18)), 0);
    assert_eq!(core.read_register(reg(20)), 0);
    assert_eq!(core.read_register(reg(22)), 0);
    assert_eq!(core.read_register(reg(24)), 0);
}

#[test]
fn supervisor_sbi_base_probe_reports_rfence_extension() {
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
    let (rfence_hi, rfence_lo) = lui_addi_parts(SBI_RFENCE_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x10)),
        (0x8004, addi(16, 0, 3)),
        (0x8008, lui(10, rfence_hi)),
        (0x800c, addi(10, 10, rfence_lo)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(5, 10, 0)),
        (0x8018, addi(6, 11, 0)),
        (0x801c, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(560 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(560), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x801c)]
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 1);
}

#[test]
fn supervisor_sbi_base_probe_reports_hsm_extension() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(73);
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
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x10)),
        (0x8004, addi(16, 0, 3)),
        (0x8008, lui(10, hsm_hi)),
        (0x800c, addi(10, 10, hsm_lo)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(5, 10, 0)),
        (0x8018, addi(6, 11, 0)),
        (0x801c, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(600 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(600), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 1);
}

#[test]
fn supervisor_sbi_hsm_reports_started_and_stopped_harts() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(74);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
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
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, 2)),
        (0x800c, addi(10, 0, 0)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(5, 10, 0)),
        (0x8018, addi(6, 11, 0)),
        (0x801c, addi(10, 0, 1)),
        (0x8020, 0x0000_0073),
        (0x8024, addi(7, 10, 0)),
        (0x8028, addi(8, 11, 0)),
        (0x802c, 0x0010_0073),
        (0x9000, addi(31, 0, 9)),
        (0x9004, 0x0000_006f),
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
            160,
            |cpu| GuestEventId::new(620 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(620), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(5)), 0);
    assert_eq!(core0.read_register(reg(6)), SBI_HSM_HART_STARTED);
    assert_eq!(core0.read_register(reg(7)), 0);
    assert_eq!(core0.read_register(reg(8)), SBI_HSM_HART_STOPPED);
    assert_eq!(core1.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_hart_start_releases_secondary_with_entry_and_opaque() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(75);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
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
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let (entry_hi, entry_lo) = lui_addi_parts(0x9100);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 1)),
        (0x8010, lui(11, entry_hi)),
        (0x8014, addi(11, 11, entry_lo)),
        (0x8018, addi(12, 0, 85)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(6, 11, 0)),
        (0x8028, bne(5, 0, 8)),
        (0x802c, 0x0000_006f),
        (0x8030, 0x0010_0073),
        (0x9000, addi(31, 0, 9)),
        (0x9004, 0x0000_006f),
        (0x9100, addi(31, 0, 12)),
        (0x9104, addi(30, 10, 0)),
        (0x9108, addi(29, 11, 0)),
        (0x910c, 0x0010_0073),
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
            240,
            |cpu| GuestEventId::new(640 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(641), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(5)), 0);
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core1.read_register(reg(31)), 12);
    assert_eq!(core1.read_register(reg(30)), 1);
    assert_eq!(core1.read_register(reg(29)), 85);
}

#[test]
fn supervisor_sbi_registration_preserves_restored_started_secondary_hart() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(76);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
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
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);

    let component = CheckpointComponentId::new("cpu1").unwrap();
    let port = RiscvCoreCheckpointPort::new(component, core1.clone());
    let mut registry = CheckpointRegistry::new();
    port.register(&mut registry).unwrap();
    let captured = port.capture_into(&mut registry).unwrap();
    assert_eq!(captured.hart_run_state(), RiscvHartRunState::Started);
    core1.set_hart_stopped();
    let restored = port.restore_from(&registry).unwrap();
    assert_eq!(restored, captured);
    assert_eq!(core1.hart_run_state(), RiscvHartRunState::Started);

    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let store = loaded_program_store(&[(0x8000, 0x0000_006f), (0x9000, 0x0010_0073)]);
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
            |cpu| GuestEventId::new(660 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(661), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core1.hart_run_state(), RiscvHartRunState::Started);
}

#[test]
fn supervisor_sbi_registration_stops_fresh_secondary_hart_after_prior_run() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(77);
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

    let mut warm_scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut warm_transport = MemoryTransport::new();
    let warm_route = warm_transport
        .add_route(
            MemoryRoute::new(
                endpoint("warm.ifetch"),
                PartitionId::new(0),
                endpoint("warm.l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let warm_core = riscv_core(0, 0, 7, "warm.ifetch", warm_route, 0x8000);
    warm_core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let warm_cluster = RiscvCluster::new([warm_core]).unwrap();
    let warm_store = loaded_program_store(&[(0x8000, 0x0000_006f)]);

    let warmup = driver.drive_until_host_stop(
        &warm_cluster,
        &mut warm_scheduler,
        &warm_transport,
        MemoryTrace::new(),
        MemoryTrace::new(),
        |_cpu| responder(Arc::clone(&warm_store)),
        |_cpu| responder(Arc::clone(&warm_store)),
        1,
        |cpu| GuestEventId::new(670 + u64::from(cpu.get())),
    );
    assert!(warmup.is_err());

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
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
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, 2)),
        (0x800c, addi(10, 0, 1)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(5, 10, 0)),
        (0x8018, addi(6, 11, 0)),
        (0x801c, 0x0010_0073),
        (0x9000, addi(31, 0, 9)),
        (0x9004, 0x0000_006f),
    ]);

    let run = driver
        .drive_until_host_stop(
            &cluster,
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            MemoryTrace::new(),
            |_cpu| responder(Arc::clone(&store)),
            |_cpu| responder(Arc::clone(&store)),
            160,
            |cpu| GuestEventId::new(680 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(680), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(5)), 0);
    assert_eq!(core0.read_register(reg(6)), SBI_HSM_HART_STOPPED);
    assert_eq!(core1.read_register(reg(31)), 0);
    assert_eq!(core1.hart_run_state(), RiscvHartRunState::Stopped);
}

#[test]
fn supervisor_sbi_hart_stop_stops_current_hart_before_next_instruction() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(78);
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
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, SBI_HSM_HART_STOP)),
        (0x800c, 0x0000_0073),
        (0x8010, addi(31, 0, 9)),
        (0x8014, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(690 + u64::from(cpu.get())),
        )
        .unwrap();

    assert!(matches!(
        run.stop_reason(),
        RiscvSystemRunStopReason::Idle { .. }
    ));
    assert_eq!(run.host_stop(), None);
    assert_eq!(core.hart_run_state(), RiscvHartRunState::Stopped);
    assert_eq!(core.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_time_set_timer_clears_and_reasserts_stip() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(64);
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
    core.set_machine_interrupt_pending(STIP);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, lui(17, 0x54495)),
        (0x8004, addi(17, 17, -699)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 2000)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(5, 10, 0)),
        (0x8018, addi(6, 11, 0)),
        (0x801c, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(420 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(420), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x801c)]
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.machine_interrupt_pending() & STIP, 0);
    assert_eq!(
        driver
            .riscv_sbi_firmware()
            .unwrap()
            .timer_deadline(CpuId::new(0)),
        Some(2000)
    );
    assert_eq!(
        scheduler.next_pending_tick(PartitionId::new(0)).unwrap(),
        Some(2000)
    );

    let timer_summary = scheduler.run_until_idle();

    assert_eq!(timer_summary.final_tick(), 2000);
    assert_eq!(core.machine_interrupt_pending() & STIP, STIP);
}

#[test]
fn supervisor_sbi_send_ipi_sets_target_hart_ssip() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(65);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
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
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, lui(17, (SBI_IPI_EXTENSION >> 12) as u32)),
        (0x8004, addi(17, 17, (SBI_IPI_EXTENSION & 0x0fff) as i32)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 2)),
        (0x8010, addi(11, 0, 0)),
        (0x8014, 0x0000_0073),
        (0x8018, addi(5, 10, 0)),
        (0x801c, addi(6, 11, 0)),
        (0x8020, 0x0010_0073),
        (0x9000, 0x0000_006f),
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
            80,
            |cpu| GuestEventId::new(440 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(440), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8020)]
    );
    assert_eq!(core0.read_register(reg(5)), 0);
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core0.machine_interrupt_pending() & SSIP, 0);
    assert_eq!(core1.machine_interrupt_pending() & SSIP, SSIP);
}

#[test]
fn supervisor_sbi_send_ipi_uses_hart_mask_base() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(66);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu7_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu7.ifetch"),
                PartitionId::new(0),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let cpu9_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu9.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core7 = riscv_core(7, 0, 7, "cpu7.ifetch", cpu7_route, 0x8000);
    let core9 = riscv_core(9, 1, 8, "cpu9.ifetch", cpu9_route, 0x9000);
    core7.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core9.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core7.clone(), core9.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, lui(17, (SBI_IPI_EXTENSION >> 12) as u32)),
        (0x8004, addi(17, 17, (SBI_IPI_EXTENSION & 0x0fff) as i32)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 4)),
        (0x8010, addi(11, 0, 7)),
        (0x8014, 0x0000_0073),
        (0x8018, addi(5, 10, 0)),
        (0x801c, addi(6, 11, 0)),
        (0x8020, 0x0010_0073),
        (0x9000, 0x0000_006f),
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
            80,
            |cpu| GuestEventId::new(460 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(467), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core7.read_register(reg(5)), 0);
    assert_eq!(core7.read_register(reg(6)), 0);
    assert_eq!(core7.machine_interrupt_pending() & SSIP, 0);
    assert_eq!(core9.machine_interrupt_pending() & SSIP, SSIP);
}

#[test]
fn supervisor_sbi_send_ipi_rejects_missing_target_without_partial_ssip() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(67);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let cpu0_route = transport
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
    let cpu1_route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("cpu1.ifetch"),
                PartitionId::new(1),
                endpoint("l1i"),
                PartitionId::new(2),
                2,
                3,
            )
            .unwrap(),
        )
        .unwrap();
    let core0 = riscv_core(0, 0, 7, "cpu0.ifetch", cpu0_route, 0x8000);
    let core1 = riscv_core(1, 1, 8, "cpu1.ifetch", cpu1_route, 0x9000);
    core0.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    core1.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, lui(17, (SBI_IPI_EXTENSION >> 12) as u32)),
        (0x8004, addi(17, 17, (SBI_IPI_EXTENSION & 0x0fff) as i32)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 6)),
        (0x8010, addi(11, 0, 0)),
        (0x8014, 0x0000_0073),
        (0x8018, addi(5, 10, 0)),
        (0x801c, addi(6, 11, 0)),
        (0x8020, 0x0010_0073),
        (0x9000, 0x0000_006f),
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
            80,
            |cpu| GuestEventId::new(480 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(480), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(5)), (-3_i64) as u64);
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core0.machine_interrupt_pending() & SSIP, 0);
    assert_eq!(core1.machine_interrupt_pending() & SSIP, 0);
}

#[test]
fn supervisor_sbi_remote_sfence_vma_rejects_missing_target() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(72);
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
    let (rfence_hi, rfence_lo) = lui_addi_parts(SBI_RFENCE_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, lui(17, rfence_hi)),
        (0x8004, addi(17, 17, rfence_lo)),
        (0x8008, addi(16, 0, 1)),
        (0x800c, addi(10, 0, 2)),
        (0x8010, addi(11, 0, 0)),
        (0x8014, addi(12, 0, 0)),
        (0x8018, addi(13, 0, 0)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(6, 11, 0)),
        (0x8028, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(580 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(580), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8028)]
    );
    assert_eq!(core.read_register(reg(5)), (-3_i64) as u64);
    assert_eq!(core.read_register(reg(6)), 0);
}

#[test]
fn supervisor_sbi_system_reset_shutdown_stops_without_returning() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(68);
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
    let store = loaded_program_store(&[
        (0x8000, lui(17, (SBI_SRST_EXTENSION >> 12) as u32)),
        (0x8004, addi(17, 17, (SBI_SRST_EXTENSION & 0x0fff) as i32)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 0)),
        (0x8010, addi(11, 0, 0)),
        (0x8014, 0x0000_0073),
        (0x8018, addi(5, 10, 0)),
        (0x801c, addi(6, 11, 0)),
        (0x8020, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(500 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(500), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(
        controller.lock().unwrap().run().deliveries(),
        &[GuestEventDelivery::new(
            stop.tick(),
            PartitionId::new(0),
            host,
            GuestEvent::new(
                GuestEventId::new(500),
                source,
                GuestEventKind::SystemReset {
                    reset_type: 0,
                    reset_reason: 0,
                    code: 0,
                },
            ),
        )]
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn supervisor_sbi_system_reset_system_failure_stops_with_failure_code() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(70);
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
    let store = loaded_program_store(&[
        (0x8000, lui(17, (SBI_SRST_EXTENSION >> 12) as u32)),
        (0x8004, addi(17, 17, (SBI_SRST_EXTENSION & 0x0fff) as i32)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 0)),
        (0x8010, addi(11, 0, 1)),
        (0x8014, 0x0000_0073),
        (0x8018, addi(5, 10, 0)),
        (0x801c, addi(6, 11, 0)),
        (0x8020, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(540 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(540), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert!(run.scheduled_traps().is_empty());
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(
        controller.lock().unwrap().run().deliveries(),
        &[GuestEventDelivery::new(
            stop.tick(),
            PartitionId::new(0),
            host,
            GuestEvent::new(
                GuestEventId::new(540),
                source,
                GuestEventKind::SystemReset {
                    reset_type: 0,
                    reset_reason: 1,
                    code: 1,
                },
            ),
        )]
    );
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn supervisor_sbi_system_reset_rejects_reserved_type_before_user_exit() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(69);
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
    let store = loaded_program_store(&[
        (0x8000, lui(17, (SBI_SRST_EXTENSION >> 12) as u32)),
        (0x8004, addi(17, 17, (SBI_SRST_EXTENSION & 0x0fff) as i32)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 3)),
        (0x8010, addi(11, 0, 0)),
        (0x8014, 0x0000_0073),
        (0x8018, addi(5, 10, 0)),
        (0x801c, addi(6, 11, 0)),
        (0x8020, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(520 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(520), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8020)]
    );
    assert_eq!(core.read_register(reg(5)), (-3_i64) as u64);
    assert_eq!(core.read_register(reg(6)), 0);
}

#[test]
fn delegated_supervisor_environment_call_is_not_consumed_as_sbi() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(63);
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
    core.set_machine_exception_delegation(1 << 9);
    core.set_supervisor_trap_vector(0x9000);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let store = loaded_program_store(&[
        (0x8000, addi(17, 0, 0x10)),
        (0x8004, addi(16, 0, 0)),
        (0x8008, 0x0000_0073),
        (0x800c, addi(5, 10, 0)),
        (0x8010, 0x0010_0073),
        (0x9000, 0x0010_0073),
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
            80,
            |cpu| GuestEventId::new(400 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(400), source, 0);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::EnvironmentCall, 0x8008)]
    );
    assert_eq!(core.pc().get(), 0x9000);
    assert_eq!(core.read_register(reg(5)), 0);
}
