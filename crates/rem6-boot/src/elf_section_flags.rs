const SHF_MERGE: u64 = 1 << 4;
const SHF_STRINGS: u64 = 1 << 5;
const SHF_INFO_LINK: u64 = 1 << 6;
const SHF_LINK_ORDER: u64 = 1 << 7;
const SHF_OS_NONCONFORMING: u64 = 1 << 8;
const SHF_GROUP: u64 = 1 << 9;
const SHF_TLS: u64 = 1 << 10;
const SHF_COMPRESSED: u64 = 1 << 11;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ElfSectionExtraFlagSummary {
    merge_count: u64,
    strings_count: u64,
    info_link_count: u64,
    link_order_count: u64,
    os_nonconforming_count: u64,
    group_count: u64,
    tls_count: u64,
    compressed_count: u64,
}

impl ElfSectionExtraFlagSummary {
    pub(crate) fn record(&mut self, flags: u64) -> bool {
        self.merge_count += u64::from(flags & SHF_MERGE != 0);
        self.strings_count += u64::from(flags & SHF_STRINGS != 0);
        self.info_link_count += u64::from(flags & SHF_INFO_LINK != 0);
        self.link_order_count += u64::from(flags & SHF_LINK_ORDER != 0);
        self.os_nonconforming_count += u64::from(flags & SHF_OS_NONCONFORMING != 0);
        self.group_count += u64::from(flags & SHF_GROUP != 0);
        self.tls_count += u64::from(flags & SHF_TLS != 0);
        self.compressed_count += u64::from(flags & SHF_COMPRESSED != 0);
        flags & SHF_TLS != 0
    }

    pub(crate) const fn merge_count(self) -> u64 {
        self.merge_count
    }

    pub(crate) const fn strings_count(self) -> u64 {
        self.strings_count
    }

    pub(crate) const fn info_link_count(self) -> u64 {
        self.info_link_count
    }

    pub(crate) const fn link_order_count(self) -> u64 {
        self.link_order_count
    }

    pub(crate) const fn os_nonconforming_count(self) -> u64 {
        self.os_nonconforming_count
    }

    pub(crate) const fn group_count(self) -> u64 {
        self.group_count
    }

    pub(crate) const fn tls_count(self) -> u64 {
        self.tls_count
    }

    pub(crate) const fn compressed_count(self) -> u64 {
        self.compressed_count
    }
}
