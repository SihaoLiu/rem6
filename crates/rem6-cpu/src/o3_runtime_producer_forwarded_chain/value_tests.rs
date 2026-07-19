use super::*;

impl O3ProducerForwardedScalarChain {
    pub(crate) fn repeated_last_for_test(&self) -> Self {
        let mut repeated = self.clone();
        if let Some(descendant) = repeated.last() {
            assert!(repeated.push(descendant));
        }
        repeated
    }
}
