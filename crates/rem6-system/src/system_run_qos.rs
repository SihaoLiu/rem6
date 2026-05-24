use rem6_fabric::{QosPriority, QosRequestorId};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn dram_qos_access_count(&self) -> usize {
        self.dram_profile().qos_access_count()
    }

    pub fn dram_qos_byte_count(&self) -> u64 {
        self.dram_profile().qos_byte_count()
    }

    pub fn dram_qos_escalated_access_count(&self) -> usize {
        self.dram_profile().qos_escalated_access_count()
    }

    pub fn dram_qos_priority_access_count(&self, priority: QosPriority) -> usize {
        self.dram_profile().qos_priority_access_count(priority)
    }

    pub fn dram_qos_priority_byte_count(&self, priority: QosPriority) -> u64 {
        self.dram_profile().qos_priority_byte_count(priority)
    }

    pub fn dram_qos_requestor_access_count(&self, requestor: QosRequestorId) -> usize {
        self.dram_profile().qos_requestor_access_count(requestor)
    }

    pub fn dram_qos_requestor_byte_count(&self, requestor: QosRequestorId) -> u64 {
        self.dram_profile().qos_requestor_byte_count(requestor)
    }

    pub fn has_dram_qos_activity(&self) -> bool {
        self.dram_qos_access_count() != 0
    }
}
