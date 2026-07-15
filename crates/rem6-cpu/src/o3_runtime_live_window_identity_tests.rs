    #[test]
    fn mismatched_live_speculative_record_does_not_claim_early_issue() {
        let mut runtime = O3RuntimeState::default();
        let younger_instruction = addi(4, 0);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(2)],
                10,
                RiscvExecutionRecord::new(
                    younger_instruction,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                    None,
                ),
            )
            .unwrap();

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = RiscvCpuExecutionEvent::new(
            fetch_event(0x8004, 2),
            younger_instruction,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 2)],
                None,
            ),
        );
        retire_live(&mut runtime, &younger, 30);
        runtime.record_retired_instruction_with_trace(&younger, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 30);
        assert_eq!(trace.commit_tick(), 30);
    }

    #[test]
    fn mismatched_split_fetch_suffix_does_not_claim_early_issue() {
        let mut runtime = O3RuntimeState::default();
        let younger_instruction = addi(4, 0);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(2), request(3)],
                10,
                RiscvExecutionRecord::new(
                    younger_instruction,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                    None,
                ),
            )
            .unwrap();
        let speculative_sequence = runtime.live_speculative_executions[0].sequence;

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = execution_event(younger_instruction, 0x8004, 2, 4);
        runtime.retire_live_staged_instruction(&younger, &[request(2), request(4)], 30);
        assert_eq!(
            runtime
                .writeback_reservation(speculative_sequence)
                .map(O3WritebackReservation::admitted_tick),
            Some(10),
            "rebinding after the admitted tick must preserve historical occupancy"
        );
        runtime.record_retired_instruction_with_trace(&younger, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 30);
        assert_eq!(trace.commit_tick(), 30);
    }

    #[test]
    fn malformed_live_speculative_fetch_identity_does_not_occupy_candidate() {
        let younger_instruction = addi(4, 0);
        let malformed_identities = [
            Vec::new(),
            vec![request(2), request(2)],
            vec![request(3), request(2)],
            vec![request(2), MemoryRequestId::new(AgentId::new(8), 3)],
            vec![request(2), request(3), request(4)],
        ];

        for consumed_requests in malformed_identities {
            let mut runtime = O3RuntimeState::default();
            runtime.stage_live_retire_window(
                Address::new(0x8000),
                div_x3(),
                29,
                Some((Address::new(0x8004), younger_instruction)),
            );
            let candidate = runtime
                .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
                .unwrap();
            runtime
                .record_live_speculative_execution(
                    candidate,
                    &consumed_requests,
                    10,
                    RiscvExecutionRecord::new(
                        younger_instruction,
                        0x8004,
                        0x8008,
                        vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                        None,
                    ),
                )
                .unwrap();

            assert!(runtime.live_speculative_executions.is_empty());
            assert!(runtime
                .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
                .is_some());
        }
    }

    #[test]
    fn live_speculative_execution_and_fetch_identity_are_transient_across_restore() {
        let mut runtime = O3RuntimeState::default();
        let younger_instruction = addi(4, 0);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(2)],
                10,
                RiscvExecutionRecord::new(
                    younger_instruction,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                    None,
                ),
            )
            .unwrap();
        assert_eq!(runtime.live_speculative_executions.len(), 1);

        let checkpoint = runtime.checkpoint_payload();
        runtime.restore_checkpoint_payload(checkpoint).unwrap();
        assert!(runtime.live_speculative_executions.is_empty());
        assert!(runtime.live_staged_fetch_identities.is_empty());
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .is_none());

        runtime.discard_live_staged_instructions();
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();

        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(3)],
                20,
                RiscvExecutionRecord::new(
                    younger_instruction,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                    None,
                ),
            )
            .unwrap();
        runtime.discard_live_speculative_executions();
        assert!(runtime.live_speculative_executions.is_empty());
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                candidate,
                &[request(4)],
                21,
                RiscvExecutionRecord::new(
                    younger_instruction,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                    None,
                ),
            )
            .unwrap();
        runtime.discard_live_staged_instructions();
        assert!(runtime.live_speculative_executions.is_empty());
        assert!(runtime.live_staged_fetch_identities.is_empty());
    }
