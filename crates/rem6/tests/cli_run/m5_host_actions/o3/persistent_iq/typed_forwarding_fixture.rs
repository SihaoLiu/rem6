use std::path::PathBuf;

use serde_json::Value;

use super::mixed_compute_fixture::*;
use super::*;

pub(super) const TYPED_FP_PRODUCER_PC: &str = "0x80000044";
pub(super) const TYPED_FP_CONSUMER_PC: &str = "0x80000048";
pub(super) const TYPED_VECTOR_PRODUCER_PC: &str = "0x8000004c";
pub(super) const TYPED_INTEGER_CONSUMER_PC: &str = "0x80000050";
pub(super) const TYPED_RESULTS: &str = "000010410a000000";

pub(super) fn typed_forwarding_binary(name: &str) -> PathBuf {
    let mut words = mixed_compute_prefix();
    append_mixed_compute_head(&mut words);
    words.extend([
        fp_add_s(4, 1, 2),            // f4 = 3.0f
        fp_mul_s(5, 4, 3),            // f5 = 9.0f, live f4
        vmv_x_s_type(3, 11),          // x11 = 9, live vector-result row
        i_type(1, 11, 0, 13, 0x13),   // x13 = 10, live x11
        fp_r_type(0x70, 0, 5, 0, 14), // fmv.x.w x14, f5
        s_type(0, 14, 12, 0b010),
        s_type(4, 13, 12, 0b010),
        i_type(0, 0, 0, 10, 0x13),
        i_type(0, 0, 0, 11, 0x13),
        m5op(M5_DUMP_STATS),
    ]);
    append_mixed_compute_data(name, words)
}

pub(super) fn run_typed_forwarding_json(
    issue_width: usize,
    memory_system: &str,
    switch_mode: &str,
    extra_args: &[&str],
) -> Value {
    let path = typed_forwarding_binary(&format!(
        "o3-typed-live-forwarding-{memory_system}-width-{issue_width}"
    ));
    let mut args = vec!["--riscv-o3-scalar-live-window-depth", "6"];
    args.extend_from_slice(extra_args);
    run_mixed_compute_path_json(&path, issue_width, memory_system, switch_mode, 8, &args)
}
