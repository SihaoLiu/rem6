use crate::{
    AccessSize, Address, AddressRange, ByteMask, CacheLineLayout, CoherenceIntent,
    MemoryAccessOrdering, MemoryAtomicOp, MemoryError, MemoryOperation, MemoryRequestId,
};

const LLSC_RESERVATION_BYTES: u64 = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRequest {
    id: MemoryRequestId,
    operation: MemoryOperation,
    range: AddressRange,
    line_layout: CacheLineLayout,
    ordering: MemoryAccessOrdering,
    uncacheable: bool,
    strict_order: bool,
    attributes: MemoryRequestAttributes,
    data: Option<Vec<u8>>,
    byte_mask: Option<ByteMask>,
    atomic_op: Option<MemoryAtomicOp>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MemoryRequestAttributes {
    privileged: bool,
    secure: bool,
    page_table_walk: bool,
    evict_next: bool,
}

impl MemoryRequestAttributes {
    pub const fn new(privileged: bool, secure: bool, page_table_walk: bool) -> Self {
        Self {
            privileged,
            secure,
            page_table_walk,
            evict_next: false,
        }
    }

    pub const fn with_evict_next(self) -> Self {
        Self {
            privileged: self.privileged,
            secure: self.secure,
            page_table_walk: self.page_table_walk,
            evict_next: true,
        }
    }

    pub const fn is_privileged(self) -> bool {
        self.privileged
    }

    pub const fn is_secure(self) -> bool {
        self.secure
    }

    pub const fn is_page_table_walk(self) -> bool {
        self.page_table_walk
    }

    pub const fn is_evict_next(self) -> bool {
        self.evict_next
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRequestSnapshot {
    id: MemoryRequestId,
    operation: MemoryOperation,
    range: AddressRange,
    line_layout: CacheLineLayout,
    ordering: MemoryAccessOrdering,
    uncacheable: bool,
    strict_order: bool,
    attributes: MemoryRequestAttributes,
    data: Option<Vec<u8>>,
    byte_mask: Option<ByteMask>,
    atomic_op: Option<MemoryAtomicOp>,
}

impl MemoryRequestSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: MemoryRequestId,
        operation: MemoryOperation,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
        ordering: MemoryAccessOrdering,
        uncacheable: bool,
        strict_order: bool,
        data: Option<Vec<u8>>,
        byte_mask: Option<ByteMask>,
        atomic_op: Option<MemoryAtomicOp>,
    ) -> Result<Self, MemoryError> {
        Self::new_with_attributes(
            id,
            operation,
            address,
            size,
            line_layout,
            ordering,
            uncacheable,
            strict_order,
            MemoryRequestAttributes::default(),
            data,
            byte_mask,
            atomic_op,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_attributes(
        id: MemoryRequestId,
        operation: MemoryOperation,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
        ordering: MemoryAccessOrdering,
        uncacheable: bool,
        strict_order: bool,
        attributes: MemoryRequestAttributes,
        data: Option<Vec<u8>>,
        byte_mask: Option<ByteMask>,
        atomic_op: Option<MemoryAtomicOp>,
    ) -> Result<Self, MemoryError> {
        let snapshot = Self {
            id,
            operation,
            range: AddressRange::new(address, size)?,
            line_layout,
            ordering,
            uncacheable,
            strict_order,
            attributes,
            data,
            byte_mask,
            atomic_op,
        };
        MemoryRequest::from_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    pub const fn id(&self) -> MemoryRequestId {
        self.id
    }

    pub const fn operation(&self) -> MemoryOperation {
        self.operation
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn ordering(&self) -> MemoryAccessOrdering {
        self.ordering
    }

    pub const fn is_uncacheable(&self) -> bool {
        self.uncacheable
    }

    pub const fn is_strict_ordered(&self) -> bool {
        self.strict_order
    }

    pub const fn attributes(&self) -> MemoryRequestAttributes {
        self.attributes
    }

    pub const fn is_privileged(&self) -> bool {
        self.attributes.is_privileged()
    }

    pub const fn is_secure(&self) -> bool {
        self.attributes.is_secure()
    }

    pub const fn is_page_table_walk(&self) -> bool {
        self.attributes.is_page_table_walk()
    }

    pub const fn is_evict_next(&self) -> bool {
        self.attributes.is_evict_next()
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn byte_mask(&self) -> Option<&ByteMask> {
        self.byte_mask.as_ref()
    }

    pub const fn atomic_op(&self) -> Option<MemoryAtomicOp> {
        self.atomic_op
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MemoryRequestPayload {
    data: Option<Vec<u8>>,
    byte_mask: Option<ByteMask>,
    atomic_op: Option<MemoryAtomicOp>,
}

impl MemoryRequestPayload {
    fn empty() -> Self {
        Self {
            data: None,
            byte_mask: None,
            atomic_op: None,
        }
    }

    fn write(data: Vec<u8>, byte_mask: ByteMask) -> Self {
        Self {
            data: Some(data),
            byte_mask: Some(byte_mask),
            atomic_op: None,
        }
    }

    fn atomic(op: MemoryAtomicOp, data: Vec<u8>, byte_mask: ByteMask) -> Self {
        Self {
            data: Some(data),
            byte_mask: Some(byte_mask),
            atomic_op: Some(op),
        }
    }

    fn writeback(data: Vec<u8>) -> Self {
        Self {
            data: Some(data),
            byte_mask: None,
            atomic_op: None,
        }
    }

    fn from_snapshot(snapshot: &MemoryRequestSnapshot) -> Self {
        Self {
            data: snapshot.data.clone(),
            byte_mask: snapshot.byte_mask.clone(),
            atomic_op: snapshot.atomic_op,
        }
    }
}

impl MemoryRequest {
    pub fn read_shared(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::ReadShared,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    pub fn read_unique(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::ReadUnique,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    pub fn prefetch_read(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::PrefetchRead,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    pub fn prefetch_write(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::PrefetchWrite,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    pub fn locked_rmw_read(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::LockedRmwRead,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    pub fn load_locked(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::LoadLocked,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    pub fn locked_rmw_write(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        data: Vec<u8>,
        byte_mask: ByteMask,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::LockedRmwWrite,
            address,
            size,
            line_layout,
            MemoryRequestPayload::write(data, byte_mask),
        )
    }

    pub fn store_conditional(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        data: Vec<u8>,
        byte_mask: ByteMask,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::StoreConditional,
            address,
            size,
            line_layout,
            MemoryRequestPayload::write(data, byte_mask),
        )
    }

    pub fn instruction_fetch(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::InstructionFetch,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    pub fn write(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        data: Vec<u8>,
        byte_mask: ByteMask,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::Write,
            address,
            size,
            line_layout,
            MemoryRequestPayload::write(data, byte_mask),
        )
    }

    pub fn cache_block_zero(
        id: MemoryRequestId,
        address: Address,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::line_maintenance(id, MemoryOperation::CacheBlockZero, address, line_layout)
    }

    pub fn atomic(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        data: Vec<u8>,
        byte_mask: ByteMask,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::atomic_with_op(
            id,
            address,
            size,
            MemoryAtomicOp::Swap,
            data,
            byte_mask,
            line_layout,
        )
    }

    pub fn atomic_with_op(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        op: MemoryAtomicOp,
        data: Vec<u8>,
        byte_mask: ByteMask,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::Atomic,
            address,
            size,
            line_layout,
            MemoryRequestPayload::atomic(op, data, byte_mask),
        )
    }

    pub fn upgrade(
        id: MemoryRequestId,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::new(
            id,
            MemoryOperation::Upgrade,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    pub fn writeback_dirty(
        id: MemoryRequestId,
        address: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::writeback(
            id,
            MemoryOperation::WritebackDirty,
            address,
            data,
            line_layout,
        )
    }

    pub fn writeback_clean(
        id: MemoryRequestId,
        address: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::writeback(
            id,
            MemoryOperation::WritebackClean,
            address,
            data,
            line_layout,
        )
    }

    pub fn write_clean(
        id: MemoryRequestId,
        address: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::writeback(id, MemoryOperation::WriteClean, address, data, line_layout)
    }

    pub fn clean_evict(
        id: MemoryRequestId,
        address: Address,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::line_maintenance(id, MemoryOperation::CleanEvict, address, line_layout)
    }

    pub fn clean_shared(
        id: MemoryRequestId,
        address: Address,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::line_maintenance(id, MemoryOperation::CleanShared, address, line_layout)
    }

    pub fn invalidate(
        id: MemoryRequestId,
        address: Address,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::line_maintenance(id, MemoryOperation::Invalidate, address, line_layout)
    }

    pub fn invalidate_writable(
        id: MemoryRequestId,
        address: Address,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        Self::line_maintenance(
            id,
            MemoryOperation::InvalidateWritable,
            address,
            line_layout,
        )
    }

    fn line_maintenance(
        id: MemoryRequestId,
        operation: MemoryOperation,
        address: Address,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        if line_layout.line_offset(address) != 0 {
            return Err(MemoryError::UnalignedLineAddress {
                address,
                line_size: line_layout.bytes(),
            });
        }

        let size = AccessSize::new(line_layout.bytes())?;
        Self::new(
            id,
            operation,
            address,
            size,
            line_layout,
            MemoryRequestPayload::empty(),
        )
    }

    fn writeback(
        id: MemoryRequestId,
        operation: MemoryOperation,
        address: Address,
        data: Vec<u8>,
        line_layout: CacheLineLayout,
    ) -> Result<Self, MemoryError> {
        if line_layout.line_offset(address) != 0 {
            return Err(MemoryError::UnalignedLineAddress {
                address,
                line_size: line_layout.bytes(),
            });
        }

        let size = AccessSize::new(line_layout.bytes())?;
        Self::new(
            id,
            operation,
            address,
            size,
            line_layout,
            MemoryRequestPayload::writeback(data),
        )
    }

    pub fn from_snapshot(snapshot: &MemoryRequestSnapshot) -> Result<Self, MemoryError> {
        if snapshot.strict_order && !snapshot.uncacheable {
            return Err(MemoryError::InvalidRequestStrictOrdering {
                request: snapshot.id,
            });
        }

        let mut request = Self::new(
            snapshot.id,
            snapshot.operation,
            snapshot.range.start(),
            snapshot.range.size(),
            snapshot.line_layout,
            MemoryRequestPayload::from_snapshot(snapshot),
        )?;
        request.ordering = snapshot.ordering;
        request.uncacheable = snapshot.uncacheable;
        request.strict_order = snapshot.strict_order;
        request.attributes = snapshot.attributes;
        Ok(request)
    }

    fn new(
        id: MemoryRequestId,
        operation: MemoryOperation,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
        payload: MemoryRequestPayload,
    ) -> Result<Self, MemoryError> {
        let range = AddressRange::new(address, size)?;
        Self::validate_payload(id, operation, size, payload.data.as_deref())?;
        Self::validate_mask(id, operation, size, payload.byte_mask.as_ref())?;
        Self::validate_atomic_op(id, operation, payload.atomic_op)?;
        Self::validate_cache_block_zero_shape(operation, address, size, line_layout)?;

        Ok(Self {
            id,
            operation,
            range,
            line_layout,
            ordering: MemoryAccessOrdering::none(),
            uncacheable: false,
            strict_order: false,
            attributes: MemoryRequestAttributes::default(),
            data: payload.data,
            byte_mask: payload.byte_mask,
            atomic_op: payload.atomic_op,
        })
    }

    fn validate_payload(
        id: MemoryRequestId,
        operation: MemoryOperation,
        size: AccessSize,
        data: Option<&[u8]>,
    ) -> Result<(), MemoryError> {
        match (operation.carries_request_data(), data) {
            (true, Some(bytes)) if bytes.len() as u64 == size.bytes() => Ok(()),
            (true, Some(bytes)) => Err(MemoryError::PayloadSizeMismatch {
                expected: size,
                actual: bytes.len() as u64,
            }),
            (true, None) => Err(MemoryError::MissingRequestData { request: id }),
            (false, Some(_)) => Err(MemoryError::UnexpectedRequestData { request: id }),
            (false, None) => Ok(()),
        }
    }

    fn validate_mask(
        id: MemoryRequestId,
        operation: MemoryOperation,
        size: AccessSize,
        byte_mask: Option<&ByteMask>,
    ) -> Result<(), MemoryError> {
        match (operation, byte_mask) {
            (
                MemoryOperation::Write
                | MemoryOperation::StoreConditional
                | MemoryOperation::LockedRmwWrite
                | MemoryOperation::Atomic,
                Some(mask),
            ) if mask.len() == size.bytes() => Ok(()),
            (
                MemoryOperation::Write
                | MemoryOperation::StoreConditional
                | MemoryOperation::LockedRmwWrite
                | MemoryOperation::Atomic,
                Some(mask),
            ) => Err(MemoryError::ByteMaskSizeMismatch {
                expected: size,
                actual: mask.len(),
            }),
            (
                MemoryOperation::Write
                | MemoryOperation::StoreConditional
                | MemoryOperation::LockedRmwWrite
                | MemoryOperation::Atomic,
                None,
            ) => Err(MemoryError::MissingByteMask { request: id }),
            (_, Some(_)) => Err(MemoryError::UnexpectedByteMask { request: id }),
            (_, None) => Ok(()),
        }
    }

    fn validate_cache_block_zero_shape(
        operation: MemoryOperation,
        address: Address,
        size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Result<(), MemoryError> {
        if operation != MemoryOperation::CacheBlockZero {
            return Ok(());
        }
        if line_layout.line_offset(address) != 0 {
            return Err(MemoryError::UnalignedLineAddress {
                address,
                line_size: line_layout.bytes(),
            });
        }
        if size.bytes() != line_layout.bytes() {
            return Err(MemoryError::PayloadSizeMismatch {
                expected: AccessSize::new(line_layout.bytes())?,
                actual: size.bytes(),
            });
        }
        Ok(())
    }

    fn validate_atomic_op(
        id: MemoryRequestId,
        operation: MemoryOperation,
        atomic_op: Option<MemoryAtomicOp>,
    ) -> Result<(), MemoryError> {
        match (operation, atomic_op) {
            (MemoryOperation::Atomic, Some(_)) => Ok(()),
            (MemoryOperation::Atomic, None) => Err(MemoryError::MissingAtomicOp { request: id }),
            (_, Some(_)) => Err(MemoryError::UnexpectedAtomicOp { request: id }),
            (_, None) => Ok(()),
        }
    }

    pub const fn id(&self) -> MemoryRequestId {
        self.id
    }

    pub const fn operation(&self) -> MemoryOperation {
        self.operation
    }

    pub const fn ordering(&self) -> MemoryAccessOrdering {
        self.ordering
    }

    pub fn with_ordering(mut self, ordering: MemoryAccessOrdering) -> Self {
        self.ordering = ordering;
        self
    }

    pub const fn is_uncacheable(&self) -> bool {
        self.uncacheable
    }

    pub const fn is_strict_ordered(&self) -> bool {
        self.strict_order
    }

    pub const fn attributes(&self) -> MemoryRequestAttributes {
        self.attributes
    }

    pub const fn is_privileged(&self) -> bool {
        self.attributes.is_privileged()
    }

    pub const fn is_secure(&self) -> bool {
        self.attributes.is_secure()
    }

    pub const fn is_page_table_walk(&self) -> bool {
        self.attributes.is_page_table_walk()
    }

    pub const fn is_evict_next(&self) -> bool {
        self.attributes.is_evict_next()
    }

    pub fn with_attributes(mut self, attributes: MemoryRequestAttributes) -> Self {
        self.attributes = attributes;
        self
    }

    pub fn with_privileged(mut self) -> Self {
        self.attributes.privileged = true;
        self
    }

    pub fn with_secure(mut self) -> Self {
        self.attributes.secure = true;
        self
    }

    pub fn with_page_table_walk(mut self) -> Self {
        self.attributes.page_table_walk = true;
        self
    }

    pub fn with_evict_next(mut self) -> Self {
        self.attributes.evict_next = true;
        self
    }

    pub fn with_uncacheable(mut self) -> Self {
        self.uncacheable = true;
        self
    }

    pub fn with_uncacheable_strict_order(mut self) -> Self {
        self.uncacheable = true;
        self.strict_order = true;
        self
    }

    pub const fn coherence_intent(&self) -> CoherenceIntent {
        self.operation.coherence_intent()
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn line_address(&self) -> Address {
        self.line_layout.line_address(self.range.start())
    }

    pub fn line_offset(&self) -> u64 {
        self.line_layout.line_offset(self.range.start())
    }

    pub fn line_span(&self) -> u64 {
        self.line_layout.line_span(self.range)
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn size(&self) -> AccessSize {
        self.range.size()
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn byte_mask(&self) -> Option<&ByteMask> {
        self.byte_mask.as_ref()
    }

    pub const fn atomic_op(&self) -> Option<MemoryAtomicOp> {
        self.atomic_op
    }

    pub fn atomic_write_data(&self, old_data: &[u8]) -> Result<Vec<u8>, MemoryError> {
        if self.operation != MemoryOperation::Atomic {
            return Err(MemoryError::UnexpectedAtomicOp { request: self.id });
        }
        if old_data.len() as u64 != self.size().bytes() {
            return Err(MemoryError::PayloadSizeMismatch {
                expected: self.size(),
                actual: old_data.len() as u64,
            });
        }
        let payload = self
            .data()
            .ok_or(MemoryError::MissingRequestData { request: self.id() })?;
        match self
            .atomic_op
            .ok_or(MemoryError::MissingAtomicOp { request: self.id() })?
        {
            MemoryAtomicOp::Swap => Ok(payload.to_vec()),
            MemoryAtomicOp::Add => self.atomic_add_data(old_data, payload),
            MemoryAtomicOp::Xor => {
                self.atomic_bitwise_data(old_data, payload, |old, new| old ^ new)
            }
            MemoryAtomicOp::Or => self.atomic_bitwise_data(old_data, payload, |old, new| old | new),
            MemoryAtomicOp::And => {
                self.atomic_bitwise_data(old_data, payload, |old, new| old & new)
            }
            MemoryAtomicOp::MinSigned => {
                self.atomic_signed_select_data(old_data, payload, |old, operand| old <= operand)
            }
            MemoryAtomicOp::MaxSigned => {
                self.atomic_signed_select_data(old_data, payload, |old, operand| old >= operand)
            }
            MemoryAtomicOp::MinUnsigned => {
                self.atomic_unsigned_select_data(old_data, payload, |old, operand| old <= operand)
            }
            MemoryAtomicOp::MaxUnsigned => {
                self.atomic_unsigned_select_data(old_data, payload, |old, operand| old >= operand)
            }
        }
    }

    fn atomic_add_data(&self, old_data: &[u8], payload: &[u8]) -> Result<Vec<u8>, MemoryError> {
        let width = self.size().as_usize()?;
        if !matches!(width, 1 | 2 | 4 | 8) {
            return Err(MemoryError::UnsupportedAtomicAccessSize {
                request: self.id(),
                op: MemoryAtomicOp::Add,
                size: self.size(),
            });
        }

        let old = read_le_u128(old_data);
        let increment = read_le_u128(payload);
        let bit_count = width * 8;
        let mask = (1u128 << bit_count) - 1;
        let sum = (old.wrapping_add(increment)) & mask;
        Ok(sum.to_le_bytes()[..width].to_vec())
    }

    fn atomic_bitwise_data(
        &self,
        old_data: &[u8],
        payload: &[u8],
        operation: fn(u128, u128) -> u128,
    ) -> Result<Vec<u8>, MemoryError> {
        let width = self.size().as_usize()?;
        if !matches!(width, 1 | 2 | 4 | 8) {
            return Err(MemoryError::UnsupportedAtomicAccessSize {
                request: self.id(),
                op: self.atomic_op().expect("validated atomic operation"),
                size: self.size(),
            });
        }

        let old = read_le_u128(old_data);
        let operand = read_le_u128(payload);
        Ok(operation(old, operand).to_le_bytes()[..width].to_vec())
    }

    fn atomic_signed_select_data(
        &self,
        old_data: &[u8],
        payload: &[u8],
        select_old: fn(i128, i128) -> bool,
    ) -> Result<Vec<u8>, MemoryError> {
        self.validate_atomic_numeric_width()?;
        let old = read_le_i128(old_data);
        let operand = read_le_i128(payload);
        if select_old(old, operand) {
            Ok(old_data.to_vec())
        } else {
            Ok(payload.to_vec())
        }
    }

    fn atomic_unsigned_select_data(
        &self,
        old_data: &[u8],
        payload: &[u8],
        select_old: fn(u128, u128) -> bool,
    ) -> Result<Vec<u8>, MemoryError> {
        self.validate_atomic_numeric_width()?;
        let old = read_le_u128(old_data);
        let operand = read_le_u128(payload);
        if select_old(old, operand) {
            Ok(old_data.to_vec())
        } else {
            Ok(payload.to_vec())
        }
    }

    fn validate_atomic_numeric_width(&self) -> Result<(), MemoryError> {
        let width = self.size().as_usize()?;
        if matches!(width, 1 | 2 | 4 | 8) {
            return Ok(());
        }

        Err(MemoryError::UnsupportedAtomicAccessSize {
            request: self.id(),
            op: self.atomic_op().expect("validated atomic operation"),
            size: self.size(),
        })
    }

    pub const fn requires_response(&self) -> bool {
        self.operation.requires_response()
    }

    pub const fn returns_data(&self) -> bool {
        self.operation.returns_data()
    }

    pub const fn carries_data(&self) -> bool {
        self.operation.carries_request_data()
    }

    pub const fn requires_writable(&self) -> bool {
        self.operation.requires_writable()
    }

    pub fn llsc_reservation_address(&self) -> Address {
        Address::new(self.range.start().get() & !(LLSC_RESERVATION_BYTES - 1))
    }

    pub fn overlaps_llsc_reservation(&self, reservation: Address) -> bool {
        Self::llsc_reservation_overlaps_range(reservation, self.range)
    }

    pub fn llsc_reservation_overlaps_range(reservation: Address, range: AddressRange) -> bool {
        let range_start = range.start().get();
        let range_end = range.end().get();
        let reservation_start = reservation.get();
        let reservation_end = reservation_start.saturating_add(LLSC_RESERVATION_BYTES);
        range_start < reservation_end && reservation_start < range_end
    }

    pub fn snapshot(&self) -> MemoryRequestSnapshot {
        MemoryRequestSnapshot {
            id: self.id,
            operation: self.operation,
            range: self.range,
            line_layout: self.line_layout,
            ordering: self.ordering,
            uncacheable: self.uncacheable,
            strict_order: self.strict_order,
            attributes: self.attributes,
            data: self.data.clone(),
            byte_mask: self.byte_mask.clone(),
            atomic_op: self.atomic_op,
        }
    }
}

fn read_le_u128(bytes: &[u8]) -> u128 {
    let mut value = 0u128;
    for (shift, byte) in bytes.iter().enumerate() {
        value |= (*byte as u128) << (shift * 8);
    }
    value
}

fn read_le_i128(bytes: &[u8]) -> i128 {
    let unsigned = read_le_u128(bytes);
    let bit_count = bytes.len() * 8;
    let sign_bit = 1u128 << (bit_count - 1);
    if unsigned & sign_bit == 0 {
        unsigned as i128
    } else {
        let extension = !0u128 << bit_count;
        (unsigned | extension) as i128
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseStatus {
    Completed,
    Retry,
    StoreConditionalFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryResponseSnapshot {
    request_id: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl MemoryResponseSnapshot {
    pub fn new(
        request_id: MemoryRequestId,
        status: ResponseStatus,
        data: Option<Vec<u8>>,
    ) -> Result<Self, MemoryError> {
        if matches!(
            status,
            ResponseStatus::Retry | ResponseStatus::StoreConditionalFailed
        ) && data.is_some()
        {
            return Err(MemoryError::UnexpectedResponseData {
                request: request_id,
            });
        }
        if data.as_ref().is_some_and(Vec::is_empty) {
            return Err(MemoryError::InvalidResponseDataLength {
                request: request_id,
                length: 0,
            });
        }

        Ok(Self {
            request_id,
            status,
            data,
        })
    }

    pub const fn request_id(&self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryResponse {
    request_id: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl MemoryResponse {
    pub fn completed(request: &MemoryRequest, data: Option<Vec<u8>>) -> Result<Self, MemoryError> {
        if !request.requires_response() {
            return Err(MemoryError::ResponseNotExpected {
                request: request.id(),
            });
        }

        Self::validate_response_data(request, data.as_deref())?;
        Ok(Self {
            request_id: request.id(),
            status: ResponseStatus::Completed,
            data,
        })
    }

    pub fn retry(request: &MemoryRequest) -> Self {
        Self {
            request_id: request.id(),
            status: ResponseStatus::Retry,
            data: None,
        }
    }

    pub fn store_conditional_failed(request: &MemoryRequest) -> Result<Self, MemoryError> {
        if request.operation() != MemoryOperation::StoreConditional {
            return Err(MemoryError::InvalidStoreConditionalFailureResponse {
                request: request.id(),
            });
        }

        Ok(Self {
            request_id: request.id(),
            status: ResponseStatus::StoreConditionalFailed,
            data: None,
        })
    }

    pub fn from_snapshot(snapshot: &MemoryResponseSnapshot) -> Result<Self, MemoryError> {
        let data = snapshot.data().map(<[u8]>::to_vec);
        MemoryResponseSnapshot::new(snapshot.request_id(), snapshot.status(), data.clone())?;
        Ok(Self {
            request_id: snapshot.request_id(),
            status: snapshot.status(),
            data,
        })
    }

    fn validate_response_data(
        request: &MemoryRequest,
        data: Option<&[u8]>,
    ) -> Result<(), MemoryError> {
        match (request.returns_data(), data) {
            (true, Some(bytes)) if bytes.len() as u64 == request.size().bytes() => Ok(()),
            (true, Some(bytes)) => Err(MemoryError::PayloadSizeMismatch {
                expected: request.size(),
                actual: bytes.len() as u64,
            }),
            (true, None) => Err(MemoryError::MissingResponseData {
                request: request.id(),
            }),
            (false, Some(_)) => Err(MemoryError::UnexpectedResponseData {
                request: request.id(),
            }),
            (false, None) => Ok(()),
        }
    }

    pub const fn request_id(&self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }

    pub fn snapshot(&self) -> MemoryResponseSnapshot {
        MemoryResponseSnapshot {
            request_id: self.request_id,
            status: self.status,
            data: self.data.clone(),
        }
    }
}
