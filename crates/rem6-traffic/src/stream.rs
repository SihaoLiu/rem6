use rem6_memory::MemoryRequest;

use crate::{common::TrafficRequestEvent, common::TrafficRng, TrafficGeneratorError};

const DEFAULT_RNG_STATE: u64 = 0x9e37_79b9_7f4a_7c15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficStreamIdMode {
    Fixed,
    Random,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficStreamConfig {
    mode: TrafficStreamIdMode,
    stream_ids: Vec<u32>,
    substream_ids: Vec<u32>,
    rng_state: u64,
}

impl TrafficStreamConfig {
    pub fn new(
        mode: TrafficStreamIdMode,
        stream_ids: Vec<u32>,
        substream_ids: Vec<u32>,
    ) -> Result<Self, TrafficGeneratorError> {
        validate_stream_ids(mode, &stream_ids, &substream_ids)?;
        Ok(Self {
            mode,
            stream_ids,
            substream_ids,
            rng_state: DEFAULT_RNG_STATE,
        })
    }

    pub fn fixed(stream_id: u32) -> Self {
        Self {
            mode: TrafficStreamIdMode::Fixed,
            stream_ids: vec![stream_id],
            substream_ids: Vec::new(),
            rng_state: DEFAULT_RNG_STATE,
        }
    }

    pub fn with_fixed_substream_id(mut self, substream_id: u32) -> Self {
        self.substream_ids = vec![substream_id];
        self
    }

    pub const fn with_rng_state(mut self, rng_state: u64) -> Self {
        self.rng_state = rng_state;
        self
    }

    pub const fn mode(&self) -> TrafficStreamIdMode {
        self.mode
    }

    pub fn stream_ids(&self) -> &[u32] {
        &self.stream_ids
    }

    pub fn substream_ids(&self) -> &[u32] {
        &self.substream_ids
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng_state
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficStreamPicker {
    config: TrafficStreamConfig,
    rng: TrafficRng,
}

impl TrafficStreamPicker {
    pub(crate) fn new(config: TrafficStreamConfig) -> Self {
        Self {
            rng: TrafficRng::new(config.rng_state()),
            config,
        }
    }

    pub(crate) fn with_rng_state(config: TrafficStreamConfig, rng_state: u64) -> Self {
        Self {
            config,
            rng: TrafficRng::new(rng_state),
        }
    }

    pub(crate) fn next_ids(&mut self) -> TrafficStreamIds {
        match self.config.mode() {
            TrafficStreamIdMode::Fixed => TrafficStreamIds::new(
                Some(self.config.stream_ids()[0]),
                self.config.substream_ids().first().copied(),
            ),
            TrafficStreamIdMode::Random => {
                let stream_id = random_pick(&mut self.rng, self.config.stream_ids());
                let substream_id = if self.config.substream_ids().is_empty() {
                    None
                } else {
                    Some(random_pick(&mut self.rng, self.config.substream_ids()))
                };
                TrafficStreamIds::new(Some(stream_id), substream_id)
            }
        }
    }

    pub(crate) const fn rng_state(&self) -> u64 {
        self.rng.state()
    }

    pub(crate) fn config(&self) -> &TrafficStreamConfig {
        &self.config
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TrafficStreamIds {
    stream_id: Option<u32>,
    substream_id: Option<u32>,
}

impl TrafficStreamIds {
    pub(crate) const fn new(stream_id: Option<u32>, substream_id: Option<u32>) -> Self {
        Self {
            stream_id,
            substream_id,
        }
    }

    pub(crate) const fn stream_id(self) -> Option<u32> {
        self.stream_id
    }

    pub(crate) const fn substream_id(self) -> Option<u32> {
        self.substream_id
    }
}

fn validate_stream_ids(
    mode: TrafficStreamIdMode,
    stream_ids: &[u32],
    substream_ids: &[u32],
) -> Result<(), TrafficGeneratorError> {
    if stream_ids.is_empty() {
        return Err(TrafficGeneratorError::TrafficStreamMissingIds);
    }
    if mode == TrafficStreamIdMode::Fixed && (stream_ids.len() != 1 || substream_ids.len() > 1) {
        return Err(TrafficGeneratorError::TrafficStreamInvalidFixedIds {
            stream_ids: stream_ids.len(),
            substream_ids: substream_ids.len(),
        });
    }
    Ok(())
}

fn random_pick(rng: &mut TrafficRng, values: &[u32]) -> u32 {
    let index = rng.next_inclusive(0, values.len() as u64 - 1);
    values[index as usize]
}

pub(crate) fn apply_stream_ids_to_event(
    event: TrafficRequestEvent,
    stream_ids: TrafficStreamIds,
) -> Result<TrafficRequestEvent, TrafficGeneratorError> {
    event.try_map_request(|request| apply_stream_ids_to_request(request, stream_ids))
}

pub(crate) fn apply_stream_ids_to_request(
    mut request: MemoryRequest,
    stream_ids: TrafficStreamIds,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    if let Some(stream_id) = stream_ids.stream_id() {
        request = request.with_stream_id(stream_id);
    }
    if let Some(substream_id) = stream_ids.substream_id() {
        request = request.with_substream_id(substream_id)?;
    }
    Ok(request)
}
