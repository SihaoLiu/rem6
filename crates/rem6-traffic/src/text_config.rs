use std::str::FromStr;

use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};

use crate::{
    DramTrafficGenerator, GupsTrafficGenerator, LinearTrafficGenerator, RandomTrafficGenerator,
    StridedTrafficGenerator, TrafficControllerConfig, TrafficControllerState,
    TrafficDramAddressMapping, TrafficDramConfig, TrafficDramMode, TrafficExitConfig,
    TrafficExitGenerator, TrafficGeneratorError, TrafficGupsConfig, TrafficHybridConfig,
    TrafficHybridSideConfig, TrafficIdleConfig, TrafficIdleGenerator, TrafficLinearConfig,
    TrafficRandomConfig, TrafficStateGenerator, TrafficStateGraphConfig, TrafficStateId,
    TrafficStateSpec, TrafficStreamConfig, TrafficStridedConfig, TrafficTrace, TrafficTraceConfig,
    TrafficTraceGenerator, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
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

    pub fn to_controller_config(
        &self,
        options: TrafficTextBindingOptions,
    ) -> Result<TrafficControllerConfig, TrafficGeneratorError> {
        let states = self
            .states
            .iter()
            .map(|state| state.to_controller_state(options.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let config = TrafficControllerConfig::new(self.graph.clone(), states)?;
        Ok(options.apply_stream(config))
    }

    pub fn to_controller_config_with_trace_resolver<F>(
        &self,
        options: TrafficTextBindingOptions,
        expected_tick_frequency: u64,
        mut resolve_trace: F,
    ) -> Result<TrafficControllerConfig, TrafficGeneratorError>
    where
        F: FnMut(&str) -> Result<Vec<u8>, TrafficGeneratorError>,
    {
        let states = self
            .states
            .iter()
            .map(|state| {
                state.to_controller_state_with_trace_resolver(
                    options.clone(),
                    expected_tick_frequency,
                    &mut resolve_trace,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let config = TrafficControllerConfig::new(self.graph.clone(), states)?;
        Ok(options.apply_stream(config))
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

    fn to_controller_state(
        &self,
        options: TrafficTextBindingOptions,
    ) -> Result<TrafficControllerState, TrafficGeneratorError> {
        let generator = match &self.mode {
            TrafficTextStateMode::Idle => TrafficStateGenerator::Idle(TrafficIdleGenerator::new(
                TrafficIdleConfig::new(self.duration),
            )),
            TrafficTextStateMode::Exit => TrafficStateGenerator::Exit(TrafficExitGenerator::new(
                TrafficExitConfig::new(self.duration),
            )),
            TrafficTextStateMode::Linear(params) => TrafficStateGenerator::Linear(
                LinearTrafficGenerator::new(linear_config_from_text(*params, options)?),
            ),
            TrafficTextStateMode::Random(params) => TrafficStateGenerator::Random(
                RandomTrafficGenerator::new(random_config_from_text(*params, options)?),
            ),
            TrafficTextStateMode::Strided(params) => TrafficStateGenerator::Strided(
                StridedTrafficGenerator::new(strided_config_from_text(*params, options)?),
            ),
            TrafficTextStateMode::Dram(params) => {
                TrafficStateGenerator::Dram(DramTrafficGenerator::new(dram_config_from_text(
                    TrafficDramMode::Dram,
                    *params,
                    options,
                )?))
            }
            TrafficTextStateMode::DramRotate(params) => {
                TrafficStateGenerator::Dram(DramTrafficGenerator::new(dram_config_from_text(
                    TrafficDramMode::DramRotate,
                    *params,
                    options,
                )?))
            }
            TrafficTextStateMode::Nvm(params) => {
                TrafficStateGenerator::Dram(DramTrafficGenerator::new(dram_config_from_text(
                    TrafficDramMode::Nvm,
                    *params,
                    options,
                )?))
            }
            TrafficTextStateMode::Hybrid(params) => TrafficStateGenerator::Hybrid(
                crate::HybridTrafficGenerator::new(hybrid_config_from_text(*params, options)?),
            ),
            TrafficTextStateMode::Gups(params) => TrafficStateGenerator::Gups(
                GupsTrafficGenerator::new(gups_config_from_text(*params, options)?),
            ),
            TrafficTextStateMode::Trace { .. } => {
                return Err(TrafficGeneratorError::TrafficConfigUnsupportedStateMode {
                    state: self.id,
                    mode: self.mode.name(),
                });
            }
        };

        Ok(TrafficControllerState::new(self.id, generator))
    }

    fn to_controller_state_with_trace_resolver<F>(
        &self,
        options: TrafficTextBindingOptions,
        expected_tick_frequency: u64,
        resolve_trace: &mut F,
    ) -> Result<TrafficControllerState, TrafficGeneratorError>
    where
        F: FnMut(&str) -> Result<Vec<u8>, TrafficGeneratorError>,
    {
        let TrafficTextStateMode::Trace {
            trace_file,
            addr_offset,
        } = &self.mode
        else {
            return self.to_controller_state(options);
        };

        let generator =
            TrafficStateGenerator::Trace(TrafficTraceGenerator::new(trace_config_from_text(
                trace_file,
                *addr_offset,
                self.duration,
                options,
                expected_tick_frequency,
                resolve_trace,
            )?));

        Ok(TrafficControllerState::new(self.id, generator))
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
    Hybrid(TrafficTextHybridParams),
    Gups(TrafficTextGupsParams),
}

impl TrafficTextStateMode {
    const fn name(&self) -> &'static str {
        match self {
            Self::Trace { .. } => "TRACE",
            Self::Idle => "IDLE",
            Self::Exit => "EXIT",
            Self::Linear(_) => "LINEAR",
            Self::Random(_) => "RANDOM",
            Self::Strided(_) => "STRIDED",
            Self::Dram(_) => "DRAM",
            Self::DramRotate(_) => "DRAM_ROTATE",
            Self::Nvm(_) => "NVM",
            Self::Hybrid(_) => "HYBRID",
            Self::Gups(_) => "GUPS",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficTextBindingOptions {
    agent: AgentId,
    line_layout: CacheLineLayout,
    elastic_requests: bool,
    stream: Option<TrafficStreamConfig>,
}

impl TrafficTextBindingOptions {
    pub const fn new(agent: AgentId, line_layout: CacheLineLayout) -> Self {
        Self {
            agent,
            line_layout,
            elastic_requests: false,
            stream: None,
        }
    }

    pub const fn with_elastic_requests(mut self, elastic_requests: bool) -> Self {
        self.elastic_requests = elastic_requests;
        self
    }

    pub fn with_stream(mut self, stream: TrafficStreamConfig) -> Self {
        self.stream = Some(stream);
        self
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub const fn elastic_requests(&self) -> bool {
        self.elastic_requests
    }

    pub fn stream(&self) -> Option<&TrafficStreamConfig> {
        self.stream.as_ref()
    }

    fn apply_stream(self, config: TrafficControllerConfig) -> TrafficControllerConfig {
        match self.stream {
            Some(stream) => config.with_stream(stream),
            None => config,
        }
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTextHybridSideParams {
    start_addr: u64,
    end_addr: u64,
    block_size: u64,
    num_seq_packets: u32,
    page_or_buffer_size: u64,
    bank_count: u32,
    bank_utilization: u32,
    rank_count: u32,
}

impl TrafficTextHybridSideParams {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        start_addr: u64,
        end_addr: u64,
        block_size: u64,
        num_seq_packets: u32,
        page_or_buffer_size: u64,
        bank_count: u32,
        bank_utilization: u32,
        rank_count: u32,
    ) -> Self {
        Self {
            start_addr,
            end_addr,
            block_size,
            num_seq_packets,
            page_or_buffer_size,
            bank_count,
            bank_utilization,
            rank_count,
        }
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

    pub const fn num_seq_packets(self) -> u32 {
        self.num_seq_packets
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

    pub const fn rank_count(self) -> u32 {
        self.rank_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTextHybridParams {
    read_percent: u8,
    min_period: u64,
    max_period: u64,
    data_limit: u64,
    dram: TrafficTextHybridSideParams,
    nvm: TrafficTextHybridSideParams,
    addr_mapping: u32,
    nvm_percent: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTextGupsParams {
    start_addr: u64,
    mem_size: u64,
    update_limit: u64,
}

impl TrafficTextGupsParams {
    pub const fn new(start_addr: u64, mem_size: u64, update_limit: u64) -> Self {
        Self {
            start_addr,
            mem_size,
            update_limit,
        }
    }

    pub const fn start_addr(self) -> u64 {
        self.start_addr
    }

    pub const fn mem_size(self) -> u64 {
        self.mem_size
    }

    pub const fn update_limit(self) -> u64 {
        self.update_limit
    }
}

impl TrafficTextHybridParams {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        read_percent: u8,
        min_period: u64,
        max_period: u64,
        data_limit: u64,
        dram: TrafficTextHybridSideParams,
        nvm: TrafficTextHybridSideParams,
        addr_mapping: u32,
        nvm_percent: u8,
    ) -> Self {
        Self {
            read_percent,
            min_period,
            max_period,
            data_limit,
            dram,
            nvm,
            addr_mapping,
            nvm_percent,
        }
    }

    pub const fn read_percent(self) -> u8 {
        self.read_percent
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

    pub const fn dram(self) -> TrafficTextHybridSideParams {
        self.dram
    }

    pub const fn nvm(self) -> TrafficTextHybridSideParams {
        self.nvm
    }

    pub const fn addr_mapping(self) -> u32 {
        self.addr_mapping
    }

    pub const fn nvm_percent(self) -> u8 {
        self.nvm_percent
    }
}

fn linear_config_from_text(
    params: TrafficTextMemoryParams,
    options: TrafficTextBindingOptions,
) -> Result<TrafficLinearConfig, TrafficGeneratorError> {
    let block_size = AccessSize::new(params.block_size())?;
    Ok(TrafficLinearConfig::new(
        options.agent(),
        options.line_layout(),
        Address::new(params.start_addr()),
        Address::new(params.end_addr()),
        block_size,
    )?
    .with_period(params.min_period(), params.max_period())?
    .with_read_percent(params.read_percent())?
    .with_data_limit(params.data_limit())?
    .with_elastic_requests(options.elastic_requests()))
}

fn random_config_from_text(
    params: TrafficTextMemoryParams,
    options: TrafficTextBindingOptions,
) -> Result<TrafficRandomConfig, TrafficGeneratorError> {
    let block_size = AccessSize::new(params.block_size())?;
    Ok(TrafficRandomConfig::new(
        options.agent(),
        options.line_layout(),
        Address::new(params.start_addr()),
        Address::new(params.end_addr()),
        block_size,
    )?
    .with_period(params.min_period(), params.max_period())?
    .with_read_percent(params.read_percent())?
    .with_data_limit(params.data_limit())?
    .with_elastic_requests(options.elastic_requests()))
}

fn strided_config_from_text(
    params: TrafficTextStridedParams,
    options: TrafficTextBindingOptions,
) -> Result<TrafficStridedConfig, TrafficGeneratorError> {
    let memory = params.memory();
    let block_size = AccessSize::new(memory.block_size())?;
    Ok(TrafficStridedConfig::new(
        options.agent(),
        options.line_layout(),
        Address::new(memory.start_addr()),
        Address::new(memory.end_addr()),
        params.offset(),
        block_size,
        params.superblock_size(),
        params.stride_size(),
    )?
    .with_period(memory.min_period(), memory.max_period())?
    .with_read_percent(memory.read_percent())?
    .with_data_limit(memory.data_limit())?
    .with_elastic_requests(options.elastic_requests()))
}

fn dram_config_from_text(
    mode: TrafficDramMode,
    params: TrafficTextDramParams,
    options: TrafficTextBindingOptions,
) -> Result<TrafficDramConfig, TrafficGeneratorError> {
    let memory = params.memory();
    let block_size = AccessSize::new(memory.block_size())?;
    let num_seq_packets = if params.stride_size() > memory.block_size() {
        params.stride_size().div_ceil(memory.block_size())
    } else {
        1
    };
    let num_seq_packets =
        u32::try_from(num_seq_packets).map_err(|_| TrafficGeneratorError::CounterOverflow {
            counter: "dram.num_seq_packets",
            value: u64::MAX,
            increment: 1,
        })?;

    Ok(TrafficDramConfig::new(
        options.agent(),
        options.line_layout(),
        mode,
        Address::new(memory.start_addr()),
        Address::new(memory.end_addr()),
        block_size,
        params.page_or_buffer_size(),
        params.bank_count(),
        params.bank_utilization(),
        TrafficDramAddressMapping::from_gem5_code(params.addr_mapping())?,
        params.rank_count(),
        num_seq_packets,
    )?
    .with_period(memory.min_period(), memory.max_period())?
    .with_read_percent(memory.read_percent())?
    .with_data_limit(memory.data_limit())?
    .with_elastic_requests(options.elastic_requests()))
}

fn hybrid_config_from_text(
    params: TrafficTextHybridParams,
    options: TrafficTextBindingOptions,
) -> Result<TrafficHybridConfig, TrafficGeneratorError> {
    Ok(TrafficHybridConfig::new(
        options.agent(),
        options.line_layout(),
        hybrid_side_config_from_text(params.dram())?,
        hybrid_side_config_from_text(params.nvm())?,
        TrafficDramAddressMapping::from_gem5_code(params.addr_mapping())?,
    )?
    .with_period(params.min_period(), params.max_period())?
    .with_read_percent(params.read_percent())?
    .with_nvm_percent(params.nvm_percent())?
    .with_data_limit(params.data_limit())?
    .with_elastic_requests(options.elastic_requests()))
}

fn gups_config_from_text(
    params: TrafficTextGupsParams,
    options: TrafficTextBindingOptions,
) -> Result<TrafficGupsConfig, TrafficGeneratorError> {
    TrafficGupsConfig::new(
        options.agent(),
        options.line_layout(),
        Address::new(params.start_addr()),
        params.mem_size(),
    )?
    .with_update_limit(params.update_limit())
}

fn hybrid_side_config_from_text(
    params: TrafficTextHybridSideParams,
) -> Result<TrafficHybridSideConfig, TrafficGeneratorError> {
    let block_size = AccessSize::new(params.block_size())?;
    TrafficHybridSideConfig::new(
        Address::new(params.start_addr()),
        Address::new(params.end_addr()),
        block_size,
        params.page_or_buffer_size(),
        params.bank_count(),
        params.bank_utilization(),
        params.rank_count(),
        params.num_seq_packets(),
    )
}

fn trace_config_from_text<F>(
    trace_file: &str,
    addr_offset: u64,
    duration: u64,
    options: TrafficTextBindingOptions,
    expected_tick_frequency: u64,
    resolve_trace: &mut F,
) -> Result<TrafficTraceConfig, TrafficGeneratorError>
where
    F: FnMut(&str) -> Result<Vec<u8>, TrafficGeneratorError>,
{
    let bytes = resolve_trace(trace_file)?;
    let trace = TrafficTrace::from_gem5_packet_trace(&bytes, expected_tick_frequency)?;
    Ok(
        TrafficTraceConfig::new(options.agent(), options.line_layout(), duration, trace)?
            .with_addr_offset(addr_offset)?
            .with_elastic(options.elastic_requests()),
    )
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
        "HYBRID" => TrafficTextStateMode::Hybrid(parse_hybrid_params(&mut parser)?),
        "GUPS" => TrafficTextStateMode::Gups(parse_gups_params(&mut parser)?),
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

fn parse_hybrid_params(
    parser: &mut LineParser<'_>,
) -> Result<TrafficTextHybridParams, TrafficGeneratorError> {
    let read_percent = parser.next_read_percent("read_percent")?;
    let dram_start_addr = parser.next_u64("dram_start_addr")?;
    let dram_end_addr = parser.next_u64("dram_end_addr")?;
    let dram_block_size = parser.next_u64("dram_block_size")?;
    let nvm_start_addr = parser.next_u64("nvm_start_addr")?;
    let nvm_end_addr = parser.next_u64("nvm_end_addr")?;
    let nvm_block_size = parser.next_u64("nvm_block_size")?;
    let min_period = parser.next_u64("min_period")?;
    let max_period = parser.next_u64("max_period")?;
    if min_period > max_period {
        return Err(TrafficGeneratorError::InvertedPeriod {
            min_period,
            max_period,
        });
    }
    let data_limit = parser.next_u64("data_limit")?;
    let dram_num_seq_packets = parser.next_u32("dram_num_seq_packets")?;
    let dram_page_size = parser.next_u64("dram_page_size")?;
    let dram_bank_count = parser.next_u32("dram_bank_count")?;
    let dram_bank_utilization = parser.next_u32("dram_bank_utilization")?;
    let nvm_num_seq_packets = parser.next_u32("nvm_num_seq_packets")?;
    let nvm_buffer_size = parser.next_u64("nvm_buffer_size")?;
    let nvm_bank_count = parser.next_u32("nvm_bank_count")?;
    let nvm_bank_utilization = parser.next_u32("nvm_bank_utilization")?;
    let addr_mapping = parser.next_u32("addr_mapping")?;
    let dram_rank_count = parser.next_u32("dram_rank_count")?;
    let nvm_rank_count = parser.next_u32("nvm_rank_count")?;
    let nvm_percent = parser.next_read_percent("nvm_percent")?;

    Ok(TrafficTextHybridParams::new(
        read_percent,
        min_period,
        max_period,
        data_limit,
        TrafficTextHybridSideParams::new(
            dram_start_addr,
            dram_end_addr,
            dram_block_size,
            dram_num_seq_packets,
            dram_page_size,
            dram_bank_count,
            dram_bank_utilization,
            dram_rank_count,
        ),
        TrafficTextHybridSideParams::new(
            nvm_start_addr,
            nvm_end_addr,
            nvm_block_size,
            nvm_num_seq_packets,
            nvm_buffer_size,
            nvm_bank_count,
            nvm_bank_utilization,
            nvm_rank_count,
        ),
        addr_mapping,
        nvm_percent,
    ))
}

fn parse_gups_params(
    parser: &mut LineParser<'_>,
) -> Result<TrafficTextGupsParams, TrafficGeneratorError> {
    let start_addr = parser.next_u64("start_addr")?;
    let mem_size = parser.next_u64("mem_size")?;
    let update_limit = parser.next_u64("update_limit")?;

    Ok(TrafficTextGupsParams::new(
        start_addr,
        mem_size,
        update_limit,
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
