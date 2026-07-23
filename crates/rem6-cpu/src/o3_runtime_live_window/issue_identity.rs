use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvInstruction};

use super::super::o3_runtime_issue::queue::O3LiveIssuePacket;
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveStagedFetchIdentity {
    instruction: RiscvInstruction,
    issue_packet: Option<O3LiveIssuePacket>,
    pub(in crate::o3_runtime) producer_forwarded_control_target:
        Option<O3ProducerForwardedControlTarget>,
    pub(in crate::o3_runtime) producer_forwarded_control_speculation: Option<BranchSpeculationId>,
    producer_forwarded_return_descendant: Option<O3ProducerForwardedReturnDescendant>,
}

impl O3LiveStagedFetchIdentity {
    pub(in crate::o3_runtime) const fn new(instruction: RiscvInstruction) -> Self {
        Self {
            instruction,
            issue_packet: None,
            producer_forwarded_control_target: None,
            producer_forwarded_control_speculation: None,
            producer_forwarded_return_descendant: None,
        }
    }
    pub(in crate::o3_runtime) const fn forwarded_control_target_identity(
        &self,
    ) -> Option<O3ProducerForwardedControlTarget> {
        self.producer_forwarded_control_target
    }
    pub(in crate::o3_runtime) fn record_forwarded_return_identity(
        &mut self,
        descendant: O3ProducerForwardedReturnDescendant,
    ) {
        self.producer_forwarded_return_descendant = Some(descendant);
    }
    pub(in crate::o3_runtime) const fn forwarded_return_identity(
        &self,
    ) -> Option<&O3ProducerForwardedReturnDescendant> {
        self.producer_forwarded_return_descendant.as_ref()
    }
    fn bind_issue_packet(
        &mut self,
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        if decoded.instruction() != self.instruction
            || !valid_live_speculative_fetch_identity(consumed_requests)
        {
            return false;
        }
        let packet = O3LiveIssuePacket::new(decoded, consumed_requests);
        if let Some(bound) = &self.issue_packet {
            return *bound == packet;
        }
        self.issue_packet = Some(packet);
        true
    }
    fn matches(
        &self,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.instruction == instruction
            && valid_live_speculative_fetch_identity(consumed_requests)
            && self.issue_packet.as_ref().is_none_or(|packet| {
                packet.instruction() == instruction
                    && packet.consumed_requests() == consumed_requests
            })
    }
    pub(super) fn matches_bound(
        &self,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.issue_packet.as_ref().is_some_and(|packet| {
            packet.decoded().instruction() == instruction
                && packet.consumed_requests() == consumed_requests
        })
    }
    pub(super) fn issue_packet(&self) -> Option<&O3LiveIssuePacket> {
        self.issue_packet.as_ref()
    }
    pub(in crate::o3_runtime) fn owns_fetch_request(&self, request: MemoryRequestId) -> bool {
        self.issue_packet
            .as_ref()
            .and_then(|packet| packet.consumed_requests().first())
            .copied()
            == Some(request)
    }
}

impl O3RuntimeState {
    pub(crate) fn bind_live_staged_issue_packet(
        &mut self,
        pc: Address,
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        let Some(sequence) = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.pc() == pc)
            .map(|entry| entry.sequence())
        else {
            return false;
        };
        self.bind_live_staged_issue_packet_at_sequence(sequence, decoded, consumed_requests)
    }
    pub(in crate::o3_runtime) fn bind_live_staged_issue_packet_at_sequence(
        &mut self,
        sequence: u64,
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.live_staged_fetch_identities
            .get_mut(&sequence)
            .is_some_and(|identity| identity.bind_issue_packet(decoded, consumed_requests))
    }
    pub(in crate::o3_runtime) fn live_staged_issue_packet(
        &self,
        sequence: u64,
    ) -> Option<&O3LiveIssuePacket> {
        self.live_staged_fetch_identities
            .get(&sequence)
            .and_then(O3LiveStagedFetchIdentity::issue_packet)
    }
    pub(in crate::o3_runtime) fn live_staged_instruction_matches(
        &self,
        sequence: u64,
        instruction: RiscvInstruction,
    ) -> bool {
        self.live_staged_fetch_identities
            .get(&sequence)
            .is_some_and(|identity| identity.instruction == instruction)
    }
    pub(in crate::o3_runtime) fn live_staged_fetch_identity_matches(
        &self,
        sequence: u64,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.live_staged_fetch_identities
            .get(&sequence)
            .is_some_and(|identity| identity.matches(instruction, consumed_requests))
    }

    pub(crate) fn live_staged_sequence_for_fetch_identity(
        &self,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<u64> {
        self.snapshot
            .reorder_buffer
            .iter()
            .filter(|entry| entry.is_live_staged() && entry.pc() == pc)
            .map(|entry| entry.sequence())
            .find(|sequence| {
                self.live_staged_issue_packet(*sequence)
                    .is_some_and(|packet| {
                        packet.instruction() == instruction
                            && packet.consumed_requests() == consumed_requests
                    })
            })
    }
}
