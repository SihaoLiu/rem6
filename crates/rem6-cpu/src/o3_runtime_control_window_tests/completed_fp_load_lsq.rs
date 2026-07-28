use rem6_isa_riscv::FloatRegister;

use super::*;

const LOAD_PC: u64 = 0x8000;
const RESPONSE_TICK: u64 = 41;

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn float_load_event() -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::FloatLoad {
        rd: f(4),
        rs1: reg(10),
        offset: Immediate::new(0),
        width: MemoryWidth::Word,
    };
    RiscvCpuExecutionEvent::new(
        fetch_event(LOAD_PC, 10),
        instruction,
        RiscvExecutionRecord::new(
            instruction,
            LOAD_PC,
            LOAD_PC + 4,
            Vec::new(),
            Some(MemoryAccessKind::FloatLoad {
                rd: f(4),
                address: 0x9000,
                width: MemoryWidth::Word,
            }),
        ),
    )
}

#[test]
fn older_fixed_fu_retirement_keeps_completed_fp_load_lsq_owner() {
    let mut runtime = O3RuntimeState::default();
    let older_instruction = RiscvInstruction::Div {
        rd: reg(3),
        rs1: reg(1),
        rs2: reg(2),
    };
    runtime.stage_live_retire_window(Address::new(0x7ffc), older_instruction, 41, None);
    bind_o3(
        &mut runtime,
        0x7ffc,
        decoded(older_instruction),
        &[request(9)],
    );

    let load = float_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    let mut completed = load;
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(
            &completed,
            request(20),
            RESPONSE_TICK,
            10,
            Some(&2.0f32.to_bits().to_le_bytes()),
        )
        .unwrap());

    let older = RiscvCpuExecutionEvent::new(
        fetch_event(0x7ffc, 9),
        older_instruction,
        RiscvExecutionRecord::new(
            older_instruction,
            0x7ffc,
            LOAD_PC,
            vec![RegisterWrite::new(reg(3), 4)],
            None,
        ),
    );
    runtime.retire_live_staged_instruction(&older, &[request(9)], RESPONSE_TICK);
    runtime.record_retired_instruction_with_trace(&older, true);

    let snapshot = runtime.snapshot();
    let [load_lsq] = snapshot.load_store_queue() else {
        panic!("completed FP load lost its LSQ owner: {snapshot:?}");
    };
    assert_eq!(load_lsq.kind(), O3LoadStoreQueueKind::Load);
    assert!(load_lsq.is_completed());
}
