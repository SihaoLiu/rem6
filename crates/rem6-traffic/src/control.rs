use crate::{
    common::{TrafficGeneratorSummary, TrafficRequestEvent},
    TrafficGeneratorError,
};

const EXIT_REASON: &str = "traffic generator exit state entered";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficIdleConfig {
    duration: u64,
}

impl TrafficIdleConfig {
    pub const fn new(duration: u64) -> Self {
        Self { duration }
    }

    pub const fn duration(self) -> u64 {
        self.duration
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficIdleSnapshot {
    config: TrafficIdleConfig,
    entered: bool,
}

impl TrafficIdleSnapshot {
    pub const fn new(config: TrafficIdleConfig, entered: bool) -> Self {
        Self { config, entered }
    }

    pub const fn config(self) -> TrafficIdleConfig {
        self.config
    }

    pub const fn entered(self) -> bool {
        self.entered
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficIdleGenerator {
    config: TrafficIdleConfig,
    entered: bool,
    summary: TrafficGeneratorSummary,
}

impl TrafficIdleGenerator {
    pub fn new(config: TrafficIdleConfig) -> Self {
        Self {
            config,
            entered: false,
            summary: TrafficGeneratorSummary::default(),
        }
    }

    pub fn restore(snapshot: TrafficIdleSnapshot) -> Self {
        Self {
            config: snapshot.config(),
            entered: snapshot.entered(),
            summary: TrafficGeneratorSummary::default(),
        }
    }

    pub const fn config(&self) -> TrafficIdleConfig {
        self.config
    }

    pub const fn entered(&self) -> bool {
        self.entered
    }

    pub const fn summary(&self) -> TrafficGeneratorSummary {
        self.summary
    }

    pub fn enter(&mut self) {
        self.entered = true;
    }

    pub const fn schedule_tick(
        &self,
        _tick: u64,
        _retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        Ok(u64::MAX)
    }

    pub const fn next_request(
        &mut self,
        _tick: u64,
        _retry_delay: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        Ok(None)
    }

    pub const fn snapshot(&self) -> TrafficIdleSnapshot {
        TrafficIdleSnapshot::new(self.config, self.entered)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficExitConfig {
    duration: u64,
}

impl TrafficExitConfig {
    pub const fn new(duration: u64) -> Self {
        Self { duration }
    }

    pub const fn duration(self) -> u64 {
        self.duration
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficExitEvent {
    tick: u64,
    duration: u64,
}

impl TrafficExitEvent {
    const fn new(tick: u64, duration: u64) -> Self {
        Self { tick, duration }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn duration(self) -> u64 {
        self.duration
    }

    pub const fn reason(self) -> &'static str {
        EXIT_REASON
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficExitSnapshot {
    config: TrafficExitConfig,
    exit_tick: Option<u64>,
}

impl TrafficExitSnapshot {
    pub const fn new(config: TrafficExitConfig, exit_tick: Option<u64>) -> Self {
        Self { config, exit_tick }
    }

    pub const fn config(self) -> TrafficExitConfig {
        self.config
    }

    pub const fn exit_tick(self) -> Option<u64> {
        self.exit_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrafficExitGenerator {
    config: TrafficExitConfig,
    exit_tick: Option<u64>,
}

impl TrafficExitGenerator {
    pub const fn new(config: TrafficExitConfig) -> Self {
        Self {
            config,
            exit_tick: None,
        }
    }

    pub const fn restore(snapshot: TrafficExitSnapshot) -> Self {
        Self {
            config: snapshot.config(),
            exit_tick: snapshot.exit_tick(),
        }
    }

    pub const fn config(&self) -> TrafficExitConfig {
        self.config
    }

    pub const fn exited(&self) -> bool {
        self.exit_tick.is_some()
    }

    pub const fn exit_tick(&self) -> Option<u64> {
        self.exit_tick
    }

    pub fn enter(&mut self, tick: u64) -> TrafficExitEvent {
        let exit_tick = match self.exit_tick {
            Some(exit_tick) => exit_tick,
            None => {
                self.exit_tick = Some(tick);
                tick
            }
        };
        TrafficExitEvent::new(exit_tick, self.config.duration())
    }

    pub const fn schedule_tick(
        &self,
        _tick: u64,
        _retry_delay: u64,
    ) -> Result<u64, TrafficGeneratorError> {
        Ok(u64::MAX)
    }

    pub const fn next_request(
        &mut self,
        _tick: u64,
        _retry_delay: u64,
    ) -> Result<Option<TrafficRequestEvent>, TrafficGeneratorError> {
        Ok(None)
    }

    pub const fn snapshot(&self) -> TrafficExitSnapshot {
        TrafficExitSnapshot::new(self.config, self.exit_tick)
    }
}
