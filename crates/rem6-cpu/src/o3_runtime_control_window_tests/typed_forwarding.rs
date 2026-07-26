use rem6_isa_riscv::{FloatRegister, FloatRegisterWrite, RiscvExecutionRecord, RiscvInstruction};

use super::*;

const BRANCH_PC: u64 = 0x8004;

fn fp_raw(funct7: u32, rs2: u8, rs1: u8, rd: u8) -> u32 {
    (funct7 << 25) | (u32::from(rs2) << 20) | (u32::from(rs1) << 15) | (u32::from(rd) << 7) | 0x53
}

fn speculative_fp_row(
    sequence: u64,
    producer_sequences: Vec<u64>,
    register: u8,
    value: u64,
    ready_tick: u64,
) -> O3LiveSpeculativeExecution {
    let instruction = RiscvInstruction::decode(fp_raw(0, 2, 1, register)).unwrap();
    O3LiveSpeculativeExecution {
        consumed_requests: vec![request(sequence)],
        sequence,
        producer_sequences,
        issue_tick: 20,
        raw_ready_tick: ready_tick,
        admitted_writeback_tick: ready_tick,
        writeback_slot: None,
        execution: RiscvExecutionRecord::new_with_instruction_bytes_and_float_register_writes(
            instruction,
            4,
            BRANCH_PC + sequence * 4,
            BRANCH_PC + sequence * 4 + 4,
            Vec::new(),
            vec![FloatRegisterWrite::new(
                FloatRegister::new(register).unwrap(),
                value,
            )],
            None,
        ),
    }
}

#[test]
fn typed_live_forwarding_recursive_invalidation_is_sequence_owned() {
    let mut runtime = O3RuntimeState::default();
    let (producer, consumer, descendant, unrelated) = (10, 11, 12, 20);
    runtime.live_speculative_executions = vec![
        speculative_fp_row(producer, Vec::new(), 4, 0xffff_ffff_4040_0000, 31),
        speculative_fp_row(consumer, vec![producer], 5, 0xffff_ffff_4110_0000, 32),
        speculative_fp_row(descendant, vec![consumer], 6, 0xffff_ffff_4190_0000, 33),
        speculative_fp_row(unrelated, Vec::new(), 7, 0xffff_ffff_3f80_0000, 34),
    ];
    runtime.snapshot.reorder_buffer = [producer, consumer, descendant, unrelated]
        .map(|sequence| {
            O3ReorderBufferEntry::new(sequence, Address::new(BRANCH_PC + sequence * 4), None)
                .with_live_staged_rename_destination(None)
        })
        .to_vec();
    runtime
        .reserve_writeback_completions([
            O3LiveWritebackReady::fixed_fu(producer, 31),
            O3LiveWritebackReady::fixed_fu(consumer, 32),
            O3LiveWritebackReady::fixed_fu(descendant, 33),
            O3LiveWritebackReady::fixed_fu(unrelated, 34),
        ])
        .unwrap();

    let producer_index = runtime
        .live_speculative_executions
        .iter()
        .position(|row| row.sequence == producer)
        .unwrap();
    runtime.live_speculative_executions.remove(producer_index);
    runtime.discard_future_writeback_sequence(producer, 30);
    runtime.invalidate_live_speculative_execution_chain_for_test(producer, 30);

    assert!(runtime
        .live_speculative_executions
        .iter()
        .all(|row| { ![producer, consumer, descendant].contains(&row.sequence) }));
    for sequence in [producer, consumer, descendant] {
        assert!(runtime.writeback_reservation(sequence).is_none());
    }
    assert_eq!(
        runtime
            .live_speculative_executions
            .iter()
            .map(|row| row.sequence)
            .collect::<Vec<_>>(),
        [unrelated],
    );
    assert!(runtime.writeback_reservation(unrelated).is_some());
}
