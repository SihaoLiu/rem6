use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest, MemoryResponse,
};
use std::collections::BTreeMap;

use crate::HarnessError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineBackingStore {
    layout: CacheLineLayout,
    line_address: Address,
    data: Vec<u8>,
    locked_reservations: BTreeMap<AgentId, Address>,
}

impl LineBackingStore {
    pub fn new(
        layout: CacheLineLayout,
        line_address: Address,
        data: Vec<u8>,
    ) -> Result<Self, HarnessError> {
        let line_address = layout.line_address(line_address);
        if data.len() as u64 != layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: layout.bytes(),
                actual: data.len() as u64,
            });
        }

        Ok(Self {
            layout,
            line_address,
            data,
            locked_reservations: BTreeMap::new(),
        })
    }

    pub fn line_address(&self) -> Address {
        self.line_address
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn replace_data(&mut self, data: Vec<u8>) -> Result<(), HarnessError> {
        if data.len() as u64 != self.layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.data = data;
        self.locked_reservations.clear();
        Ok(())
    }

    pub fn respond(&mut self, request: &MemoryRequest) -> Result<MemoryResponse, HarnessError> {
        self.check_line(request)?;
        match request.operation() {
            MemoryOperation::ReadShared
            | MemoryOperation::ReadUnique
            | MemoryOperation::LockedRmwRead => {
                let data = self.read_slice(request)?;
                MemoryResponse::completed(request, Some(data)).map_err(HarnessError::Memory)
            }
            MemoryOperation::LoadLocked => {
                let data = self.read_slice(request)?;
                self.track_load_locked(request);
                MemoryResponse::completed(request, Some(data)).map_err(HarnessError::Memory)
            }
            MemoryOperation::Upgrade => {
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::StoreConditional => {
                if !self.store_conditional_allowed(request) {
                    return MemoryResponse::store_conditional_failed(request)
                        .map_err(HarnessError::Memory);
                }

                self.apply_write(request)?;
                self.clear_locked_reservations(request);
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::StoreConditionalFail => {
                MemoryResponse::store_conditional_failed(request).map_err(HarnessError::Memory)
            }
            MemoryOperation::Write | MemoryOperation::LockedRmwWrite => {
                self.apply_write(request)?;
                self.clear_locked_reservations(request);
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::Atomic => {
                let data = self.read_slice(request)?;
                let write_data = request
                    .atomic_write_data(&data)
                    .map_err(HarnessError::Memory)?;
                self.apply_write_data(request, &write_data)?;
                self.clear_locked_reservations(request);
                MemoryResponse::completed(request, Some(data)).map_err(HarnessError::Memory)
            }
            MemoryOperation::WriteClean
            | MemoryOperation::WritebackClean
            | MemoryOperation::WritebackDirty => {
                self.replace_line(request)?;
                self.clear_locked_reservations(request);
                Ok(MemoryResponse::retry(request))
            }
            _ => MemoryResponse::completed(request, None).map_err(HarnessError::Memory),
        }
    }

    fn check_line(&self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let actual = request.line_address();
        if actual != self.line_address {
            return Err(HarnessError::WrongLine {
                expected: self.line_address,
                actual,
            });
        }

        Ok(())
    }

    fn track_load_locked(&mut self, request: &MemoryRequest) {
        self.locked_reservations
            .insert(request.id().agent(), request.llsc_reservation_address());
    }

    fn store_conditional_allowed(&self, request: &MemoryRequest) -> bool {
        self.locked_reservations
            .get(&request.id().agent())
            .is_some_and(|reserved| *reserved == request.llsc_reservation_address())
    }

    fn clear_locked_reservations(&mut self, request: &MemoryRequest) {
        self.locked_reservations
            .retain(|_, reserved| !request.overlaps_llsc_reservation(*reserved));
    }

    fn apply_write(&mut self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let payload =
            request
                .data()
                .ok_or(HarnessError::Memory(MemoryError::MissingRequestData {
                    request: request.id(),
                }))?;
        self.apply_write_data(request, payload)
    }

    fn apply_write_data(
        &mut self,
        request: &MemoryRequest,
        payload: &[u8],
    ) -> Result<(), HarnessError> {
        let offset = request.line_offset() as usize;
        let mask = request.byte_mask();
        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                self.data[offset + index] = *byte;
            }
        }

        Ok(())
    }

    fn read_slice(&self, request: &MemoryRequest) -> Result<Vec<u8>, HarnessError> {
        let offset = request.line_offset() as usize;
        let end = offset + request.size().bytes() as usize;
        if end > self.data.len() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: end as u64,
            });
        }
        Ok(self.data[offset..end].to_vec())
    }

    fn replace_line(&mut self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let data = request
            .data()
            .ok_or(HarnessError::Memory(MemoryError::MissingRequestData {
                request: request.id(),
            }))?;
        if data.len() as u64 != self.layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.data = data.to_vec();
        Ok(())
    }
}
