set windows-shell := ["pwsh.exe", "-NoLogo", "-NoProfile", "-Command"]

default:
    @just --list

# mise's paranoid mode loads this checkout's configuration only after a trust that records its current contents.
[private]
trust:
    mise trust --quiet {{quote(justfile_directory() / "mise.toml")}}

# Formatting, lints, the structural gates, the tests, and the dependency checks.
check *args: trust
    mise x -- cargo xtask check {{args}}

# The quick staged-file checks the pre-commit hook runs.
check-staged: trust
    mise x -- cargo xtask check --staged

# The deterministic push gate.
push-check: trust
    mise x -- cargo xtask push-check

# The pure core keeps its boundary and every crate keeps its place.
structure: trust
    mise x -- cargo xtask structure

# Kani, Miri, fuzz corpus replays, mutation testing, and every other verifier of the obligation manifest.
proofs *args: trust
    mise x -- cargo xtask proofs {{args}}

# An explicit proof campaign without mutation testing.
proofs-ci: trust
    mise x -- cargo xtask proofs --ci

# Mutation testing of the decision core alone.
mutants: trust
    mise x -- cargo xtask proofs --only mutants

# Fuzz one target beyond its corpus for a bounded number of runs.
fuzz target runs="20000": trust
    mise x -- cargo xtask fuzz {{quote(target)}} {{runs}}

# The generated documents, such as the diagnostics catalog, or `--check` to refuse drift.
gen *args: trust
    mise x -- cargo xtask gen {{args}}

# Regenerate the font catalog from google/fonts at the pinned commit, which reaches the network.
fonts-catalog: trust
    mise x -- cargo xtask fonts catalog

# Regenerate the code themes from tinted-theming/schemes at the pinned commit, which reaches the network.
code-themes: trust
    mise x -- cargo xtask code-themes fetch

# Build one deck with the development binary.
build deck *args: trust
    mise x -- cargo run --quiet --locked --package sivi -- build {{quote(deck)}} {{args}}

# Review the fixture snapshots after a change of a verdict, a number, or a wording, and accept them.
snapshots *args: trust
    mise x -- cargo insta test --package sivi --test snapshots --review {{args}}

# Render every showcase fixture to PNG pages under `target/gallery/`, one file per slide, for a look at each theme.
gallery: trust
    mise x -- cargo nextest run --package sivi --test gallery --run-ignored only --no-capture
