//! The order of the steps that publish a PDF atomically, and what a failure at each step leaves behind.
//!
//! The owner of the effect, `sivi::publish`, follows these decisions step by step; `specs/publish.tla` checks the protocol over every interleaving with a crash, and the fault injection tests of the owner drive each failure.

/// How far the publication has come: the last step that completed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Stage {
    /// Nothing has been written.
    #[default]
    Fresh,
    /// The staging file exists beside the output.
    Created,
    /// The staging file holds every byte.
    Written,
    /// The staging file's bytes are on the disk.
    Synced,
    /// The staging file has become the output.
    Renamed,
}

/// One step of the publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Action {
    Create,
    Write,
    Sync,
    Rename,
}

/// What a failure leaves to undo.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Cleanup {
    /// The staging file exists and must go; the output is untouched.
    RemoveStaged,
    /// There is nothing to undo: either nothing was created, or the output is already complete.
    Nothing,
}

/// The step that follows `stage`, or `None` once the output is in place.
#[must_use]
pub const fn next(stage: Stage) -> Option<Action> {
    match stage {
        Stage::Fresh => Some(Action::Create),
        Stage::Created => Some(Action::Write),
        Stage::Written => Some(Action::Sync),
        Stage::Synced => Some(Action::Rename),
        Stage::Renamed => None,
    }
}

/// The stage after `action` completed.
#[must_use]
pub const fn after(action: Action) -> Stage {
    match action {
        Action::Create => Stage::Created,
        Action::Write => Stage::Written,
        Action::Sync => Stage::Synced,
        Action::Rename => Stage::Renamed,
    }
}

/// What to undo when the step after `stage` fails.
#[must_use]
pub const fn cleanup(stage: Stage) -> Cleanup {
    match stage {
        Stage::Fresh | Stage::Renamed => Cleanup::Nothing,
        Stage::Created | Stage::Written | Stage::Synced => Cleanup::RemoveStaged,
    }
}

/// The properties Kani proves for every stage and the tests check over the stages they list.
#[cfg(any(test, kani))]
mod proofs {
    use super::{Action, Cleanup, Stage, after, cleanup, next};

    /// The most steps a publication takes.
    const STEPS: usize = 4;

    /// The output is replaced only by a rename, which comes only after the bytes are synced, and every stage reaches the end.
    pub(super) fn spec_order(stage: Stage) {
        if let Some(action) = next(stage) {
            assert_eq!(action == Action::Rename, stage == Stage::Synced);
            assert_ne!(after(action), stage);
        } else {
            assert_eq!(stage, Stage::Renamed);
        }
        let mut current = stage;
        let mut steps: usize = 0;
        while let Some(action) = next(current) {
            current = after(action);
            steps = steps.saturating_add(1);
        }
        assert_eq!(current, Stage::Renamed);
        assert!(steps <= STEPS);
    }

    /// A failure removes the staging file exactly when it exists and the output is not yet replaced.
    pub(super) fn spec_cleanup(stage: Stage) {
        let staged = matches!(stage, Stage::Created | Stage::Written | Stage::Synced);
        assert_eq!(cleanup(stage) == Cleanup::RemoveStaged, staged);
    }

    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(6)]
    fn the_rename_comes_last_after_the_sync_and_a_failure_leaves_the_output_alone() {
        let stage: Stage = kani::any();
        spec_order(stage);
        spec_cleanup(stage);
        kani::cover!(next(stage) == Some(Action::Rename));
        kani::cover!(cleanup(stage) == Cleanup::Nothing);
    }

    #[cfg(kani)]
    #[kani::proof]
    fn rejects_a_rename_before_the_sync() {
        kani::assert(
            next(Stage::Written) == Some(Action::Rename),
            "NEGATIVE_PUBLISH_CONTROL",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{Action, Cleanup, Stage, after, cleanup, next, proofs};

    const STAGES: [Stage; 5] = [
        Stage::Fresh,
        Stage::Created,
        Stage::Written,
        Stage::Synced,
        Stage::Renamed,
    ];

    #[test]
    fn the_steps_run_in_order_and_each_stage_meets_its_specification() {
        assert_eq!(next(Stage::Fresh), Some(Action::Create));
        assert_eq!(next(Stage::Created), Some(Action::Write));
        assert_eq!(next(Stage::Written), Some(Action::Sync));
        assert_eq!(next(Stage::Synced), Some(Action::Rename));
        assert_eq!(next(Stage::Renamed), None);
        assert_eq!(after(Action::Sync), Stage::Synced);
        assert_eq!(cleanup(Stage::Fresh), Cleanup::Nothing);
        assert_eq!(cleanup(Stage::Written), Cleanup::RemoveStaged);
        assert_eq!(cleanup(Stage::Renamed), Cleanup::Nothing);
        for stage in STAGES {
            proofs::spec_order(stage);
            proofs::spec_cleanup(stage);
        }
    }
}
