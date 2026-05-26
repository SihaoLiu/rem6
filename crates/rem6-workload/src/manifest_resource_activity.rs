use crate::{
    WorkloadError, WorkloadExpectedResourceActivity, WorkloadManifest, WorkloadManifestBuilder,
    WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_resource_activity(&self) -> &[WorkloadExpectedResourceActivity] {
        &self.expected_resource_activity
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_resource_activity(
        mut self,
        expected: WorkloadExpectedResourceActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_resource_activity(&mut self.expected_resource_activity, expected)?;
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_resource_activity(
        mut self,
        expected: WorkloadExpectedResourceActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_resource_activity(&mut self.expected_resource_activity, expected)?;
        Ok(self)
    }

    pub fn expected_resource_activity(&self) -> &[WorkloadExpectedResourceActivity] {
        &self.expected_resource_activity
    }
}

fn add_expected_resource_activity(
    activity: &mut Vec<WorkloadExpectedResourceActivity>,
    expected: WorkloadExpectedResourceActivity,
) -> Result<(), WorkloadError> {
    if activity
        .iter()
        .any(|existing| existing.sort_key() == expected.sort_key())
    {
        return Err(WorkloadError::DuplicateExpectedResourceActivity {
            scope: expected.scope(),
        });
    }
    activity.push(expected);
    activity.sort_by_key(|activity| activity.sort_key());
    Ok(())
}
