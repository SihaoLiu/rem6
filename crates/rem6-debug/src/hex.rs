use crate::GdbRemoteError;

pub(crate) fn encode_hex_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(encode_hex_nibble(byte >> 4));
        encoded.push(encode_hex_nibble(byte & 0x0f));
    }
    encoded
}

pub(crate) fn encode_hex_u64(value: u64) -> Vec<u8> {
    format!("{value:x}").into_bytes()
}

pub(crate) fn decode_hex_bytes(digits: &[u8]) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(digits.len() / 2);
    let mut chunks = digits.chunks_exact(2);
    for chunk in &mut chunks {
        let high = decode_hex_nibble(chunk[0]).ok()?;
        let low = decode_hex_nibble(chunk[1]).ok()?;
        bytes.push((high << 4) | low);
    }
    if chunks.remainder().is_empty() {
        Some(bytes)
    } else {
        None
    }
}

pub(crate) fn decode_hex_u64(digits: &[u8]) -> Option<u64> {
    if digits.is_empty() {
        return None;
    }

    let mut value = 0u64;
    for digit in digits {
        let nibble = match digit {
            b'0'..=b'9' => digit - b'0',
            b'a'..=b'f' => digit - b'a' + 10,
            b'A'..=b'F' => digit - b'A' + 10,
            _ => return None,
        };
        value = value.checked_mul(16)?;
        value = value.checked_add(u64::from(nibble))?;
    }
    Some(value)
}

pub(crate) fn decode_hex_usize(digits: &[u8]) -> Option<usize> {
    usize::try_from(decode_hex_u64(digits)?).ok()
}

pub(crate) fn decode_checksum(high: u8, low: u8) -> Result<u8, GdbRemoteError> {
    let high = decode_hex_nibble(high)?;
    let low = decode_hex_nibble(low)?;
    Ok((high << 4) | low)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, GdbRemoteError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(GdbRemoteError::InvalidChecksumHex { byte }),
    }
}

pub(crate) fn encode_hex_nibble(nibble: u8) -> u8 {
    debug_assert!(nibble < 16);
    match nibble {
        0..=9 => b'0' + nibble,
        10..=15 => b'a' + (nibble - 10),
        _ => unreachable!("nibble must be less than 16"),
    }
}
