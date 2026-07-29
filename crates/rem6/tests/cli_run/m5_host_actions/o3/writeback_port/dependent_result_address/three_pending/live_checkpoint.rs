use super::*;

#[path = "live_checkpoint_support.rs"]
mod live_checkpoint_support;
use live_checkpoint_support::*;

#[test]
fn rem6_run_o3_three_pending_load_live_checkpoint_sibling_width_two_direct() {
    run_three_pending_live_checkpoint_row(
        row(ThreePendingTopology::Sibling, "direct", 2, 2, 9, 1_200),
        ThreePendingLiveCalibration::new(307, 308, 330, 331),
    );
}

#[test]
fn rem6_run_o3_three_pending_load_live_checkpoint_chain_width_four_direct() {
    run_three_pending_live_checkpoint_row(
        row(ThreePendingTopology::Chain, "direct", 4, 4, 9, 1_400),
        ThreePendingLiveCalibration::new(307, 308, 366, 367),
    );
}

#[test]
fn rem6_run_o3_three_pending_load_live_checkpoint_mixed_fanout_width_two_hierarchy() {
    run_three_pending_live_checkpoint_row(
        row(
            ThreePendingTopology::MixedFanout,
            "cache-fabric-dram",
            2,
            2,
            80,
            12_000,
        ),
        ThreePendingLiveCalibration::new(2_798, 2_799, 3_149, 3_150),
    );
}

#[test]
fn rem6_run_o3_three_pending_load_live_checkpoint_source_progress_discriminator() {
    assert_three_pending_live_checkpoint_source_progress();
}

#[test]
fn rem6_run_o3_three_pending_load_live_checkpoint_boundaries() {
    assert_post_publication_graph_checkpoint_supported();
    super::boundaries::assert_three_pending_checkpoint_boundaries();
    assert_live_graph_handoff_rejected();
}
