#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{GuestTrap, GuestTrapKind, RiscvSystemRun};
use support::*;

const REM6_SBI_IMPL_ID: u64 = 0x7265_6d36;
const SBI_SPEC_VERSION_2_0: u64 = 2 << 24;
const SBI_BASE_EXTENSION: u64 = 0x10;
const SBI_BASE_GET_SPEC_VERSION: i32 = 0;
const SBI_BASE_GET_IMPL_ID: i32 = 1;
const SBI_BASE_GET_IMPL_VERSION: i32 = 2;
const SBI_BASE_PROBE_EXTENSION: i32 = 3;
const SBI_BASE_GET_MVENDORID: i32 = 4;
const SBI_BASE_GET_MARCHID: i32 = 5;
const SBI_BASE_GET_MIMPID: i32 = 6;
const SBI_HSM_EXTENSION: u64 = 0x0048_534d;
const SBI_RFENCE_EXTENSION: u64 = 0x5246_4e43;
const SBI_SRST_EXTENSION: u64 = 0x5352_5354;

fn lui_addi_parts(value: u64) -> (u32, i32) {
    let hi = ((value + 0x800) >> 12) as u32;
    let lo = (value as i64 - ((u64::from(hi) << 12) as i64)) as i32;
    (hi, lo)
}

fn run_single_core_sbi_program(
    instructions: &[(u64, u32)],
    source_event_base: u64,
    max_turns: usize,
) -> (RiscvCore, RiscvSystemRun, Arc<Mutex<SystemHostController>>) {
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
    let store = loaded_program_store(instructions);
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
            max_turns,
            |cpu| GuestEventId::new(source_event_base + u64::from(cpu.get())),
        )
        .unwrap();

    (core, run, controller)
}

#[test]
fn supervisor_sbi_base_get_spec_version_returns_sbi_2_0() {
    let source = GuestSourceId::new(61);
    let (core, run, controller) = run_single_core_sbi_program(
        &[
            (0x8000, addi(17, 0, SBI_BASE_EXTENSION as i32)),
            (0x8004, addi(16, 0, SBI_BASE_GET_SPEC_VERSION)),
            (0x8008, 0x0000_0073),
            (0x800c, addi(5, 10, 0)),
            (0x8010, addi(6, 11, 0)),
            (0x8014, 0x0010_0073),
        ],
        360,
        80,
    );

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
    assert_eq!(core.read_register(reg(6)), SBI_SPEC_VERSION_2_0);
    assert_eq!(core.read_register(reg(10)), 0);
    assert_eq!(
        controller.lock().unwrap().run().action_outcomes(),
        &[SystemActionOutcome::Stop(stop)]
    );
}

#[test]
fn supervisor_sbi_unknown_extension_returns_not_supported_before_user_exit() {
    let source = GuestSourceId::new(61);
    let (core, run, controller) = run_single_core_sbi_program(
        &[
            (0x8000, addi(17, 0, 0x55)),
            (0x8004, addi(16, 0, 0)),
            (0x8008, 0x0000_0073),
            (0x800c, addi(5, 10, 0)),
            (0x8010, addi(6, 11, 0)),
            (0x8014, 0x0010_0073),
        ],
        380,
        80,
    );

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
    let (srst_hi, srst_lo) = lui_addi_parts(SBI_SRST_EXTENSION);
    let (core, run, _) = run_single_core_sbi_program(
        &[
            (0x8000, addi(17, 0, SBI_BASE_EXTENSION as i32)),
            (0x8004, addi(16, 0, SBI_BASE_GET_IMPL_ID)),
            (0x8008, 0x0000_0073),
            (0x800c, addi(5, 10, 0)),
            (0x8010, addi(6, 11, 0)),
            (0x8014, addi(16, 0, SBI_BASE_GET_IMPL_VERSION)),
            (0x8018, 0x0000_0073),
            (0x801c, addi(7, 10, 0)),
            (0x8020, addi(8, 11, 0)),
            (0x8024, addi(10, 0, SBI_BASE_EXTENSION as i32)),
            (0x8028, addi(16, 0, SBI_BASE_PROBE_EXTENSION)),
            (0x802c, 0x0000_0073),
            (0x8030, addi(9, 10, 0)),
            (0x8034, addi(12, 11, 0)),
            (0x8038, lui(10, srst_hi)),
            (0x803c, addi(10, 10, srst_lo)),
            (0x8040, addi(16, 0, SBI_BASE_PROBE_EXTENSION)),
            (0x8044, 0x0000_0073),
            (0x8048, addi(13, 10, 0)),
            (0x804c, addi(14, 11, 0)),
            (0x8050, addi(10, 0, 0x11)),
            (0x8054, addi(16, 0, SBI_BASE_PROBE_EXTENSION)),
            (0x8058, 0x0000_0073),
            (0x805c, addi(15, 10, 0)),
            (0x8060, addi(18, 11, 0)),
            (0x8064, addi(16, 0, SBI_BASE_GET_MVENDORID)),
            (0x8068, 0x0000_0073),
            (0x806c, addi(19, 10, 0)),
            (0x8070, addi(20, 11, 0)),
            (0x8074, addi(16, 0, SBI_BASE_GET_MARCHID)),
            (0x8078, 0x0000_0073),
            (0x807c, addi(21, 10, 0)),
            (0x8080, addi(22, 11, 0)),
            (0x8084, addi(16, 0, SBI_BASE_GET_MIMPID)),
            (0x8088, 0x0000_0073),
            (0x808c, addi(23, 10, 0)),
            (0x8090, addi(24, 11, 0)),
            (0x8094, 0x0010_0073),
        ],
        400,
        200,
    );

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(400),
        GuestSourceId::new(61),
        1,
    );
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
fn supervisor_sbi_base_probe_reports_rfence_and_hsm_extensions() {
    let (rfence_hi, rfence_lo) = lui_addi_parts(SBI_RFENCE_EXTENSION);
    let (hsm_hi, hsm_lo) = lui_addi_parts(SBI_HSM_EXTENSION);
    let (core, run, _) = run_single_core_sbi_program(
        &[
            (0x8000, addi(17, 0, SBI_BASE_EXTENSION as i32)),
            (0x8004, addi(16, 0, SBI_BASE_PROBE_EXTENSION)),
            (0x8008, lui(10, rfence_hi)),
            (0x800c, addi(10, 10, rfence_lo)),
            (0x8010, 0x0000_0073),
            (0x8014, addi(5, 10, 0)),
            (0x8018, addi(6, 11, 0)),
            (0x801c, lui(10, hsm_hi)),
            (0x8020, addi(10, 10, hsm_lo)),
            (0x8024, 0x0000_0073),
            (0x8028, addi(7, 10, 0)),
            (0x802c, addi(8, 11, 0)),
            (0x8030, 0x0010_0073),
        ],
        560,
        120,
    );

    let stop = StopRequest::new(
        run.final_tick().unwrap(),
        GuestEventId::new(560),
        GuestSourceId::new(61),
        1,
    );
    assert_eq!(run.host_stop(), Some(stop));
    assert_eq!(
        run.scheduled_traps()
            .iter()
            .map(|record| record.trap())
            .collect::<Vec<_>>(),
        vec![GuestTrap::new(GuestTrapKind::Breakpoint, 0x8030)]
    );
    assert_eq!(core.read_register(reg(5)), 0);
    assert_eq!(core.read_register(reg(6)), 1);
    assert_eq!(core.read_register(reg(7)), 0);
    assert_eq!(core.read_register(reg(8)), 1);
}
