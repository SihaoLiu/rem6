use crate::{
    parse_gdb_remote_frame_with_config, GdbRemoteError, GdbRemoteFrame, GdbRemoteSession, ACK_BYTE,
    INTERRUPT_BYTE, NEGATIVE_ACK_BYTE, NOTIFICATION_START_BYTE, PACKET_START_BYTE,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GdbRemoteByteStreamResult {
    frames: Vec<GdbRemoteFrame>,
    consumed_bytes: usize,
    skipped_bytes: Vec<u8>,
}

impl GdbRemoteByteStreamResult {
    const fn new(
        frames: Vec<GdbRemoteFrame>,
        consumed_bytes: usize,
        skipped_bytes: Vec<u8>,
    ) -> Self {
        Self {
            frames,
            consumed_bytes,
            skipped_bytes,
        }
    }

    pub fn frames(&self) -> &[GdbRemoteFrame] {
        &self.frames
    }

    pub const fn consumed_bytes(&self) -> usize {
        self.consumed_bytes
    }

    pub fn skipped_bytes(&self) -> &[u8] {
        &self.skipped_bytes
    }
}

impl GdbRemoteSession {
    pub fn handle_bytes(
        &mut self,
        input: &[u8],
    ) -> Result<GdbRemoteByteStreamResult, GdbRemoteError> {
        let mut consumed_bytes = 0;
        let mut skipped_bytes = Vec::new();
        let mut frames = Vec::new();

        while consumed_bytes < input.len() {
            let remaining = &input[consumed_bytes..];
            let Some(marker_offset) = first_frame_marker_offset(remaining) else {
                skipped_bytes.extend_from_slice(remaining);
                consumed_bytes = input.len();
                break;
            };
            if marker_offset != 0 {
                skipped_bytes.extend_from_slice(&remaining[..marker_offset]);
                consumed_bytes += marker_offset;
                continue;
            }

            match parse_gdb_remote_frame_with_config(remaining, self.response_config()) {
                Ok(Some(parsed)) => {
                    consumed_bytes += parsed.consumed_bytes();
                    skipped_bytes.extend_from_slice(parsed.skipped_bytes());
                    frames.extend(self.handle_frame(parsed.frame())?);
                }
                Ok(None) => break,
                Err(GdbRemoteError::MissingChecksumSeparator | GdbRemoteError::ShortChecksum) => {
                    break;
                }
                Err(error) => return Err(error),
            }
        }

        Ok(GdbRemoteByteStreamResult::new(
            frames,
            consumed_bytes,
            skipped_bytes,
        ))
    }
}

fn first_frame_marker_offset(input: &[u8]) -> Option<usize> {
    input.iter().position(|byte| {
        matches!(
            *byte,
            ACK_BYTE
                | NEGATIVE_ACK_BYTE
                | INTERRUPT_BYTE
                | PACKET_START_BYTE
                | NOTIFICATION_START_BYTE
        )
    })
}
