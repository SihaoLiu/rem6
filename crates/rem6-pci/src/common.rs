use crate::{PCI_CONFIG_SPACE_SIZE, PCI_STATUS_OFFSET};

const PCI_COMMAND_WRITABLE_MASK: u16 = 0x03ff;

pub(crate) fn write_common_command(
    config: &mut [u8; PCI_CONFIG_SPACE_SIZE],
    offset: usize,
    value: u16,
) {
    write_u16_at(config, offset, value & PCI_COMMAND_WRITABLE_MASK);
}

pub(crate) fn write_common_status(
    config: &mut [u8; PCI_CONFIG_SPACE_SIZE],
    value: u16,
    read_only_mask: u16,
) {
    let current = u16::from_le_bytes(
        config[PCI_STATUS_OFFSET..PCI_STATUS_OFFSET + 2]
            .try_into()
            .unwrap(),
    );
    let writable_clear_mask = value & !read_only_mask;
    write_u16_at(config, PCI_STATUS_OFFSET, current & !writable_clear_mask);
}

pub(crate) fn write_u16_at(config: &mut [u8; PCI_CONFIG_SPACE_SIZE], offset: usize, value: u16) {
    config[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn write_u32_at(config: &mut [u8; PCI_CONFIG_SPACE_SIZE], offset: usize, value: u32) {
    config[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
