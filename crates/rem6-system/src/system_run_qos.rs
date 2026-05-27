use rem6_fabric::{QosPriority, QosRequestorId};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn dram_operation_count(&self) -> usize {
        let dram = self.dram_profile();
        let qos_priority_access_count = dram
            .qos_priorities()
            .into_iter()
            .fold(0usize, |count, priority| {
                count.saturating_add(dram.qos_priority_access_count(priority))
            });
        let qos_requestor_access_count = dram
            .qos_requestors()
            .into_iter()
            .fold(0usize, |count, requestor| {
                count.saturating_add(dram.qos_requestor_access_count(requestor))
            });

        dram.access_count()
            .max(dram.read_count().saturating_add(dram.write_count()))
            .max(dram.row_hit_count().saturating_add(dram.row_miss_count()))
            .max(dram.command_count())
            .max(dram.qos_access_count())
            .max(qos_priority_access_count)
            .max(qos_requestor_access_count)
    }

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
        let dram = self.dram_profile();
        self.dram_qos_access_count() != 0
            || self.dram_qos_byte_count() != 0
            || self.dram_qos_escalated_access_count() != 0
            || !dram.qos_priorities().is_empty()
            || !dram.qos_requestors().is_empty()
    }
}
