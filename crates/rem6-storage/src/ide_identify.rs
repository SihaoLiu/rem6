use crate::{IdeDiskError, STORAGE_SECTOR_BYTES};

const IDE_MAX_MULTI_SECTORS: u16 = 128;
const IDE_ATA7_MAJOR: u16 = 0x0080;
const IDE_MODEL: &[u8] = b"5MI EDD si k";

pub(crate) fn ide_identify_payload(capacity: u32) -> Result<Vec<u8>, IdeDiskError> {
    let geometry = IdeGeometry::from_capacity(capacity)?;
    let mut bytes = vec![0_u8; STORAGE_SECTOR_BYTES as usize];
    put_word(&mut bytes, 1, geometry.cylinders);
    put_word(&mut bytes, 3, u16::from(geometry.heads));
    put_word(&mut bytes, 6, u16::from(geometry.sectors));
    bytes[54..54 + IDE_MODEL.len()].copy_from_slice(IDE_MODEL);
    put_word(&mut bytes, 47, IDE_MAX_MULTI_SECTORS);
    bytes[99] = 0x07;
    put_word(&mut bytes, 53, 0x0006);
    bytes[118] = IDE_MAX_MULTI_SECTORS as u8;
    bytes[119] = 0x01;
    put_dword(&mut bytes, 60, capacity);
    bytes[126] = 0x04;
    bytes[128] = 0x03;
    put_word(&mut bytes, 80, IDE_ATA7_MAJOR);
    bytes[176] = 0x1f;
    put_word(&mut bytes, 93, 0x4001);
    Ok(bytes)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IdeGeometry {
    cylinders: u16,
    heads: u8,
    sectors: u8,
}

impl IdeGeometry {
    fn from_capacity(capacity: u32) -> Result<Self, IdeDiskError> {
        if capacity == 0 {
            return Err(IdeDiskError::InvalidCapacity {
                sectors: u64::from(capacity),
            });
        }
        if capacity >= 16_383 * 16 * 63 {
            return Ok(Self {
                cylinders: 16_383,
                heads: 16,
                sectors: 63,
            });
        }

        let sectors = if capacity >= 63 { 63 } else { capacity };
        let heads = if capacity / sectors >= 16 {
            16
        } else {
            capacity / sectors
        };
        let cylinders = capacity / (heads * sectors);
        Ok(Self {
            cylinders: cylinders as u16,
            heads: heads as u8,
            sectors: sectors as u8,
        })
    }
}

fn put_word(bytes: &mut [u8], word: usize, value: u16) {
    bytes[word * 2..word * 2 + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_dword(bytes: &mut [u8], word: usize, value: u32) {
    bytes[word * 2..word * 2 + 4].copy_from_slice(&value.to_le_bytes());
}
