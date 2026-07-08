use rem6_cpu::{BranchTargetKind, O3RuntimeTraceRecord};

use super::o3_branch_repair::{o3_branch_targetless_mismatch, o3_branch_wrong_target};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6O3BranchTargetMismatchTotals {
    targetless_mismatches: u64,
    targetless_mismatch_without_link_writes: u64,
    targetless_mismatch_squashed_targets: u64,
    targetless_mismatch_squashed_target_without_link_writes: u64,
    wrong_targets: u64,
    wrong_target_squashed_targets: u64,
    wrong_target_squashed_target_without_link_writes: u64,
    wrong_target_link_writes: u64,
    targetless_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    targetless_mismatch_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
    targetless_mismatch_squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    targetless_mismatch_squashed_target_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
    wrong_target_kinds: [u64; BranchTargetKind::COUNT],
    wrong_target_squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    wrong_target_squashed_target_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
    wrong_target_link_write_kinds: [u64; BranchTargetKind::COUNT],
}

impl Rem6O3BranchTargetMismatchTotals {
    fn add_event(&mut self, event: &O3RuntimeTraceRecord) {
        if !event.branch_event() {
            return;
        }

        let index = event.branch_kind().index();
        let link_write = event.branch_link_register_write();
        let squashed_target = event.branch_squashed_target().is_some();
        let targetless_mismatch = o3_branch_targetless_mismatch(event);
        let wrong_target = o3_branch_wrong_target(event);

        if targetless_mismatch {
            self.targetless_mismatches = self.targetless_mismatches.saturating_add(1);
            self.targetless_mismatch_kinds[index] =
                self.targetless_mismatch_kinds[index].saturating_add(1);
            if !link_write {
                self.targetless_mismatch_without_link_writes = self
                    .targetless_mismatch_without_link_writes
                    .saturating_add(1);
                self.targetless_mismatch_without_link_write_kinds[index] =
                    self.targetless_mismatch_without_link_write_kinds[index].saturating_add(1);
            }
            if squashed_target {
                self.targetless_mismatch_squashed_targets =
                    self.targetless_mismatch_squashed_targets.saturating_add(1);
                self.targetless_mismatch_squashed_target_kinds[index] =
                    self.targetless_mismatch_squashed_target_kinds[index].saturating_add(1);
                if !link_write {
                    self.targetless_mismatch_squashed_target_without_link_writes = self
                        .targetless_mismatch_squashed_target_without_link_writes
                        .saturating_add(1);
                    self.targetless_mismatch_squashed_target_without_link_write_kinds[index] = self
                        .targetless_mismatch_squashed_target_without_link_write_kinds[index]
                        .saturating_add(1);
                }
            }
        }

        if wrong_target {
            self.wrong_targets = self.wrong_targets.saturating_add(1);
            self.wrong_target_kinds[index] = self.wrong_target_kinds[index].saturating_add(1);
            if link_write {
                self.wrong_target_link_writes = self.wrong_target_link_writes.saturating_add(1);
                self.wrong_target_link_write_kinds[index] =
                    self.wrong_target_link_write_kinds[index].saturating_add(1);
            }
            if squashed_target {
                self.wrong_target_squashed_targets =
                    self.wrong_target_squashed_targets.saturating_add(1);
                self.wrong_target_squashed_target_kinds[index] =
                    self.wrong_target_squashed_target_kinds[index].saturating_add(1);
                if !link_write {
                    self.wrong_target_squashed_target_without_link_writes = self
                        .wrong_target_squashed_target_without_link_writes
                        .saturating_add(1);
                    self.wrong_target_squashed_target_without_link_write_kinds[index] = self
                        .wrong_target_squashed_target_without_link_write_kinds[index]
                        .saturating_add(1);
                }
            }
        }
    }

    fn wrong_target_without_link_writes(self) -> u64 {
        self.wrong_targets
            .saturating_sub(self.wrong_target_link_writes)
    }

    fn wrong_target_squashed_target_link_writes(self) -> u64 {
        self.wrong_target_squashed_targets
            .saturating_sub(self.wrong_target_squashed_target_without_link_writes)
    }

    fn wrong_target_squashed_target_link_write_kind(self, kind: BranchTargetKind) -> u64 {
        self.wrong_target_squashed_target_kinds[kind.index()].saturating_sub(
            self.wrong_target_squashed_target_without_link_write_kinds[kind.index()],
        )
    }

    fn wrong_target_without_link_write_kind(self, kind: BranchTargetKind) -> u64 {
        self.wrong_target_kinds[kind.index()]
            .saturating_sub(self.wrong_target_link_write_kinds[kind.index()])
    }
}

fn o3_branch_target_mismatch_kind_json<F>(count: F) -> String
where
    F: Fn(BranchTargetKind) -> u64,
{
    let fields = BranchTargetKind::ALL
        .into_iter()
        .map(|kind| format!("\"{}\":{}", kind.canonical_stat_name(), count(kind)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

pub(crate) fn o3_branch_target_mismatch_to_json(events: &[O3RuntimeTraceRecord]) -> String {
    let mut totals = Rem6O3BranchTargetMismatchTotals::default();
    for event in events {
        totals.add_event(event);
    }

    let targetless_mismatch_kind =
        o3_branch_target_mismatch_kind_json(|kind| totals.targetless_mismatch_kinds[kind.index()]);
    let targetless_mismatch_without_link_write_kind = o3_branch_target_mismatch_kind_json(|kind| {
        totals.targetless_mismatch_without_link_write_kinds[kind.index()]
    });
    let targetless_mismatch_squashed_target_kind = o3_branch_target_mismatch_kind_json(|kind| {
        totals.targetless_mismatch_squashed_target_kinds[kind.index()]
    });
    let targetless_mismatch_squashed_target_without_link_write_kind =
        o3_branch_target_mismatch_kind_json(|kind| {
            totals.targetless_mismatch_squashed_target_without_link_write_kinds[kind.index()]
        });
    let wrong_target_kind =
        o3_branch_target_mismatch_kind_json(|kind| totals.wrong_target_kinds[kind.index()]);
    let wrong_target_squashed_target_kind = o3_branch_target_mismatch_kind_json(|kind| {
        totals.wrong_target_squashed_target_kinds[kind.index()]
    });
    let wrong_target_squashed_target_without_link_write_kind =
        o3_branch_target_mismatch_kind_json(|kind| {
            totals.wrong_target_squashed_target_without_link_write_kinds[kind.index()]
        });
    let wrong_target_squashed_target_link_write_kind =
        o3_branch_target_mismatch_kind_json(|kind| {
            totals.wrong_target_squashed_target_link_write_kind(kind)
        });
    let wrong_target_link_write_kind = o3_branch_target_mismatch_kind_json(|kind| {
        totals.wrong_target_link_write_kinds[kind.index()]
    });
    let wrong_target_without_link_write_kind = o3_branch_target_mismatch_kind_json(|kind| {
        totals.wrong_target_without_link_write_kind(kind)
    });
    format!(
        "{{\"targetless_mismatches\":{},\"targetless_mismatch_without_link_writes\":{},\"targetless_mismatch_squashed_targets\":{},\"targetless_mismatch_squashed_target_without_link_writes\":{},\"wrong_targets\":{},\"wrong_target_squashed_targets\":{},\"wrong_target_squashed_target_without_link_writes\":{},\"wrong_target_squashed_target_link_writes\":{},\"wrong_target_link_writes\":{},\"wrong_target_without_link_writes\":{},\"targetless_mismatch_kind\":{},\"targetless_mismatch_without_link_write_kind\":{},\"targetless_mismatch_squashed_target_kind\":{},\"targetless_mismatch_squashed_target_without_link_write_kind\":{},\"wrong_target_kind\":{},\"wrong_target_squashed_target_kind\":{},\"wrong_target_squashed_target_without_link_write_kind\":{},\"wrong_target_squashed_target_link_write_kind\":{},\"wrong_target_link_write_kind\":{},\"wrong_target_without_link_write_kind\":{}}}",
        totals.targetless_mismatches,
        totals.targetless_mismatch_without_link_writes,
        totals.targetless_mismatch_squashed_targets,
        totals.targetless_mismatch_squashed_target_without_link_writes,
        totals.wrong_targets,
        totals.wrong_target_squashed_targets,
        totals.wrong_target_squashed_target_without_link_writes,
        totals.wrong_target_squashed_target_link_writes(),
        totals.wrong_target_link_writes,
        totals.wrong_target_without_link_writes(),
        targetless_mismatch_kind,
        targetless_mismatch_without_link_write_kind,
        targetless_mismatch_squashed_target_kind,
        targetless_mismatch_squashed_target_without_link_write_kind,
        wrong_target_kind,
        wrong_target_squashed_target_kind,
        wrong_target_squashed_target_without_link_write_kind,
        wrong_target_squashed_target_link_write_kind,
        wrong_target_link_write_kind,
        wrong_target_without_link_write_kind,
    )
}
