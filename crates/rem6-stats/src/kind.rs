use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StatKind {
    Counter,
    Average,
    Histogram,
}

impl fmt::Display for StatKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Counter => formatter.write_str("counter"),
            Self::Average => formatter.write_str("average"),
            Self::Histogram => formatter.write_str("histogram"),
        }
    }
}
