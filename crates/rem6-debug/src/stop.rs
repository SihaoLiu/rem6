use crate::hex::{encode_hex_nibble, encode_hex_u64};
use crate::GdbRemoteTrapKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GdbRemoteStopReply {
    Signal {
        signal: u8,
    },
    DataWatchpoint {
        signal: u8,
        kind: GdbRemoteTrapKind,
        address: u64,
    },
}

impl GdbRemoteStopReply {
    pub const fn signal(signal: u8) -> Self {
        Self::Signal { signal }
    }

    pub const fn data_watchpoint(
        signal: u8,
        kind: GdbRemoteTrapKind,
        address: u64,
    ) -> Option<Self> {
        match kind {
            GdbRemoteTrapKind::WriteWatchpoint
            | GdbRemoteTrapKind::ReadWatchpoint
            | GdbRemoteTrapKind::AccessWatchpoint => Some(Self::DataWatchpoint {
                signal,
                kind,
                address,
            }),
            GdbRemoteTrapKind::SoftwareBreakpoint | GdbRemoteTrapKind::HardwareBreakpoint => None,
        }
    }

    pub fn encode_payload(&self) -> Vec<u8> {
        match self {
            Self::Signal { signal } => {
                vec![
                    b'S',
                    encode_hex_nibble(signal >> 4),
                    encode_hex_nibble(signal & 0x0f),
                ]
            }
            Self::DataWatchpoint {
                signal,
                kind,
                address,
            } => {
                let mut payload = vec![
                    b'T',
                    encode_hex_nibble(signal >> 4),
                    encode_hex_nibble(signal & 0x0f),
                ];
                payload.extend_from_slice(data_watchpoint_stop_reason(*kind));
                payload.push(b':');
                payload.extend_from_slice(&encode_hex_u64(*address));
                payload.push(b';');
                payload
            }
        }
    }
}

fn data_watchpoint_stop_reason(kind: GdbRemoteTrapKind) -> &'static [u8] {
    match kind {
        GdbRemoteTrapKind::WriteWatchpoint => b"watch",
        GdbRemoteTrapKind::ReadWatchpoint => b"rwatch",
        GdbRemoteTrapKind::AccessWatchpoint => b"awatch",
        GdbRemoteTrapKind::SoftwareBreakpoint | GdbRemoteTrapKind::HardwareBreakpoint => {
            unreachable!("validated data watchpoint kind")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_data_watchpoint_stop_replies() {
        assert_eq!(
            GdbRemoteStopReply::data_watchpoint(0x05, GdbRemoteTrapKind::WriteWatchpoint, 0x1040)
                .unwrap()
                .encode_payload(),
            b"T05watch:1040;"
        );
        assert_eq!(
            GdbRemoteStopReply::data_watchpoint(0x05, GdbRemoteTrapKind::ReadWatchpoint, 0x1040)
                .unwrap()
                .encode_payload(),
            b"T05rwatch:1040;"
        );
        assert_eq!(
            GdbRemoteStopReply::data_watchpoint(0x05, GdbRemoteTrapKind::AccessWatchpoint, 0x1040)
                .unwrap()
                .encode_payload(),
            b"T05awatch:1040;"
        );
    }
}
