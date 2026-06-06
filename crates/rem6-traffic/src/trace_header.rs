use crate::{
    trace_proto::{read_string_field, read_u32_field, ProtoMessageParser},
    TrafficGeneratorError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTraceIdString {
    key: u32,
    value: String,
}

impl TrafficTraceIdString {
    const fn new(key: u32, value: String) -> Self {
        Self { key, value }
    }

    pub const fn key(&self) -> u32 {
        self.key
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TrafficTraceHeader {
    object_id: Option<String>,
    version: u32,
    tick_frequency: u64,
    id_strings: Vec<TrafficTraceIdString>,
}

impl TrafficTraceHeader {
    pub(crate) fn new(
        object_id: Option<String>,
        version: u32,
        tick_frequency: u64,
        id_strings: Vec<TrafficTraceIdString>,
    ) -> Self {
        Self {
            object_id,
            version,
            tick_frequency,
            id_strings,
        }
    }

    pub(crate) fn object_id(&self) -> Option<&str> {
        self.object_id.as_deref()
    }

    pub(crate) const fn version(&self) -> u32 {
        self.version
    }

    pub(crate) const fn tick_frequency(&self) -> u64 {
        self.tick_frequency
    }

    pub(crate) fn id_strings(&self) -> &[TrafficTraceIdString] {
        &self.id_strings
    }

    pub(crate) fn id_string(&self, key: u32) -> Option<&str> {
        self.id_strings
            .iter()
            .rev()
            .find(|entry| entry.key == key)
            .map(TrafficTraceIdString::value)
    }
}

pub(crate) fn parse_gem5_packet_header(
    message: &[u8],
) -> Result<TrafficTraceHeader, TrafficGeneratorError> {
    let mut parser = ProtoMessageParser::new(message);
    let mut object_id = None;
    let mut version = 0;
    let mut tick_frequency = None;
    let mut id_strings = Vec::new();

    while let Some(field) = parser.next_field()? {
        match field.number {
            1 => object_id = Some(read_string_field(field, message, "PacketHeader", "obj_id")?),
            2 => version = read_u32_field(field, "PacketHeader", "ver")?,
            3 => tick_frequency = Some(field.varint("PacketHeader", "tick_freq")?),
            4 => id_strings.push(parse_id_string_entry(field.length_delimited(
                message,
                "PacketHeader",
                "id_strings",
            )?)?),
            _ => {}
        }
        parser.skip(field)?;
    }

    let tick_frequency = tick_frequency.ok_or(TrafficGeneratorError::TraceMissingField {
        message: "PacketHeader",
        field: "tick_freq",
    })?;
    Ok(TrafficTraceHeader::new(
        object_id.map(str::to_owned),
        version,
        tick_frequency,
        id_strings,
    ))
}

fn parse_id_string_entry(message: &[u8]) -> Result<TrafficTraceIdString, TrafficGeneratorError> {
    let mut parser = ProtoMessageParser::new(message);
    let mut key = 0;
    let mut value = "";

    while let Some(field) = parser.next_field()? {
        match field.number {
            1 => key = read_u32_field(field, "PacketHeader.IdStringEntry", "key")?,
            2 => value = read_string_field(field, message, "PacketHeader.IdStringEntry", "value")?,
            _ => {}
        }
        parser.skip(field)?;
    }

    Ok(TrafficTraceIdString::new(key, value.to_owned()))
}
