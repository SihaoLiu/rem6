use std::str::FromStr;

use crate::{
    TrafficGeneratorError, TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec,
    TrafficTransition, TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTextConfig {
    graph: TrafficStateGraphConfig,
    states: Vec<TrafficTextState>,
}

impl TrafficTextConfig {
    pub fn parse(input: &str) -> Result<Self, TrafficGeneratorError> {
        let mut states = Vec::new();
        let mut graph_states = Vec::new();
        let mut transitions = Vec::new();
        let mut initial_state = None;

        for (index, raw_line) in input.lines().enumerate() {
            let line = index + 1;
            let content = raw_line
                .split_once('#')
                .map_or(raw_line, |(before, _)| before)
                .trim();
            if content.is_empty() {
                continue;
            }

            let tokens = content.split_whitespace().collect::<Vec<_>>();
            match tokens[0] {
                "STATE" => {
                    let state = parse_state(line, &tokens[1..])?;
                    graph_states.push(TrafficStateSpec::new(state.id(), state.duration()));
                    states.push(state);
                }
                "INIT" => {
                    let mut parser = LineParser::new(line, "INIT", &tokens[1..]);
                    let state = TrafficStateId::new(parser.next_u32("state")?);
                    parser.finish()?;
                    if initial_state.replace(state).is_some() {
                        return Err(TrafficGeneratorError::TrafficConfigDuplicateInitial { line });
                    }
                }
                "TRANSITION" => {
                    transitions.push(parse_transition(line, &tokens[1..])?);
                }
                keyword => {
                    return Err(TrafficGeneratorError::TrafficConfigUnknownKeyword {
                        line,
                        keyword: keyword.to_string(),
                    });
                }
            }
        }

        let initial_state =
            initial_state.ok_or(TrafficGeneratorError::TrafficConfigMissingInitial)?;
        let graph = TrafficStateGraphConfig::new(graph_states, initial_state, transitions)?;
        validate_dense_state_ids(&states)?;
        states.sort_by_key(|state| state.id());

        Ok(Self { graph, states })
    }

    pub const fn graph(&self) -> &TrafficStateGraphConfig {
        &self.graph
    }

    pub fn states(&self) -> &[TrafficTextState] {
        &self.states
    }

    pub fn state(&self, id: TrafficStateId) -> Option<&TrafficTextState> {
        self.states.iter().find(|state| state.id() == id)
    }
}

impl FromStr for TrafficTextConfig {
    type Err = TrafficGeneratorError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        Self::parse(input)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTextState {
    id: TrafficStateId,
    duration: u64,
    mode: TrafficTextStateMode,
}

impl TrafficTextState {
    pub const fn new(id: TrafficStateId, duration: u64, mode: TrafficTextStateMode) -> Self {
        Self { id, duration, mode }
    }

    pub const fn id(&self) -> TrafficStateId {
        self.id
    }

    pub const fn duration(&self) -> u64 {
        self.duration
    }

    pub const fn mode(&self) -> &TrafficTextStateMode {
        &self.mode
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTextStateMode {
    Trace {
        trace_file: String,
        addr_offset: u64,
    },
    Idle,
    Exit,
    Linear(TrafficTextMemoryParams),
    Random(TrafficTextMemoryParams),
    Strided(TrafficTextStridedParams),
    Dram(TrafficTextDramParams),
    DramRotate(TrafficTextDramParams),
    Nvm(TrafficTextDramParams),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTextMemoryParams {
    read_percent: u8,
    start_addr: u64,
    end_addr: u64,
    block_size: u64,
    min_period: u64,
    max_period: u64,
    data_limit: u64,
}

impl TrafficTextMemoryParams {
    pub const fn new(
        read_percent: u8,
        start_addr: u64,
        end_addr: u64,
        block_size: u64,
        min_period: u64,
        max_period: u64,
        data_limit: u64,
    ) -> Self {
        Self {
            read_percent,
            start_addr,
            end_addr,
            block_size,
            min_period,
            max_period,
            data_limit,
        }
    }

    pub const fn read_percent(self) -> u8 {
        self.read_percent
    }

    pub const fn start_addr(self) -> u64 {
        self.start_addr
    }

    pub const fn end_addr(self) -> u64 {
        self.end_addr
    }

    pub const fn block_size(self) -> u64 {
        self.block_size
    }

    pub const fn min_period(self) -> u64 {
        self.min_period
    }

    pub const fn max_period(self) -> u64 {
        self.max_period
    }

    pub const fn data_limit(self) -> u64 {
        self.data_limit
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTextStridedParams {
    memory: TrafficTextMemoryParams,
    offset: u64,
    superblock_size: u64,
    stride_size: u64,
}

impl TrafficTextStridedParams {
    pub const fn new(
        memory: TrafficTextMemoryParams,
        offset: u64,
        superblock_size: u64,
        stride_size: u64,
    ) -> Self {
        Self {
            memory,
            offset,
            superblock_size,
            stride_size,
        }
    }

    pub const fn memory(self) -> TrafficTextMemoryParams {
        self.memory
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn superblock_size(self) -> u64 {
        self.superblock_size
    }

    pub const fn stride_size(self) -> u64 {
        self.stride_size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTextDramParams {
    memory: TrafficTextMemoryParams,
    stride_size: u64,
    page_or_buffer_size: u64,
    bank_count: u32,
    bank_utilization: u32,
    addr_mapping: u32,
    rank_count: u32,
}

impl TrafficTextDramParams {
    pub const fn new(
        memory: TrafficTextMemoryParams,
        stride_size: u64,
        page_or_buffer_size: u64,
        bank_count: u32,
        bank_utilization: u32,
        addr_mapping: u32,
        rank_count: u32,
    ) -> Self {
        Self {
            memory,
            stride_size,
            page_or_buffer_size,
            bank_count,
            bank_utilization,
            addr_mapping,
            rank_count,
        }
    }

    pub const fn memory(self) -> TrafficTextMemoryParams {
        self.memory
    }

    pub const fn stride_size(self) -> u64 {
        self.stride_size
    }

    pub const fn page_or_buffer_size(self) -> u64 {
        self.page_or_buffer_size
    }

    pub const fn bank_count(self) -> u32 {
        self.bank_count
    }

    pub const fn bank_utilization(self) -> u32 {
        self.bank_utilization
    }

    pub const fn addr_mapping(self) -> u32 {
        self.addr_mapping
    }

    pub const fn rank_count(self) -> u32 {
        self.rank_count
    }
}

fn parse_state(line: usize, tokens: &[&str]) -> Result<TrafficTextState, TrafficGeneratorError> {
    let mut parser = LineParser::new(line, "STATE", tokens);
    let id = TrafficStateId::new(parser.next_u32("state")?);
    let duration = parser.next_u64("duration")?;
    let mode_token = parser.next_token("mode")?;
    let mode = match mode_token {
        "TRACE" => {
            let trace_file = parser.next_token("trace_file")?.to_string();
            let addr_offset = parser.next_u64("addr_offset")?;
            TrafficTextStateMode::Trace {
                trace_file,
                addr_offset,
            }
        }
        "IDLE" => TrafficTextStateMode::Idle,
        "EXIT" => TrafficTextStateMode::Exit,
        "LINEAR" => TrafficTextStateMode::Linear(parse_memory_params(&mut parser)?),
        "RANDOM" => TrafficTextStateMode::Random(parse_memory_params(&mut parser)?),
        "STRIDED" => TrafficTextStateMode::Strided(parse_strided_params(&mut parser)?),
        "DRAM" => TrafficTextStateMode::Dram(parse_dram_params(&mut parser)?),
        "DRAM_ROTATE" => TrafficTextStateMode::DramRotate(parse_dram_params(&mut parser)?),
        "NVM" => TrafficTextStateMode::Nvm(parse_dram_params(&mut parser)?),
        mode => {
            return Err(TrafficGeneratorError::TrafficConfigUnknownStateMode {
                line,
                mode: mode.to_string(),
            });
        }
    };
    parser.finish()?;

    Ok(TrafficTextState::new(id, duration, mode))
}

fn parse_transition(
    line: usize,
    tokens: &[&str],
) -> Result<TrafficTransition, TrafficGeneratorError> {
    let mut parser = LineParser::new(line, "TRANSITION", tokens);
    let from = TrafficStateId::new(parser.next_u32("from")?);
    let to = TrafficStateId::new(parser.next_u32("to")?);
    let probability = parser.next_probability("probability")?;
    parser.finish()?;

    Ok(TrafficTransition::new(from, to, probability))
}

fn validate_dense_state_ids(states: &[TrafficTextState]) -> Result<(), TrafficGeneratorError> {
    let mut ids = states.iter().map(TrafficTextState::id).collect::<Vec<_>>();
    ids.sort();

    for (expected, actual) in ids.into_iter().enumerate() {
        let expected = u32::try_from(expected).unwrap_or(u32::MAX);
        if actual.get() != expected {
            return Err(TrafficGeneratorError::TrafficConfigSparseStateIds { expected, actual });
        }
    }

    Ok(())
}

fn parse_memory_params(
    parser: &mut LineParser<'_>,
) -> Result<TrafficTextMemoryParams, TrafficGeneratorError> {
    let read_percent = parser.next_read_percent("read_percent")?;
    let start_addr = parser.next_u64("start_addr")?;
    let end_addr = parser.next_u64("end_addr")?;
    let block_size = parser.next_u64("block_size")?;
    let min_period = parser.next_u64("min_period")?;
    let max_period = parser.next_u64("max_period")?;
    if min_period > max_period {
        return Err(TrafficGeneratorError::InvertedPeriod {
            min_period,
            max_period,
        });
    }
    let data_limit = parser.next_u64("data_limit")?;

    Ok(TrafficTextMemoryParams::new(
        read_percent,
        start_addr,
        end_addr,
        block_size,
        min_period,
        max_period,
        data_limit,
    ))
}

fn parse_strided_params(
    parser: &mut LineParser<'_>,
) -> Result<TrafficTextStridedParams, TrafficGeneratorError> {
    let read_percent = parser.next_read_percent("read_percent")?;
    let start_addr = parser.next_u64("start_addr")?;
    let end_addr = parser.next_u64("end_addr")?;
    let offset = parser.next_u64("offset")?;
    let block_size = parser.next_u64("block_size")?;
    let superblock_size = parser.next_u64("superblock_size")?;
    let stride_size = parser.next_u64("stride_size")?;
    let min_period = parser.next_u64("min_period")?;
    let max_period = parser.next_u64("max_period")?;
    if min_period > max_period {
        return Err(TrafficGeneratorError::InvertedPeriod {
            min_period,
            max_period,
        });
    }
    let data_limit = parser.next_u64("data_limit")?;

    let memory = TrafficTextMemoryParams::new(
        read_percent,
        start_addr,
        end_addr,
        block_size,
        min_period,
        max_period,
        data_limit,
    );
    Ok(TrafficTextStridedParams::new(
        memory,
        offset,
        superblock_size,
        stride_size,
    ))
}

fn parse_dram_params(
    parser: &mut LineParser<'_>,
) -> Result<TrafficTextDramParams, TrafficGeneratorError> {
    let memory = parse_memory_params(parser)?;
    let stride_size = parser.next_u64("stride_size")?;
    let page_or_buffer_size = parser.next_u64("page_or_buffer_size")?;
    let bank_count = parser.next_u32("bank_count")?;
    let bank_utilization = parser.next_u32("bank_utilization")?;
    let addr_mapping = parser.next_u32("addr_mapping")?;
    let rank_count = parser.next_u32("rank_count")?;

    Ok(TrafficTextDramParams::new(
        memory,
        stride_size,
        page_or_buffer_size,
        bank_count,
        bank_utilization,
        addr_mapping,
        rank_count,
    ))
}

struct LineParser<'a> {
    line: usize,
    record: &'static str,
    tokens: &'a [&'a str],
    cursor: usize,
}

impl<'a> LineParser<'a> {
    const fn new(line: usize, record: &'static str, tokens: &'a [&'a str]) -> Self {
        Self {
            line,
            record,
            tokens,
            cursor: 0,
        }
    }

    fn next_token(&mut self, field: &'static str) -> Result<&'a str, TrafficGeneratorError> {
        let token = self.tokens.get(self.cursor).copied().ok_or(
            TrafficGeneratorError::TrafficConfigMissingToken {
                line: self.line,
                record: self.record,
                field,
            },
        )?;
        self.cursor += 1;
        Ok(token)
    }

    fn next_u32(&mut self, field: &'static str) -> Result<u32, TrafficGeneratorError> {
        let token = self.next_token(field)?;
        token
            .parse::<u32>()
            .map_err(|_| TrafficGeneratorError::TrafficConfigInvalidNumber {
                line: self.line,
                field,
                token: token.to_string(),
            })
    }

    fn next_u64(&mut self, field: &'static str) -> Result<u64, TrafficGeneratorError> {
        let token = self.next_token(field)?;
        token
            .parse::<u64>()
            .map_err(|_| TrafficGeneratorError::TrafficConfigInvalidNumber {
                line: self.line,
                field,
                token: token.to_string(),
            })
    }

    fn next_read_percent(&mut self, field: &'static str) -> Result<u8, TrafficGeneratorError> {
        let value = self.next_u32(field)?;
        if value > 100 {
            return Err(TrafficGeneratorError::TrafficConfigReadPercentOutOfRange {
                line: self.line,
                read_percent: value,
            });
        }

        Ok(value as u8)
    }

    fn next_probability(
        &mut self,
        field: &'static str,
    ) -> Result<TrafficTransitionProbability, TrafficGeneratorError> {
        let token = self.next_token(field)?;
        parse_probability(self.line, field, token)
    }

    fn finish(&self) -> Result<(), TrafficGeneratorError> {
        if let Some(token) = self.tokens.get(self.cursor) {
            return Err(TrafficGeneratorError::TrafficConfigUnexpectedToken {
                line: self.line,
                record: self.record,
                token: (*token).to_string(),
            });
        }

        Ok(())
    }
}

fn parse_probability(
    line: usize,
    field: &'static str,
    token: &str,
) -> Result<TrafficTransitionProbability, TrafficGeneratorError> {
    let (whole, fractional) = token.split_once('.').map_or((token, ""), |parts| parts);
    if whole.is_empty() && fractional.is_empty() {
        return Err(TrafficGeneratorError::TrafficConfigInvalidNumber {
            line,
            field,
            token: token.to_string(),
        });
    }
    let whole = if whole.is_empty() {
        0
    } else {
        whole
            .parse::<u32>()
            .map_err(|_| TrafficGeneratorError::TrafficConfigInvalidNumber {
                line,
                field,
                token: token.to_string(),
            })?
    };

    let fractional = fractional.trim_end_matches('0');
    if fractional.len() > 6 {
        return Err(TrafficGeneratorError::TrafficConfigProbabilityTooPrecise {
            line,
            token: token.to_string(),
            scale: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
        });
    }
    if !fractional
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        return Err(TrafficGeneratorError::TrafficConfigInvalidNumber {
            line,
            field,
            token: token.to_string(),
        });
    }

    let mut fractional_micros = if fractional.is_empty() {
        0
    } else {
        fractional.parse::<u32>().map_err(|_| {
            TrafficGeneratorError::TrafficConfigInvalidNumber {
                line,
                field,
                token: token.to_string(),
            }
        })?
    };
    for _ in fractional.len()..6 {
        fractional_micros *= 10;
    }

    let micros = u128::from(whole) * u128::from(TRAFFIC_TRANSITION_PROBABILITY_SCALE)
        + u128::from(fractional_micros);
    if micros > u128::from(u32::MAX) {
        return Err(
            TrafficGeneratorError::TrafficTransitionProbabilityOutOfRange {
                probability: u32::MAX,
                scale: TRAFFIC_TRANSITION_PROBABILITY_SCALE,
            },
        );
    }

    TrafficTransitionProbability::from_micros(micros as u32)
}
