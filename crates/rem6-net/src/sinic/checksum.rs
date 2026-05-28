use super::SinicDoneStatus;

const ETHERNET_HEADER_LEN: usize = 14;
const VLAN_HEADER_LEN: usize = 18;
const ETHERTYPE_IPV4: u16 = 0x0800;
const ETHERTYPE_VLAN: u16 = 0x8100;
const IPV4_MIN_HEADER_LEN: usize = 20;
const IPV4_CHECKSUM_OFFSET: usize = 10;
const IPV4_SOURCE_OFFSET: usize = 12;
const IPV4_ADDR_BYTES: usize = 8;
const TCP_PROTOCOL: u8 = 6;
const TCP_MIN_HEADER_LEN: usize = 20;
const TCP_CHECKSUM_OFFSET: usize = 16;
const UDP_PROTOCOL: u8 = 17;
const UDP_HEADER_LEN: usize = 8;
const UDP_CHECKSUM_OFFSET: usize = 6;

pub(super) fn rx_done_status(frame: &[u8], base: SinicDoneStatus) -> SinicDoneStatus {
    let Some(ipv4) = Ipv4Layout::parse(frame) else {
        return base;
    };

    let mut status = base
        .with_ip_packet(true)
        .with_ip_error(internet_checksum(ipv4.header(frame)) != 0);

    match ipv4.protocol {
        TCP_PROTOCOL if tcp_header_is_present(frame, ipv4) => {
            status = status
                .with_tcp_packet(true)
                .with_tcp_error(transport_checksum(frame, ipv4) != 0);
        }
        UDP_PROTOCOL if udp_header_is_present(frame, ipv4) => {
            status = status
                .with_udp_packet(true)
                .with_udp_error(transport_checksum(frame, ipv4) != 0);
        }
        _ => {}
    }

    status
}

pub(super) fn apply_tx_checksum(frame: &mut [u8]) {
    let Some(ipv4) = Ipv4Layout::parse(frame) else {
        return;
    };

    match ipv4.protocol {
        TCP_PROTOCOL if tcp_header_is_present(frame, ipv4) => {
            write_u16(frame, ipv4.transport_start + TCP_CHECKSUM_OFFSET, 0);
            let checksum = transport_checksum(frame, ipv4);
            write_u16(frame, ipv4.transport_start + TCP_CHECKSUM_OFFSET, checksum);
        }
        UDP_PROTOCOL if udp_header_is_present(frame, ipv4) => {
            write_u16(frame, ipv4.transport_start + UDP_CHECKSUM_OFFSET, 0);
            let checksum = transport_checksum(frame, ipv4);
            write_u16(frame, ipv4.transport_start + UDP_CHECKSUM_OFFSET, checksum);
        }
        _ => {}
    }

    write_u16(frame, ipv4.ip_start + IPV4_CHECKSUM_OFFSET, 0);
    let checksum = internet_checksum(ipv4.header(frame));
    write_u16(frame, ipv4.ip_start + IPV4_CHECKSUM_OFFSET, checksum);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Ipv4Layout {
    ip_start: usize,
    header_len: usize,
    protocol: u8,
    transport_start: usize,
    transport_len: usize,
}

impl Ipv4Layout {
    fn parse(frame: &[u8]) -> Option<Self> {
        let (ethertype, ip_start) = ipv4_ethertype_and_offset(frame)?;
        if ethertype != ETHERTYPE_IPV4 {
            return None;
        }
        let min_end = ip_start.checked_add(IPV4_MIN_HEADER_LEN)?;
        if frame.len() < min_end || frame[ip_start] >> 4 != 4 {
            return None;
        }
        let header_len = usize::from(frame[ip_start] & 0x0f) * 4;
        if header_len < IPV4_MIN_HEADER_LEN {
            return None;
        }
        let header_end = ip_start.checked_add(header_len)?;
        if frame.len() < header_end {
            return None;
        }
        let total_len = usize::from(read_u16(frame, ip_start + 2));
        if total_len < header_len {
            return None;
        }
        let packet_end = ip_start.checked_add(total_len)?;
        if frame.len() < packet_end {
            return None;
        }
        let transport_start = header_end;
        let transport_len = total_len - header_len;
        Some(Self {
            ip_start,
            header_len,
            protocol: frame[ip_start + 9],
            transport_start,
            transport_len,
        })
    }

    fn header(self, frame: &[u8]) -> &[u8] {
        &frame[self.ip_start..self.ip_start + self.header_len]
    }

    fn transport(self, frame: &[u8]) -> &[u8] {
        &frame[self.transport_start..self.transport_start + self.transport_len]
    }
}

fn ipv4_ethertype_and_offset(frame: &[u8]) -> Option<(u16, usize)> {
    if frame.len() < ETHERNET_HEADER_LEN {
        return None;
    }
    let ethertype = read_u16(frame, 12);
    if ethertype == ETHERTYPE_VLAN {
        if frame.len() < VLAN_HEADER_LEN {
            return None;
        }
        return Some((read_u16(frame, 16), VLAN_HEADER_LEN));
    }
    Some((ethertype, ETHERNET_HEADER_LEN))
}

fn tcp_header_is_present(frame: &[u8], ipv4: Ipv4Layout) -> bool {
    if ipv4.transport_len < TCP_MIN_HEADER_LEN {
        return false;
    }
    let data_offset = usize::from(frame[ipv4.transport_start + 12] >> 4) * 4;
    data_offset >= TCP_MIN_HEADER_LEN && data_offset <= ipv4.transport_len
}

fn udp_header_is_present(frame: &[u8], ipv4: Ipv4Layout) -> bool {
    if ipv4.transport_len < UDP_HEADER_LEN {
        return false;
    }
    let udp_len = usize::from(read_u16(frame, ipv4.transport_start + 4));
    udp_len >= UDP_HEADER_LEN && udp_len <= ipv4.transport_len
}

fn transport_checksum(frame: &[u8], ipv4: Ipv4Layout) -> u16 {
    let mut sum = ChecksumSum::new();
    let transport_len = u16::try_from(ipv4.transport_len).expect("IPv4 transport length fits u16");
    sum.add_bytes(
        &frame[ipv4.ip_start + IPV4_SOURCE_OFFSET
            ..ipv4.ip_start + IPV4_SOURCE_OFFSET + IPV4_ADDR_BYTES],
    );
    sum.add_word(u16::from(ipv4.protocol) + transport_len);
    sum.add_bytes(ipv4.transport(frame));
    sum.finish()
}

fn internet_checksum(bytes: &[u8]) -> u16 {
    let mut sum = ChecksumSum::new();
    sum.add_bytes(bytes);
    sum.finish()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChecksumSum {
    value: u32,
}

impl ChecksumSum {
    const fn new() -> Self {
        Self { value: 0 }
    }

    fn add_bytes(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(2);
        for chunk in &mut chunks {
            self.value += u32::from(u16::from_be_bytes([chunk[0], chunk[1]]));
        }
        if let [last] = chunks.remainder() {
            self.value += u32::from(*last) << 8;
        }
    }

    fn add_word(&mut self, value: u16) {
        self.value += u32::from(value);
    }

    fn finish(mut self) -> u16 {
        while (self.value >> 16) != 0 {
            self.value = (self.value & 0xffff) + (self.value >> 16);
        }
        !(self.value as u16)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([bytes[offset], bytes[offset + 1]])
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    let [high, low] = value.to_be_bytes();
    bytes[offset] = high;
    bytes[offset + 1] = low;
}
