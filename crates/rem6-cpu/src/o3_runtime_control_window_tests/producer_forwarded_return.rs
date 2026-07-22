use super::*;

pub(super) fn decoded(instruction: RiscvInstruction) -> rem6_isa_riscv::RiscvDecodedInstruction {
    let raw = match instruction {
        RiscvInstruction::Addi { rd, rs1, imm } => {
            i_type(imm.value(), rs1.index(), rd.index(), 0x13)
        }
        RiscvInstruction::Jalr { rd, rs1, offset } => {
            i_type(offset.value(), rs1.index(), rd.index(), 0x67)
        }
        _ => panic!("unsupported producer-forwarded test instruction: {instruction:?}"),
    };
    RiscvInstruction::decode_with_length(raw).unwrap()
}

fn i_type(imm: i64, rs1: u8, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0xfff) << 20) | (u32::from(rs1) << 15) | (u32::from(rd) << 7) | opcode
}

pub(super) fn recorded_linked_runtime(
    target_source: u8,
    link: u8,
) -> (O3RuntimeState, O3ProducerForwardedControlTarget, u64) {
    let mut runtime = O3RuntimeState::default();
    runtime.set_scalar_memory_window_limit(4);
    let load = scalar_load_event();
    let producer = addi(target_source, 11, 0);
    let call = jalr_link(link, target_source);
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), producer),
                (Address::new(0x8008), call),
            ],
        ),
        2
    );

    let producer_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8004), producer)
        .expect("linked-call target producer candidate");
    bind_o3(&mut runtime, 0x8004, decoded(producer), &[request(11)]);
    assert!(runtime
        .record_live_speculative_execution(
            producer_candidate,
            &[request(11)],
            20,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(target_source), 0x9000)],
                None,
            ),
        )
        .unwrap());
    let call_candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x8008), call)
        .expect("linked-call control candidate");
    let consumer_sequence = call_candidate.sequence();
    bind_o3(&mut runtime, 0x8008, decoded(call), &[request(12)]);
    assert!(runtime
        .record_live_speculative_execution(
            call_candidate,
            &[request(12)],
            21,
            RiscvExecutionRecord::new(
                call,
                0x8008,
                0x9000,
                vec![RegisterWrite::new(reg(link), 0x800c)],
                None,
            ),
        )
        .unwrap());
    let forwarded = runtime
        .producer_forwarded_control_target()
        .expect("linked-call forwarded target authority");
    assert!(
        runtime.record_producer_forwarded_control_target(forwarded, BranchSpeculationId::new(1),)
    );
    (runtime, forwarded, consumer_sequence)
}

#[test]
fn producer_forwarded_linked_calls_append_target_returns() {
    for (target_source, link) in [(1, 1), (11, 5)] {
        let (mut runtime, forwarded, consumer_sequence) =
            recorded_linked_runtime(target_source, link);
        let return_jump = jalr_return(link);
        let return_sequence = runtime
            .append_producer_forwarded_control_descendant(
                forwarded,
                Address::new(0x9000),
                decoded(return_jump),
                &[request(13)],
            )
            .expect("linked call target return append");

        assert_eq!(
            runtime.pending_live_control_lineage_parent_for_test(return_sequence),
            Some(consumer_sequence)
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x9000), return_jump)
            .expect("linked call target return candidate");
        assert_eq!(candidate.destination(), None);
        assert_eq!(candidate.producer_sequences(), &[consumer_sequence]);
        assert_eq!(
            candidate.forwarded_register_writes(),
            &[RegisterWrite::new(reg(link), 0x800c)]
        );
        assert_eq!(candidate.control_dependency(), Some(consumer_sequence));
        assert!(runtime
            .record_live_speculative_execution(
                candidate,
                &[request(13)],
                22,
                RiscvExecutionRecord::new(return_jump, 0x9000, 0x800c, Vec::new(), None),
            )
            .unwrap());
        let descendant = runtime
            .producer_forwarded_return_descendant()
            .expect("linked call target return lineage");
        assert_eq!(descendant.parent(), forwarded);
        assert_eq!(descendant.fetch_request(), request(13));
        assert_eq!(descendant.pc(), Address::new(0x9000));
        assert_eq!(descendant.target(), Address::new(0x800c));

        runtime
            .live_speculative_executions
            .iter_mut()
            .find(|execution| execution.sequence == forwarded.producer_sequence())
            .expect("linked-call target producer execution")
            .admitted_writeback_tick += 2;
        assert_eq!(
            runtime
                .producer_forwarded_return_descendant()
                .expect("scheduling-only retime keeps return lineage"),
            descendant
        );
    }
}

#[test]
fn producer_forwarded_linked_call_rejects_nonordinary_target_controls() {
    for instruction in [
        RiscvInstruction::Jalr {
            rd: reg(0),
            rs1: reg(1),
            offset: Immediate::new(4),
        },
        jalr_return(5),
        jalr_link(5, 1),
    ] {
        let (mut runtime, forwarded, _) = recorded_linked_runtime(1, 1);
        assert_eq!(
            runtime.append_producer_forwarded_control_descendant(
                forwarded,
                Address::new(0x9000),
                decoded(instruction),
                &[request(13)],
            ),
            None,
            "unexpected producer-forwarded target control admission for {instruction:?}"
        );
    }
}

#[test]
fn producer_forwarded_split_link_call_appends_return_after_data_head_retires() {
    let (mut runtime, forwarded, consumer_sequence) = recorded_linked_runtime(11, 5);
    runtime.live_data_accesses.clear();
    runtime.snapshot.reorder_buffer.remove(0);

    let return_jump = jalr_return(5);
    let return_sequence = runtime
        .append_producer_forwarded_control_descendant(
            forwarded,
            Address::new(0x9000),
            decoded(return_jump),
            &[request(13)],
        )
        .expect("recorded split-link call survives successful data-head retirement");
    assert_eq!(
        runtime.pending_live_control_lineage_parent_for_test(return_sequence),
        Some(consumer_sequence)
    );
    let candidate = runtime
        .live_speculative_issue_candidate(Address::new(0x9000), return_jump)
        .expect("post-head split-link return candidate");
    assert_eq!(candidate.producer_sequences(), &[consumer_sequence]);
}
