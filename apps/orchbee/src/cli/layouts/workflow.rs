//! Phase order and the compensation boundary, independent of Windows effects.
use super::template::Plan;
use crate::cli::{Failure, FailureKind};

pub(super) trait Port {
    fn create(&mut self) -> Result<(), Failure>;
    fn launch(&mut self, panel: usize, member: usize) -> Result<(), Failure>;
    fn settle(&mut self, panel: usize, member: usize) -> Result<(), Failure>;
    fn ratio(&mut self, split: usize) -> Result<(), Failure>;
    fn ready(&mut self) -> Result<(), Failure>;
    fn focus(&mut self) -> Result<(), Failure>;
    fn replace(&mut self) -> Result<(), Failure>;
    fn observe(&mut self) -> Result<(), Failure>;
    /// The adapter must fence owned identity, preserve uncertainty and use the original budget.
    fn rollback(&mut self);
}

pub(super) fn execute(plan: &Plan, port: &mut impl Port) -> Result<(), Failure> {
    let before_ready = (|| {
        port.create()?;
        for (panel, leaf) in plan.leaves.iter().enumerate() {
            for (member, intent) in leaf.members().enumerate() {
                if intent.start {
                    port.launch(panel, member)?;
                    port.settle(panel, member)?;
                }
            }
        }
        for index in 0..plan.splits.len() {
            port.ratio(index)?;
        }
        port.ready()
    })();
    if let Err(error) = before_ready {
        if error.kind != FailureKind::OutcomeUnknown {
            port.rollback();
        }
        return Err(error);
    }
    // A usable complete new layout is preserved once ready, including on later Close failure.
    port.focus()?;
    port.replace()?;
    port.observe()
}

#[cfg(test)]
mod tests;
