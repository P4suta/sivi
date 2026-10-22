//! The resource boundary of a verification worker.

pub const CPU_NANOS: i64 = 2_000_000_000;
pub const MEMORY_BYTES: i64 = 4_294_967_296;
pub const PROCESS_LIMIT: i64 = 256;
const CPU_COUNT: i64 = 2;
const FIRST_RESERVED_EXIT: i32 = 124;
const KANI_BIT: u16 = 0x001;
const VERUS_BIT: u16 = 0x002;
const SMT_BIT: u16 = 0x004;
const ALLOY_BIT: u16 = 0x008;
const QUINT_BIT: u16 = 0x010;
const PROLOG_BIT: u16 = 0x020;
const TLA_BIT: u16 = 0x040;
const MIRI_BIT: u16 = 0x080;
const FUZZ_BIT: u16 = 0x100;
const MUTANTS_BIT: u16 = 0x200;
const ALL_BITS: u16 = KANI_BIT
    | VERUS_BIT
    | SMT_BIT
    | ALLOY_BIT
    | QUINT_BIT
    | PROLOG_BIT
    | TLA_BIT
    | MIRI_BIT
    | FUZZ_BIT
    | MUTANTS_BIT;

/// One verifier of the obligation portfolio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub enum Verifier {
    Kani,
    Verus,
    Smt,
    Alloy,
    Quint,
    Prolog,
    Tla,
    Miri,
    Fuzz,
    Mutants,
}

impl Verifier {
    pub const ALL: [Self; 10] = [
        Self::Kani,
        Self::Verus,
        Self::Smt,
        Self::Alloy,
        Self::Quint,
        Self::Prolog,
        Self::Tla,
        Self::Miri,
        Self::Fuzz,
        Self::Mutants,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Kani => "kani",
            Self::Verus => "verus",
            Self::Smt => "smt",
            Self::Alloy => "alloy",
            Self::Quint => "quint",
            Self::Prolog => "prolog",
            Self::Tla => "tla",
            Self::Miri => "miri",
            Self::Fuzz => "fuzz",
            Self::Mutants => "mutants",
        }
    }

    #[must_use]
    pub fn named(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|step| step.name() == word)
    }

    const fn bit(self) -> u16 {
        match self {
            Self::Kani => KANI_BIT,
            Self::Verus => VERUS_BIT,
            Self::Smt => SMT_BIT,
            Self::Alloy => ALLOY_BIT,
            Self::Quint => QUINT_BIT,
            Self::Prolog => PROLOG_BIT,
            Self::Tla => TLA_BIT,
            Self::Miri => MIRI_BIT,
            Self::Fuzz => FUZZ_BIT,
            Self::Mutants => MUTANTS_BIT,
        }
    }
}

/// A monotone union of the verifiers affected by changed inputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Selection(u16);

impl Selection {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(ALL_BITS);

    #[must_use]
    pub const fn one(verifier: Verifier) -> Self {
        Self(verifier.bit())
    }
    #[must_use]
    pub const fn contains(self, verifier: Verifier) -> bool {
        self.0 & verifier.bit() != 0
    }
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// A known model selects its checker; documentation selects none; unknown inputs select everything.
#[must_use]
pub fn affected(path: &str) -> Selection {
    let extension = path.rsplit_once('.').map(|(_, extension)| extension);
    if path.starts_with("specs/") {
        if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("als")) {
            return Selection::one(Verifier::Alloy);
        }
        if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("qnt"))
            || path.starts_with("specs/traces/")
        {
            return Selection::one(Verifier::Quint);
        }
        if extension.is_some_and(|extension| extension.eq_ignore_ascii_case("pl")) {
            return Selection::one(Verifier::Prolog);
        }
        if extension.is_some_and(|extension| {
            extension.eq_ignore_ascii_case("tla") || extension.eq_ignore_ascii_case("cfg")
        }) {
            return Selection::one(Verifier::Tla);
        }
    }
    if (path.starts_with("docs/") && extension == Some("md"))
        || matches!(path, "README.md" | "LICENSE-MIT" | "LICENSE-APACHE")
    {
        return Selection::NONE;
    }
    Selection::ALL
}

/// Whether an observed worker termination supplies a complete result.
#[must_use]
pub const fn completed(exit_code: i32, oom: bool, infrastructure_error: bool) -> bool {
    !oom && !infrastructure_error && exit_code >= 0 && exit_code < FIRST_RESERVED_EXIT
}

/// The hard limits Docker reports for a container.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[cfg_attr(kani, derive(kani::Arbitrary))]
pub struct Limits {
    pub cpu_nanos: i64,
    pub cpu_quota: i64,
    pub cpu_period: i64,
    pub memory: i64,
    pub memory_swap: i64,
    pub processes: i64,
}

impl Limits {
    pub const STANDARD: Self = Self {
        cpu_nanos: CPU_NANOS,
        cpu_quota: 0,
        cpu_period: 0,
        memory: MEMORY_BYTES,
        memory_swap: MEMORY_BYTES,
        processes: PROCESS_LIMIT,
    };

    /// Whether every resource has a hard ceiling within the campaign budget.
    #[must_use]
    pub const fn bounded(self) -> bool {
        let cpu = if self.cpu_nanos > 0 {
            self.cpu_nanos <= CPU_NANOS && self.cpu_quota == 0 && self.cpu_period == 0
        } else if self.cpu_nanos == 0 && self.cpu_quota > 0 && self.cpu_period > 0 {
            match self.cpu_period.checked_mul(CPU_COUNT) {
                Some(ceiling) => self.cpu_quota <= ceiling,
                None => false,
            }
        } else {
            false
        };
        cpu && self.memory > 0
            && self.memory <= MEMORY_BYTES
            && self.memory_swap == self.memory
            && self.processes > 0
            && self.processes <= PROCESS_LIMIT
    }
}

#[cfg(kani)]
mod proofs {
    use super::{
        CPU_NANOS, FIRST_RESERVED_EXIT, Limits, MEMORY_BYTES, PROCESS_LIMIT, Selection, Verifier,
        completed,
    };

    #[kani::proof]
    fn every_admitted_worker_has_hard_cpu_memory_and_process_ceilings() {
        let limits: Limits = kani::any();
        if limits.bounded() {
            assert!(limits.memory > 0 && limits.memory <= MEMORY_BYTES);
            assert_eq!(limits.memory_swap, limits.memory);
            assert!(limits.processes > 0 && limits.processes <= PROCESS_LIMIT);
            if limits.cpu_nanos > 0 {
                assert!(limits.cpu_nanos <= CPU_NANOS);
            } else {
                assert_eq!(limits.cpu_nanos, 0);
                assert!(limits.cpu_quota > 0 && limits.cpu_period > 0);
                assert!(
                    i128::from(limits.cpu_quota) <= i128::from(limits.cpu_period).wrapping_mul(2)
                );
            }
        }
        kani::cover!(limits.bounded());
        kani::cover!(!limits.bounded());
    }

    #[kani::proof]
    fn rejects_a_worker_with_unlimited_memory() {
        kani::assert(
            Limits {
                memory: 0,
                ..Limits::STANDARD
            }
            .bounded(),
            "NEGATIVE_VERIFICATION_CONTROL",
        );
    }

    #[kani::proof]
    fn adding_changed_inputs_never_removes_a_selected_verifier() {
        let first = Selection(kani::any::<u16>() & Selection::ALL.0);
        let second = Selection(kani::any::<u16>() & Selection::ALL.0);
        let verifier: Verifier = kani::any();
        let combined = first.union(second);
        assert_eq!(
            combined.contains(verifier),
            first.contains(verifier) || second.contains(verifier)
        );
        assert!(Selection::ALL.contains(verifier));
        assert!(!Selection::NONE.contains(verifier));
        kani::cover!(first.contains(verifier) && !second.contains(verifier));
        kani::cover!(!first.contains(verifier) && second.contains(verifier));
    }

    #[kani::proof]
    fn exhausted_workers_never_supply_a_complete_verdict() {
        let exit: i32 = kani::any();
        let oom: bool = kani::any();
        let infrastructure_error: bool = kani::any();
        let complete = completed(exit, oom, infrastructure_error);
        if oom || infrastructure_error || !(0..FIRST_RESERVED_EXIT).contains(&exit) {
            assert!(!complete);
        }
        if complete {
            assert!(!oom && !infrastructure_error);
            assert!((0..FIRST_RESERVED_EXIT).contains(&exit));
        }
        kani::cover!(complete && exit != 0);
        kani::cover!(!complete && exit == 0);
    }
}

#[cfg(test)]
mod tests {
    use super::{CPU_NANOS, Limits, MEMORY_BYTES, PROCESS_LIMIT, Selection, Verifier, affected};

    #[test]
    fn unknown_inputs_run_everything_and_known_models_keep_their_checker() {
        for path in [
            "new/input",
            "Cargo.lock",
            "xtask/src/proofs.rs",
            "crates/sivi/src/args.rs",
            "verification/obligations.toml",
        ] {
            assert_eq!(affected(path), Selection::ALL);
        }
        for (path, verifier) in [
            ("specs/a.als", Verifier::Alloy),
            ("specs/a.qnt", Verifier::Quint),
            ("specs/traces/a.json", Verifier::Quint),
            ("specs/binding.pl", Verifier::Prolog),
            ("specs/a.tla", Verifier::Tla),
            ("specs/a.cfg", Verifier::Tla),
        ] {
            assert_eq!(affected(path), Selection::one(verifier));
        }
        assert_eq!(affected("docs/architecture.md"), Selection::NONE);
        assert!(
            affected("specs/a.als")
                .union(affected("Cargo.lock"))
                .contains(Verifier::Kani)
        );
    }

    #[test]
    fn verifier_names_and_selections_are_distinct_and_unions_preserve_both_inputs() {
        for first in Verifier::ALL {
            assert_eq!(Verifier::named(first.name()), Some(first));
            assert!(Selection::ALL.contains(first));
            assert!(!Selection::NONE.contains(first));
            for second in Verifier::ALL {
                assert_eq!(Selection::one(first).contains(second), first == second);
                let combined = Selection::one(first).union(Selection::one(second));
                assert!(combined.contains(first) && combined.contains(second));
            }
        }
        assert_eq!(Verifier::named("unknown"), None);
    }

    #[test]
    fn a_status_alone_never_overrides_exhaustion() {
        use super::completed;
        for exit in [0, 1, 12, 42, 123] {
            assert!(completed(exit, false, false));
            assert!(!completed(exit, true, false));
            assert!(!completed(exit, false, true));
        }
        for exit in [
            i32::MIN,
            -1,
            124,
            125,
            126,
            127,
            130,
            137,
            143,
            255,
            i32::MAX,
        ] {
            assert!(!completed(exit, false, false));
        }
    }

    #[test]
    fn the_standard_budget_and_a_smaller_budget_are_admitted() {
        assert!(Limits::STANDARD.bounded());
        assert!(
            Limits {
                cpu_nanos: 1,
                memory: 1,
                memory_swap: 1,
                processes: 1,
                ..Limits::STANDARD
            }
            .bounded()
        );
        assert!(
            Limits {
                cpu_nanos: 0,
                cpu_quota: 200_000,
                cpu_period: 100_000,
                ..Limits::STANDARD
            }
            .bounded()
        );
    }

    #[test]
    fn each_missing_or_excessive_limit_is_refused() {
        assert!(
            !Limits {
                cpu_quota: 1,
                ..Limits::STANDARD
            }
            .bounded()
        );
        assert!(
            !Limits {
                cpu_period: 1,
                ..Limits::STANDARD
            }
            .bounded()
        );
        for value in [i64::MIN, -1, 0, CPU_NANOS + 1, i64::MAX] {
            assert!(
                !Limits {
                    cpu_nanos: value,
                    ..Limits::STANDARD
                }
                .bounded()
            );
        }
        for value in [i64::MIN, -1, 0, MEMORY_BYTES + 1, i64::MAX] {
            assert!(
                !Limits {
                    memory: value,
                    ..Limits::STANDARD
                }
                .bounded()
            );
        }
        for value in [i64::MIN, -1, 0, PROCESS_LIMIT + 1, i64::MAX] {
            assert!(
                !Limits {
                    processes: value,
                    ..Limits::STANDARD
                }
                .bounded()
            );
        }
        for value in [i64::MIN, -1, 0, MEMORY_BYTES + 1, i64::MAX] {
            assert!(
                !Limits {
                    memory_swap: value,
                    ..Limits::STANDARD
                }
                .bounded()
            );
        }
        assert!(
            !Limits {
                cpu_nanos: 0,
                cpu_quota: 300_000,
                cpu_period: 100_000,
                ..Limits::STANDARD
            }
            .bounded()
        );
    }
}
