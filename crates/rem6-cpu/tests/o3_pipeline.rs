use rem6_cpu::{O3PipelineError, O3PipelineStage, O3UnblockDecisionReason, O3UnblockPolicy};

#[test]
fn o3_unblock_policy_signals_before_skid_buffer_is_empty() {
    let policy =
        O3UnblockPolicy::new(O3PipelineStage::Fetch, O3PipelineStage::Decode, 1, 2).unwrap();

    assert_eq!(policy.upstream(), O3PipelineStage::Fetch);
    assert_eq!(policy.downstream(), O3PipelineStage::Decode);
    assert_eq!(policy.backward_signal_delay_cycles(), 1);
    assert_eq!(policy.downstream_width(), 2);
    assert_eq!(policy.early_unblock_threshold_entries(), 2);

    let still_draining = policy.decision(3);
    assert!(!still_draining.should_signal_unblock());
    assert_eq!(
        still_draining.reason(),
        O3UnblockDecisionReason::SkidBufferAboveEarlyThreshold
    );
    assert_eq!(still_draining.cycles_to_drain(), 2);

    let early = policy.decision(2);
    assert!(early.should_signal_unblock());
    assert_eq!(early.reason(), O3UnblockDecisionReason::SignalDelayCovered);
    assert_eq!(early.cycles_to_drain(), 1);
    assert!(!policy.empty_only_would_signal(2));

    let empty = policy.decision(0);
    assert!(empty.should_signal_unblock());
    assert_eq!(empty.reason(), O3UnblockDecisionReason::SkidBufferEmpty);
}

#[test]
fn o3_unblock_policy_validates_zero_width_and_zero_delay_boundaries() {
    assert_eq!(
        O3UnblockPolicy::new(O3PipelineStage::Rename, O3PipelineStage::Iew, 1, 0).unwrap_err(),
        O3PipelineError::ZeroDownstreamWidth {
            downstream: O3PipelineStage::Iew,
        }
    );

    let zero_delay =
        O3UnblockPolicy::new(O3PipelineStage::Decode, O3PipelineStage::Rename, 0, 4).unwrap();
    assert_eq!(zero_delay.early_unblock_threshold_entries(), 0);
    assert!(!zero_delay.decision(1).should_signal_unblock());
    assert!(zero_delay.decision(0).should_signal_unblock());
    assert!(zero_delay.empty_only_would_signal(0));
}
