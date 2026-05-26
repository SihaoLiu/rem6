use crate::{
    WorkloadError, WorkloadExpectedFabricLaneActivity, WorkloadManifest, WorkloadManifestBuilder,
    WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_fabric_lane_activity(&self) -> &[WorkloadExpectedFabricLaneActivity] {
        &self.expected_fabric_lane_activity
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_fabric_lane_activity(
        mut self,
        expected: WorkloadExpectedFabricLaneActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_fabric_lane_activity(&mut self.expected_fabric_lane_activity, expected)?;
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_fabric_lane_activity(
        mut self,
        expected: WorkloadExpectedFabricLaneActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_fabric_lane_activity(&mut self.expected_fabric_lane_activity, expected)?;
        Ok(self)
    }

    pub fn expected_fabric_lane_activity(&self) -> &[WorkloadExpectedFabricLaneActivity] {
        &self.expected_fabric_lane_activity
    }
}

fn add_expected_fabric_lane_activity(
    activity: &mut Vec<WorkloadExpectedFabricLaneActivity>,
    expected: WorkloadExpectedFabricLaneActivity,
) -> Result<(), WorkloadError> {
    if activity.iter().any(|existing| {
        existing.link() == expected.link()
            && existing.virtual_network() == expected.virtual_network()
    }) {
        return Err(WorkloadError::DuplicateExpectedFabricLaneActivity {
            link: expected.link().clone(),
            virtual_network: expected.virtual_network(),
        });
    }
    activity.push(expected);
    activity.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    Ok(())
}
