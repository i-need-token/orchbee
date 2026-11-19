//! Exclusive admission for the suites whose deadlines sibling process startup would consume.
use crate::cohort_fixture::CohortFixture;

impl CohortFixture {
    pub(super) fn deadline_sensitive() -> Self {
        // Reach the intended protocol phase without sibling process startup consuming its budget.
        Self::acquire(true)
    }
}
