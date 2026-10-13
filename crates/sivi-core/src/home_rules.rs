//! Where the binary keeps what it installs: the data directory the environment names, decided from values alone.
//!
//! `SIVI_HOME` overrides every convention; otherwise Windows keeps data under `LOCALAPPDATA`, and every other host under `XDG_DATA_HOME` or `HOME/.local/share`.
//! An empty value counts as unset, so an empty variable never names a directory.

/// What the environment says about where data lives.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HomeFacts<'a> {
    /// `SIVI_HOME`, which overrides every convention.
    pub sivi_home: Option<&'a str>,
    /// `XDG_DATA_HOME`, the data base of Unix desktops.
    pub xdg_data_home: Option<&'a str>,
    /// `HOME`, under which `.local/share` is the conventional data base.
    pub home: Option<&'a str>,
    /// `LOCALAPPDATA`, the per-user data base of Windows.
    pub local_app_data: Option<&'a str>,
    /// Whether the host is Windows, where `LOCALAPPDATA` is the convention.
    pub windows: bool,
}

/// The directory of the binary under a base.
pub const DIRECTORY: &str = "sivi";
/// The segments of the Unix data base under the home directory, when `XDG_DATA_HOME` is unset.
pub const UNIX_DATA: [&str; 2] = [".local", "share"];

/// Which variable the data directory stands on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Base {
    SiviHome,
    LocalAppData,
    XdgDataHome,
    Home,
}

/// The data directory: a base from the environment and the segments to join under it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DataDir<'a> {
    pub base: Base,
    /// The value of the base variable.
    pub root: &'a str,
    /// The segments under the root, ending in the binary's directory.
    pub tail: &'static [&'static str],
}

/// The segments under `SIVI_HOME`, which already names the directory.
const DIRECT: [&str; 0] = [];
/// The segments under `LOCALAPPDATA` or `XDG_DATA_HOME`.
const UNDER_BASE: [&str; 1] = [DIRECTORY];
/// The segments under `HOME`.
const UNDER_HOME: [&str; 3] = [UNIX_DATA[0], UNIX_DATA[1], DIRECTORY];

/// A value that names something: present and not empty.
const fn usable(value: Option<&str>) -> Option<&str> {
    match value {
        Some(text) if !text.is_empty() => Some(text),
        Some(_) | None => None,
    }
}

/// The data directory the facts name, or `None` when no usable variable is set.
#[must_use]
pub const fn data_dir<'a>(facts: &HomeFacts<'a>) -> Option<DataDir<'a>> {
    if let Some(root) = usable(facts.sivi_home) {
        return Some(DataDir {
            base: Base::SiviHome,
            root,
            tail: &DIRECT,
        });
    }
    if facts.windows {
        return match usable(facts.local_app_data) {
            Some(root) => Some(DataDir {
                base: Base::LocalAppData,
                root,
                tail: &UNDER_BASE,
            }),
            None => None,
        };
    }
    if let Some(root) = usable(facts.xdg_data_home) {
        return Some(DataDir {
            base: Base::XdgDataHome,
            root,
            tail: &UNDER_BASE,
        });
    }
    match usable(facts.home) {
        Some(root) => Some(DataDir {
            base: Base::Home,
            root,
            tail: &UNDER_HOME,
        }),
        None => None,
    }
}

/// The properties Kani proves over every combination of set, empty, and unset variables.
#[cfg(any(test, kani))]
mod proofs {
    use super::{Base, DataDir, HomeFacts, data_dir, usable};

    /// A variable as the harness draws it: unset, empty, or a name.
    pub(super) const fn variable(choice: u8, name: &'static str) -> Option<&'static str> {
        match choice {
            0 => None,
            1 => Some(""),
            _ => Some(name),
        }
    }

    /// The decision restated: the first usable variable in precedence, with the tail its base owns.
    pub(super) fn spec(facts: &HomeFacts<'_>) -> bool {
        let found = data_dir(facts);
        let sivi = usable(facts.sivi_home);
        let local = usable(facts.local_app_data);
        let xdg = usable(facts.xdg_data_home);
        let home = usable(facts.home);
        let expected_base = if sivi.is_some() {
            Some(Base::SiviHome)
        } else if facts.windows {
            local.map(|_| Base::LocalAppData)
        } else if xdg.is_some() {
            Some(Base::XdgDataHome)
        } else {
            home.map(|_| Base::Home)
        };
        match (found, expected_base) {
            (None, None) => true,
            (Some(DataDir { base, root, tail }), Some(expected)) => {
                let root_matches = match base {
                    Base::SiviHome => Some(root) == sivi,
                    Base::LocalAppData => Some(root) == local,
                    Base::XdgDataHome => Some(root) == xdg,
                    Base::Home => Some(root) == home,
                };
                let tail_matches = match base {
                    Base::SiviHome => tail.is_empty(),
                    Base::LocalAppData | Base::XdgDataHome => tail == [super::DIRECTORY],
                    Base::Home => tail.last() == Some(&super::DIRECTORY) && tail.len() == 3,
                };
                base == expected && root_matches && tail_matches && !root.is_empty()
            }
            (Some(_), None) | (None, Some(_)) => false,
        }
    }

    /// The longest drawn name has nine bytes, so every byte comparison ends within ten unwindings.
    #[cfg(kani)]
    #[kani::proof]
    #[kani::unwind(10)]
    fn the_data_directory_follows_the_first_usable_variable_in_precedence() {
        let facts = HomeFacts {
            sivi_home: variable(kani::any(), "sivi-home"),
            xdg_data_home: variable(kani::any(), "xdg"),
            home: variable(kani::any(), "home"),
            local_app_data: variable(kani::any(), "local"),
            windows: kani::any(),
        };
        kani::assert(spec(&facts), "the data directory follows the precedence");
        kani::cover!(data_dir(&facts).is_none(), "no variable names a directory");
        kani::cover!(
            matches!(
                data_dir(&facts),
                Some(DataDir {
                    base: Base::Home,
                    ..
                })
            ),
            "the home directory stands in"
        );
    }

    /// An empty `SIVI_HOME` must not name the directory; this probe claims it does and must fail.
    #[cfg(kani)]
    #[kani::proof]
    fn rejects_an_empty_sivi_home() {
        let facts = HomeFacts {
            sivi_home: Some(""),
            xdg_data_home: None,
            home: None,
            local_app_data: None,
            windows: false,
        };
        kani::assert(data_dir(&facts).is_some(), "NEGATIVE_HOME_CONTROL");
    }
}

#[cfg(test)]
mod tests {
    use super::proofs::{spec, variable};
    use super::{Base, DataDir, HomeFacts, data_dir};

    #[test]
    fn sivi_home_wins_then_the_platform_convention_then_the_home_directory() {
        let all = HomeFacts {
            sivi_home: Some("/opt/sivi"),
            xdg_data_home: Some("/xdg"),
            home: Some("/home/me"),
            local_app_data: Some("C:\\Users\\me\\AppData\\Local"),
            windows: false,
        };
        assert_eq!(
            data_dir(&all),
            Some(DataDir {
                base: Base::SiviHome,
                root: "/opt/sivi",
                tail: &[],
            })
        );
        let unix = HomeFacts {
            sivi_home: None,
            ..all
        };
        assert_eq!(
            data_dir(&unix),
            Some(DataDir {
                base: Base::XdgDataHome,
                root: "/xdg",
                tail: &["sivi"],
            })
        );
        let home_only = HomeFacts {
            xdg_data_home: Some(""),
            ..unix
        };
        assert_eq!(
            data_dir(&home_only),
            Some(DataDir {
                base: Base::Home,
                root: "/home/me",
                tail: &[".local", "share", "sivi"],
            })
        );
        let windows = HomeFacts {
            windows: true,
            ..unix
        };
        assert_eq!(
            data_dir(&windows).map(|dir| dir.base),
            Some(Base::LocalAppData)
        );
        let windows_without = HomeFacts {
            local_app_data: None,
            ..windows
        };
        assert_eq!(data_dir(&windows_without), None);
        assert_eq!(data_dir(&HomeFacts::default()), None);
    }

    #[test]
    fn the_specification_agrees_over_every_drawn_combination() {
        for sivi in 0..3 {
            for xdg in 0..3 {
                for home in 0..3 {
                    for local in 0..3 {
                        for windows in [false, true] {
                            let facts = HomeFacts {
                                sivi_home: variable(sivi, "sivi-home"),
                                xdg_data_home: variable(xdg, "xdg"),
                                home: variable(home, "home"),
                                local_app_data: variable(local, "local"),
                                windows,
                            };
                            assert!(spec(&facts), "{facts:?}");
                        }
                    }
                }
            }
        }
    }
}
