use rem6_isa_riscv::{MemoryAccessKind, Register, RegisterWrite};
use rem6_memory::{AccessSize, Address, AddressRange};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3StoreForwardingEntry {
    range: AddressRange,
    value: u64,
}

impl O3StoreForwardingEntry {
    pub(super) const fn range(self) -> AddressRange {
        self.range
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
    forwarded_mask: u8,
    data: [u8; 8],
}

impl O3StoreLoadForwardingPlan {
    pub(crate) fn data(self) -> Vec<u8> {
        assert_eq!(self.forwarded_bytes(), self.bytes());
        let mut data = vec![0; self.bytes_usize()];
        assert!(self.overlay_response_data(&mut data));
        data
    }

    pub(crate) const fn load_range(self) -> AddressRange {
        self.load_range
    }

    pub(crate) fn bytes(self) -> u32 {
        u32::try_from(self.load_range.size().bytes()).expect("scalar load width fits u32")
    }

    pub(super) fn matches_value(self, value: u64) -> bool {
        self.matches_data(&value.to_le_bytes())
    }

    pub(super) fn matches_data(self, data: &[u8]) -> bool {
        if data.len() < self.bytes_usize() {
            return false;
        }
        (0..self.bytes_usize())
            .all(|index| !self.forwards_index(index) || data[index] == self.data[index])
    }

    pub(crate) fn forwarded_bytes(self) -> u32 {
        self.forwarded_mask.count_ones()
    }

    pub(crate) const fn forwarded_mask(self) -> u8 {
        self.forwarded_mask
    }

    pub(crate) const fn forwarded_data(self) -> [u8; 8] {
        self.data
    }

    pub(crate) fn is_partial(self) -> bool {
        self.forwarded_bytes() < self.bytes()
    }

    pub(crate) fn overlay_response_data(self, data: &mut [u8]) -> bool {
        if data.len() < self.bytes_usize() {
            return false;
        }
        for index in 0..self.bytes_usize() {
            if self.forwards_index(index) {
                data[index] = self.data[index];
            }
        }
        true
    }

    fn empty(load_range: AddressRange) -> Self {
        debug_assert!(load_range.size().bytes() <= 8);
        Self {
            load_range,
            forwarded_mask: 0,
            data: [0; 8],
        }
    }

    fn overlay_store(&mut self, store: O3StoreForwardingEntry) {
        let store_bytes = store.value.to_le_bytes();
        for address in self.overlap_addresses(store.range) {
            let load_index = self.load_byte_index(address);
            let store_index = usize::try_from(address - store.range.start().get())
                .expect("scalar store byte index fits usize");
            self.data[load_index] = store_bytes[store_index];
            self.forwarded_mask |= 1 << load_index;
        }
    }

    fn bytes_usize(self) -> usize {
        usize::try_from(self.load_range.size().bytes()).expect("scalar load width fits usize")
    }

    fn overlap_addresses(self, store_range: AddressRange) -> std::ops::Range<u64> {
        self.load_range.start().get().max(store_range.start().get())
            ..self.load_range.end().get().min(store_range.end().get())
    }

    fn load_byte_index(self, address: u64) -> usize {
        usize::try_from(address - self.load_range.start().get())
            .expect("scalar load byte index fits usize")
    }

    fn forwards_index(self, index: usize) -> bool {
        self.forwarded_mask & (1 << index) != 0
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

pub(super) fn o3_store_load_composition<I>(
    stores: I,
    load_range: AddressRange,
) -> Option<O3StoreLoadRelation>
where
    I: IntoIterator<Item = O3StoreForwardingEntry>,
{
    let mut plan = O3StoreLoadForwardingPlan::empty(load_range);
    let mut saw_store = false;
    for store in stores {
        saw_store = true;
        plan.overlay_store(store);
    }
    if !saw_store {
        return None;
    }
    if plan.forwarded_bytes() == 0 {
        return Some(O3StoreLoadRelation::Independent(
            O3StoreLoadSuppressionReason::AddressMismatch,
        ));
    }
    if plan.is_partial() {
        Some(O3StoreLoadRelation::Overlay(plan))
    } else {
        Some(O3StoreLoadRelation::Forwarded(plan))
    }
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
            let Some(O3StoreLoadRelation::Forwarded(plan)) =
                o3_store_load_composition([store], range)
            else {
                panic!("load range at {address:#x} should be fully forwarded");
            };
            assert_eq!(plan.data(), expected);
        }

        assert_eq!(
            o3_store_load_composition([store], scalar_range(0x9004, 4).unwrap()),
            Some(O3StoreLoadRelation::Independent(
                O3StoreLoadSuppressionReason::AddressMismatch
            ))
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
            let Some(O3StoreLoadRelation::Overlay(plan)) =
                o3_store_load_composition([store], load_range)
            else {
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

    #[test]
    fn multiple_stores_compose_partial_load_with_youngest_byte_precedence() {
        let load_range = scalar_range(0x9000, 8).unwrap();
        let stores = [
            O3StoreForwardingEntry {
                range: scalar_range(0x9000, 4).unwrap(),
                value: 0x4433_bbaa,
            },
            O3StoreForwardingEntry {
                range: scalar_range(0x9002, 2).unwrap(),
                value: 0xccbb,
            },
            O3StoreForwardingEntry {
                range: scalar_range(0x9002, 1).unwrap(),
                value: 0xdd,
            },
        ];

        let Some(O3StoreLoadRelation::Overlay(plan)) =
            o3_store_load_composition(stores, load_range)
        else {
            panic!("three partial stores should compose one response overlay");
        };
        assert_eq!(plan.forwarded_bytes(), 4);
        assert!(plan.is_partial());
        let mut data = vec![0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];
        assert!(plan.overlay_response_data(&mut data));
        assert_eq!(data, vec![0xaa, 0xbb, 0xdd, 0xcc, 0x55, 0x66, 0x77, 0x88]);
        assert!(plan.matches_data(&data));
    }

    #[test]
    fn multiple_stores_fully_forward_load_bytes() {
        let load_range = scalar_range(0x9000, 4).unwrap();
        let stores = [
            O3StoreForwardingEntry {
                range: scalar_range(0x9000, 4).unwrap(),
                value: 0x4433_2211,
            },
            O3StoreForwardingEntry {
                range: scalar_range(0x9002, 2).unwrap(),
                value: 0xddcc,
            },
            O3StoreForwardingEntry {
                range: scalar_range(0x9002, 1).unwrap(),
                value: 0xee,
            },
        ];

        let Some(O3StoreLoadRelation::Forwarded(plan)) =
            o3_store_load_composition(stores, load_range)
        else {
            panic!("the store prefix should fully cover the load");
        };
        assert_eq!(plan.forwarded_bytes(), 4);
        assert!(!plan.is_partial());
        assert_eq!(plan.data(), vec![0x11, 0x22, 0xee, 0xdd]);
    }
}
