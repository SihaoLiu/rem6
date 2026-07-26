use rem6_isa_riscv::{FloatRegister, FloatRegisterWrite};

use super::super::o3_runtime_issue::O3LiveIssueForwardedValue;
use super::*;

const LOAD_PC: u64 = 0x8000;
const RESPONSE_TICK: u64 = 41;

fn f(index: u8) -> FloatRegister {
    FloatRegister::new(index).unwrap()
}

fn float_load_event(width: MemoryWidth) -> RiscvCpuExecutionEvent {
    let instruction = RiscvInstruction::FloatLoad {
        rd: f(4),
        rs1: reg(10),
        offset: Immediate::new(0),
        width,
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
                width,
            }),
        ),
    )
}

fn completed_float_load_response(
    width: MemoryWidth,
    data: Option<&[u8]>,
) -> (O3RuntimeState, u64, u64) {
    let mut runtime = O3RuntimeState::default();
    let load = float_load_event(width);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    let sequence = runtime.live_data_accesses[0].sequence;
    let mut completed = load;
    completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
    assert!(runtime
        .complete_live_data_access_response(&completed, request(20), RESPONSE_TICK, 10, data,)
        .unwrap());
    let admitted_tick = runtime
        .writeback_reservation(sequence)
        .expect("FP load owns a memory-result writeback reservation")
        .admitted_tick();
    (runtime, sequence, admitted_tick)
}

fn completed_float_load(width: MemoryWidth, data: &[u8]) -> (O3RuntimeState, u64, u64) {
    completed_float_load_response(width, Some(data))
}

#[test]
fn fp_load_source_materializes_word_and_double_values_at_admitted_writeback() {
    for (width, data, expected) in [
        (
            MemoryWidth::Word,
            2.0f32.to_bits().to_le_bytes().to_vec(),
            O3LiveIssueForwardedValue::FloatingPoint(FloatRegisterWrite::new(
                f(4),
                0xffff_ffff_4000_0000,
            )),
        ),
        (
            MemoryWidth::Doubleword,
            2.0f64.to_bits().to_le_bytes().to_vec(),
            O3LiveIssueForwardedValue::FloatingPoint(FloatRegisterWrite::new(
                f(4),
                2.0f64.to_bits(),
            )),
        ),
    ] {
        let (runtime, sequence, admitted_tick) = completed_float_load(width, &data);

        assert_eq!(
            runtime
                .live_issue_source_value(sequence, O3ArchitecturalRegister::floating_point(f(4)),),
            Some((expected, admitted_tick)),
        );
    }
}

#[test]
fn fp_load_source_rejects_wrong_class_wrong_register_and_missing_reservation() {
    let data = 2.0f32.to_bits().to_le_bytes();
    let (mut runtime, sequence, _) = completed_float_load(MemoryWidth::Word, &data);

    assert_eq!(
        runtime.live_issue_source_value(sequence, O3ArchitecturalRegister::integer(reg(4))),
        None,
    );
    assert_eq!(
        runtime.live_issue_source_value(sequence, O3ArchitecturalRegister::floating_point(f(5)),),
        None,
    );

    runtime.discard_future_writeback_sequence(sequence, 0);
    assert!(runtime.writeback_reservation(sequence).is_none());
    assert_eq!(
        runtime.live_issue_source_value(sequence, O3ArchitecturalRegister::floating_point(f(4)),),
        None,
    );
}

#[test]
fn fp_load_missing_or_short_response_never_materializes_a_source() {
    for (label, width, data) in [
        ("flw missing", MemoryWidth::Word, None),
        (
            "flw short",
            MemoryWidth::Word,
            Some(&[0x00, 0x00, 0x00][..]),
        ),
        (
            "fld short",
            MemoryWidth::Doubleword,
            Some(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..]),
        ),
    ] {
        let (runtime, sequence, _) = completed_float_load_response(width, data);
        let source = O3ArchitecturalRegister::floating_point(f(4));

        assert_eq!(
            runtime.live_issue_source_value(sequence, source),
            None,
            "{label}"
        );
        assert_eq!(
            runtime.completed_live_data_access_ready_tick(sequence),
            None,
            "{label}: malformed bytes never advertise a wake tick"
        );
        assert!(
            runtime.writeback_reservation(sequence).is_some(),
            "{label}: fail-closed lookup is independent of reservation ownership"
        );
    }
}
