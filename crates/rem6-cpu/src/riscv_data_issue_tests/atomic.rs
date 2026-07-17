use super::*;

#[test]
fn atomic_result_execution_is_deferred_o3_data_access() {
    let event = atomic_memory_event(0x8000, 1, 0x9000);

    assert!(!event.is_scalar_memory_access());
    assert!(event.is_deferred_o3_data_access());
}

#[test]
fn denied_atomic_write_never_stages_live_result_authority() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    core.write_pmp_addr(0, 0x8800 >> 2).unwrap();
    core.write_pmp_config(
        0,
        rem6_isa_riscv::RiscvPmpConfig::new(rem6_isa_riscv::RiscvPmpAddressMode::Tor)
            .with_read(true)
            .with_write(true)
            .with_execute(true),
    )
    .unwrap();
    core.write_pmp_addr(1, 0xa000 >> 2).unwrap();
    core.write_pmp_config(
        1,
        rem6_isa_riscv::RiscvPmpConfig::new(rem6_isa_riscv::RiscvPmpAddressMode::Tor)
            .with_locked(true),
    )
    .unwrap();
    let event = atomic_memory_event(0x8000, 1, 0x9000);
    core.state
        .lock()
        .expect("riscv core lock")
        .events
        .push(event);

    let error = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("PMP-denied AMO must not issue"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataPmpAccess {
            error: rem6_isa_riscv::RiscvPmpError::AccessDenied {
                kind: rem6_isa_riscv::RiscvPmpAccessKind::Write,
                ..
            },
            ..
        }
    ));
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.is_empty());
    assert!(state.o3_runtime.live_data_access_lifecycle_is_quiescent());
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
    drop(state);
    let diagnostic = core.try_failure_diagnostic_snapshot().unwrap();
    assert_eq!(diagnostic.completed_data_access_events(), 0);
    assert_eq!(diagnostic.rob_entries(), 0);
    assert_eq!(diagnostic.lsq_entries(), 0);
    assert_eq!(diagnostic.writeback_reservations(), 0);
}

#[test]
fn pma_denied_atomic_write_never_stages_live_result_authority() {
    let (mut scheduler, transport, fetch_route, data_route) = memory_routes();
    let core = RiscvCore::with_data(
        cpu_core(fetch_route, 0x8000),
        CpuDataConfig::new(endpoint("cpu0.dmem"), data_route, line_layout()),
    );
    core.set_detailed_live_retire_gate_enabled(true);
    let fetch = fetch_event(0x8000, 1).request_id();
    core.state
        .lock()
        .expect("riscv core lock")
        .events
        .push(atomic_memory_event(0x8000, 1, 0x9004));

    let error = core
        .issue_next_data_access(
            &mut scheduler,
            &transport,
            MemoryTrace::new(),
            |_delivery, _context| panic!("PMA-denied AMO must not issue"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::DataPmaAccess {
            fetch: actual_fetch,
            error: rem6_isa_riscv::RiscvPmaError::MisalignedDataAccess {
                address: 0x9004,
                size: 8,
                kind: rem6_isa_riscv::RiscvPmaAccessKind::Write,
            },
        } if actual_fetch == fetch
    ));
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.outstanding_data.is_empty());
    assert!(state.o3_runtime.live_data_access_lifecycle_is_quiescent());
    assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
    assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
}

fn atomic_memory_event(pc: u64, sequence: u64, address: u64) -> RiscvCpuExecutionEvent {
    let instruction = rem6_isa_riscv::RiscvInstruction::AtomicMemory {
        rd: reg(5),
        rs1: reg(2),
        rs2: reg(3),
        width: MemoryWidth::Doubleword,
        op: AtomicMemoryOp::Add,
        acquire: false,
        release: false,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(pc, sequence),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            Vec::new(),
            Some(MemoryAccessKind::AtomicMemory {
                rd: reg(5),
                address,
                width: MemoryWidth::Doubleword,
                op: AtomicMemoryOp::Add,
                value: 7,
                acquire: false,
                release: false,
            }),
        ),
    )
}
