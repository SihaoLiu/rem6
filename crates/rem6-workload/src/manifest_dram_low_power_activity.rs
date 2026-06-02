use crate::{
    WorkloadError, WorkloadExpectedDramLowPowerActivity, WorkloadManifest, WorkloadManifestBuilder,
    WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub const fn expected_dram_low_power_activity(
        &self,
    ) -> Option<WorkloadExpectedDramLowPowerActivity> {
        self.expected_dram_low_power_activity
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_dram_low_power_activity(
        mut self,
        expected: WorkloadExpectedDramLowPowerActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_dram_low_power_activity(&mut self.expected_dram_low_power_activity, expected)?;
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_dram_low_power_activity(
        mut self,
        expected: WorkloadExpectedDramLowPowerActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_dram_low_power_activity(&mut self.expected_dram_low_power_activity, expected)?;
        Ok(self)
    }

    pub const fn expected_dram_low_power_activity(
        &self,
    ) -> Option<WorkloadExpectedDramLowPowerActivity> {
        self.expected_dram_low_power_activity
    }
}

fn add_expected_dram_low_power_activity(
    activity: &mut Option<WorkloadExpectedDramLowPowerActivity>,
    expected: WorkloadExpectedDramLowPowerActivity,
) -> Result<(), WorkloadError> {
    if activity.is_some() {
        return Err(WorkloadError::DuplicateExpectedResourceActivity {
            scope: crate::WorkloadResourceActivityScope::Dram,
        });
    }
    *activity = Some(expected);
    Ok(())
}
