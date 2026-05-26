use crate::{
    WorkloadError, WorkloadExpectedFabricVirtualNetworkActivity, WorkloadManifest,
    WorkloadManifestBuilder, WorkloadReplayPlan,
};

impl WorkloadManifest {
    pub fn expected_fabric_virtual_network_activity(
        &self,
    ) -> &[WorkloadExpectedFabricVirtualNetworkActivity] {
        &self.expected_fabric_virtual_network_activity
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_fabric_virtual_network_activity(
        mut self,
        expected: WorkloadExpectedFabricVirtualNetworkActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_fabric_virtual_network_activity(
            &mut self.expected_fabric_virtual_network_activity,
            expected,
        )?;
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_fabric_virtual_network_activity(
        mut self,
        expected: WorkloadExpectedFabricVirtualNetworkActivity,
    ) -> Result<Self, WorkloadError> {
        add_expected_fabric_virtual_network_activity(
            &mut self.expected_fabric_virtual_network_activity,
            expected,
        )?;
        Ok(self)
    }

    pub fn expected_fabric_virtual_network_activity(
        &self,
    ) -> &[WorkloadExpectedFabricVirtualNetworkActivity] {
        &self.expected_fabric_virtual_network_activity
    }
}

fn add_expected_fabric_virtual_network_activity(
    activity: &mut Vec<WorkloadExpectedFabricVirtualNetworkActivity>,
    expected: WorkloadExpectedFabricVirtualNetworkActivity,
) -> Result<(), WorkloadError> {
    if activity
        .iter()
        .any(|existing| existing.virtual_network() == expected.virtual_network())
    {
        return Err(
            WorkloadError::DuplicateExpectedFabricVirtualNetworkActivity {
                virtual_network: expected.virtual_network(),
            },
        );
    }
    activity.push(expected);
    activity.sort_by_key(|activity| activity.sort_key());
    Ok(())
}
