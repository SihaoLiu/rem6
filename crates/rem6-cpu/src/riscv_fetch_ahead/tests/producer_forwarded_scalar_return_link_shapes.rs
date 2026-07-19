use super::producer_forwarded_scalar_return::{
    record_call_and_scalar, retire_data_head, scalar_return_core,
};
use super::*;

#[test]
fn retired_data_head_admits_same_and_split_link_scalar_return_predictions() {
    for (target_source, link_destination) in [(1, 1), (11, 5)] {
        let core = scalar_return_core(2, true, target_source, link_destination);
        record_call_and_scalar(&core);
        retire_data_head(&core, 30);

        let decision = core
            .next_pending_data_fetch_ahead(false)
            .expect("linked-call scalar-return decision");
        assert_eq!(decision.pc(), Address::new(0x800c));
        let speculation = decision.branch_speculation().unwrap();
        assert_eq!(speculation.pc(), Address::new(0x9004));
        assert_eq!(speculation.target(), Some(Address::new(0x800c)));
        let PredictedControlTargetAuthority::ProducerForwardedReturn(descendant) =
            speculation.target_authority()
        else {
            panic!("expected typed producer-forwarded return descendant");
        };
        assert!(descendant.scalar_chain().is_one_step());
        assert_eq!(
            descendant.parent().target_source(),
            Register::new(target_source).unwrap()
        );
        assert_eq!(
            descendant.parent().link_destination(),
            Some(Register::new(link_destination).unwrap())
        );

        let prepared = core
            .prepare_fetch_ahead_speculation(&decision)
            .unwrap()
            .expect("linked-call scalar-return preparation");
        core.record_prepared_fetch_ahead_speculation(Some(prepared));

        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
        assert_eq!(
            state
                .o3_runtime
                .producer_forwarded_return_descendant()
                .as_ref(),
            Some(descendant)
        );
        assert_eq!(
            state.branch_speculation_kinds.get(&4),
            Some(&BranchTargetKind::Return)
        );
        assert!(state.return_address_stack_operations.contains_key(&4));
    }
}
