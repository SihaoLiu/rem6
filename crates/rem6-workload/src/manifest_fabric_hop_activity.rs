use crate::{
    WorkloadError, WorkloadExpectedFabricHopActivity, WorkloadManifest, WorkloadManifestBuilder,
    WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_fabric_hop_activity(&self) -> &[WorkloadExpectedFabricHopActivity] {
        &self.expected_fabric_hop_activity
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_fabric_hop_activity(
        mut self,
        expected: WorkloadExpectedFabricHopActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_fabric_hop_activity(&mut self.expected_fabric_hop_activity, expected)?;
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_fabric_hop_activity(
        mut self,
        expected: WorkloadExpectedFabricHopActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_fabric_hop_activity(&mut self.expected_fabric_hop_activity, expected)?;
        Ok(self)
    }

    pub fn expected_fabric_hop_activity(&self) -> &[WorkloadExpectedFabricHopActivity] {
        &self.expected_fabric_hop_activity
    }
}

fn add_expected_fabric_hop_activity(
    activity: &mut Vec<WorkloadExpectedFabricHopActivity>,
    expected: WorkloadExpectedFabricHopActivity,
) -> Result<(), WorkloadError> {
    if activity.iter().any(|existing| {
        existing.hop_index() == expected.hop_index()
            && existing.link() == expected.link()
            && existing.virtual_network() == expected.virtual_network()
    }) {
        return Err(WorkloadError::DuplicateExpectedFabricHopActivity {
            hop_index: expected.hop_index(),
            link: expected.link().clone(),
            virtual_network: expected.virtual_network(),
        });
    }
    activity.push(expected);
    activity.sort_by(|left, right| left.sort_key().cmp(&right.sort_key()));
    Ok(())
}
