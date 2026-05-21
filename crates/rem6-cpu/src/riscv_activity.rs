use rem6_kernel::PartitionId;

use crate::RiscvCoreDriveAction;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RiscvCoreDriveActivity {
    fetch_issue_count: usize,
    instruction_execution_count: usize,
    data_access_issue_count: usize,
}

impl RiscvCoreDriveActivity {
    pub const fn new(
        fetch_issue_count: usize,
        instruction_execution_count: usize,
        data_access_issue_count: usize,
    ) -> Self {
        Self {
            fetch_issue_count,
            instruction_execution_count,
            data_access_issue_count,
        }
    }

    pub const fn fetch_issue_count(self) -> usize {
        self.fetch_issue_count
    }

    pub const fn instruction_execution_count(self) -> usize {
        self.instruction_execution_count
    }

    pub const fn data_access_issue_count(self) -> usize {
        self.data_access_issue_count
    }

    pub const fn total_drive_action_count(self) -> usize {
        self.fetch_issue_count + self.instruction_execution_count + self.data_access_issue_count
    }

    pub const fn has_activity(self) -> bool {
        self.total_drive_action_count() != 0
    }

    pub(crate) fn record_action(&mut self, action: &RiscvCoreDriveAction) {
        match action {
            RiscvCoreDriveAction::FetchIssued { .. } => self.fetch_issue_count += 1,
            RiscvCoreDriveAction::InstructionExecuted(_) => {
                self.instruction_execution_count += 1;
            }
            RiscvCoreDriveAction::DataAccessIssued { .. } => self.data_access_issue_count += 1,
        }
    }

    pub(crate) fn merge(self, other: Self) -> Self {
        Self {
            fetch_issue_count: self.fetch_issue_count + other.fetch_issue_count,
            instruction_execution_count: self.instruction_execution_count
                + other.instruction_execution_count,
            data_access_issue_count: self.data_access_issue_count + other.data_access_issue_count,
        }
    }
}

pub(crate) fn drive_action_partition(action: &RiscvCoreDriveAction) -> PartitionId {
    match action {
        RiscvCoreDriveAction::FetchIssued { event }
        | RiscvCoreDriveAction::DataAccessIssued { event } => event.partition(),
        RiscvCoreDriveAction::InstructionExecuted(event) => event.fetch().partition(),
    }
}
