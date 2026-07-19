use super::producer_forwarded_return::live_return_core;
use super::*;

#[test]
fn pending_data_gate_admits_same_and_split_link_direct_returns() {
    for (target_source, link_destination) in [(1, 1), (11, 5)] {
        let core = live_return_core(2, target_source, link_destination);
        let call_decision = core
            .next_fetch_ahead_before_retire()
            .expect("producer-forwarded linked-call decision");
        assert_eq!(call_decision.pc(), Address::new(0x9000));
        let call_speculation = call_decision.branch_speculation().unwrap();
        let PredictedControlTargetAuthority::ProducerForwarded(parent) =
            call_speculation.target_authority()
        else {
            panic!("expected producer-forwarded linked-call authority");
        };
        assert_eq!(
            parent.target_source(),
            Register::new(target_source).unwrap()
        );
        assert_eq!(
            parent.link_destination(),
            Some(Register::new(link_destination).unwrap())
        );
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&call_decision)
                .unwrap(),
        );

        let return_decision = core
            .next_pending_data_fetch_ahead(true)
            .expect("producer-forwarded direct-return decision");
        assert_eq!(return_decision.pc(), Address::new(0x800c));
        let return_speculation = return_decision.branch_speculation().unwrap();
        let PredictedControlTargetAuthority::ProducerForwardedReturn(descendant) =
            return_speculation.target_authority()
        else {
            panic!("expected producer-forwarded direct-return authority");
        };
        assert_eq!(descendant.parent(), parent);
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&return_decision)
                .unwrap(),
        );

        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.o3_runtime.producer_forwarded_return_descendant(),
            Some(descendant)
        );
        assert_eq!(
            state.branch_speculation_kinds.get(&2),
            Some(&BranchTargetKind::CallIndirect)
        );
        assert_eq!(
            state.branch_speculation_kinds.get(&3),
            Some(&BranchTargetKind::Return)
        );
        assert!(state.return_address_stack_operations.contains_key(&2));
        assert!(state.return_address_stack_operations.contains_key(&3));
    }
}
