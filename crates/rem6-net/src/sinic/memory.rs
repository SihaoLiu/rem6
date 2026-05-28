use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, ByteMask, CacheLineLayout, MemoryError,
    MemoryRequest, MemoryRequestId, MemoryTargetId, PartitionedMemoryStore,
};

use crate::{SinicError, SinicFifoDevice, SinicRxDmaCompletionRecord, SinicTxDmaCompletionRecord};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicDmaMemoryBackend {
    agent: AgentId,
    line_layout: CacheLineLayout,
    next_sequence: u64,
}

impl SinicDmaMemoryBackend {
    pub const fn new(agent: AgentId, line_layout: CacheLineLayout) -> Self {
        Self {
            agent,
            line_layout,
            next_sequence: 0,
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn next_sequence(&self) -> u64 {
        self.next_sequence
    }

    pub fn complete_rx_dma_copy_to_memory(
        &mut self,
        device: &mut SinicFifoDevice,
        memory: &mut PartitionedMemoryStore,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicRxDmaMemoryCompletionRecord, SinicError> {
        let (plan, payload) = device.pending_rx_dma_payload()?;
        let transactions =
            self.write_bytes(memory, Address::new(plan.guest_address()), &payload)?;
        let completion = device.complete_rx_dma_copy(current_tick, interrupt_delay_ticks)?;
        Ok(SinicRxDmaMemoryCompletionRecord::new(
            completion,
            transactions,
        ))
    }

    pub fn complete_tx_dma_copy_from_memory(
        &mut self,
        device: &mut SinicFifoDevice,
        memory: &mut PartitionedMemoryStore,
        current_tick: u64,
        interrupt_delay_ticks: u64,
    ) -> Result<SinicTxDmaMemoryCompletionRecord, SinicError> {
        let plan = device.pending_tx_dma_copy_plan()?;
        let (payload, transactions) = self.read_bytes(
            memory,
            Address::new(plan.guest_address()),
            u64::from(plan.copy_len()),
        )?;
        let completion =
            device.complete_tx_dma_copy(&payload, current_tick, interrupt_delay_ticks)?;
        Ok(SinicTxDmaMemoryCompletionRecord::new(
            completion,
            transactions,
        ))
    }

    fn write_bytes(
        &mut self,
        memory: &mut PartitionedMemoryStore,
        address: Address,
        bytes: &[u8],
    ) -> Result<Vec<SinicDmaMemoryTransaction>, SinicError> {
        let chunks = split_access(address, bytes.len() as u64, self.line_layout)?;
        let mut offset = 0usize;
        let mut transactions = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let chunk_len = chunk.byte_len as usize;
            let size = AccessSize::new(chunk.byte_len).map_err(memory_error)?;
            let request_id = self.next_request_id();
            let request = MemoryRequest::write(
                request_id,
                chunk.address,
                size,
                bytes[offset..offset + chunk_len].to_vec(),
                ByteMask::full(size).map_err(memory_error)?,
                self.line_layout,
            )
            .map_err(memory_error)?;
            let outcome = memory.respond(&request).map_err(memory_error)?;
            transactions.push(SinicDmaMemoryTransaction::new(
                request_id,
                outcome.target(),
                chunk.address,
                chunk.byte_len,
            ));
            offset += chunk_len;
        }
        Ok(transactions)
    }

    fn read_bytes(
        &mut self,
        memory: &mut PartitionedMemoryStore,
        address: Address,
        byte_len: u64,
    ) -> Result<(Vec<u8>, Vec<SinicDmaMemoryTransaction>), SinicError> {
        let chunks = split_access(address, byte_len, self.line_layout)?;
        let mut bytes = Vec::new();
        let mut transactions = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let size = AccessSize::new(chunk.byte_len).map_err(memory_error)?;
            let request_id = self.next_request_id();
            let request =
                MemoryRequest::read_shared(request_id, chunk.address, size, self.line_layout)
                    .map_err(memory_error)?;
            let outcome = memory.respond(&request).map_err(memory_error)?;
            let response = outcome.response().ok_or_else(|| {
                memory_error(MemoryError::MissingResponseData {
                    request: request_id,
                })
            })?;
            let data = response.data().ok_or_else(|| {
                memory_error(MemoryError::MissingResponseData {
                    request: request_id,
                })
            })?;
            bytes.extend_from_slice(data);
            transactions.push(SinicDmaMemoryTransaction::new(
                request_id,
                outcome.target(),
                chunk.address,
                chunk.byte_len,
            ));
        }
        Ok((bytes, transactions))
    }

    fn next_request_id(&mut self) -> MemoryRequestId {
        let request_id = MemoryRequestId::new(self.agent, self.next_sequence);
        self.next_sequence = self.next_sequence.saturating_add(1);
        request_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SinicDmaMemoryTransaction {
    request_id: MemoryRequestId,
    target: MemoryTargetId,
    address: Address,
    byte_len: u64,
}

impl SinicDmaMemoryTransaction {
    const fn new(
        request_id: MemoryRequestId,
        target: MemoryTargetId,
        address: Address,
        byte_len: u64,
    ) -> Self {
        Self {
            request_id,
            target,
            address,
            byte_len,
        }
    }

    pub const fn request_id(self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn target(self) -> MemoryTargetId {
        self.target
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicRxDmaMemoryCompletionRecord {
    completion: SinicRxDmaCompletionRecord,
    transactions: Vec<SinicDmaMemoryTransaction>,
}

impl SinicRxDmaMemoryCompletionRecord {
    fn new(
        completion: SinicRxDmaCompletionRecord,
        transactions: Vec<SinicDmaMemoryTransaction>,
    ) -> Self {
        Self {
            completion,
            transactions,
        }
    }

    pub const fn completion(&self) -> &SinicRxDmaCompletionRecord {
        &self.completion
    }

    pub fn transactions(&self) -> &[SinicDmaMemoryTransaction] {
        &self.transactions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicTxDmaMemoryCompletionRecord {
    completion: SinicTxDmaCompletionRecord,
    transactions: Vec<SinicDmaMemoryTransaction>,
}

impl SinicTxDmaMemoryCompletionRecord {
    fn new(
        completion: SinicTxDmaCompletionRecord,
        transactions: Vec<SinicDmaMemoryTransaction>,
    ) -> Self {
        Self {
            completion,
            transactions,
        }
    }

    pub const fn completion(&self) -> &SinicTxDmaCompletionRecord {
        &self.completion
    }

    pub fn transactions(&self) -> &[SinicDmaMemoryTransaction] {
        &self.transactions
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SinicDmaMemoryChunk {
    address: Address,
    byte_len: u64,
}

fn split_access(
    address: Address,
    byte_len: u64,
    line_layout: CacheLineLayout,
) -> Result<Vec<SinicDmaMemoryChunk>, SinicError> {
    let size = AccessSize::new(byte_len).map_err(memory_error)?;
    AddressRange::new(address, size).map_err(memory_error)?;

    let mut chunks = Vec::new();
    let mut copied = 0u64;
    while copied < byte_len {
        let chunk_address = Address::new(address.get() + copied);
        let line_offset = line_layout.line_offset(chunk_address);
        let line_remaining = line_layout.bytes() - line_offset;
        let chunk_len = (byte_len - copied).min(line_remaining);
        chunks.push(SinicDmaMemoryChunk {
            address: chunk_address,
            byte_len: chunk_len,
        });
        copied += chunk_len;
    }
    Ok(chunks)
}

fn memory_error(source: MemoryError) -> SinicError {
    SinicError::Memory { source }
}
