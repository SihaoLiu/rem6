use std::fmt;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatScope {
    spelling: String,
    segments: Vec<String>,
}

impl StatScope {
    pub fn new<I, S>(segments: I) -> Result<Self, StatPathError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::from_segments(segments.into_iter().map(Into::into).collect())
    }

    pub fn from_segments(segments: Vec<String>) -> Result<Self, StatPathError> {
        let spelling = segments.join(".");
        validate_stat_segments(segments.iter().map(String::as_str))?;
        Ok(Self { spelling, segments })
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    pub fn stat_path(&self, name: impl Into<String>) -> Result<StatPath, StatPathError> {
        let mut segments = self.segments.clone();
        segments.push(name.into());
        StatPath::from_segments(segments)
    }
}

impl fmt::Display for StatScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatPath {
    spelling: String,
    segments: Vec<String>,
}

impl StatPath {
    pub fn parse(path: impl Into<String>) -> Result<Self, StatPathError> {
        let spelling = path.into();
        validate_stat_path(&spelling)?;
        let segments = spelling.split('.').map(str::to_string).collect();
        Ok(Self { spelling, segments })
    }

    pub fn new<I, S>(scope: I, name: impl Into<String>) -> Result<Self, StatPathError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut segments = scope.into_iter().map(Into::into).collect::<Vec<_>>();
        segments.push(name.into());
        Self::from_segments(segments)
    }

    pub fn from_segments(segments: Vec<String>) -> Result<Self, StatPathError> {
        let spelling = segments.join(".");
        validate_stat_segments(segments.iter().map(String::as_str))?;
        Ok(Self { spelling, segments })
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    pub fn scope(&self) -> &[String] {
        let name_index = self.segments.len().saturating_sub(1);
        &self.segments[..name_index]
    }

    pub fn name(&self) -> &str {
        self.segments
            .last()
            .map(String::as_str)
            .expect("stat path must have a name segment")
    }

    pub fn segments(&self) -> &[String] {
        &self.segments
    }
}

impl fmt::Display for StatPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatUnitKind {
    Cycle,
    Tick,
    Second,
    Bit,
    Byte,
    Watt,
    Joule,
    Volt,
    Celsius,
    Count,
    Ratio,
    Unspecified,
    Custom(String),
    Rate {
        numerator: Box<StatUnitKind>,
        denominator: Box<StatUnitKind>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatUnit {
    spelling: String,
    kind: StatUnitKind,
}

impl StatUnit {
    pub fn parse(unit: impl Into<String>) -> Result<Self, StatUnitError> {
        let spelling = unit.into();
        validate_stat_unit_characters(&spelling)?;
        let (kind, consumed) = parse_stat_unit_kind(&spelling, 0)?;
        if consumed != spelling.len() {
            let character = spelling.as_bytes()[consumed] as char;
            return Err(StatUnitError::TrailingInput {
                index: consumed,
                character,
            });
        }
        let spelling = if spelling == "DegreeCelsius" {
            "Celsius".to_string()
        } else {
            spelling
        };
        Ok(Self { spelling, kind })
    }

    pub fn cycle() -> Self {
        Self::builtin("Cycle", StatUnitKind::Cycle)
    }

    pub fn tick() -> Self {
        Self::builtin("Tick", StatUnitKind::Tick)
    }

    pub fn second() -> Self {
        Self::builtin("Second", StatUnitKind::Second)
    }

    pub fn bit() -> Self {
        Self::builtin("Bit", StatUnitKind::Bit)
    }

    pub fn byte() -> Self {
        Self::builtin("Byte", StatUnitKind::Byte)
    }

    pub fn watt() -> Self {
        Self::builtin("Watt", StatUnitKind::Watt)
    }

    pub fn joule() -> Self {
        Self::builtin("Joule", StatUnitKind::Joule)
    }

    pub fn volt() -> Self {
        Self::builtin("Volt", StatUnitKind::Volt)
    }

    pub fn celsius() -> Self {
        Self::builtin("Celsius", StatUnitKind::Celsius)
    }

    pub fn degree_celsius() -> Self {
        Self::celsius()
    }

    pub fn count() -> Self {
        Self::builtin("Count", StatUnitKind::Count)
    }

    pub fn ratio() -> Self {
        Self::builtin("Ratio", StatUnitKind::Ratio)
    }

    pub fn unspecified() -> Self {
        Self::builtin("Unspecified", StatUnitKind::Unspecified)
    }

    pub fn rate(numerator: Self, denominator: Self) -> Self {
        let numerator_spelling = numerator.spelling;
        let numerator_kind = numerator.kind;
        let denominator_spelling = denominator.spelling;
        let denominator_kind = denominator.kind;
        Self {
            spelling: format!("({numerator_spelling}/{denominator_spelling})"),
            kind: StatUnitKind::Rate {
                numerator: Box::new(numerator_kind),
                denominator: Box::new(denominator_kind),
            },
        }
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }

    pub const fn kind(&self) -> &StatUnitKind {
        &self.kind
    }

    fn builtin(spelling: &str, kind: StatUnitKind) -> Self {
        Self {
            spelling: spelling.to_string(),
            kind,
        }
    }
}

impl fmt::Display for StatUnit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StatDescription {
    spelling: String,
}

impl StatDescription {
    pub fn new(description: impl Into<String>) -> Result<Self, StatDescriptionError> {
        let spelling = description.into();
        validate_stat_description(&spelling)?;
        Ok(Self { spelling })
    }

    pub fn as_str(&self) -> &str {
        &self.spelling
    }
}

impl fmt::Display for StatDescription {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatPathError {
    EmptySegment { index: usize },
    InvalidSegmentStart { segment: String, character: char },
    InvalidSegmentCharacter { segment: String, character: char },
}

impl fmt::Display for StatPathError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySegment { index } => {
                write!(formatter, "segment {index} must not be empty")
            }
            Self::InvalidSegmentStart { segment, character } => write!(
                formatter,
                "segment {segment} starts with invalid character {character:?}"
            ),
            Self::InvalidSegmentCharacter { segment, character } => write!(
                formatter,
                "segment {segment} contains invalid character {character:?}"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatUnitError {
    Empty,
    InvalidCharacter { character: char },
    ExpectedTerm { index: usize },
    ExpectedRateSeparator { index: usize },
    ExpectedRateTerminator { index: usize },
    TrailingInput { index: usize, character: char },
}

impl fmt::Display for StatUnitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(formatter, "unit must not be empty"),
            Self::InvalidCharacter { character } => {
                write!(formatter, "unit contains invalid character {character:?}")
            }
            Self::ExpectedTerm { index } => {
                write!(formatter, "unit needs a term at byte {index}")
            }
            Self::ExpectedRateSeparator { index } => {
                write!(formatter, "unit rate needs '/' at byte {index}")
            }
            Self::ExpectedRateTerminator { index } => {
                write!(formatter, "unit rate needs ')' at byte {index}")
            }
            Self::TrailingInput { index, character } => write!(
                formatter,
                "unit has trailing character {character:?} at byte {index}"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatDescriptionError {
    Empty,
    InvalidCharacter { character: char },
}

impl fmt::Display for StatDescriptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(formatter, "description must not be empty"),
            Self::InvalidCharacter { character } => {
                write!(
                    formatter,
                    "description contains invalid character {character:?}"
                )
            }
        }
    }
}

fn validate_stat_path(path: &str) -> Result<(), StatPathError> {
    validate_stat_segments(path.split('.'))
}

fn validate_stat_segments<'a>(
    segments: impl IntoIterator<Item = &'a str>,
) -> Result<(), StatPathError> {
    let mut saw_segment = false;
    for (index, segment) in segments.into_iter().enumerate() {
        saw_segment = true;
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            return Err(StatPathError::EmptySegment { index });
        };
        if !first.is_ascii_alphabetic() && first != '_' {
            return Err(StatPathError::InvalidSegmentStart {
                segment: segment.to_string(),
                character: first,
            });
        }
        for character in chars {
            if !character.is_ascii_alphanumeric() && character != '_' {
                return Err(StatPathError::InvalidSegmentCharacter {
                    segment: segment.to_string(),
                    character,
                });
            }
        }
    }
    if !saw_segment {
        return Err(StatPathError::EmptySegment { index: 0 });
    }
    Ok(())
}

fn validate_stat_description(description: &str) -> Result<(), StatDescriptionError> {
    if description.trim().is_empty() {
        return Err(StatDescriptionError::Empty);
    }
    for character in description.chars() {
        if character.is_control() {
            return Err(StatDescriptionError::InvalidCharacter { character });
        }
    }
    Ok(())
}

fn validate_stat_unit_characters(unit: &str) -> Result<(), StatUnitError> {
    if unit.is_empty() {
        return Err(StatUnitError::Empty);
    }
    for character in unit.chars() {
        if !character.is_ascii_alphanumeric()
            && character != '_'
            && character != '/'
            && character != '('
            && character != ')'
        {
            return Err(StatUnitError::InvalidCharacter { character });
        }
    }
    Ok(())
}

fn parse_stat_unit_kind(unit: &str, index: usize) -> Result<(StatUnitKind, usize), StatUnitError> {
    let Some(character) = unit.as_bytes().get(index).copied().map(char::from) else {
        return Err(StatUnitError::ExpectedTerm { index });
    };
    match character {
        '(' => {
            let (numerator, after_numerator) = parse_stat_unit_kind(unit, index + 1)?;
            if unit.as_bytes().get(after_numerator).copied() != Some(b'/') {
                return Err(StatUnitError::ExpectedRateSeparator {
                    index: after_numerator,
                });
            }
            let (denominator, after_denominator) = parse_stat_unit_kind(unit, after_numerator + 1)?;
            if unit.as_bytes().get(after_denominator).copied() != Some(b')') {
                return Err(StatUnitError::ExpectedRateTerminator {
                    index: after_denominator,
                });
            }
            Ok((
                StatUnitKind::Rate {
                    numerator: Box::new(numerator),
                    denominator: Box::new(denominator),
                },
                after_denominator + 1,
            ))
        }
        ')' | '/' => Err(StatUnitError::ExpectedTerm { index }),
        _ => {
            let mut end = index;
            while let Some(character) = unit.as_bytes().get(end).copied().map(char::from) {
                if !character.is_ascii_alphanumeric() && character != '_' {
                    break;
                }
                end += 1;
            }
            if end == index {
                return Err(StatUnitError::ExpectedTerm { index });
            }
            Ok((stat_unit_symbol_kind(&unit[index..end]), end))
        }
    }
}

fn stat_unit_symbol_kind(symbol: &str) -> StatUnitKind {
    match symbol {
        "Cycle" => StatUnitKind::Cycle,
        "Tick" => StatUnitKind::Tick,
        "Second" => StatUnitKind::Second,
        "Bit" => StatUnitKind::Bit,
        "Byte" => StatUnitKind::Byte,
        "Watt" => StatUnitKind::Watt,
        "Joule" => StatUnitKind::Joule,
        "Volt" => StatUnitKind::Volt,
        "Celsius" | "DegreeCelsius" => StatUnitKind::Celsius,
        "Count" => StatUnitKind::Count,
        "Ratio" => StatUnitKind::Ratio,
        "Unspecified" => StatUnitKind::Unspecified,
        _ => StatUnitKind::Custom(symbol.to_string()),
    }
}
