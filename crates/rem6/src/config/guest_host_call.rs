use rem6_system::GuestHostCallResponse;

use crate::Rem6CliError;

use super::parse::parse_number;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GuestHostCallResponseConfig {
    selector: u64,
    response: GuestHostCallResponse,
}

impl GuestHostCallResponseConfig {
    fn new(selector: u64, response: GuestHostCallResponse) -> Self {
        Self { selector, response }
    }

    pub(crate) const fn selector(&self) -> u64 {
        self.selector
    }

    pub(crate) const fn response(&self) -> &GuestHostCallResponse {
        &self.response
    }
}

pub(super) fn parse_guest_host_call_response(
    value: &str,
) -> Result<GuestHostCallResponseConfig, Rem6CliError> {
    let mut selector = None;
    let mut status = None;
    let mut return_values = None;
    let mut payload = None;
    for field in value.split(',') {
        let Some((key, field_value)) = field.split_once('=') else {
            return Err(invalid_response(value));
        };
        match key {
            "selector" => {
                reject_duplicate(selector.is_some(), value)?;
                selector = Some(parse_number(field_value).ok_or_else(|| invalid_response(value))?);
            }
            "status" => {
                reject_duplicate(status.is_some(), value)?;
                status = Some(
                    field_value
                        .parse::<i32>()
                        .map_err(|_| invalid_response(value))?,
                );
            }
            "returns" => {
                reject_duplicate(return_values.is_some(), value)?;
                return_values = Some(parse_return_values(field_value, value)?);
            }
            "payload" => {
                reject_duplicate(payload.is_some(), value)?;
                payload = Some(parse_payload(field_value, value)?);
            }
            _ => return Err(invalid_response(value)),
        }
    }
    let (Some(selector), Some(status), Some(return_values), Some(payload)) =
        (selector, status, return_values, payload)
    else {
        return Err(invalid_response(value));
    };
    Ok(GuestHostCallResponseConfig::new(
        selector,
        GuestHostCallResponse::new(status, return_values, payload),
    ))
}

fn reject_duplicate(duplicate: bool, full_value: &str) -> Result<(), Rem6CliError> {
    if duplicate {
        Err(invalid_response(full_value))
    } else {
        Ok(())
    }
}

fn parse_return_values(field_value: &str, full_value: &str) -> Result<Vec<u64>, Rem6CliError> {
    if field_value.is_empty() {
        return Ok(Vec::new());
    }
    field_value
        .split('|')
        .map(|entry| parse_number(entry).ok_or_else(|| invalid_response(full_value)))
        .collect()
}

fn parse_payload(field_value: &str, full_value: &str) -> Result<Vec<u8>, Rem6CliError> {
    let hex = field_value.as_bytes();
    if hex.len() % 2 != 0 {
        return Err(invalid_response(full_value));
    }
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for pair in hex.chunks_exact(2) {
        let high = hex_nibble(pair[0], full_value)?;
        let low = hex_nibble(pair[1], full_value)?;
        bytes.push((high << 4) | low);
    }
    Ok(bytes)
}

fn hex_nibble(byte: u8, full_value: &str) -> Result<u8, Rem6CliError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(invalid_response(full_value)),
    }
}

fn invalid_response(value: &str) -> Rem6CliError {
    Rem6CliError::InvalidGuestHostCallResponse {
        value: value.to_string(),
    }
}
