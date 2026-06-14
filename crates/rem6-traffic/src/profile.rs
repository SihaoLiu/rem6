use crate::{
    controller::{
        TrafficController, TrafficControllerConfig, TrafficControllerSnapshot,
        TrafficControllerState, TrafficStateGenerator, TrafficStateGeneratorSnapshot,
        TrafficStateGeneratorSnapshotEntry,
    },
    dram::TrafficDramMode,
    TrafficGeneratorSummary, TrafficStateId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficGeneratorClass {
    Idle,
    Exit,
    Linear,
    Random,
    Strided,
    Dram,
    DramRotate,
    Nvm,
    Hybrid,
    Gups,
    Trace,
}

impl TrafficGeneratorClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Exit => "exit",
            Self::Linear => "linear",
            Self::Random => "random",
            Self::Strided => "strided",
            Self::Dram => "dram",
            Self::DramRotate => "dram_rotate",
            Self::Nvm => "nvm",
            Self::Hybrid => "hybrid",
            Self::Gups => "gups",
            Self::Trace => "trace",
        }
    }

    pub const fn stat_code(self) -> u64 {
        match self {
            Self::Idle => 0,
            Self::Exit => 1,
            Self::Linear => 2,
            Self::Random => 3,
            Self::Strided => 4,
            Self::Dram => 5,
            Self::DramRotate => 6,
            Self::Nvm => 7,
            Self::Hybrid => 8,
            Self::Gups => 9,
            Self::Trace => 10,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficMemoryProfile {
    NoMemory,
    FlatAddressRange,
    Dram,
    Nvm,
    Hybrid,
    GupsTable,
    TraceReplay,
}

impl TrafficMemoryProfile {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoMemory => "no_memory",
            Self::FlatAddressRange => "flat_address_range",
            Self::Dram => "dram",
            Self::Nvm => "nvm",
            Self::Hybrid => "hybrid",
            Self::GupsTable => "gups_table",
            Self::TraceReplay => "trace_replay",
        }
    }

    pub const fn stat_code(self) -> u64 {
        match self {
            Self::NoMemory => 0,
            Self::FlatAddressRange => 1,
            Self::Dram => 2,
            Self::Nvm => 3,
            Self::Hybrid => 4,
            Self::GupsTable => 5,
            Self::TraceReplay => 6,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficProfileSummary {
    generator_class: TrafficGeneratorClass,
    memory_profile: TrafficMemoryProfile,
    summary: TrafficGeneratorSummary,
}

impl TrafficProfileSummary {
    pub const fn new(
        generator_class: TrafficGeneratorClass,
        memory_profile: TrafficMemoryProfile,
        summary: TrafficGeneratorSummary,
    ) -> Self {
        Self {
            generator_class,
            memory_profile,
            summary,
        }
    }

    pub const fn generator_class(self) -> TrafficGeneratorClass {
        self.generator_class
    }

    pub const fn memory_profile(self) -> TrafficMemoryProfile {
        self.memory_profile
    }

    pub const fn summary(self) -> TrafficGeneratorSummary {
        self.summary
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficStateProfileSummary {
    state: TrafficStateId,
    profile: TrafficProfileSummary,
}

impl TrafficStateProfileSummary {
    pub const fn new(state: TrafficStateId, profile: TrafficProfileSummary) -> Self {
        Self { state, profile }
    }

    pub const fn state(self) -> TrafficStateId {
        self.state
    }

    pub const fn profile(&self) -> &TrafficProfileSummary {
        &self.profile
    }
}

impl TrafficControllerConfig {
    pub fn profile_summaries(&self) -> Vec<TrafficStateProfileSummary> {
        self.states()
            .iter()
            .map(TrafficControllerState::profile_summary)
            .collect()
    }
}

impl TrafficControllerState {
    pub fn profile_summary(&self) -> TrafficStateProfileSummary {
        TrafficStateProfileSummary::new(self.id(), self.generator().profile_summary())
    }
}

impl TrafficController {
    pub fn current_profile_summary(&self) -> Option<TrafficStateProfileSummary> {
        self.snapshot().current_profile_summary()
    }
}

impl TrafficControllerSnapshot {
    pub fn current_profile_summary(&self) -> Option<TrafficStateProfileSummary> {
        let current = self.machine().current_state()?;
        self.generators()
            .iter()
            .find(|entry| entry.id() == current)
            .map(TrafficStateGeneratorSnapshotEntry::profile_summary)
    }
}

impl TrafficStateGeneratorSnapshotEntry {
    pub fn profile_summary(&self) -> TrafficStateProfileSummary {
        TrafficStateProfileSummary::new(self.id(), self.generator().profile_summary())
    }
}

impl TrafficStateGenerator {
    pub fn profile_summary(&self) -> TrafficProfileSummary {
        match self {
            Self::Idle(generator) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Idle,
                TrafficMemoryProfile::NoMemory,
                generator.summary(),
            ),
            Self::Exit(_) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Exit,
                TrafficMemoryProfile::NoMemory,
                TrafficGeneratorSummary::default(),
            ),
            Self::Linear(generator) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Linear,
                TrafficMemoryProfile::FlatAddressRange,
                generator.summary(),
            ),
            Self::Random(generator) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Random,
                TrafficMemoryProfile::FlatAddressRange,
                generator.summary(),
            ),
            Self::Strided(generator) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Strided,
                TrafficMemoryProfile::FlatAddressRange,
                generator.summary(),
            ),
            Self::Dram(generator) => {
                let (generator_class, memory_profile) = dram_profile(generator.config().mode());
                TrafficProfileSummary::new(generator_class, memory_profile, generator.summary())
            }
            Self::Hybrid(generator) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Hybrid,
                TrafficMemoryProfile::Hybrid,
                generator.summary(),
            ),
            Self::Gups(generator) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Gups,
                TrafficMemoryProfile::GupsTable,
                generator.summary(),
            ),
            Self::Trace(generator) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Trace,
                TrafficMemoryProfile::TraceReplay,
                generator.summary(),
            ),
        }
    }
}

impl TrafficStateGeneratorSnapshot {
    pub fn profile_summary(&self) -> TrafficProfileSummary {
        match self {
            Self::Idle(_) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Idle,
                TrafficMemoryProfile::NoMemory,
                TrafficGeneratorSummary::default(),
            ),
            Self::Exit(_) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Exit,
                TrafficMemoryProfile::NoMemory,
                TrafficGeneratorSummary::default(),
            ),
            Self::Linear(snapshot) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Linear,
                TrafficMemoryProfile::FlatAddressRange,
                snapshot.summary(),
            ),
            Self::Random(snapshot) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Random,
                TrafficMemoryProfile::FlatAddressRange,
                snapshot.summary(),
            ),
            Self::Strided(snapshot) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Strided,
                TrafficMemoryProfile::FlatAddressRange,
                snapshot.summary(),
            ),
            Self::Dram(snapshot) => {
                let (generator_class, memory_profile) = dram_profile(snapshot.config().mode());
                TrafficProfileSummary::new(generator_class, memory_profile, snapshot.summary())
            }
            Self::Hybrid(snapshot) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Hybrid,
                TrafficMemoryProfile::Hybrid,
                snapshot.summary(),
            ),
            Self::Gups(snapshot) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Gups,
                TrafficMemoryProfile::GupsTable,
                snapshot.summary(),
            ),
            Self::Trace(snapshot) => TrafficProfileSummary::new(
                TrafficGeneratorClass::Trace,
                TrafficMemoryProfile::TraceReplay,
                snapshot.summary(),
            ),
        }
    }
}

fn dram_profile(mode: TrafficDramMode) -> (TrafficGeneratorClass, TrafficMemoryProfile) {
    match mode {
        TrafficDramMode::Dram => (TrafficGeneratorClass::Dram, TrafficMemoryProfile::Dram),
        TrafficDramMode::DramRotate => (
            TrafficGeneratorClass::DramRotate,
            TrafficMemoryProfile::Dram,
        ),
        TrafficDramMode::Nvm => (TrafficGeneratorClass::Nvm, TrafficMemoryProfile::Nvm),
    }
}
