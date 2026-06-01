use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    PartitionedMemoryStore,
};

use crate::VirtioError;

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

    pub(crate) fn validate_write_exact(
        &mut self,
        address: Address,
        bytes: u64,
    ) -> Result<(), VirtioError> {
        let mut cursor = address;
        let mut remaining = bytes;
        while remaining > 0 {
            let line_remaining = self.line_layout.bytes() - self.line_layout.line_offset(cursor);
            let chunk = remaining.min(line_remaining);
            let target = self
                .store
                .validate_access_range(cursor, AccessSize::new(chunk).map_err(virtio_memory_error)?)
                .map_err(virtio_memory_error)?;
            let actual_layout = self
                .store
                .partition_layout(target)
                .map_err(virtio_memory_error)?;
            if actual_layout != self.line_layout {
                return Err(VirtioError::PciTransportRuntimeConfig {
                    message: format!(
                        "VirtIO guest memory access failed: target {} uses {}-byte lines but VirtIO guest memory uses {}-byte lines",
                        target.get(),
                        actual_layout.bytes(),
                        self.line_layout.bytes()
                    ),
                });
            }
            cursor = add_address(cursor, chunk)?;
            remaining -= chunk;
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

fn virtio_memory_error(error: rem6_memory::MemoryError) -> VirtioError {
    VirtioError::PciTransportRuntimeConfig {
        message: format!("VirtIO guest memory access failed: {error}"),
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
