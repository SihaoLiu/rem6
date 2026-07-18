use super::*;

#[test]
fn rem6_run_in_order_pipeline_models_vector_indexed_e8_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-e8-m1-load-store",
        &indexed_e8_m1_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        30,
        "indexed e8,m1 vector memory should move selected byte offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-e8-m1-load-store",
        &indexed_e8_m1_vector_memory_program(),
        1040,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        30,
        "cache-backed indexed e8,m1 vector memory should move selected byte offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_indexed_e8_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-e8-m1-load-store",
        &masked_indexed_e8_m1_vector_memory_program(),
        560,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        35,
        "masked indexed e8,m1 vector memory should preserve inactive compact byte lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-e8-m1-load-store",
        &masked_indexed_e8_m1_vector_memory_program(),
        1400,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        35,
        "cache-backed masked indexed e8,m1 vector memory should preserve inactive compact byte lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e8_m1_data_e16_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e8-data-e16-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e8,m1 data with e16 indices should move selected byte offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e8-data-e16-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e8,m1 data with e16 indices should move selected byte offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e8_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e8-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e8,m1 data with e16 indices should preserve inactive compact byte lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e8-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e8,m1 data with e16 indices should preserve inactive compact byte lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e8_m1_data_e32_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e8-data-e32-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e8,m1 data with e32 indices should move selected byte offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e8-data-e32-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e8,m1 data with e32 indices should move selected byte offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e8_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e8-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e8,m1 data with e32 indices should preserve inactive compact byte lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e8-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e8,m1 data with e32 indices should preserve inactive compact byte lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e8_m1_data_e64_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e8-data-e64-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e8,m1 data with e64 indices should move selected byte offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e8-data-e64-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e8,m1 data with e64 indices should move selected byte offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e8_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e8-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e8,m1 data with e64 indices should preserve inactive compact byte lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e8-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e8,m1 data with e64 indices should preserve inactive compact byte lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_indexed_e16_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-e16-m1-load-store",
        &indexed_e16_m1_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        30,
        "indexed e16,m1 vector memory should move selected halfword offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-e16-m1-load-store",
        &indexed_e16_m1_vector_memory_program(),
        1040,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        30,
        "cache-backed indexed e16,m1 vector memory should move selected halfword offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_indexed_e16_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-e16-m1-load-store",
        &masked_indexed_e16_m1_vector_memory_program(),
        560,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        35,
        "masked indexed e16,m1 vector memory should preserve inactive compact halfword lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-e16-m1-load-store",
        &masked_indexed_e16_m1_vector_memory_program(),
        1400,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        35,
        "cache-backed masked indexed e16,m1 vector memory should preserve inactive compact halfword lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e16_m1_data_e8_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e16-data-e8-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e16,m1 data with e8 indices should move selected halfword offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e16-data-e8-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e16,m1 data with e8 indices should move selected halfword offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e16_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e16-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e16,m1 data with e8 indices should preserve inactive compact halfword lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e16-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e16,m1 data with e8 indices should preserve inactive compact halfword lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e16_m1_data_e32_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e16-data-e32-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e16,m1 data with e32 indices should move selected halfword offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e16-data-e32-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e16,m1 data with e32 indices should move selected halfword offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e16_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e16-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e16,m1 data with e32 indices should preserve inactive compact halfword lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e16-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e16,m1 data with e32 indices should preserve inactive compact halfword lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e16_m1_data_e64_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e16-data-e64-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e16,m1 data with e64 indices should move selected halfword offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e16-data-e64-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e16,m1 data with e64 indices should move selected halfword offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e16_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e16-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e16,m1 data with e64 indices should preserve inactive compact halfword lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e16-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e16,m1 data with e64 indices should preserve inactive compact halfword lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e32_m1_data_e8_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e32-data-e8-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "mixed-width indexed e32,m1 data with e8 indices should move selected contiguous word offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e32-data-e8-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed mixed-width indexed e32,m1 data with e8 indices should move selected contiguous word offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e32_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e32-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked mixed-width indexed e32,m1 data with e8 indices should preserve inactive word lanes and skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e32-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked mixed-width indexed e32,m1 data with e8 indices should preserve inactive word lanes and skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e32_m1_data_e16_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e32-data-e16-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "mixed-width indexed e32,m1 data with e16 indices should move selected contiguous word offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e32-data-e16-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed mixed-width indexed e32,m1 data with e16 indices should move selected contiguous word offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e32_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e32-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked mixed-width indexed e32,m1 data with e16 indices should preserve inactive compact word lanes and skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e32-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked mixed-width indexed e32,m1 data with e16 indices should preserve inactive compact word lanes and skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e32_m1_data_e64_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e32-data-e64-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "mixed-width indexed e32,m1 data with e64 indices should move selected contiguous word offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e32-data-e64-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed mixed-width indexed e32,m1 data with e64 indices should move selected contiguous word offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e32_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e32-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked mixed-width indexed e32,m1 data with e64 indices should preserve inactive compact word lanes and skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e32-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked mixed-width indexed e32,m1 data with e64 indices should preserve inactive compact word lanes and skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e32_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e32-data-e64-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e32,m1 data with e64 indices should preserve interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e32-data-e64-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e32,m1 data with e64 indices should preserve interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e32_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e32-data-e16-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e32,m1 data with e16 indices should preserve interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e32-data-e16-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e32,m1 data with e16 indices should preserve interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e32_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e32-data-e8-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e32,m1 data with e8 indices should preserve interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e32-data-e8-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e32,m1 data with e8 indices should preserve interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_indexed_e32_m1_memory() {
    const EXPECTED_INDEXED_MEMORY_EXTRA_EXECUTE_CYCLES: u64 = 3;

    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-e32-m1-load-store",
        &indexed_e32_m1_vector_memory_program(),
        300,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "indexed e32,m1 vector memory should move selected offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-e32-m1-load-store",
        &indexed_e32_m1_vector_memory_program(),
        820,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed indexed e32,m1 vector memory should move selected offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );

    let unit_stride_stats = in_order_pipeline_payload_stats(
        "in-order-vector-indexed-unit-stride-latency-baseline",
        &unit_stride_memory_program(true),
    );
    assert_eq!(
        stat_value(
            &direct_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ) - stat_value(
            &unit_stride_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        EXPECTED_INDEXED_MEMORY_EXTRA_EXECUTE_CYCLES,
        "indexed vector load/store should add the fixed vector LSU execute latency over the unit-stride vector memory baseline\nindexed stats:\n{direct_stats}\nbaseline stats:\n{unit_stride_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-e32-m1-load-store",
        &sparse_indexed_e32_m1_vector_memory_program(),
        340,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "sparse indexed e32,m1 vector memory should preserve two interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-e32-m1-load-store",
        &sparse_indexed_e32_m1_vector_memory_program(),
        900,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed sparse indexed e32,m1 vector memory should preserve two interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_leading_gap_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-leading-gap-e32-m1-load-store",
        &leading_gap_indexed_e32_m1_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "leading-gap indexed e32,m1 vector memory should preserve leading and interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-leading-gap-e32-m1-load-store",
        &leading_gap_indexed_e32_m1_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed leading-gap indexed e32,m1 vector memory should preserve leading and interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_reversed_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-reversed-e32-m1-load-store",
        &reversed_indexed_e32_m1_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "reversed indexed e32,m1 vector memory should follow non-monotonic offsets and preserve skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-reversed-e32-m1-load-store",
        &reversed_indexed_e32_m1_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed reversed indexed e32,m1 vector memory should follow non-monotonic offsets and preserve skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e64_m1_data_e8_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e64-data-e8-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        28,
        "mixed-width indexed e64,m1 data with e8 indices should move selected contiguous 64-bit offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e64-data-e8-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        840,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        28,
        "cache-backed mixed-width indexed e64,m1 data with e8 indices should move selected contiguous 64-bit offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e64_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e64-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "masked mixed-width indexed e64,m1 data with e8 indices should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e64-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed masked mixed-width indexed e64,m1 data with e8 indices should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e64_m1_data_e16_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e64-data-e16-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        28,
        "mixed-width indexed e64,m1 data with e16 indices should move selected contiguous 64-bit offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e64-data-e16-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        840,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        28,
        "cache-backed mixed-width indexed e64,m1 data with e16 indices should move selected contiguous 64-bit offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e64_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e64-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "masked mixed-width indexed e64,m1 data with e16 indices should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e64-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed masked mixed-width indexed e64,m1 data with e16 indices should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e64_m1_data_e32_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e64-data-e32-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        28,
        "mixed-width indexed e64,m1 data with e32 indices should move selected contiguous 64-bit offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e64-data-e32-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        840,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        28,
        "cache-backed mixed-width indexed e64,m1 data with e32 indices should move selected contiguous 64-bit offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e64_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e64-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "masked mixed-width indexed e64,m1 data with e32 indices should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e64-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed masked mixed-width indexed e64,m1 data with e32 indices should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_indexed_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-e64-m1-load-store",
        &indexed_e64_m1_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        27,
        "indexed e64,m1 vector memory should move two 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-e64-m1-load-store",
        &indexed_e64_m1_vector_memory_program(),
        760,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        27,
        "cache-backed indexed e64,m1 vector memory should move two 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_vector_indexed_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-e64-m1-load-store",
        &sparse_indexed_e64_m1_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "sparse indexed e64,m1 vector memory should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-e64-m1-load-store",
        &sparse_indexed_e64_m1_vector_memory_program(),
        980,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed sparse indexed e64,m1 vector memory should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e64_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e64-data-e8-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e64,m1 data with e8 indices should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e64-data-e8-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        1020,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e64,m1 data with e8 indices should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e64_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e64-data-e16-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e64,m1 data with e16 indices should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e64-data-e16-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        1020,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e64,m1 data with e16 indices should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e64_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e64-data-e32-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e64,m1 data with e32 indices should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e64-data-e32-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        1020,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e64,m1 data with e32 indices should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_indexed_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-e64-m1-load-store",
        &masked_indexed_e64_m1_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        32,
        "masked indexed e64,m1 vector memory should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-e64-m1-load-store",
        &masked_indexed_e64_m1_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        32,
        "cache-backed masked indexed e64,m1 vector memory should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_vector_indexed_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-e64-m1-load-store",
        &masked_sparse_indexed_e64_m1_vector_memory_program(),
        520,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked sparse indexed e64,m1 vector memory should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-e64-m1-load-store",
        &masked_sparse_indexed_e64_m1_vector_memory_program(),
        1320,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked sparse indexed e64,m1 vector memory should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e64_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e64-data-e8-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        540,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e64,m1 data with e8 indices should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e64-data-e8-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        1360,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e64,m1 data with e8 indices should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e64_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e64-data-e16-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        540,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e64,m1 data with e16 indices should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e64-data-e16-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        1360,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e64,m1 data with e16 indices should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e64_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e64-data-e32-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        540,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e64,m1 data with e32 indices should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e64-data-e32-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        1360,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e64,m1 data with e32 indices should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-e32-m1-load-store",
        &masked_indexed_e32_m1_vector_memory_program(),
        380,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked indexed e32,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-e32-m1-load-store",
        &masked_indexed_e32_m1_vector_memory_program(),
        1040,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked indexed e32,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e32_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e32-data-e64-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e32,m1 data with e64 indices should preserve inactive compact word lanes and interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e32-data-e64-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e32,m1 data with e64 indices should preserve inactive compact word lanes and interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e32_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e32-data-e16-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e32,m1 data with e16 indices should preserve inactive compact word lanes and interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e32-data-e16-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e32,m1 data with e16 indices should preserve inactive compact word lanes and interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e32_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e32-data-e8-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e32,m1 data with e8 indices should preserve inactive compact word lanes and interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e32-data-e8-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e32,m1 data with e8 indices should preserve inactive compact word lanes and interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-e32-m1-load-store",
        &masked_sparse_indexed_e32_m1_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked sparse indexed e32,m1 vector memory should preserve interior gaps and the inactive sparse lane through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-e32-m1-load-store",
        &masked_sparse_indexed_e32_m1_vector_memory_program(),
        1120,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked sparse indexed e32,m1 vector memory should preserve interior gaps and the inactive sparse lane through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_leading_gap_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-leading-gap-e32-m1-load-store",
        &masked_leading_gap_indexed_e32_m1_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked leading-gap indexed e32,m1 vector memory should preserve the leading gap, interior gap, and inactive sparse lane through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-leading-gap-e32-m1-load-store",
        &masked_leading_gap_indexed_e32_m1_vector_memory_program(),
        1140,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked leading-gap indexed e32,m1 vector memory should preserve the leading gap, interior gap, and inactive sparse lane through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_trailing_active_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-trailing-active-e32-m1-load-store",
        &masked_trailing_active_indexed_e32_m1_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked trailing-active indexed e32,m1 vector memory should suppress the first lane while loading and storing the later active indexed lane through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-trailing-active-e32-m1-load-store",
        &masked_trailing_active_indexed_e32_m1_vector_memory_program(),
        1140,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked trailing-active indexed e32,m1 vector memory should suppress the first lane while loading and storing the later active indexed lane through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_reversed_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-reversed-e32-m1-load-store",
        &masked_reversed_indexed_e32_m1_vector_memory_program(),
        460,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked reversed indexed e32,m1 vector memory should keep the inactive lower-offset lane untouched while storing only the active high-offset lane through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-reversed-e32-m1-load-store",
        &masked_reversed_indexed_e32_m1_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked reversed indexed e32,m1 vector memory should keep the inactive lower-offset lane untouched while storing only the active high-offset lane through the top-level run path\nstats:\n{cache_stats}"
    );
}
