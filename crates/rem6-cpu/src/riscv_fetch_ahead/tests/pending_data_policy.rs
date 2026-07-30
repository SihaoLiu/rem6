use super::*;

#[test]
fn disabled_detailed_policy_blocks_normal_pending_data_fetch_authority() {
    let addi = i_type(0, 0, 0, 1, 0x13).to_le_bytes().to_vec();
    let ordinary = core_with_completed_fetch(addi.clone());
    assert!(ordinary.next_fetch_ahead_before_retire().is_some());

    let pending = core_with_completed_fetch(addi);
    assert_eq!(pending.next_pending_data_fetch_ahead(true), None);
}

#[test]
fn disabled_detailed_policy_allows_typed_producer_forwarded_continuation() {
    let forwarded = super::producer_forwarded_scalar_return::scalar_return_core(2, false, 1, 1);
    super::producer_forwarded_scalar_return::record_call_and_scalar(&forwarded);
    forwarded.set_detailed_live_retire_gate_enabled(false);

    let decision = next_pending_data_fetch_ahead_after_o3_wake(&forwarded, true)
        .expect("producer-forwarded scalar continuation");
    assert_eq!(decision.pc(), Address::new(0x9004));
    assert!(decision.producer_forwarded_scalar_continuation.is_some());
}
