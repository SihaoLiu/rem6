use rem6_memory::{
    Address, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest, MemoryResponse,
};

use crate::HarnessError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineBackingStore {
    layout: CacheLineLayout,
    line_address: Address,
    data: Vec<u8>,
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
        Ok(())
    }

    pub fn respond(&mut self, request: &MemoryRequest) -> Result<MemoryResponse, HarnessError> {
        self.check_line(request)?;
        match request.operation() {
            MemoryOperation::ReadShared | MemoryOperation::ReadUnique => {
                MemoryResponse::completed(request, Some(self.data.clone()))
                    .map_err(HarnessError::Memory)
            }
            MemoryOperation::Upgrade => {
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::Write | MemoryOperation::Atomic => {
                self.apply_write(request)?;
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::WritebackClean | MemoryOperation::WritebackDirty => {
                self.replace_line(request)?;
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

    fn apply_write(&mut self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let offset = request.line_offset() as usize;
        let payload =
            request
                .data()
                .ok_or(HarnessError::Memory(MemoryError::MissingRequestData {
                    request: request.id(),
                }))?;
        let mask = request.byte_mask();
        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                self.data[offset + index] = *byte;
            }
        }

        Ok(())
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
