pub const AMBA_PERIPHERAL_ID0_OFFSET: u64 = 0xfe0;
pub const AMBA_PERIPHERAL_ID1_OFFSET: u64 = 0xfe4;
pub const AMBA_PERIPHERAL_ID2_OFFSET: u64 = 0xfe8;
pub const AMBA_PERIPHERAL_ID3_OFFSET: u64 = 0xfec;
pub const AMBA_CELL_ID0_OFFSET: u64 = 0xff0;
pub const AMBA_CELL_ID1_OFFSET: u64 = 0xff4;
pub const AMBA_CELL_ID2_OFFSET: u64 = 0xff8;
pub const AMBA_CELL_ID3_OFFSET: u64 = 0xffc;

const AMBA_VENDOR_ID: u64 = 0xb105_f00d_0000_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArmPrimecellId {
    value: u64,
}

impl ArmPrimecellId {
    pub const fn new(device_id: u32) -> Self {
        Self {
            value: AMBA_VENDOR_ID | device_id as u64,
        }
    }

    pub const fn value(self) -> u64 {
        self.value
    }

    pub fn read_u32(self, offset: u64) -> Option<u32> {
        let byte_shift = match offset {
            AMBA_PERIPHERAL_ID0_OFFSET => 0,
            AMBA_PERIPHERAL_ID1_OFFSET => 8,
            AMBA_PERIPHERAL_ID2_OFFSET => 16,
            AMBA_PERIPHERAL_ID3_OFFSET => 24,
            AMBA_CELL_ID0_OFFSET => 32,
            AMBA_CELL_ID1_OFFSET => 40,
            AMBA_CELL_ID2_OFFSET => 48,
            AMBA_CELL_ID3_OFFSET => 56,
            _ => return None,
        };
        Some(((self.value >> byte_shift) & 0xff) as u32)
    }

    pub fn contains_offset(offset: u64) -> bool {
        Self::new(0).read_u32(offset).is_some()
    }
}
