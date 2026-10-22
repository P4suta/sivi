# Engineering rules

Every rule here removes a class of bugs.
Each rule is enforced by the compiler, a lint, a gate, a test, or a hook, so a change cannot quietly break it.
When a new class of bug appears, the fix adds a rule to this list, not only a patch.

| Rule | Bug class it removes | Enforced by |
| --- | --- | --- |
| Every decision lives in `sivi-core`, which reaches no dependency, no standard library, and no effect | A decision that depends on a file, a clock, or an environment, which no proof can cover | `cargo xtask structure`; the workspace lint policy; the machine-wide structural catalog |
| A public core function returns a named type, never bare text | Text that a caller parses back, with two spellings of one fact | The machine-wide `core-bare-text` rule; review |
| Lengths in the core are integer millipoints, converted once at the Typst boundary with a range check | Float comparisons that differ by host, and silent truncation | `Length` in `sivi-core`; the single `#[expect(clippy::as_conversions)]` in `sivi-typst` |
| Every `*_rules.rs` module has Kani harnesses and a `rejects_*` probe that fails on a named assertion | A proof that passes because it proves nothing | `cargo xtask structure`; `cargo xtask proofs`, which requires each probe to fail |
| The binary reaches the network in `net.rs` alone, for `sivi fonts install`, and checks every fetched file against the digest the catalog pins | A build whose output depends on what a server answered today | The lint policy, which disallows every other `ureq` entry point; `cargo xtask fonts verify` |
| The obligation manifest matches the harnesses the rule modules define | A harness added or removed without its contract and bound being stated | `cargo xtask proofs` |
| Each verifier of the portfolio owns one class of property | Two proofs of one property, which diverge and double the cost | `verification/obligations.toml`; the verification portfolio record |
| Every verifier is shown to refuse a false claim before its result counts | A tool error or an empty run read as a pass | The fixtures under `xtask/proofs/` |
| A parser has a fuzz target with a committed corpus, replayed at the release gate and on demand | A crash on a shape no test wrote | The machine-wide parser shape; `cargo xtask proofs` |
| Each effect has one owner, and only its private `raw` module calls the reserved API | Effects scattered where no policy applies | Clippy `disallowed-methods`, `disallowed-macros`, `disallowed-types` |
| Errors are matched, never turned into absence | A failure mistaken for "nothing there" | `disallowed-methods` on `Result::ok`, `unwrap_or`, `Path::exists`, and similar |
| Every match is exhaustive | A new variant silently taking a default path | `clippy::wildcard_enum_match_arm` |
| No decision reads the wall clock | Output that differs by the day it was built | `disallowed-methods` on `SystemTime::now` and `Instant::now`; the PDF date comes from `SOURCE_DATE_EPOCH` |
| Standard output carries only the PDF bytes or the JSON document | A stray line corrupting the document a caller reads | `disallowed-macros` on `print!` and `println!`; `disallowed-methods` on `io::stdout` |
| A number, URL, or path with a meaning gets a named constant | A magic value whose meaning drifts | The machine-wide `magic-number` and `hard-coded-location` rules |
| Lint exceptions use `#[expect]` with a reason | Suppressions that outlive their cause | `allow_attributes = "deny"`; `unfulfilled_lint_expectations = "deny"` |
| A fix lands with a test that fails without it | The same bug returning in a rewrite | The `commit-msg` hook (`cargo xtask commit-msg`) |
| A durable decision is a dated record named by its decision | A decision that lives only in a conversation, and numbered records that collide between branches | `cargo xtask check` through the record gate |
| Each test runs in its own process | Shared state between tests that passes only in one order | `cargo nextest` in every gate |
| Every diagnostic ships with a fixture deck or theme that earns it, and every fixture's output is a snapshot | A verdict, a number, or a wording that changes unnoticed | `crates/sivi/tests/snapshots.rs` and `crates/sivi/tests/geometry.rs`, reviewed with `just snapshots` |
| The diagnostics catalog is generated from the enum of codes | A code documented in one place and defined in another | `cargo xtask gen --check` inside `cargo xtask check` |

## Effects

An effect is anything the program does outside its own memory: files, processes, the clock, standard output.
The lints forbid the standard library's entry points for each effect.
The one module that owns an effect wraps those entry points in a private module named `raw`, whose functions forward in a single statement and whose first attribute expects the reserved lints.
Everything outside `raw`, including the rest of the owner, stays fully linted.

| Effect | Owner |
| --- | --- |
| Reading deck, theme, font, and image files | `sivi::io` |
| Writing the PDF | `sivi::publish` |
| Standard output | `sivi::output` |
| Reading and writing files in the gates | `xtask::fs` |
| Child processes in the gates | `xtask::tool` |

## Verification

- `just check` runs the lint, structure, record, test, and dependency gates.
- `just push-check` runs only that fast gate on the current host; pull-request CI runs it on one pinned Linux runner.
- The manually dispatched release workflow runs `just release-check`, which adds the full proof inventory, Miri, fuzz corpus replays, and mutation testing, and adds native macOS and Windows tests.
- `just proofs`, `just mutants`, and `just fuzz` remain on-demand commands, and `just proofs-ci` is a reduced on-demand inventory without mutation testing.
- Heavy local commands run through `just bounded IMAGE COMMAND...` under the owner's container resource policy, as [Contributing](../CONTRIBUTING.md) describes.
- The generated repository files are checked with the owner's `dotfiles repository check`; a host without that tool notes its absence instead of claiming the check.
