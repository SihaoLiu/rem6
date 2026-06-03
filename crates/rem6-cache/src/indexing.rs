use std::error::Error;
use std::fmt;

use rem6_memory::{Address, CacheLineLayout};

const SKEWING_FUNCTIONS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheIndexingPolicyKind {
    SetAssociative,
    SkewedAssociative,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CacheIndexingLocation {
    set: usize,
    way: usize,
}

impl CacheIndexingLocation {
    pub const fn new(set: usize, way: usize) -> Self {
        Self { set, way }
    }

    pub const fn set(self) -> usize {
        self.set
    }

    pub const fn way(self) -> usize {
        self.way
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheIndexingPolicyConfig {
    kind: CacheIndexingPolicyKind,
    line_layout: CacheLineLayout,
    sets: usize,
    ways: usize,
    set_shift: u32,
    set_bits: u32,
    tag_shift: u32,
    set_mask: u64,
}

impl CacheIndexingPolicyConfig {
    pub fn new(
        kind: CacheIndexingPolicyKind,
        line_layout: CacheLineLayout,
        sets: usize,
        ways: usize,
    ) -> Result<Self, CacheIndexingPolicyError> {
        if sets == 0 {
            return Err(CacheIndexingPolicyError::ZeroSets);
        }
        if ways == 0 {
            return Err(CacheIndexingPolicyError::ZeroWays);
        }
        if !sets.is_power_of_two() {
            return Err(CacheIndexingPolicyError::SetsNotPowerOfTwo { sets });
        }

        let set_shift = line_layout.bytes().trailing_zeros();
        let set_bits = sets.trailing_zeros();

        if kind == CacheIndexingPolicyKind::SkewedAssociative {
            if sets <= 2 {
                return Err(CacheIndexingPolicyError::SkewedAssociativeTooFewSets { sets });
            }
            if set_shift + 2 * set_bits > 64 {
                return Err(
                    CacheIndexingPolicyError::SkewedAssociativeAddressBitsTooWide {
                        set_shift,
                        set_bits,
                    },
                );
            }
        }

        Ok(Self {
            kind,
            line_layout,
            sets,
            ways,
            set_shift,
            set_bits,
            tag_shift: set_shift + set_bits,
            set_mask: sets as u64 - 1,
        })
    }

    pub const fn kind(&self) -> CacheIndexingPolicyKind {
        self.kind
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn sets(&self) -> usize {
        self.sets
    }

    pub const fn ways(&self) -> usize {
        self.ways
    }

    pub fn extract_tag(&self, address: Address) -> u64 {
        right_shift(self.normalized_line_address(address), self.tag_shift)
    }

    pub fn candidate_locations(&self, address: Address) -> Vec<CacheIndexingLocation> {
        let block_address = self.block_address(address);
        (0..self.ways)
            .map(|way| {
                let set = match self.kind {
                    CacheIndexingPolicyKind::SetAssociative => block_address & self.set_mask,
                    CacheIndexingPolicyKind::SkewedAssociative => {
                        self.skew(block_address, way) & self.set_mask
                    }
                };
                CacheIndexingLocation::new(set as usize, way)
            })
            .collect()
    }

    pub fn regenerate_address(
        &self,
        tag: u64,
        location: CacheIndexingLocation,
    ) -> Result<Address, CacheIndexingPolicyError> {
        self.check_location(location)?;

        let set = match self.kind {
            CacheIndexingPolicyKind::SetAssociative => location.set as u64,
            CacheIndexingPolicyKind::SkewedAssociative => {
                let skewed_block = left_shift(tag, self.set_bits) | location.set as u64;
                self.deskew(skewed_block, location.way) & self.set_mask
            }
        };

        Ok(Address::new(
            left_shift(tag, self.tag_shift) | left_shift(set, self.set_shift),
        ))
    }

    fn check_location(
        &self,
        location: CacheIndexingLocation,
    ) -> Result<(), CacheIndexingPolicyError> {
        if location.set >= self.sets {
            return Err(CacheIndexingPolicyError::UnknownSet {
                set: location.set,
                sets: self.sets,
            });
        }
        if location.way >= self.ways {
            return Err(CacheIndexingPolicyError::UnknownWay {
                way: location.way,
                ways: self.ways,
            });
        }
        Ok(())
    }

    fn normalized_line_address(&self, address: Address) -> u64 {
        self.line_layout.line_address(address).get()
    }

    fn block_address(&self, address: Address) -> u64 {
        self.normalized_line_address(address) >> self.set_shift
    }

    fn msb_shift(&self) -> u32 {
        self.set_bits - 1
    }

    fn skew(&self, block_address: u64, way: usize) -> u64 {
        let mut addr1 = lower_bits(block_address, self.set_bits);
        let addr2 = bits(block_address, 2 * self.set_bits - 1, self.set_bits);

        addr1 = match way % SKEWING_FUNCTIONS {
            0 => self.hash(addr1) ^ self.hash(addr2) ^ addr2,
            1 => self.hash(addr1) ^ self.hash(addr2) ^ addr1,
            2 => self.hash(addr1) ^ self.dehash(addr2) ^ addr2,
            3 => self.hash(addr1) ^ self.dehash(addr2) ^ addr1,
            4 => self.dehash(addr1) ^ self.hash(addr2) ^ addr2,
            5 => self.dehash(addr1) ^ self.hash(addr2) ^ addr1,
            6 => self.dehash(addr1) ^ self.dehash(addr2) ^ addr2,
            7 => self.dehash(addr1) ^ self.dehash(addr2) ^ addr1,
            _ => unreachable!(),
        };

        for _ in 0..way / SKEWING_FUNCTIONS {
            addr1 = self.hash(addr1);
        }

        addr1
    }

    fn deskew(&self, block_address: u64, way: usize) -> u64 {
        let mut addr1 = lower_bits(block_address, self.set_bits);
        let addr2 = bits(block_address, 2 * self.set_bits - 1, self.set_bits);

        for _ in 0..way / SKEWING_FUNCTIONS {
            addr1 = self.dehash(addr1);
        }

        match way % SKEWING_FUNCTIONS {
            0 => self.dehash(addr1 ^ self.hash(addr2) ^ addr2),
            1 => {
                addr1 ^= self.hash(addr2);
                for _ in 0..self.msb_shift() {
                    addr1 = self.hash(addr1);
                }
                addr1
            }
            2 => self.dehash(addr1 ^ self.dehash(addr2) ^ addr2),
            3 => {
                addr1 ^= self.dehash(addr2);
                for _ in 0..self.msb_shift() {
                    addr1 = self.hash(addr1);
                }
                addr1
            }
            4 => self.hash(addr1 ^ self.hash(addr2) ^ addr2),
            5 => {
                addr1 ^= self.hash(addr2);
                for _ in 0..=self.msb_shift() {
                    addr1 = self.hash(addr1);
                }
                addr1
            }
            6 => self.hash(addr1 ^ self.dehash(addr2) ^ addr2),
            7 => {
                addr1 ^= self.dehash(addr2);
                for _ in 0..=self.msb_shift() {
                    addr1 = self.hash(addr1);
                }
                addr1
            }
            _ => unreachable!(),
        }
    }

    fn hash(&self, value: u64) -> u64 {
        let msb_shift = self.msb_shift();
        let lsb = bit(value, 0);
        let msb = bit(value, msb_shift);
        set_bit(value >> 1, msb_shift, msb ^ lsb)
    }

    fn dehash(&self, value: u64) -> u64 {
        let msb_shift = self.msb_shift();
        let msb = bit(value, msb_shift - 1);
        let xor_bit = bit(value, msb_shift);
        let addr_no_msb = lower_bits(value, msb_shift);
        set_bit(addr_no_msb << 1, 0, msb ^ xor_bit)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheIndexingPolicyError {
    ZeroSets,
    ZeroWays,
    SetsNotPowerOfTwo { sets: usize },
    SkewedAssociativeTooFewSets { sets: usize },
    SkewedAssociativeAddressBitsTooWide { set_shift: u32, set_bits: u32 },
    UnknownSet { set: usize, sets: usize },
    UnknownWay { way: usize, ways: usize },
}

impl fmt::Display for CacheIndexingPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSets => write!(formatter, "cache indexing policy has no sets"),
            Self::ZeroWays => write!(formatter, "cache indexing policy has no ways"),
            Self::SetsNotPowerOfTwo { sets } => write!(
                formatter,
                "cache indexing policy needs a power-of-two set count, got {sets}"
            ),
            Self::SkewedAssociativeTooFewSets { sets } => write!(
                formatter,
                "skewed associative indexing needs more than two sets, got {sets}"
            ),
            Self::SkewedAssociativeAddressBitsTooWide {
                set_shift,
                set_bits,
            } => write!(
                formatter,
                "skewed associative indexing set shift {set_shift} plus twice set bits {set_bits} exceeds 64 address bits"
            ),
            Self::UnknownSet { set, sets } => write!(
                formatter,
                "cache indexing location set {set} is outside {sets} sets"
            ),
            Self::UnknownWay { way, ways } => write!(
                formatter,
                "cache indexing location way {way} is outside {ways} ways"
            ),
        }
    }
}

impl Error for CacheIndexingPolicyError {}

fn right_shift(value: u64, shift: u32) -> u64 {
    value.checked_shr(shift).unwrap_or(0)
}

fn left_shift(value: u64, shift: u32) -> u64 {
    value.checked_shl(shift).unwrap_or(0)
}

fn bit(value: u64, index: u32) -> u64 {
    right_shift(value, index) & 1
}

fn bits(value: u64, high: u32, low: u32) -> u64 {
    lower_bits(right_shift(value, low), high - low + 1)
}

fn lower_bits(value: u64, count: u32) -> u64 {
    value & bit_mask(count)
}

fn set_bit(value: u64, index: u32, bit: u64) -> u64 {
    let mask = left_shift(1, index);
    if bit & 1 == 0 {
        value & !mask
    } else {
        value | mask
    }
}

fn bit_mask(count: u32) -> u64 {
    if count >= 64 {
        u64::MAX
    } else {
        (1_u64 << count) - 1
    }
}
