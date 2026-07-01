pub(super) fn saturating_branch_counter(counter: u8, taken: bool) -> u8 {
    match taken {
        true => counter.saturating_add(1).min(super::STRONGLY_TAKEN),
        false => counter.saturating_sub(1),
    }
}

pub(super) fn history_mask(bits: u8) -> u64 {
    if bits == 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}
