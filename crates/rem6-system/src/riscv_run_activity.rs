use std::collections::BTreeMap;

use rem6_cpu::{CpuId, RiscvClusterTurn, RiscvCoreDriveActivity};

use crate::ScheduledRiscvTrap;

pub type RiscvSystemRunPartitionActivity = RiscvSystemRunCpuActivity;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvSystemRunCpuActivity {
    core: RiscvCoreDriveActivity,
    scheduled_trap_count: usize,
}

impl RiscvSystemRunCpuActivity {
    pub const fn new(
        fetch_issue_count: usize,
        instruction_execution_count: usize,
        data_access_issue_count: usize,
        scheduled_trap_count: usize,
    ) -> Self {
        Self {
            core: RiscvCoreDriveActivity::new(
                fetch_issue_count,
                instruction_execution_count,
                data_access_issue_count,
            ),
            scheduled_trap_count,
        }
    }

    pub const fn from_core_activity(core: RiscvCoreDriveActivity) -> Self {
        Self {
            core,
            scheduled_trap_count: 0,
        }
    }

    pub const fn core_activity(self) -> RiscvCoreDriveActivity {
        self.core
    }

    pub const fn fetch_issue_count(self) -> usize {
        self.core.fetch_issue_count()
    }

    pub const fn instruction_execution_count(self) -> usize {
        self.core.instruction_execution_count()
    }

    pub const fn data_access_issue_count(self) -> usize {
        self.core.data_access_issue_count()
    }

    pub const fn scheduled_trap_count(self) -> usize {
        self.scheduled_trap_count
    }

    pub const fn total_core_action_count(self) -> usize {
        self.core.total_drive_action_count()
    }

    pub const fn total_activity_count(self) -> usize {
        self.total_core_action_count() + self.scheduled_trap_count
    }

    pub const fn has_core_activity(self) -> bool {
        self.core.has_activity()
    }

    pub const fn has_trap_activity(self) -> bool {
        self.scheduled_trap_count != 0
    }

    pub const fn has_activity(self) -> bool {
        self.total_activity_count() != 0
    }

    fn merge_core_activity(&mut self, core: RiscvCoreDriveActivity) {
        *self = Self::new(
            self.fetch_issue_count() + core.fetch_issue_count(),
            self.instruction_execution_count() + core.instruction_execution_count(),
            self.data_access_issue_count() + core.data_access_issue_count(),
            self.scheduled_trap_count,
        );
    }

    fn record_scheduled_trap(&mut self) {
        self.scheduled_trap_count += 1;
    }
}

pub(crate) fn collect_riscv_system_run_cpu_activity(
    turns: &[RiscvClusterTurn],
    scheduled_traps: &[ScheduledRiscvTrap],
) -> BTreeMap<CpuId, RiscvSystemRunCpuActivity> {
    let mut activities = BTreeMap::new();
    for turn in turns {
        for (cpu, core_activity) in turn.cpu_activities() {
            activities
                .entry(cpu)
                .and_modify(|activity: &mut RiscvSystemRunCpuActivity| {
                    activity.merge_core_activity(core_activity);
                })
                .or_insert_with(|| RiscvSystemRunCpuActivity::from_core_activity(core_activity));
        }
    }
    for trap in scheduled_traps {
        activities
            .entry(trap.cpu())
            .or_default()
            .record_scheduled_trap();
    }
    activities
}

pub(crate) fn collect_riscv_system_run_partition_activity(
    turns: &[RiscvClusterTurn],
    scheduled_traps: &[ScheduledRiscvTrap],
) -> BTreeMap<rem6_kernel::PartitionId, RiscvSystemRunPartitionActivity> {
    let mut activities = BTreeMap::new();
    for turn in turns {
        for (partition, core_activity) in turn.partition_activities() {
            activities
                .entry(partition)
                .and_modify(|activity: &mut RiscvSystemRunPartitionActivity| {
                    activity.merge_core_activity(core_activity);
                })
                .or_insert_with(|| {
                    RiscvSystemRunPartitionActivity::from_core_activity(core_activity)
                });
        }
    }
    for trap in scheduled_traps {
        activities
            .entry(trap.source_partition())
            .or_default()
            .record_scheduled_trap();
    }
    activities
}
