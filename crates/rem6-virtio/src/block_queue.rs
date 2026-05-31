use std::collections::{BTreeMap, BTreeSet};

use rem6_interrupt::InterruptSourceId;
use rem6_kernel::{ParallelSchedulerContext, PartitionEventId, SchedulerContext};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    PartitionedMemoryStore,
};
use rem6_pci::{PciEndpointConfig, PciLegacyInterruptPort, PciMsiPort, PciMsixPort};

use crate::{
    VirtioBlockCompletion, VirtioBlockRequest, VirtioBlockRequestId, VirtioBlockRequestKind,
    VirtioError, VirtioPciIsrDevice, VirtioQueueIndex, VirtioRngCompletion, VirtioRngRequest,
    VirtioRngRequestId, VIRTIO_BLOCK_T_FLUSH, VIRTIO_BLOCK_T_GET_ID, VIRTIO_BLOCK_T_IN,
    VIRTIO_BLOCK_T_OUT,
};

pub const VIRTIO_SPLIT_DESC_F_NEXT: u16 = 1;
pub const VIRTIO_SPLIT_DESC_F_WRITE: u16 = 2;
pub const VIRTIO_SPLIT_DESC_F_INDIRECT: u16 = 4;
pub const VIRTIO_SPLIT_AVAIL_F_NO_INTERRUPT: u16 = 1;

#[derive(Clone, Copy, Debug)]
pub struct VirtioBlockIntxCompletionTarget<'a> {
    isr: &'a VirtioPciIsrDevice,
    port: &'a PciLegacyInterruptPort,
    source: InterruptSourceId,
}

impl<'a> VirtioBlockIntxCompletionTarget<'a> {
    pub const fn new(
        isr: &'a VirtioPciIsrDevice,
        port: &'a PciLegacyInterruptPort,
        source: InterruptSourceId,
    ) -> Self {
        Self { isr, port, source }
    }

    pub const fn isr(self) -> &'a VirtioPciIsrDevice {
        self.isr
    }

    pub const fn port(self) -> &'a PciLegacyInterruptPort {
        self.port
    }

    pub const fn source(self) -> InterruptSourceId {
        self.source
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VirtioBlockMsiCompletionTarget<'a> {
    isr: &'a VirtioPciIsrDevice,
    endpoint: &'a PciEndpointConfig,
    port: &'a PciMsiPort,
    source: InterruptSourceId,
}

impl<'a> VirtioBlockMsiCompletionTarget<'a> {
    pub const fn new(
        isr: &'a VirtioPciIsrDevice,
        endpoint: &'a PciEndpointConfig,
        port: &'a PciMsiPort,
        source: InterruptSourceId,
    ) -> Self {
        Self {
            isr,
            endpoint,
            port,
            source,
        }
    }

    pub const fn isr(self) -> &'a VirtioPciIsrDevice {
        self.isr
    }

    pub const fn endpoint(self) -> &'a PciEndpointConfig {
        self.endpoint
    }

    pub const fn port(self) -> &'a PciMsiPort {
        self.port
    }

    pub const fn source(self) -> InterruptSourceId {
        self.source
    }
}

#[derive(Debug)]
pub struct VirtioBlockMsixCompletionTarget<'a> {
    isr: &'a VirtioPciIsrDevice,
    endpoint: &'a mut PciEndpointConfig,
    port: &'a PciMsixPort,
    source: InterruptSourceId,
}

impl<'a> VirtioBlockMsixCompletionTarget<'a> {
    pub fn new(
        isr: &'a VirtioPciIsrDevice,
        endpoint: &'a mut PciEndpointConfig,
        port: &'a PciMsixPort,
        source: InterruptSourceId,
    ) -> Self {
        Self {
            isr,
            endpoint,
            port,
            source,
        }
    }

    pub const fn isr(&self) -> &'a VirtioPciIsrDevice {
        self.isr
    }

    pub fn send(
        &mut self,
        context: &mut SchedulerContext<'_>,
    ) -> Result<Option<PartitionEventId>, rem6_pci::PciError> {
        self.port.send(self.endpoint, context, self.source)
    }

    pub fn send_parallel(
        &mut self,
        context: &mut ParallelSchedulerContext<'_>,
    ) -> Result<Option<PartitionEventId>, rem6_pci::PciError> {
        self.port.send_parallel(self.endpoint, context, self.source)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioBlockInterruptCompletion {
    writeback: VirtioBlockQueueCompletionWrite,
    interrupt_delivery: Option<PartitionEventId>,
}

impl VirtioBlockInterruptCompletion {
    pub const fn new(
        writeback: VirtioBlockQueueCompletionWrite,
        interrupt_delivery: Option<PartitionEventId>,
    ) -> Self {
        Self {
            writeback,
            interrupt_delivery,
        }
    }

    pub const fn writeback(&self) -> &VirtioBlockQueueCompletionWrite {
        &self.writeback
    }

    pub const fn interrupt_delivery(&self) -> Option<PartitionEventId> {
        self.interrupt_delivery
    }

    pub fn into_writeback(self) -> VirtioBlockQueueCompletionWrite {
        self.writeback
    }
}

pub struct VirtioGuestMemory<'a> {
    store: &'a mut PartitionedMemoryStore,
    line_layout: CacheLineLayout,
    agent: AgentId,
    sequence: u64,
}

impl<'a> VirtioGuestMemory<'a> {
    pub const fn new(
        store: &'a mut PartitionedMemoryStore,
        line_layout: CacheLineLayout,
        agent: AgentId,
    ) -> Self {
        Self {
            store,
            line_layout,
            agent,
            sequence: 0,
        }
    }

    pub(crate) fn read_exact(
        &mut self,
        address: Address,
        bytes: u64,
    ) -> Result<Vec<u8>, VirtioError> {
        let mut data = Vec::new();
        let mut cursor = address;
        let mut remaining = bytes;
        while remaining > 0 {
            let line_remaining = self.line_layout.bytes() - self.line_layout.line_offset(cursor);
            let chunk = remaining.min(line_remaining);
            let request = MemoryRequest::read_shared(
                self.next_request_id(),
                cursor,
                AccessSize::new(chunk).map_err(virtio_memory_error)?,
                self.line_layout,
            )
            .map_err(virtio_memory_error)?;
            let outcome = self.store.respond(&request).map_err(virtio_memory_error)?;
            let response =
                outcome
                    .response()
                    .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
                        message: "VirtIO guest memory read completed without a response".into(),
                    })?;
            let bytes = response
                .data()
                .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
                    message: "VirtIO guest memory read completed without data".into(),
                })?;
            data.extend_from_slice(bytes);
            cursor = add_address(cursor, chunk)?;
            remaining -= chunk;
        }
        Ok(data)
    }

    pub(crate) fn read_u16(&mut self, address: Address) -> Result<u16, VirtioError> {
        let bytes = self.read_exact(address, 2)?;
        Ok(u16::from_le_bytes(bytes.as_slice().try_into().unwrap()))
    }

    pub(crate) fn write_exact(
        &mut self,
        address: Address,
        bytes: &[u8],
    ) -> Result<(), VirtioError> {
        let mut cursor = address;
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let line_remaining = self.line_layout.bytes() - self.line_layout.line_offset(cursor);
            let chunk = remaining.len().min(line_remaining as usize);
            let request = MemoryRequest::write(
                self.next_request_id(),
                cursor,
                AccessSize::new(chunk as u64).map_err(virtio_memory_error)?,
                remaining[..chunk].to_vec(),
                ByteMask::from_bits(vec![true; chunk]).map_err(virtio_memory_error)?,
                self.line_layout,
            )
            .map_err(virtio_memory_error)?;
            self.store.respond(&request).map_err(virtio_memory_error)?;
            cursor = add_address(cursor, chunk as u64)?;
            remaining = &remaining[chunk..];
        }
        Ok(())
    }

    pub(crate) fn write_u16(&mut self, address: Address, value: u16) -> Result<(), VirtioError> {
        self.write_exact(address, &value.to_le_bytes())
    }

    fn next_request_id(&mut self) -> MemoryRequestId {
        let id = MemoryRequestId::new(self.agent, self.sequence);
        self.sequence = self.sequence.wrapping_add(1);
        id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioSplitQueue {
    queue_size: u16,
    descriptor_table: Address,
    available_ring: Address,
    used_ring: Address,
    last_available_index: u16,
    event_index: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioSplitQueueSnapshot {
    queue_size: u16,
    descriptor_table: Address,
    available_ring: Address,
    used_ring: Address,
    last_available_index: u16,
    event_index: bool,
}

impl VirtioSplitQueueSnapshot {
    pub fn new(
        queue_size: u16,
        descriptor_table: Address,
        available_ring: Address,
        used_ring: Address,
        last_available_index: u16,
        event_index: bool,
    ) -> Result<Self, VirtioError> {
        if queue_size == 0 || !queue_size.is_power_of_two() {
            return Err(VirtioError::InvalidQueueSize {
                index: 0,
                size: queue_size,
            });
        }
        Ok(Self {
            queue_size,
            descriptor_table,
            available_ring,
            used_ring,
            last_available_index,
            event_index,
        })
    }

    pub const fn queue_size(&self) -> u16 {
        self.queue_size
    }

    pub const fn descriptor_table(&self) -> Address {
        self.descriptor_table
    }

    pub const fn available_ring(&self) -> Address {
        self.available_ring
    }

    pub const fn used_ring(&self) -> Address {
        self.used_ring
    }

    pub const fn last_available_index(&self) -> u16 {
        self.last_available_index
    }

    pub const fn event_index_enabled(&self) -> bool {
        self.event_index
    }
}

impl VirtioSplitQueue {
    pub fn new(
        queue_size: u16,
        descriptor_table: Address,
        available_ring: Address,
        used_ring: Address,
        last_available_index: u16,
    ) -> Result<Self, VirtioError> {
        if queue_size == 0 || !queue_size.is_power_of_two() {
            return Err(VirtioError::InvalidQueueSize {
                index: 0,
                size: queue_size,
            });
        }
        Ok(Self {
            queue_size,
            descriptor_table,
            available_ring,
            used_ring,
            last_available_index,
            event_index: false,
        })
    }

    pub const fn with_event_index(mut self, event_index: bool) -> Self {
        self.event_index = event_index;
        self
    }

    pub const fn snapshot(&self) -> VirtioSplitQueueSnapshot {
        VirtioSplitQueueSnapshot {
            queue_size: self.queue_size,
            descriptor_table: self.descriptor_table,
            available_ring: self.available_ring,
            used_ring: self.used_ring,
            last_available_index: self.last_available_index,
            event_index: self.event_index,
        }
    }

    pub fn restore(&mut self, snapshot: &VirtioSplitQueueSnapshot) -> Result<(), VirtioError> {
        self.validate_snapshot_shape(snapshot)?;
        self.last_available_index = snapshot.last_available_index;
        self.event_index = snapshot.event_index;
        Ok(())
    }

    pub fn validate_snapshot_shape(
        &self,
        snapshot: &VirtioSplitQueueSnapshot,
    ) -> Result<(), VirtioError> {
        if self.queue_size != snapshot.queue_size
            || self.descriptor_table != snapshot.descriptor_table
            || self.available_ring != snapshot.available_ring
            || self.used_ring != snapshot.used_ring
        {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message: "VirtIO split queue snapshot shape mismatch".to_string(),
            });
        }
        Ok(())
    }

    pub const fn queue_size(&self) -> u16 {
        self.queue_size
    }

    pub const fn descriptor_table(&self) -> Address {
        self.descriptor_table
    }

    pub const fn available_ring(&self) -> Address {
        self.available_ring
    }

    pub const fn used_ring(&self) -> Address {
        self.used_ring
    }

    pub const fn last_available_index(&self) -> u16 {
        self.last_available_index
    }

    pub const fn event_index_enabled(&self) -> bool {
        self.event_index
    }

    pub fn consume_available_block(
        &mut self,
        guest: &mut VirtioGuestMemory<'_>,
        queue: VirtioQueueIndex,
    ) -> Result<Option<VirtioBlockDecodedRequest>, VirtioError> {
        let Some(chain) = self.consume_available_chain(guest)? else {
            return Ok(None);
        };
        let decoded = chain.decode_block_request(queue)?;
        self.advance_available_index();
        Ok(Some(decoded))
    }

    pub fn consume_available_rng(
        &mut self,
        guest: &mut VirtioGuestMemory<'_>,
        queue: VirtioQueueIndex,
    ) -> Result<Option<VirtioRngDecodedRequest>, VirtioError> {
        let Some(chain) = self.consume_available_chain(guest)? else {
            return Ok(None);
        };
        let decoded = chain.decode_rng_request(queue)?;
        self.advance_available_index();
        Ok(Some(decoded))
    }

    pub(crate) fn consume_available_chain(
        &mut self,
        guest: &mut VirtioGuestMemory<'_>,
    ) -> Result<Option<VirtioSplitDescriptorChain>, VirtioError> {
        let available_index = guest.read_u16(add_address(self.available_ring, 2)?)?;
        if available_index == self.last_available_index {
            return Ok(None);
        }
        let slot = self.last_available_index % self.queue_size;
        let head = guest.read_u16(add_address(self.available_ring, 4 + u64::from(slot) * 2)?)?;
        let chain = self.load_descriptor_chain(guest, head)?;
        Ok(Some(chain))
    }

    pub(crate) fn advance_available_index(&mut self) {
        self.last_available_index = self.last_available_index.wrapping_add(1);
    }

    pub fn complete_block_request(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
    ) -> Result<VirtioBlockQueueCompletionWrite, VirtioError> {
        let used_index = guest.read_u16(add_address(self.used_ring, 2)?)?;
        let mut used_ring = VirtioSplitUsedRing::new(self.queue_size, used_index)?;
        let writeback = used_ring.complete_block_request(decoded, completion)?;
        for write in writeback.data_writes() {
            self.write_descriptor(guest, write)?;
        }
        self.write_descriptor(guest, writeback.status_write())?;
        guest.write_exact(
            add_address(self.used_ring, 4 + u64::from(writeback.used_slot()) * 8)?,
            &writeback.used_element().to_le_bytes(),
        )?;
        guest.write_u16(add_address(self.used_ring, 2)?, writeback.used_index())?;
        Ok(writeback)
    }

    pub fn complete_rng_request(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioRngDecodedRequest,
        completion: &VirtioRngCompletion,
    ) -> Result<VirtioRngQueueCompletionWrite, VirtioError> {
        let used_index = guest.read_u16(add_address(self.used_ring, 2)?)?;
        let mut used_ring = VirtioSplitUsedRing::new(self.queue_size, used_index)?;
        let writeback = used_ring.complete_rng_request(decoded, completion)?;
        for write in writeback.data_writes() {
            self.write_rng_descriptor(guest, write)?;
        }
        guest.write_exact(
            add_address(self.used_ring, 4 + u64::from(writeback.used_slot()) * 8)?,
            &writeback.used_element().to_le_bytes(),
        )?;
        guest.write_u16(add_address(self.used_ring, 2)?, writeback.used_index())?;
        Ok(writeback)
    }

    pub fn complete_rng_request_and_raise_isr(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioRngDecodedRequest,
        completion: &VirtioRngCompletion,
        isr: &VirtioPciIsrDevice,
    ) -> Result<VirtioRngQueueCompletionWrite, VirtioError> {
        let writeback = self.complete_rng_request(guest, decoded, completion)?;
        if self.completion_interrupt_enabled(guest, writeback.used_index())? {
            isr.raise_queue_interrupt(completion.tick());
        }
        Ok(writeback)
    }

    pub fn complete_block_request_and_raise_isr(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
        isr: &VirtioPciIsrDevice,
    ) -> Result<VirtioBlockQueueCompletionWrite, VirtioError> {
        let (writeback, _) =
            self.complete_block_request_and_maybe_raise_isr(guest, decoded, completion, isr)?;
        Ok(writeback)
    }

    pub fn complete_block_request_and_post_intx(
        &self,
        context: &mut SchedulerContext<'_>,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
        target: VirtioBlockIntxCompletionTarget<'_>,
    ) -> Result<VirtioBlockInterruptCompletion, VirtioError> {
        let (writeback, should_deliver) = self.complete_block_request_and_maybe_raise_isr(
            guest,
            decoded,
            completion,
            target.isr(),
        )?;
        if !should_deliver {
            return Ok(VirtioBlockInterruptCompletion::new(writeback, None));
        }
        let delivery = target
            .port()
            .post(context, target.source())
            .map_err(virtio_pci_error)?;
        Ok(VirtioBlockInterruptCompletion::new(
            writeback,
            Some(delivery),
        ))
    }

    pub fn complete_block_request_and_post_intx_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
        target: VirtioBlockIntxCompletionTarget<'_>,
    ) -> Result<VirtioBlockInterruptCompletion, VirtioError> {
        let (writeback, should_deliver) = self.complete_block_request_and_maybe_raise_isr(
            guest,
            decoded,
            completion,
            target.isr(),
        )?;
        if !should_deliver {
            return Ok(VirtioBlockInterruptCompletion::new(writeback, None));
        }
        let delivery = target
            .port()
            .post_parallel(context, target.source())
            .map_err(virtio_pci_error)?;
        Ok(VirtioBlockInterruptCompletion::new(
            writeback,
            Some(delivery),
        ))
    }

    pub fn complete_block_request_and_post_msi(
        &self,
        context: &mut SchedulerContext<'_>,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
        target: VirtioBlockMsiCompletionTarget<'_>,
    ) -> Result<VirtioBlockInterruptCompletion, VirtioError> {
        let (writeback, should_deliver) = self.complete_block_request_and_maybe_raise_isr(
            guest,
            decoded,
            completion,
            target.isr(),
        )?;
        if !should_deliver {
            return Ok(VirtioBlockInterruptCompletion::new(writeback, None));
        }
        let delivery = target
            .port()
            .send(target.endpoint(), context, target.source())
            .map_err(virtio_pci_error)?;
        Ok(VirtioBlockInterruptCompletion::new(writeback, delivery))
    }

    pub fn complete_block_request_and_post_msi_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
        target: VirtioBlockMsiCompletionTarget<'_>,
    ) -> Result<VirtioBlockInterruptCompletion, VirtioError> {
        let (writeback, should_deliver) = self.complete_block_request_and_maybe_raise_isr(
            guest,
            decoded,
            completion,
            target.isr(),
        )?;
        if !should_deliver {
            return Ok(VirtioBlockInterruptCompletion::new(writeback, None));
        }
        let delivery = target
            .port()
            .send_parallel(target.endpoint(), context, target.source())
            .map_err(virtio_pci_error)?;
        Ok(VirtioBlockInterruptCompletion::new(writeback, delivery))
    }

    pub fn complete_block_request_and_post_msix(
        &self,
        context: &mut SchedulerContext<'_>,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
        mut target: VirtioBlockMsixCompletionTarget<'_>,
    ) -> Result<VirtioBlockInterruptCompletion, VirtioError> {
        let (writeback, should_deliver) = self.complete_block_request_and_maybe_raise_isr(
            guest,
            decoded,
            completion,
            target.isr(),
        )?;
        if !should_deliver {
            return Ok(VirtioBlockInterruptCompletion::new(writeback, None));
        }
        let delivery = target.send(context).map_err(virtio_pci_error)?;
        Ok(VirtioBlockInterruptCompletion::new(writeback, delivery))
    }

    pub fn complete_block_request_and_post_msix_parallel(
        &self,
        context: &mut ParallelSchedulerContext<'_>,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
        mut target: VirtioBlockMsixCompletionTarget<'_>,
    ) -> Result<VirtioBlockInterruptCompletion, VirtioError> {
        let (writeback, should_deliver) = self.complete_block_request_and_maybe_raise_isr(
            guest,
            decoded,
            completion,
            target.isr(),
        )?;
        if !should_deliver {
            return Ok(VirtioBlockInterruptCompletion::new(writeback, None));
        }
        let delivery = target.send_parallel(context).map_err(virtio_pci_error)?;
        Ok(VirtioBlockInterruptCompletion::new(writeback, delivery))
    }

    fn complete_block_request_and_maybe_raise_isr(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
        isr: &VirtioPciIsrDevice,
    ) -> Result<(VirtioBlockQueueCompletionWrite, bool), VirtioError> {
        let writeback = self.complete_block_request(guest, decoded, completion)?;
        let should_deliver = self.completion_interrupt_enabled(guest, writeback.used_index())?;
        if should_deliver {
            isr.raise_queue_interrupt(completion.tick());
        }
        Ok((writeback, should_deliver))
    }

    pub(crate) fn completion_interrupt_enabled(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        used_index: u16,
    ) -> Result<bool, VirtioError> {
        if self.event_index {
            let used_event = guest.read_u16(add_address(
                self.available_ring,
                4 + u64::from(self.queue_size) * 2,
            )?)?;
            let previous_used_index = used_index.wrapping_sub(1);
            return Ok(previous_used_index == used_event);
        }
        let flags = guest.read_u16(self.available_ring)?;
        Ok(flags & VIRTIO_SPLIT_AVAIL_F_NO_INTERRUPT == 0)
    }

    fn write_descriptor(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        write: &VirtioBlockDescriptorWrite,
    ) -> Result<(), VirtioError> {
        let address = write
            .address()
            .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
                message: format!(
                    "VirtIO split descriptor {} has no guest address for writeback",
                    write.descriptor()
                ),
            })?;
        guest.write_exact(
            add_address(address, u64::from(write.offset()))?,
            write.bytes(),
        )
    }

    fn write_rng_descriptor(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        write: &VirtioRngDescriptorWrite,
    ) -> Result<(), VirtioError> {
        let address = write
            .address()
            .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
                message: format!(
                    "VirtIO rng descriptor {} has no guest address for writeback",
                    write.descriptor()
                ),
            })?;
        guest.write_exact(
            add_address(address, u64::from(write.offset()))?,
            write.bytes(),
        )
    }

    fn load_descriptor_chain(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        head: u16,
    ) -> Result<VirtioSplitDescriptorChain, VirtioError> {
        let mut descriptors = Vec::new();
        let mut visited = BTreeSet::new();
        let mut index = head;
        loop {
            if index >= self.queue_size {
                return Err(VirtioError::MissingVirtioDescriptor { index });
            }
            if !visited.insert(index) {
                return Err(VirtioError::VirtioDescriptorLoop { index });
            }
            let raw = self.load_descriptor_raw(
                guest,
                add_address(
                    self.descriptor_table,
                    u64::from(index) * VIRTIO_SPLIT_DESCRIPTOR_SIZE,
                )?,
            )?;
            if raw.is_indirect() {
                if raw.has_next() {
                    return Err(VirtioError::PciTransportRuntimeConfig {
                        message: format!(
                            "VirtIO split descriptor {index} cannot combine indirect and next chaining"
                        ),
                    });
                }
                let indirect_descriptors = self.load_indirect_descriptors(guest, raw)?;
                let chain_len = descriptors.len() + indirect_descriptors.len();
                if chain_len > usize::from(self.queue_size) {
                    return Err(VirtioError::PciTransportRuntimeConfig {
                        message: format!(
                            "VirtIO split descriptor chain has {chain_len} descriptors, exceeding queue size {}",
                            self.queue_size
                        ),
                    });
                }
                descriptors.extend(indirect_descriptors);
                return VirtioSplitDescriptorChain::from_ordered(head, descriptors);
            }
            let descriptor = self.load_direct_descriptor(guest, index, raw)?;
            let next = descriptor.next();
            descriptors.push(descriptor);
            let Some(next) = next else {
                break;
            };
            index = next;
        }
        VirtioSplitDescriptorChain::new(head, descriptors)
    }

    fn load_indirect_descriptors(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        raw: RawVirtioSplitDescriptor,
    ) -> Result<Vec<VirtioSplitDescriptor>, VirtioError> {
        if raw.length == 0 || !raw.length.is_multiple_of(VIRTIO_SPLIT_DESCRIPTOR_SIZE_U32) {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message: format!(
                    "VirtIO indirect descriptor table length {} is not a nonzero multiple of {VIRTIO_SPLIT_DESCRIPTOR_SIZE}",
                    raw.length
                ),
            });
        }

        let descriptor_count = raw.length / VIRTIO_SPLIT_DESCRIPTOR_SIZE_U32;
        if descriptor_count > u32::from(u16::MAX) {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message: format!(
                    "VirtIO indirect descriptor table has {descriptor_count} descriptors"
                ),
            });
        }
        let descriptor_count = descriptor_count as u16;
        let mut descriptors = Vec::new();
        let mut visited = BTreeSet::new();
        let mut index = 0_u16;
        loop {
            if index >= descriptor_count {
                return Err(VirtioError::MissingVirtioDescriptor { index });
            }
            if !visited.insert(index) {
                return Err(VirtioError::VirtioDescriptorLoop { index });
            }
            let descriptor_address =
                add_address(raw.address, u64::from(index) * VIRTIO_SPLIT_DESCRIPTOR_SIZE)?;
            let descriptor_raw = self.load_descriptor_raw(guest, descriptor_address)?;
            if descriptor_raw.is_indirect() {
                return Err(VirtioError::PciTransportRuntimeConfig {
                    message: format!(
                        "VirtIO indirect descriptor {index} cannot reference another indirect table"
                    ),
                });
            }
            let descriptor = self.load_direct_descriptor(guest, index, descriptor_raw)?;
            let next = descriptor.next();
            descriptors.push(descriptor);
            let Some(next) = next else {
                break;
            };
            index = next;
        }
        Ok(descriptors)
    }

    fn load_descriptor_raw(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        descriptor_address: Address,
    ) -> Result<RawVirtioSplitDescriptor, VirtioError> {
        let bytes = guest.read_exact(descriptor_address, VIRTIO_SPLIT_DESCRIPTOR_SIZE)?;
        Ok(RawVirtioSplitDescriptor {
            address: Address::new(u64::from_le_bytes(bytes[0..8].try_into().unwrap())),
            length: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            flags: u16::from_le_bytes(bytes[12..14].try_into().unwrap()),
            next: u16::from_le_bytes(bytes[14..16].try_into().unwrap()),
        })
    }

    fn load_direct_descriptor(
        &self,
        guest: &mut VirtioGuestMemory<'_>,
        index: u16,
        raw: RawVirtioSplitDescriptor,
    ) -> Result<VirtioSplitDescriptor, VirtioError> {
        let next = raw.has_next().then_some(raw.next);
        if raw.is_writable() {
            return Ok(VirtioSplitDescriptor {
                index,
                address: Some(raw.address),
                bytes: Vec::new(),
                length: raw.length,
                writable: true,
                next,
            });
        }
        let data = guest.read_exact(raw.address, u64::from(raw.length))?;
        Ok(VirtioSplitDescriptor {
            index,
            address: Some(raw.address),
            length: data.len() as u32,
            bytes: data,
            writable: false,
            next,
        })
    }
}

const VIRTIO_SPLIT_DESCRIPTOR_SIZE: u64 = 16;
const VIRTIO_SPLIT_DESCRIPTOR_SIZE_U32: u32 = VIRTIO_SPLIT_DESCRIPTOR_SIZE as u32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawVirtioSplitDescriptor {
    address: Address,
    length: u32,
    flags: u16,
    next: u16,
}

impl RawVirtioSplitDescriptor {
    const fn has_next(self) -> bool {
        self.flags & VIRTIO_SPLIT_DESC_F_NEXT != 0
    }

    const fn is_writable(self) -> bool {
        self.flags & VIRTIO_SPLIT_DESC_F_WRITE != 0
    }

    const fn is_indirect(self) -> bool {
        self.flags & VIRTIO_SPLIT_DESC_F_INDIRECT != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioSplitDescriptor {
    index: u16,
    address: Option<Address>,
    bytes: Vec<u8>,
    length: u32,
    writable: bool,
    next: Option<u16>,
}

impl VirtioSplitDescriptor {
    pub fn device_readable(index: u16, bytes: Vec<u8>, next: Option<u16>) -> Self {
        Self {
            index,
            address: None,
            length: bytes.len() as u32,
            bytes,
            writable: false,
            next,
        }
    }

    pub const fn device_writable(index: u16, length: u32, next: Option<u16>) -> Self {
        Self {
            index,
            address: None,
            bytes: Vec::new(),
            length,
            writable: true,
            next,
        }
    }

    pub const fn index(&self) -> u16 {
        self.index
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn length(&self) -> u32 {
        self.length
    }

    pub const fn is_writable(&self) -> bool {
        self.writable
    }

    pub const fn next(&self) -> Option<u16> {
        self.next
    }

    pub const fn flags(&self) -> u16 {
        let mut flags = 0;
        if self.next.is_some() {
            flags |= VIRTIO_SPLIT_DESC_F_NEXT;
        }
        if self.writable {
            flags |= VIRTIO_SPLIT_DESC_F_WRITE;
        }
        flags
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioSplitDescriptorChain {
    head: u16,
    descriptors: Vec<VirtioSplitDescriptor>,
}

impl VirtioSplitDescriptorChain {
    pub fn new(
        head: u16,
        descriptors: impl IntoIterator<Item = VirtioSplitDescriptor>,
    ) -> Result<Self, VirtioError> {
        let mut descriptor_map = BTreeMap::new();
        for descriptor in descriptors {
            let index = descriptor.index();
            if descriptor_map.insert(index, descriptor).is_some() {
                return Err(VirtioError::DuplicateVirtioDescriptor { index });
            }
        }
        let mut ordered = Vec::new();
        let mut visited = BTreeSet::new();
        let mut cursor = head;
        loop {
            if !visited.insert(cursor) {
                return Err(VirtioError::VirtioDescriptorLoop { index: cursor });
            }
            let descriptor = descriptor_map
                .get(&cursor)
                .cloned()
                .ok_or(VirtioError::MissingVirtioDescriptor { index: cursor })?;
            let next = descriptor.next();
            ordered.push(descriptor);
            let Some(next) = next else {
                break;
            };
            cursor = next;
        }
        Ok(Self {
            head,
            descriptors: ordered,
        })
    }

    fn from_ordered(
        head: u16,
        descriptors: impl IntoIterator<Item = VirtioSplitDescriptor>,
    ) -> Result<Self, VirtioError> {
        let descriptors = descriptors.into_iter().collect::<Vec<_>>();
        if descriptors.is_empty() {
            return Err(VirtioError::MissingVirtioDescriptor { index: 0 });
        }
        Ok(Self { head, descriptors })
    }

    pub const fn head(&self) -> u16 {
        self.head
    }

    pub fn descriptors(&self) -> &[VirtioSplitDescriptor] {
        &self.descriptors
    }

    pub fn decode_block_request(
        &self,
        queue: VirtioQueueIndex,
    ) -> Result<VirtioBlockDecodedRequest, VirtioError> {
        let (raw_type, sector) = self.block_header()?;
        let status = self.status_descriptor()?;
        let payload = &self.descriptors[1..self.descriptors.len() - 1];
        let used_length = self.chain_used_length(raw_type)?;
        let mut writable_data_descriptors = Vec::new();
        let request = match raw_type {
            VIRTIO_BLOCK_T_IN => {
                writable_data_descriptors = self.writable_payload_targets(raw_type, payload)?;
                let bytes = writable_data_descriptors
                    .iter()
                    .map(|target| u64::from(target.length()))
                    .sum();
                VirtioBlockRequest::read(
                    VirtioBlockRequestId::new(u64::from(self.head)),
                    queue,
                    sector,
                    bytes,
                )?
            }
            VIRTIO_BLOCK_T_OUT => {
                let data = self.readable_payload_bytes(raw_type, payload)?;
                VirtioBlockRequest::write(
                    VirtioBlockRequestId::new(u64::from(self.head)),
                    queue,
                    sector,
                    data,
                )?
            }
            VIRTIO_BLOCK_T_FLUSH => {
                self.require_no_payload(raw_type, payload)?;
                VirtioBlockRequest::flush(VirtioBlockRequestId::new(u64::from(self.head)), queue)?
            }
            VIRTIO_BLOCK_T_GET_ID => {
                writable_data_descriptors = self.writable_payload_targets(raw_type, payload)?;
                let bytes = writable_data_descriptors
                    .iter()
                    .map(|target| u64::from(target.length()))
                    .sum();
                if bytes < 20 {
                    return Err(VirtioError::InvalidVirtioBlockDeviceIdOutput { bytes });
                }
                VirtioBlockRequest::get_id(VirtioBlockRequestId::new(u64::from(self.head)), queue)
            }
            _ => VirtioBlockRequest::unsupported(
                VirtioBlockRequestId::new(u64::from(self.head)),
                queue,
                raw_type,
                self.readable_payload_bytes_lossy(payload),
            )?,
        };
        let writable_data_bytes = match request.kind() {
            VirtioBlockRequestKind::Read { bytes } => *bytes,
            VirtioBlockRequestKind::GetId => self.writable_payload_bytes(raw_type, payload)?,
            _ => 0,
        };
        Ok(VirtioBlockDecodedRequest {
            request,
            status_descriptor: status.index(),
            status_descriptor_address: status.address(),
            writable_data_descriptors,
            writable_data_bytes,
            used_length,
        })
    }

    pub fn decode_rng_request(
        &self,
        queue: VirtioQueueIndex,
    ) -> Result<VirtioRngDecodedRequest, VirtioError> {
        let mut writable_descriptors = Vec::new();
        let mut writable_data_bytes = 0_u64;
        for descriptor in &self.descriptors {
            if !descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioRngReadableDescriptor {
                    index: descriptor.index(),
                });
            }
            writable_data_bytes = writable_data_bytes
                .checked_add(u64::from(descriptor.length()))
                .ok_or(VirtioError::VirtioRngPayloadLengthOverflow)?;
            writable_descriptors.push(VirtioRngWritableDescriptor {
                index: descriptor.index(),
                address: descriptor.address(),
                length: descriptor.length(),
            });
        }
        if writable_data_bytes == 0 {
            return Err(VirtioError::MissingVirtioRngWritableDescriptor);
        }
        let used_length = u32::try_from(writable_data_bytes)
            .map_err(|_| VirtioError::VirtioRngPayloadLengthOverflow)?;
        let request = VirtioRngRequest::new(
            VirtioRngRequestId::new(u64::from(self.head)),
            queue,
            writable_data_bytes,
        )?;
        Ok(VirtioRngDecodedRequest {
            request,
            writable_descriptors,
            writable_data_bytes,
            used_length,
        })
    }

    fn block_header(&self) -> Result<(u32, u64), VirtioError> {
        let Some(first) = self.descriptors.first() else {
            return Err(VirtioError::ShortVirtioBlockHeader { bytes: 0 });
        };
        if first.is_writable() {
            return Err(VirtioError::InvalidVirtioBlockReadableDescriptor {
                raw_type: 0,
                index: first.index(),
            });
        }
        if first.bytes().len() < 16 {
            return Err(VirtioError::ShortVirtioBlockHeader {
                bytes: first.bytes().len() as u64,
            });
        }
        let raw_type = u32::from_le_bytes(first.bytes()[0..4].try_into().unwrap());
        let sector = u64::from_le_bytes(first.bytes()[8..16].try_into().unwrap());
        Ok((raw_type, sector))
    }

    fn status_descriptor(&self) -> Result<&VirtioSplitDescriptor, VirtioError> {
        let Some(status) = self.descriptors.last() else {
            return Err(VirtioError::MissingVirtioBlockStatusDescriptor);
        };
        if !status.is_writable() || status.length() < 1 {
            return Err(VirtioError::InvalidVirtioBlockStatusDescriptor {
                index: status.index(),
                length: status.length(),
                writable: status.is_writable(),
            });
        }
        if self.descriptors.len() < 2 {
            return Err(VirtioError::MissingVirtioBlockStatusDescriptor);
        }
        Ok(status)
    }

    fn readable_payload_bytes(
        &self,
        raw_type: u32,
        payload: &[VirtioSplitDescriptor],
    ) -> Result<Vec<u8>, VirtioError> {
        let mut bytes = Vec::new();
        for descriptor in payload {
            if descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioBlockReadableDescriptor {
                    raw_type,
                    index: descriptor.index(),
                });
            }
            bytes.extend_from_slice(descriptor.bytes());
        }
        Ok(bytes)
    }

    fn readable_payload_bytes_lossy(&self, payload: &[VirtioSplitDescriptor]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for descriptor in payload {
            if !descriptor.is_writable() {
                bytes.extend_from_slice(descriptor.bytes());
            }
        }
        bytes
    }

    fn writable_payload_bytes(
        &self,
        raw_type: u32,
        payload: &[VirtioSplitDescriptor],
    ) -> Result<u64, VirtioError> {
        let mut bytes = 0_u64;
        for descriptor in payload {
            if !descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioBlockWritableDescriptor {
                    raw_type,
                    index: descriptor.index(),
                });
            }
            bytes = bytes
                .checked_add(u64::from(descriptor.length()))
                .ok_or(VirtioError::VirtioBlockPayloadLengthOverflow { raw_type })?;
        }
        Ok(bytes)
    }

    fn writable_payload_targets(
        &self,
        raw_type: u32,
        payload: &[VirtioSplitDescriptor],
    ) -> Result<Vec<VirtioBlockWritableDescriptor>, VirtioError> {
        let mut targets = Vec::new();
        for descriptor in payload {
            if !descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioBlockWritableDescriptor {
                    raw_type,
                    index: descriptor.index(),
                });
            }
            targets.push(VirtioBlockWritableDescriptor {
                index: descriptor.index(),
                address: descriptor.address(),
                length: descriptor.length(),
            });
        }
        Ok(targets)
    }

    fn chain_used_length(&self, raw_type: u32) -> Result<u32, VirtioError> {
        let mut bytes = 0_u64;
        for descriptor in &self.descriptors {
            bytes = bytes
                .checked_add(u64::from(descriptor.length()))
                .ok_or(VirtioError::VirtioBlockPayloadLengthOverflow { raw_type })?;
        }
        u32::try_from(bytes).map_err(|_| VirtioError::VirtioBlockPayloadLengthOverflow { raw_type })
    }

    fn require_no_payload(
        &self,
        raw_type: u32,
        payload: &[VirtioSplitDescriptor],
    ) -> Result<(), VirtioError> {
        if let Some(descriptor) = payload.first() {
            if descriptor.is_writable() {
                return Err(VirtioError::InvalidVirtioBlockReadableDescriptor {
                    raw_type,
                    index: descriptor.index(),
                });
            }
            return Err(VirtioError::InvalidVirtioBlockWritableDescriptor {
                raw_type,
                index: descriptor.index(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioBlockDecodedRequest {
    request: VirtioBlockRequest,
    status_descriptor: u16,
    status_descriptor_address: Option<Address>,
    writable_data_descriptors: Vec<VirtioBlockWritableDescriptor>,
    writable_data_bytes: u64,
    used_length: u32,
}

impl VirtioBlockDecodedRequest {
    pub const fn request(&self) -> &VirtioBlockRequest {
        &self.request
    }

    pub const fn status_descriptor(&self) -> u16 {
        self.status_descriptor
    }

    pub const fn status_descriptor_address(&self) -> Option<Address> {
        self.status_descriptor_address
    }

    pub const fn writable_data_bytes(&self) -> u64 {
        self.writable_data_bytes
    }

    pub const fn used_length(&self) -> u32 {
        self.used_length
    }

    pub fn into_request(self) -> VirtioBlockRequest {
        self.request
    }

    fn data_writes(&self, data: &[u8]) -> Result<Vec<VirtioBlockDescriptorWrite>, VirtioError> {
        if data.len() as u64 > self.writable_data_bytes {
            return Err(VirtioError::VirtioBlockPayloadLengthOverflow {
                raw_type: self.request.kind().raw_type(),
            });
        }
        let mut cursor = data;
        let mut writes = Vec::new();
        for target in &self.writable_data_descriptors {
            if cursor.is_empty() {
                break;
            }
            let bytes = cursor.len().min(target.length() as usize);
            writes.push(VirtioBlockDescriptorWrite {
                descriptor: target.index(),
                address: target.address(),
                offset: 0,
                bytes: cursor[..bytes].to_vec(),
            });
            cursor = &cursor[bytes..];
        }
        if !cursor.is_empty() {
            return Err(VirtioError::VirtioBlockPayloadLengthOverflow {
                raw_type: self.request.kind().raw_type(),
            });
        }
        Ok(writes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioRngDecodedRequest {
    request: VirtioRngRequest,
    writable_descriptors: Vec<VirtioRngWritableDescriptor>,
    writable_data_bytes: u64,
    used_length: u32,
}

impl VirtioRngDecodedRequest {
    pub const fn request(&self) -> &VirtioRngRequest {
        &self.request
    }

    pub const fn writable_data_bytes(&self) -> u64 {
        self.writable_data_bytes
    }

    pub const fn used_length(&self) -> u32 {
        self.used_length
    }

    pub fn into_request(self) -> VirtioRngRequest {
        self.request
    }

    fn data_writes(&self, data: &[u8]) -> Result<Vec<VirtioRngDescriptorWrite>, VirtioError> {
        if data.len() as u64 > self.writable_data_bytes {
            return Err(VirtioError::VirtioRngPayloadLengthOverflow);
        }
        let mut cursor = data;
        let mut writes = Vec::new();
        for target in &self.writable_descriptors {
            if cursor.is_empty() {
                break;
            }
            let bytes = cursor.len().min(target.length() as usize);
            writes.push(VirtioRngDescriptorWrite {
                descriptor: target.index(),
                address: target.address(),
                offset: 0,
                bytes: cursor[..bytes].to_vec(),
            });
            cursor = &cursor[bytes..];
        }
        if !cursor.is_empty() {
            return Err(VirtioError::VirtioRngPayloadLengthOverflow);
        }
        Ok(writes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VirtioBlockWritableDescriptor {
    index: u16,
    address: Option<Address>,
    length: u32,
}

impl VirtioBlockWritableDescriptor {
    const fn index(self) -> u16 {
        self.index
    }

    const fn address(self) -> Option<Address> {
        self.address
    }

    const fn length(self) -> u32 {
        self.length
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VirtioRngWritableDescriptor {
    index: u16,
    address: Option<Address>,
    length: u32,
}

impl VirtioRngWritableDescriptor {
    const fn index(self) -> u16 {
        self.index
    }

    const fn address(self) -> Option<Address> {
        self.address
    }

    const fn length(self) -> u32 {
        self.length
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioBlockDescriptorWrite {
    descriptor: u16,
    address: Option<Address>,
    offset: u32,
    bytes: Vec<u8>,
}

impl VirtioBlockDescriptorWrite {
    pub const fn descriptor(&self) -> u16 {
        self.descriptor
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioRngDescriptorWrite {
    descriptor: u16,
    address: Option<Address>,
    offset: u32,
    bytes: Vec<u8>,
}

impl VirtioRngDescriptorWrite {
    pub const fn descriptor(&self) -> u16 {
        self.descriptor
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VirtioSplitUsedElement {
    id: u32,
    len: u32,
}

impl VirtioSplitUsedElement {
    pub const fn new(id: u32, len: u32) -> Self {
        Self { id, len }
    }

    pub const fn id(self) -> u32 {
        self.id
    }

    pub const fn used_length(self) -> u32 {
        self.len
    }

    pub fn to_le_bytes(self) -> [u8; 8] {
        let id = self.id.to_le_bytes();
        let len = self.len.to_le_bytes();
        [id[0], id[1], id[2], id[3], len[0], len[1], len[2], len[3]]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioBlockQueueCompletionWrite {
    data_writes: Vec<VirtioBlockDescriptorWrite>,
    status_write: VirtioBlockDescriptorWrite,
    used_slot: u16,
    used_index: u16,
    used_element: VirtioSplitUsedElement,
}

impl VirtioBlockQueueCompletionWrite {
    pub fn data_writes(&self) -> &[VirtioBlockDescriptorWrite] {
        &self.data_writes
    }

    pub const fn status_write(&self) -> &VirtioBlockDescriptorWrite {
        &self.status_write
    }

    pub const fn used_slot(&self) -> u16 {
        self.used_slot
    }

    pub const fn used_index(&self) -> u16 {
        self.used_index
    }

    pub const fn used_element(&self) -> VirtioSplitUsedElement {
        self.used_element
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioRngQueueCompletionWrite {
    data_writes: Vec<VirtioRngDescriptorWrite>,
    used_slot: u16,
    used_index: u16,
    used_element: VirtioSplitUsedElement,
}

impl VirtioRngQueueCompletionWrite {
    pub fn data_writes(&self) -> &[VirtioRngDescriptorWrite] {
        &self.data_writes
    }

    pub const fn used_slot(&self) -> u16 {
        self.used_slot
    }

    pub const fn used_index(&self) -> u16 {
        self.used_index
    }

    pub const fn used_element(&self) -> VirtioSplitUsedElement {
        self.used_element
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioSplitUsedRing {
    queue_size: u16,
    index: u16,
    entries: Vec<Option<VirtioSplitUsedElement>>,
}

impl VirtioSplitUsedRing {
    pub fn new(queue_size: u16, index: u16) -> Result<Self, VirtioError> {
        if queue_size == 0 || !queue_size.is_power_of_two() {
            return Err(VirtioError::InvalidQueueSize {
                index: 0,
                size: queue_size,
            });
        }
        Ok(Self {
            queue_size,
            index,
            entries: vec![None; queue_size as usize],
        })
    }

    pub const fn queue_size(&self) -> u16 {
        self.queue_size
    }

    pub const fn index(&self) -> u16 {
        self.index
    }

    pub fn entry(&self, slot: u16) -> Option<&VirtioSplitUsedElement> {
        self.entries
            .get(usize::from(slot % self.queue_size))
            .and_then(Option::as_ref)
    }

    pub(crate) fn set_entry(&mut self, slot: u16, element: VirtioSplitUsedElement) {
        self.entries[usize::from(slot % self.queue_size)] = Some(element);
    }

    pub(crate) fn set_index(&mut self, index: u16) {
        self.index = index;
    }

    pub fn complete_block_request(
        &mut self,
        decoded: &VirtioBlockDecodedRequest,
        completion: &VirtioBlockCompletion,
    ) -> Result<VirtioBlockQueueCompletionWrite, VirtioError> {
        if completion.request() != decoded.request().id()
            || completion.queue() != decoded.request().queue()
        {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message: "VirtIO block completion does not match decoded descriptor chain".into(),
            });
        }
        let data_writes = decoded.data_writes(completion.data().unwrap_or(&[]))?;
        let status_write = VirtioBlockDescriptorWrite {
            descriptor: decoded.status_descriptor(),
            address: decoded.status_descriptor_address(),
            offset: 0,
            bytes: vec![completion.status_byte()],
        };
        let used_slot = self.index % self.queue_size;
        let used_index = self.index.wrapping_add(1);
        let used_element = VirtioSplitUsedElement::new(
            u32::try_from(decoded.request().id().get()).map_err(|_| {
                VirtioError::VirtioBlockPayloadLengthOverflow {
                    raw_type: decoded.request().kind().raw_type(),
                }
            })?,
            decoded.used_length(),
        );
        self.entries[used_slot as usize] = Some(used_element);
        self.index = used_index;
        Ok(VirtioBlockQueueCompletionWrite {
            data_writes,
            status_write,
            used_slot,
            used_index,
            used_element,
        })
    }

    pub fn complete_rng_request(
        &mut self,
        decoded: &VirtioRngDecodedRequest,
        completion: &VirtioRngCompletion,
    ) -> Result<VirtioRngQueueCompletionWrite, VirtioError> {
        if completion.request() != decoded.request().id()
            || completion.queue() != decoded.request().queue()
        {
            return Err(VirtioError::PciTransportRuntimeConfig {
                message: "VirtIO rng completion does not match decoded descriptor chain".into(),
            });
        }
        let data_writes = decoded.data_writes(completion.bytes())?;
        let used_slot = self.index % self.queue_size;
        let used_index = self.index.wrapping_add(1);
        let used_element = VirtioSplitUsedElement::new(
            u32::try_from(decoded.request().id().get())
                .map_err(|_| VirtioError::VirtioRngPayloadLengthOverflow)?,
            decoded.used_length(),
        );
        self.entries[used_slot as usize] = Some(used_element);
        self.index = used_index;
        Ok(VirtioRngQueueCompletionWrite {
            data_writes,
            used_slot,
            used_index,
            used_element,
        })
    }
}

pub(crate) fn add_address(address: Address, offset: u64) -> Result<Address, VirtioError> {
    address
        .get()
        .checked_add(offset)
        .map(Address::new)
        .ok_or_else(|| VirtioError::PciTransportRuntimeConfig {
            message: format!(
                "VirtIO guest memory address {:#x} overflows with offset {offset}",
                address.get()
            ),
        })
}

fn virtio_memory_error(error: rem6_memory::MemoryError) -> VirtioError {
    VirtioError::PciTransportRuntimeConfig {
        message: format!("VirtIO guest memory access failed: {error}"),
    }
}

fn virtio_pci_error(error: rem6_pci::PciError) -> VirtioError {
    VirtioError::PciTransportRuntimeConfig {
        message: format!("VirtIO PCI interrupt delivery failed: {error}"),
    }
}
