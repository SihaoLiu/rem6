use super::*;

fn stage_result(
    runtime: &mut O3RuntimeState,
    event: &RiscvCpuExecutionEvent,
    request_sequence: u64,
    issue_tick: u64,
) -> bool {
    runtime.stage_live_data_access_issue(
        event,
        request(request_sequence),
        issue_tick,
        O3DataAccessWindowPolicy::MemoryResultWindow,
    )
}

fn disjoint_atomic_event(pc: u64, sequence: u64, rd: u8) -> RiscvCpuExecutionEvent {
    atomic_event_with(pc, sequence, rd, 0x9010, false, false)
}

fn atomic_event_with(
    pc: u64,
    sequence: u64,
    rd: u8,
    address: u64,
    acquire: bool,
    release: bool,
) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::AtomicMemory {
        rd: reg(rd),
        rs1: reg(10),
        rs2: reg(11),
        width: MemoryWidth::Doubleword,
        op: AtomicMemoryOp::Add,
        acquire,
        release,
    };
    execution_event(
        pc,
        sequence,
        instruction,
        MemoryAccessKind::AtomicMemory {
            rd: reg(rd),
            address,
            width: MemoryWidth::Doubleword,
            op: AtomicMemoryOp::Add,
            value: 3,
            acquire,
            release,
        },
    )
}

fn div_x20() -> RiscvInstruction {
    RiscvInstruction::Div {
        rd: reg(20),
        rs1: reg(1),
        rs2: reg(2),
    }
}

fn addi_from_atomic() -> RiscvInstruction {
    RiscvInstruction::Addi {
        rd: reg(21),
        rs1: reg(11),
        imm: Immediate::new(1),
    }
}

#[test]
fn result_window_accepts_a_disjoint_unordered_atomic_as_row_two() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let head = float_load_event(0x8000, 1);
    let atomic = disjoint_atomic_event(0x8004, 2, 11);

    assert!(stage_result(&mut runtime, &head, 20, 31));
    assert!(stage_result(&mut runtime, &atomic, 21, 32));
    assert_eq!(runtime.live_data_accesses.len(), 2);
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 3);
}

#[test]
fn younger_atomic_result_stages_two_scalar_rows() {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let head = float_load_event(0x8000, 1);
    let atomic = disjoint_atomic_event(0x8004, 2, 11);
    assert!(stage_result(&mut runtime, &head, 20, 31));
    assert!(stage_result(&mut runtime, &atomic, 21, 32));

    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            atomic.fetch().request_id(),
            [
                (Address::new(0x8008), div_x20()),
                (Address::new(0x800c), addi_from_atomic()),
            ],
        ),
        2
    );
    assert_eq!(runtime.snapshot().reorder_buffer().len(), 4);
    assert_eq!(runtime.snapshot().load_store_queue().len(), 3);
}

#[test]
fn younger_atomic_rejects_nonread_heads_ordering_overlap_and_zero_destination() {
    for (label, head, younger) in [
        (
            "atomic head",
            atomic_event_with(0x8000, 1, 7, 0x9000, false, false),
            disjoint_atomic_event(0x8004, 2, 11),
        ),
        (
            "load-reserved head",
            load_reserved_event(0x8000, 1, 7, 0x9000),
            disjoint_atomic_event(0x8004, 2, 11),
        ),
        (
            "acquire",
            float_load_event(0x8000, 1),
            atomic_event_with(0x8004, 2, 11, 0x9010, true, false),
        ),
        (
            "release",
            float_load_event(0x8000, 1),
            atomic_event_with(0x8004, 2, 11, 0x9010, false, true),
        ),
        (
            "overlap",
            float_load_event(0x8000, 1),
            atomic_event_with(0x8004, 2, 11, 0x9000, false, false),
        ),
        (
            "zero destination",
            float_load_event(0x8000, 1),
            atomic_event_with(0x8004, 2, 0, 0x9010, false, false),
        ),
    ] {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        assert!(stage_result(&mut runtime, &head, 20, 31), "{label} head");
        assert!(
            !stage_result(&mut runtime, &younger, 21, 32),
            "{label} younger"
        );
        assert_eq!(runtime.live_data_accesses.len(), 1, "{label}");
    }
}
