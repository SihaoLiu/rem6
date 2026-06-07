use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::Tick;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Cycles(u64);

impl Cycles {
    pub const ZERO: Self = Self(0);

    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for Cycles {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClockError {
    ZeroPeriod,
    ZeroClockDivider,
    EmptyPerformancePoints,
    UnsortedPerformancePoints,
    InvalidPerformanceLevel { level: usize, count: usize },
    MissingClockDomainId,
    DuplicateClockDomain { domain: ClockDomainId },
    UnknownClockDomain { domain: ClockDomainId },
    NotSourceClockDomain { domain: ClockDomainId },
    NotDerivedClockDomain { domain: ClockDomainId },
    TickOverflow { period: Tick, cycles: Cycles },
    DerivedClockOverflow { period: Tick, divider: u64 },
}

impl fmt::Display for ClockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPeriod => write!(formatter, "clock period must be greater than zero"),
            Self::ZeroClockDivider => {
                write!(formatter, "derived clock divider must be greater than zero")
            }
            Self::EmptyPerformancePoints => {
                write!(formatter, "source clock domain requires performance points")
            }
            Self::UnsortedPerformancePoints => write!(
                formatter,
                "source clock periods must be sorted from fastest to slowest"
            ),
            Self::InvalidPerformanceLevel { level, count } => write!(
                formatter,
                "source clock performance level {level} is outside {count} configured points"
            ),
            Self::MissingClockDomainId => {
                write!(formatter, "source clock domain must have a domain id")
            }
            Self::DuplicateClockDomain { domain } => {
                write!(
                    formatter,
                    "clock domain {} is already registered",
                    domain.get()
                )
            }
            Self::UnknownClockDomain { domain } => {
                write!(formatter, "clock domain {} is not registered", domain.get())
            }
            Self::NotSourceClockDomain { domain } => write!(
                formatter,
                "clock domain {} is not a source clock domain",
                domain.get()
            ),
            Self::NotDerivedClockDomain { domain } => write!(
                formatter,
                "clock domain {} is not a derived clock domain",
                domain.get()
            ),
            Self::TickOverflow { period, cycles } => write!(
                formatter,
                "clock period {period} overflows tick conversion for {} cycles",
                cycles.get()
            ),
            Self::DerivedClockOverflow { period, divider } => write!(
                formatter,
                "clock period {period} overflows derived clock divider {divider}"
            ),
        }
    }
}

impl Error for ClockError {}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ClockDomain {
    period: Tick,
}

impl ClockDomain {
    pub fn new(period: Tick) -> Result<Self, ClockError> {
        if period == 0 {
            return Err(ClockError::ZeroPeriod);
        }

        Ok(Self { period })
    }

    pub const fn period(self) -> Tick {
        self.period
    }

    pub fn frequency_hz(self, tick_frequency_hz: u64) -> u64 {
        tick_frequency_hz / self.period
    }

    pub fn cycles_to_ticks(self, cycles: Cycles) -> Result<Tick, ClockError> {
        self.period
            .checked_mul(cycles.get())
            .ok_or(ClockError::TickOverflow {
                period: self.period,
                cycles,
            })
    }

    pub fn ticks_to_cycles_ceil(self, ticks: Tick) -> Cycles {
        if ticks == 0 {
            return Cycles::ZERO;
        }

        Cycles::new(((ticks - 1) / self.period) + 1)
    }

    pub fn clock_edge(self, now: Tick, cycles: Cycles) -> Result<Tick, ClockError> {
        let remainder = now % self.period;
        let aligned = if remainder == 0 {
            now
        } else {
            now.checked_add(self.period - remainder)
                .ok_or(ClockError::TickOverflow {
                    period: self.period,
                    cycles,
                })?
        };
        let offset = self.cycles_to_ticks(cycles)?;

        aligned.checked_add(offset).ok_or(ClockError::TickOverflow {
            period: self.period,
            cycles,
        })
    }

    pub fn derived(self, divider: u64) -> Result<Self, ClockError> {
        if divider == 0 {
            return Err(ClockError::ZeroClockDivider);
        }

        let period = self
            .period
            .checked_mul(divider)
            .ok_or(ClockError::DerivedClockOverflow {
                period: self.period,
                divider,
            })?;
        Self::new(period)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ClockDomainId(u32);

impl ClockDomainId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceClockDomain {
    domain_id: Option<ClockDomainId>,
    periods: Vec<Tick>,
    performance_level: usize,
}

impl SourceClockDomain {
    pub fn new(
        domain_id: Option<ClockDomainId>,
        periods: Vec<Tick>,
        performance_level: usize,
    ) -> Result<Self, ClockError> {
        validate_performance_points(&periods)?;
        validate_performance_level(performance_level, periods.len())?;

        Ok(Self {
            domain_id,
            periods,
            performance_level,
        })
    }

    pub const fn domain_id(&self) -> Option<ClockDomainId> {
        self.domain_id
    }

    pub const fn performance_level(&self) -> usize {
        self.performance_level
    }

    pub fn performance_point_count(&self) -> usize {
        self.periods.len()
    }

    pub fn valid_performance_level(&self, level: usize) -> bool {
        level < self.performance_point_count()
    }

    pub fn period(&self) -> Tick {
        self.periods[self.performance_level]
    }

    pub fn period_at_performance_level(&self, level: usize) -> Result<Tick, ClockError> {
        validate_performance_level(level, self.periods.len())?;
        Ok(self.periods[level])
    }

    pub fn performance_points(&self) -> &[Tick] {
        &self.periods
    }

    pub fn clock_domain(&self) -> ClockDomain {
        ClockDomain::new(self.period()).expect("validated source clock period")
    }

    pub fn frequency_hz(&self, tick_frequency_hz: u64) -> Result<u64, ClockError> {
        Ok(self.clock_domain().frequency_hz(tick_frequency_hz))
    }

    pub fn set_performance_level(&mut self, level: usize) -> Result<bool, ClockError> {
        validate_performance_level(level, self.periods.len())?;
        if level == self.performance_level {
            return Ok(false);
        }
        self.performance_level = level;
        Ok(true)
    }

    pub fn derived_clock_domain(&self, divider: u64) -> Result<ClockDomain, ClockError> {
        self.clock_domain().derived(divider)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DerivedClockDomain {
    domain_id: ClockDomainId,
    parent_id: ClockDomainId,
    divider: u64,
    clock_domain: ClockDomain,
}

impl DerivedClockDomain {
    fn new(
        domain_id: ClockDomainId,
        parent_id: ClockDomainId,
        parent: ClockDomain,
        divider: u64,
    ) -> Result<Self, ClockError> {
        Ok(Self {
            domain_id,
            parent_id,
            divider,
            clock_domain: parent.derived(divider)?,
        })
    }

    pub const fn domain_id(&self) -> ClockDomainId {
        self.domain_id
    }

    pub const fn parent_id(&self) -> ClockDomainId {
        self.parent_id
    }

    pub const fn divider(&self) -> u64 {
        self.divider
    }

    pub const fn clock_domain(&self) -> ClockDomain {
        self.clock_domain
    }

    pub const fn period(&self) -> Tick {
        self.clock_domain.period()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClockDomainTree {
    domains: BTreeMap<ClockDomainId, ClockDomainNode>,
    children: BTreeMap<ClockDomainId, Vec<ClockDomainId>>,
}

impl ClockDomainTree {
    pub fn new() -> Self {
        Self {
            domains: BTreeMap::new(),
            children: BTreeMap::new(),
        }
    }

    pub fn insert_source(&mut self, source: SourceClockDomain) -> Result<(), ClockError> {
        let domain = source.domain_id().ok_or(ClockError::MissingClockDomainId)?;
        self.ensure_domain_absent(domain)?;
        self.domains.insert(domain, ClockDomainNode::Source(source));
        self.children.entry(domain).or_default();
        Ok(())
    }

    pub fn insert_derived(
        &mut self,
        domain: ClockDomainId,
        parent: ClockDomainId,
        divider: u64,
    ) -> Result<(), ClockError> {
        self.ensure_domain_absent(domain)?;
        let parent_domain = self.clock_domain(parent)?;
        let derived = DerivedClockDomain::new(domain, parent, parent_domain, divider)?;
        self.domains
            .insert(domain, ClockDomainNode::Derived(derived));
        self.children.entry(parent).or_default().push(domain);
        self.children.entry(domain).or_default();
        Ok(())
    }

    pub fn clock_domain(&self, domain: ClockDomainId) -> Result<ClockDomain, ClockError> {
        match self.domain(domain)? {
            ClockDomainNode::Source(source) => Ok(source.clock_domain()),
            ClockDomainNode::Derived(derived) => Ok(derived.clock_domain()),
        }
    }

    pub fn source_clock_domain(
        &self,
        domain: ClockDomainId,
    ) -> Result<&SourceClockDomain, ClockError> {
        match self.domain(domain)? {
            ClockDomainNode::Source(source) => Ok(source),
            ClockDomainNode::Derived(_) => Err(ClockError::NotSourceClockDomain { domain }),
        }
    }

    pub fn derived_clock_domain(
        &self,
        domain: ClockDomainId,
    ) -> Result<&DerivedClockDomain, ClockError> {
        match self.domain(domain)? {
            ClockDomainNode::Source(_) => Err(ClockError::NotDerivedClockDomain { domain }),
            ClockDomainNode::Derived(derived) => Ok(derived),
        }
    }

    pub fn set_source_performance_level(
        &mut self,
        domain: ClockDomainId,
        level: usize,
    ) -> Result<bool, ClockError> {
        let source = self.source_clock_domain(domain)?;
        validate_performance_level(level, source.performance_point_count())?;
        if level == source.performance_level() {
            return Ok(false);
        }

        let source_domain = ClockDomain::new(source.period_at_performance_level(level)?)?;
        let mut derived_updates = Vec::new();
        self.collect_derived_updates(domain, source_domain, &mut derived_updates)?;

        match self
            .domains
            .get_mut(&domain)
            .expect("validated source clock domain remains registered")
        {
            ClockDomainNode::Source(source) => {
                source.set_performance_level(level)?;
            }
            ClockDomainNode::Derived(_) => unreachable!("validated source clock domain"),
        }
        for (domain, clock_domain) in derived_updates {
            match self
                .domains
                .get_mut(&domain)
                .expect("validated derived clock domain remains registered")
            {
                ClockDomainNode::Derived(derived) => {
                    derived.clock_domain = clock_domain;
                }
                ClockDomainNode::Source(_) => unreachable!("validated derived clock domain"),
            }
        }
        Ok(true)
    }

    fn ensure_domain_absent(&self, domain: ClockDomainId) -> Result<(), ClockError> {
        if self.domains.contains_key(&domain) {
            return Err(ClockError::DuplicateClockDomain { domain });
        }
        Ok(())
    }

    fn domain(&self, domain: ClockDomainId) -> Result<&ClockDomainNode, ClockError> {
        self.domains
            .get(&domain)
            .ok_or(ClockError::UnknownClockDomain { domain })
    }

    fn collect_derived_updates(
        &self,
        parent: ClockDomainId,
        parent_domain: ClockDomain,
        updates: &mut Vec<(ClockDomainId, ClockDomain)>,
    ) -> Result<(), ClockError> {
        let Some(children) = self.children.get(&parent) else {
            return Ok(());
        };
        for child in children {
            let ClockDomainNode::Derived(derived) = self.domain(*child)? else {
                unreachable!("source clock domains are never registered as children");
            };
            let child_domain = parent_domain.derived(derived.divider())?;
            updates.push((*child, child_domain));
            self.collect_derived_updates(*child, child_domain, updates)?;
        }
        Ok(())
    }
}

impl Default for ClockDomainTree {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ClockDomainNode {
    Source(SourceClockDomain),
    Derived(DerivedClockDomain),
}

fn validate_performance_points(periods: &[Tick]) -> Result<(), ClockError> {
    if periods.is_empty() {
        return Err(ClockError::EmptyPerformancePoints);
    }
    if periods.contains(&0) {
        return Err(ClockError::ZeroPeriod);
    }
    if !periods.windows(2).all(|window| window[0] <= window[1]) {
        return Err(ClockError::UnsortedPerformancePoints);
    }
    Ok(())
}

fn validate_performance_level(level: usize, count: usize) -> Result<(), ClockError> {
    if level >= count {
        return Err(ClockError::InvalidPerformanceLevel { level, count });
    }
    Ok(())
}
