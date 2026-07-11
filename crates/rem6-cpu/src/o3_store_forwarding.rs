use rem6_isa_riscv::{MemoryAccessKind, Register, RegisterWrite};
use rem6_memory::{AccessSize, Address, AddressRange};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3StoreForwardingEntry {
    range: AddressRange,
    value: u64,
}

impl O3StoreForwardingEntry {
    pub(super) fn relation(self, load_range: AddressRange) -> O3StoreLoadRelation {
        let plan = O3StoreLoadForwardingPlan {
            load_range,
            store_range: self.range,
            store_value: self.value,
        };
        if self.range.contains_range(load_range) {
            return O3StoreLoadRelation::Forwarded(plan);
        }
        if self.range.overlaps(load_range) {
            O3StoreLoadRelation::Overlay(plan)
        } else {
            O3StoreLoadRelation::Independent(O3StoreLoadSuppressionReason::AddressMismatch)
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
pub(crate) struct O3StoreLoadForwardingPlan {
    load_range: AddressRange,
    store_range: AddressRange,
    store_value: u64,
}

impl O3StoreLoadForwardingPlan {
    pub(crate) fn data(self) -> Vec<u8> {
        assert_eq!(self.forwarded_bytes(), self.bytes());
        let mut data = vec![0; self.bytes_usize()];
        assert!(self.overlay_response_data(&mut data));
        data
    }

    pub(super) const fn load_range(self) -> AddressRange {
        self.load_range
    }

    pub(super) fn bytes(self) -> u32 {
        u32::try_from(self.load_range.size().bytes()).expect("scalar load width fits u32")
    }

    pub(super) fn matches_value(self, value: u64) -> bool {
        self.matches_data(&value.to_le_bytes())
    }

    pub(super) fn matches_data(self, data: &[u8]) -> bool {
        if data.len() < self.bytes_usize() {
            return false;
        }
        let store_bytes = self.store_value.to_le_bytes();
        self.overlap_addresses().all(|address| {
            let load_index = self.load_byte_index(address);
            let store_index = self.store_byte_index(address);
            data[load_index] == store_bytes[store_index]
        })
    }

    pub(crate) fn forwarded_bytes(self) -> u32 {
        u32::try_from(self.overlap_addresses().count())
            .expect("scalar forwarding byte count fits u32")
    }

    pub(crate) fn is_partial(self) -> bool {
        self.forwarded_bytes() < self.bytes()
    }

    pub(crate) fn overlay_response_data(self, data: &mut [u8]) -> bool {
        if data.len() < self.bytes_usize() {
            return false;
        }
        let store_bytes = self.store_value.to_le_bytes();
        for address in self.overlap_addresses() {
            data[self.load_byte_index(address)] = store_bytes[self.store_byte_index(address)];
        }
        true
    }

    fn bytes_usize(self) -> usize {
        usize::try_from(self.load_range.size().bytes()).expect("scalar load width fits usize")
    }

    fn overlap_addresses(self) -> std::ops::Range<u64> {
        self.load_range
            .start()
            .get()
            .max(self.store_range.start().get())
            ..self
                .load_range
                .end()
                .get()
                .min(self.store_range.end().get())
    }

    fn load_byte_index(self, address: u64) -> usize {
        usize::try_from(address - self.load_range.start().get())
            .expect("scalar load byte index fits usize")
    }

    fn store_byte_index(self, address: u64) -> usize {
        usize::try_from(address - self.store_range.start().get())
            .expect("scalar store byte index fits usize")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum O3StoreLoadRelation {
    Forwarded(O3StoreLoadForwardingPlan),
    Overlay(O3StoreLoadForwardingPlan),
    Independent(O3StoreLoadSuppressionReason),
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

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::MemoryWidth;

    use super::*;

    #[test]
    fn word_store_classifies_contained_and_independent_load_ranges() {
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
    }

    #[test]
    fn partial_store_overlap_plans_merge_only_store_owned_bytes() {
        for (store, load_range, response, expected, forwarded_bytes) in [
            (
                O3StoreForwardingEntry {
                    range: scalar_range(0x9001, 1).unwrap(),
                    value: 0x5a,
                },
                scalar_range(0x9000, 4).unwrap(),
                vec![0x11, 0x22, 0x33, 0x80],
                vec![0x11, 0x5a, 0x33, 0x80],
                1,
            ),
            (
                O3StoreForwardingEntry {
                    range: scalar_range(0x9002, 2).unwrap(),
                    value: 0x065a,
                },
                scalar_range(0x9000, 4).unwrap(),
                vec![0x11, 0x22, 0x33, 0x80],
                vec![0x11, 0x22, 0x5a, 0x06],
                2,
            ),
            (
                O3StoreForwardingEntry {
                    range: scalar_range(0x9000, 4).unwrap(),
                    value: 0x0000_065a,
                },
                scalar_range(0x9000, 8).unwrap(),
                vec![0x11, 0x22, 0x33, 0x80, 0x55, 0x66, 0x77, 0x00],
                vec![0x5a, 0x06, 0x00, 0x00, 0x55, 0x66, 0x77, 0x00],
                4,
            ),
            (
                O3StoreForwardingEntry {
                    range: scalar_range(0x9004, 4).unwrap(),
                    value: 0x0000_065a,
                },
                scalar_range(0x9000, 8).unwrap(),
                vec![0x11, 0x22, 0x33, 0x80, 0x55, 0x66, 0x77, 0x00],
                vec![0x11, 0x22, 0x33, 0x80, 0x5a, 0x06, 0x00, 0x00],
                4,
            ),
            (
                O3StoreForwardingEntry {
                    range: scalar_range(0x9000, 4).unwrap(),
                    value: 0xddcc_bbaa,
                },
                scalar_range(0x9002, 4).unwrap(),
                vec![0x11, 0x22, 0x33, 0x44],
                vec![0xcc, 0xdd, 0x33, 0x44],
                2,
            ),
            (
                O3StoreForwardingEntry {
                    range: scalar_range(0x9000, 4).unwrap(),
                    value: 0xddcc_bbaa,
                },
                scalar_range(0x8fff, 4).unwrap(),
                vec![0x11, 0x22, 0x33, 0x44],
                vec![0x11, 0xaa, 0xbb, 0xcc],
                3,
            ),
        ] {
            let O3StoreLoadRelation::Overlay(plan) = store.relation(load_range) else {
                panic!("partial overlap should produce a response overlay");
            };
            assert_eq!(plan.forwarded_bytes(), forwarded_bytes);
            let mut actual = response.clone();
            assert!(plan.overlay_response_data(&mut actual));
            assert_eq!(actual, expected);
            assert!(plan.matches_data(&actual));
            assert!(!plan.matches_data(&response));
        }
    }
}
