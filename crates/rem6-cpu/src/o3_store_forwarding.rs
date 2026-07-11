use rem6_isa_riscv::{MemoryAccessKind, Register, RegisterWrite};
use rem6_memory::{AccessSize, Address, AddressRange};

const U64_BYTES: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3StoreForwardingEntry {
    range: AddressRange,
    value: u64,
}

impl O3StoreForwardingEntry {
    pub(super) fn relation(self, load_range: AddressRange) -> O3StoreLoadRelation {
        if self.range.contains_range(load_range) {
            let byte_offset = usize::try_from(
                load_range
                    .start()
                    .get()
                    .checked_sub(self.range.start().get())
                    .expect("contained load starts within store range"),
            )
            .expect("scalar store byte offset fits usize");
            let shift = u32::try_from(
                byte_offset
                    .checked_mul(8)
                    .expect("scalar store byte shift fits usize"),
            )
            .expect("scalar store byte shift fits u32");
            return O3StoreLoadRelation::Forwarded(O3StoreLoadForwardingPlan {
                load_range,
                value: self.value.checked_shr(shift).unwrap_or(0),
            });
        }
        if self.range.overlaps(load_range) {
            O3StoreLoadRelation::Blocked(self.suppression_reason(load_range))
        } else {
            O3StoreLoadRelation::Independent(O3StoreLoadSuppressionReason::AddressMismatch)
        }
    }

    fn suppression_reason(self, load_range: AddressRange) -> O3StoreLoadSuppressionReason {
        if self.range.start() == load_range.start() {
            O3StoreLoadSuppressionReason::ByteMismatch
        } else {
            O3StoreLoadSuppressionReason::AddressMismatch
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3LoadForwardingAccess {
    register: Register,
    range: AddressRange,
}

impl O3LoadForwardingAccess {
    pub(super) const fn register(self) -> Register {
        self.register
    }

    pub(super) const fn range(self) -> AddressRange {
        self.range
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3StoreLoadForwardingPlan {
    load_range: AddressRange,
    value: u64,
}

impl O3StoreLoadForwardingPlan {
    pub(super) fn data(self) -> Vec<u8> {
        self.value.to_le_bytes()[..self.bytes_usize()].to_vec()
    }

    pub(super) const fn load_range(self) -> AddressRange {
        self.load_range
    }

    pub(super) fn value(self) -> u64 {
        low_bytes(self.value, self.bytes())
    }

    pub(super) fn bytes(self) -> u32 {
        u32::try_from(self.load_range.size().bytes()).expect("scalar load width fits u32")
    }

    pub(super) fn matches_value(self, value: u64) -> bool {
        low_bytes(value, self.bytes()) == self.value()
    }

    pub(super) fn matches_data(self, data: &[u8]) -> bool {
        o3_load_data_value(data, self.bytes()).is_some_and(|value| self.matches_value(value))
    }

    fn bytes_usize(self) -> usize {
        usize::try_from(self.load_range.size().bytes()).expect("scalar load width fits usize")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum O3StoreLoadRelation {
    Forwarded(O3StoreLoadForwardingPlan),
    Independent(O3StoreLoadSuppressionReason),
    Blocked(O3StoreLoadSuppressionReason),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum O3StoreLoadSuppressionReason {
    AddressMismatch,
    ByteMismatch,
}

pub(super) fn o3_store_load_relation(
    store: &MemoryAccessKind,
    load_range: AddressRange,
) -> Option<O3StoreLoadRelation> {
    Some(o3_store_forwarding_entry(store)?.relation(load_range))
}

pub(super) fn o3_store_forwarding_entry(
    access: &MemoryAccessKind,
) -> Option<O3StoreForwardingEntry> {
    match access {
        MemoryAccessKind::Store {
            address,
            width,
            value,
        } => Some(O3StoreForwardingEntry {
            range: scalar_range(*address, width.bytes())?,
            value: *value,
        }),
        _ => None,
    }
}

pub(super) fn o3_load_forwarding_access(
    access: &MemoryAccessKind,
) -> Option<O3LoadForwardingAccess> {
    match access {
        MemoryAccessKind::Load {
            rd, address, width, ..
        } => Some(O3LoadForwardingAccess {
            register: *rd,
            range: scalar_range(*address, width.bytes())?,
        }),
        _ => None,
    }
}

pub(super) fn o3_load_register_value(
    register_writes: &[RegisterWrite],
    register: Register,
) -> Option<u64> {
    register_writes
        .iter()
        .find(|write| write.register() == register)
        .map(RegisterWrite::value)
}

fn scalar_range(address: u64, bytes: usize) -> Option<AddressRange> {
    AddressRange::new(
        Address::new(address),
        AccessSize::new(u64::try_from(bytes).ok()?).ok()?,
    )
    .ok()
}

fn o3_load_data_value(data: &[u8], bytes: u32) -> Option<u64> {
    let width = usize::try_from(bytes).ok()?;
    if data.len() < width || width > U64_BYTES {
        return None;
    }
    let mut value = 0_u64;
    for (index, byte) in data.iter().take(width).copied().enumerate() {
        value |= u64::from(byte) << (index * 8);
    }
    Some(value)
}

fn low_bytes(value: u64, bytes: u32) -> u64 {
    let bits = bytes.saturating_mul(8);
    let mask = if bits >= u64::BITS {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    value & mask
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::MemoryWidth;

    use super::*;

    #[test]
    fn word_store_classifies_contained_independent_and_partial_load_ranges() {
        let store = o3_store_forwarding_entry(&MemoryAccessKind::Store {
            address: 0x9000,
            width: MemoryWidth::Word,
            value: 0x007f_80ff,
        })
        .unwrap();

        for (address, bytes, expected) in [
            (0x9000, 4, vec![0xff, 0x80, 0x7f, 0x00]),
            (0x9001, 1, vec![0x80]),
            (0x9000, 2, vec![0xff, 0x80]),
            (0x9002, 2, vec![0x7f, 0x00]),
        ] {
            let range = scalar_range(address, bytes).unwrap();
            let O3StoreLoadRelation::Forwarded(plan) = store.relation(range) else {
                panic!("load range at {address:#x} should be fully forwarded");
            };
            assert_eq!(plan.data(), expected);
        }

        assert_eq!(
            store.relation(scalar_range(0x9004, 4).unwrap()),
            O3StoreLoadRelation::Independent(O3StoreLoadSuppressionReason::AddressMismatch)
        );
        for (address, bytes, reason) in [
            (0x8fff, 2, O3StoreLoadSuppressionReason::AddressMismatch),
            (0x9000, 8, O3StoreLoadSuppressionReason::ByteMismatch),
            (0x9003, 2, O3StoreLoadSuppressionReason::AddressMismatch),
        ] {
            assert_eq!(
                store.relation(scalar_range(address, bytes).unwrap()),
                O3StoreLoadRelation::Blocked(reason)
            );
        }
    }
}
