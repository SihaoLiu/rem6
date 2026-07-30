use super::*;

#[test]
fn checkpoint_finalization_clears_consumed_calendar_history() {
    let core = core();
    core.reserve_test_fixed_fu_writeback(4, 20).unwrap();
    assert!(!core.data_access_lifecycle_is_quiescent());

    core.finalize_quiescent_o3_writeback_for_checkpoint();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.writeback_reservation(4).is_none());
    assert!(state.o3_writeback_wake.owned_wakes().is_empty());
    drop(state);
    assert!(core.data_access_lifecycle_is_quiescent());
    assert_eq!(
        core.reserve_test_fixed_fu_writeback(5, 20).unwrap_err(),
        O3RuntimeError::WritebackReservationTickClosed {
            sequence: 5,
            raw_ready_tick: 20,
            closed_before_tick: 21,
        }
    );
}

#[test]
fn checkpoint_finalization_keeps_scheduled_writeback_wake_nonquiescent() {
    let core = core();
    core.reserve_test_fixed_fu_writeback(4, 20).unwrap();
    let (scheduler, event) = wake(20);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.o3_writeback_wake.set_desired_tick(Some(20), 10);
        state.o3_writeback_wake.mark_scheduled(scheduler, event);
    }

    core.finalize_quiescent_o3_writeback_for_checkpoint();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.writeback_reservation(4).is_some());
    assert_eq!(state.o3_writeback_wake.owned_wakes().len(), 1);
    drop(state);
    assert!(!core.data_access_lifecycle_is_quiescent());
}

#[test]
fn checkpoint_finalization_retains_nonterminal_no_writeback_history() {
    let core = core();
    let instruction = RiscvInstruction::Jal {
        rd: register(0),
        offset: Immediate::new(4),
    };
    let request = memory_request(100);
    let execution = RiscvExecutionRecord::new(instruction, 0x8100, 0x8104, Vec::new(), None);
    let ready_tick = {
        let mut state = core.state.lock().expect("riscv core lock");
        let sequence = state
            .o3_runtime
            .stage_live_retire_window(Address::new(0x8100), instruction, 0, [])
            .expect("staged no-link control");
        assert!(state.o3_runtime.bind_live_staged_issue_packet(
            Address::new(0x8100),
            RiscvInstruction::decode_with_length(0x0040_006f).unwrap(),
            &[request],
            20,
        ));
        let candidate = state
            .o3_runtime
            .live_speculative_issue_candidate(Address::new(0x8100), instruction)
            .expect("no-link control issue candidate");
        assert!(state
            .o3_runtime
            .record_live_speculative_execution(candidate, &[request], 20, execution.clone())
            .unwrap());
        assert!(!state.o3_runtime.has_live_writeback_owner());
        let ready_tick = state
            .o3_runtime
            .live_speculative_execution_ready_tick(&[request], &execution)
            .expect("recorded no-link control execution");
        assert!(state.o3_runtime.live_issue_is_quiescent());
        assert!(!state.o3_runtime.checkpoint_history_is_terminal());
        state
            .o3_runtime
            .observe_live_issue_decision_for_test(20, &[sequence], &[], &[], 1);
        assert!(state.o3_runtime.has_active_live_issue_decision_for_test());
        ready_tick
    };

    core.finalize_quiescent_o3_writeback_for_checkpoint();

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .o3_runtime
            .live_speculative_execution_ready_tick(&[request], &execution),
        Some(ready_tick)
    );
    assert!(state.o3_runtime.has_active_live_issue_decision_for_test());
}

#[test]
fn checkpoint_finalization_reports_max_tick_seal_error_without_clearing() {
    let core = core();
    core.reserve_test_fixed_fu_writeback(4, u64::MAX).unwrap();

    core.finalize_quiescent_o3_writeback_for_checkpoint();

    assert_eq!(
        core.pending_callback_error(),
        Some(RiscvCpuError::O3Runtime(
            O3RuntimeError::WritebackClosureTickOverflow { tick: u64::MAX }
        ))
    );
    assert_eq!(core.o3_runtime_writeback_reservations().len(), 1);
}
