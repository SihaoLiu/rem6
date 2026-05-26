use crate::{
    WorkloadError, WorkloadExpectedFabricLinkActivity, WorkloadManifest, WorkloadManifestBuilder,
    WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_fabric_link_activity(&self) -> &[WorkloadExpectedFabricLinkActivity] {
        &self.expected_fabric_link_activity
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_fabric_link_activity(
        mut self,
        expected: WorkloadExpectedFabricLinkActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_fabric_link_activity(&mut self.expected_fabric_link_activity, expected)?;
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_fabric_link_activity(
        mut self,
        expected: WorkloadExpectedFabricLinkActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_fabric_link_activity(&mut self.expected_fabric_link_activity, expected)?;
        Ok(self)
    }

    pub fn expected_fabric_link_activity(&self) -> &[WorkloadExpectedFabricLinkActivity] {
        &self.expected_fabric_link_activity
    }
}

fn add_expected_fabric_link_activity(
    activity: &mut Vec<WorkloadExpectedFabricLinkActivity>,
    expected: WorkloadExpectedFabricLinkActivity,
) -> Result<(), WorkloadError> {
    if activity
        .iter()
        .any(|existing| existing.link() == expected.link())
    {
        return Err(WorkloadError::DuplicateExpectedFabricLinkActivity {
            link: expected.link().clone(),
        });
    }
    activity.push(expected);
    activity.sort_by(|left, right| left.sort_key().cmp(right.sort_key()));
    Ok(())
}
