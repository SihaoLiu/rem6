#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_cpu::RiscvHartRunState;
use rem6_system::{GuestTrap, GuestTrapKind, RiscvSystemRunStopReason};
use support::*;

const SBI_ERR_INVALID_PARAM: u64 = (-3_i64) as u64;
const SBI_ERR_INVALID_ADDRESS: u64 = (-5_i64) as u64;
const SBI_ERR_ALREADY_AVAILABLE: u64 = (-6_i64) as u64;
const SBI_HSM_EXTENSION: u64 = 0x0048_534d;
const SBI_IPI_EXTENSION: u64 = 0x0073_5049;
const SBI_HSM_HART_SUSPEND: i32 = 3;
const SBI_HSM_HART_SUSPENDED: u64 = 4;
const SSIP: u64 = 1 << 1;

fn reg(index: u8) -> Register {
    Register::new(index).unwrap()
}

fn lui_addi_parts(value: u64) -> (u32, i32) {
    let hi = ((value + 0x800) >> 12) as u32;
    let lo = (value as i64 - ((u64::from(hi) << 12) as i64)) as i32;
    (hi, lo)
}

fn slli(rd: u8, rs1: u8, shamt: u8) -> u32 {
    (u32::from(shamt & 0x3f) << 20)
        | (u32::from(rs1) << 15)
        | (0x1 << 12)
        | (u32::from(rd) << 7)
        | 0x13
}

fn bne(rs1: u8, rs2: u8, offset: i32) -> u32 {
    let imm = offset as u32;
    ((imm & 0x1000) << 19)
        | ((imm & 0x07e0) << 20)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (0x1 << 12)
        | ((imm & 0x001e) << 7)
        | ((imm & 0x0800) >> 4)
        | 0x63
}

fn csr_read(csr: u32, rd: u8) -> u32 {
    (csr << 20) | (0x2 << 12) | (u32::from(rd) << 7) | 0x73
}

#[test]
fn supervisor_sbi_hsm_reports_suspended_hart() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(81);
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
    core1.set_hart_suspended();
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
            |cpu| GuestEventId::new(720 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(720), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(5)), 0);
    assert_eq!(core0.read_register(reg(6)), SBI_HSM_HART_SUSPENDED);
    assert_eq!(core1.hart_run_state(), RiscvHartRunState::Suspended);
    assert_eq!(core1.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_hart_start_reports_invalid_param_for_suspended_secondary_hart() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(80);
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
    core1.set_hart_suspended();
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let (entry_hi, entry_lo) = lui_addi_parts(0x9101);
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
        (0x8028, 0x0010_0073),
        (0x9100, addi(31, 0, 12)),
        (0x9104, 0x0010_0073),
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
            |cpu| GuestEventId::new(710 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(710), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(5)), SBI_ERR_INVALID_PARAM);
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core1.hart_run_state(), RiscvHartRunState::Suspended);
    assert_eq!(core1.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_hart_start_reports_already_available_for_started_secondary_hart() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(88);
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
    core1.set_hart_started();
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let (entry_hi, entry_lo) = lui_addi_parts(0x9101);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 1)),
        (0x8010, lui(11, entry_hi)),
        (0x8014, addi(11, 11, entry_lo)),
        (0x8018, addi(12, 0, 51)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(6, 11, 0)),
        (0x8028, 0x0010_0073),
        (0x9000, 0x0000_006f),
        (0x9100, addi(31, 0, 14)),
        (0x9104, 0x0010_0073),
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
            |cpu| GuestEventId::new(790 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(790), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(5)), SBI_ERR_ALREADY_AVAILABLE);
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core1.hart_run_state(), RiscvHartRunState::Started);
    assert_eq!(core1.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_hart_start_reports_invalid_address_for_stopped_hart() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(84);
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
    let (entry_hi, entry_lo) = lui_addi_parts(0x9101);
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
        (0x8028, 0x0010_0073),
        (0x9000, addi(31, 0, 9)),
        (0x9004, 0x0000_006f),
        (0x9100, addi(31, 0, 12)),
        (0x9104, 0x0010_0073),
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
            |cpu| GuestEventId::new(750 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(750), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(5)), SBI_ERR_INVALID_ADDRESS);
    assert_eq!(core0.read_register(reg(6)), 0);
    assert_eq!(core1.hart_run_state(), RiscvHartRunState::Stopped);
    assert_eq!(core1.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_hart_start_reports_invalid_param_for_unknown_hart_before_address() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(83);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 1).unwrap();
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
    let (entry_hi, entry_lo) = lui_addi_parts(0x9001);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 9)),
        (0x8010, lui(11, entry_hi)),
        (0x8014, addi(11, 11, entry_lo)),
        (0x8018, addi(12, 0, 29)),
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
        SystemHostEventPort::with_controller(host, 1, Arc::clone(&controller)).unwrap(),
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
            |cpu| GuestEventId::new(740 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(740), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core.read_register(reg(5)), SBI_ERR_INVALID_PARAM);
    assert_eq!(core.read_register(reg(6)), 0);
}

#[test]
fn supervisor_sbi_hart_start_reports_already_available_for_started_hart() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(82);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 1).unwrap();
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
    let (entry_hi, entry_lo) = lui_addi_parts(0x9001);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, 0)),
        (0x800c, addi(10, 0, 0)),
        (0x8010, lui(11, entry_hi)),
        (0x8014, addi(11, 11, entry_lo)),
        (0x8018, addi(12, 0, 29)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(6, 11, 0)),
        (0x8028, 0x0010_0073),
        (0x9000, addi(31, 0, 13)),
        (0x9004, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 1, Arc::clone(&controller)).unwrap(),
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
            |cpu| GuestEventId::new(730 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(730), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core.read_register(reg(5)), SBI_ERR_ALREADY_AVAILABLE);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_hart_suspend_ignores_resume_addr_for_retentive_suspend() {
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
    core.set_privilege_mode(RiscvPrivilegeMode::Supervisor);
    let cluster = RiscvCluster::new([core.clone()]).unwrap();
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, SBI_HSM_HART_SUSPEND)),
        (0x800c, addi(10, 0, 0)),
        (0x8010, addi(11, 0, 1)),
        (0x8014, addi(12, 0, 91)),
        (0x8018, 0x0000_0073),
        (0x801c, addi(31, 0, 9)),
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
            120,
            |cpu| GuestEventId::new(700 + u64::from(cpu.get())),
        )
        .unwrap();

    assert!(matches!(
        run.stop_reason(),
        RiscvSystemRunStopReason::Idle { .. }
    ));
    assert_eq!(run.host_stop(), None);
    assert_eq!(core.hart_run_state(), RiscvHartRunState::Suspended);
    assert_eq!(core.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_hart_suspend_resumes_default_non_retentive_suspend_at_resume_addr() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(85);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 1).unwrap();
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
    let (resume_hi, resume_lo) = lui_addi_parts(0x9000);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, SBI_HSM_HART_SUSPEND)),
        (0x800c, addi(10, 0, 1)),
        (0x8010, slli(10, 10, 31)),
        (0x8014, lui(11, resume_hi)),
        (0x8018, addi(11, 11, resume_lo)),
        (0x801c, addi(12, 0, 91)),
        (0x8020, 0x0000_0073),
        (0x8024, addi(5, 10, 0)),
        (0x8028, addi(6, 11, 0)),
        (0x802c, addi(31, 0, 9)),
        (0x8030, 0x0010_0073),
        (0x9000, addi(30, 10, 0)),
        (0x9004, addi(29, 11, 0)),
        (0x9008, csr_read(0x180, 28)),
        (0x900c, csr_read(0x100, 27)),
        (0x9010, addi(17, 0, 16)),
        (0x9014, addi(16, 0, 0)),
        (0x9018, 0x0000_0073),
        (0x901c, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 1, Arc::clone(&controller)).unwrap(),
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
            |cpu| GuestEventId::new(760 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(760), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x901c)]
    );
    assert_eq!(core.read_register(reg(30)), 0);
    assert_eq!(core.read_register(reg(29)), 91);
    assert_eq!(core.read_register(reg(28)), 0);
    assert_eq!(core.read_register(reg(27)) & (1 << 1), 0);
    assert_eq!(core.hart_run_state(), RiscvHartRunState::Started);
    assert_eq!(core.read_register(reg(31)), 0);
}

#[test]
fn supervisor_sbi_hart_suspend_reports_invalid_param_for_reserved_suspend_type() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(86);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(4, 1).unwrap();
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
    let (resume_hi, resume_lo) = lui_addi_parts(0x9000);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, SBI_HSM_HART_SUSPEND)),
        (0x800c, addi(10, 0, 1)),
        (0x8010, lui(11, resume_hi)),
        (0x8014, addi(11, 11, resume_lo)),
        (0x8018, addi(12, 0, 91)),
        (0x801c, 0x0000_0073),
        (0x8020, addi(5, 10, 0)),
        (0x8024, addi(6, 11, 0)),
        (0x8028, addi(31, 0, 9)),
        (0x802c, 0x0010_0073),
        (0x9000, addi(30, 0, 13)),
        (0x9004, 0x0010_0073),
    ]);
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(host, 1, Arc::clone(&controller)).unwrap(),
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
            |cpu| GuestEventId::new(770 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(770), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core.read_register(reg(5)), SBI_ERR_INVALID_PARAM);
    assert_eq!(core.read_register(reg(6)), 0);
    assert_eq!(core.hart_run_state(), RiscvHartRunState::Started);
    assert_eq!(core.read_register(reg(31)), 9);
    assert_eq!(core.read_register(reg(30)), 0);
}

#[test]
fn supervisor_sbi_ipi_wakes_retentive_suspended_hart() {
    let host = PartitionId::new(3);
    let source = GuestSourceId::new(87);
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
    core1.set_hart_started();
    let cluster = RiscvCluster::new([core0.clone(), core1.clone()]).unwrap();
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let (ipi_hi, ipi_lo) = lui_addi_parts(SBI_IPI_EXTENSION);
    let store = loaded_program_store(&[
        (0x8000, lui(17, hsm_hi)),
        (0x8004, addi(17, 17, hsm_lo)),
        (0x8008, addi(16, 0, 2)),
        (0x800c, addi(10, 0, 1)),
        (0x8010, 0x0000_0073),
        (0x8014, addi(6, 0, SBI_HSM_HART_SUSPENDED as i32)),
        (0x8018, bne(11, 6, -0x10)),
        (0x801c, lui(17, ipi_hi)),
        (0x8020, addi(17, 17, ipi_lo)),
        (0x8024, addi(16, 0, 0)),
        (0x8028, addi(10, 0, 2)),
        (0x802c, addi(11, 0, 0)),
        (0x8030, 0x0000_0073),
        (0x8034, addi(30, 10, 0)),
        (0x8038, addi(29, 11, 0)),
        (0x803c, 0x0000_006f),
        (0x9000, lui(17, hsm_hi)),
        (0x9004, addi(17, 17, hsm_lo)),
        (0x9008, addi(16, 0, SBI_HSM_HART_SUSPEND)),
        (0x900c, addi(10, 0, 0)),
        (0x9010, addi(11, 0, 0)),
        (0x9014, addi(12, 0, 91)),
        (0x9018, 0x0000_0073),
        (0x901c, addi(31, 0, 21)),
        (0x9020, addi(28, 10, 0)),
        (0x9024, addi(27, 11, 0)),
        (0x9028, 0x0010_0073),
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
            480,
            |cpu| GuestEventId::new(780 + u64::from(cpu.get())),
        )
        .unwrap();

    let stop = StopRequest::new(run.final_tick().unwrap(), GuestEventId::new(781), source, 1);
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(core0.read_register(reg(30)), 0);
    assert_eq!(core0.read_register(reg(29)), 0);
    assert_eq!(core1.hart_run_state(), RiscvHartRunState::Started);
    assert_eq!(core1.machine_interrupt_pending() & SSIP, SSIP);
    assert_eq!(core1.read_register(reg(31)), 21);
    assert_eq!(core1.read_register(reg(28)), 0);
    assert_eq!(core1.read_register(reg(27)), 0);
}
