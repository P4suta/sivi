# Architecture

sivi compiles a Markdown deck to a PDF through the Typst compiler, driven in process.
Markdown carries content only.
Every visual decision lives in a theme whose slide masters declare typed slots.
When content does not fit, the compiler never shrinks or splits it; it reports a diagnostic with the source span, the measured numbers, and a computed change that would make it fit.

## Pipeline

```text
deck text ─ scan + parse ─> Deck ─ deck and front matter rules ─> Slides
          ─ binder, slot and master rules ─> BoundDeck
          ─ emit::typst_source(&Theme, &BoundDeck) ─> (TypstSource, Expected)
          ─ sivi_typst::compile_and_measure ─> Measurement
          ─ report::diagnose(&Theme, &BoundDeck, &Measurement) ─> Report
          ─ (no errors) sivi_typst::pdf ─> PdfBytes ─ publish ─> deck.pdf
```

Every arrow without a crate name is a pure function of `sivi-core`.
The adapters read files, run Typst, and write the PDF; they turn every fact into a value before the core sees it.
`Expected` lists the measurement markers the adapter must read back, so a missing marker is an error and never a silent "fits".

## Crates

| Crate | Depends on | Role |
| --- | --- | --- |
| `sivi-core` | nothing | Values in, values out. `#![no_std]`, no dependencies, the panic lints denied. |
| `sivi-markdown` | `sivi-core`, `comrak` | Deck scanning, CommonMark parsing, source spans, refusal of forbidden constructs. |
| `sivi-theme` | `sivi-core`, `toml`, `serde`, `ttf-parser` | Theme files into validated masters, with font metrics, and the catalog of installable families. |
| `sivi-typst` | `sivi-core`, `typst` | The Typst `World`, bundled fonts, compilation, measurement read-back, frame walk, PDF export. |
| `sivi` | all of the above, `clap`, `annotate-snippets`, `serde_json`, `sha2`, `ureq` | The command line, file reads, diagnostics rendering, atomic publication, and the font library with its one network owner. |
| `xtask` | `sivi-core`, `sivi-markdown`, `sivi-theme` | The gates and the generators, among them the font catalog. |

Each effect has one owner in `sivi`: `io.rs` reads files, `publish.rs` writes them, `output.rs` reaches the streams, and `net.rs` reaches the network for `sivi fonts install` alone.
A build never touches the network; a family the library lacks is a diagnostic that names the installing command.

`cargo xtask structure` refuses a dependency of the core, a `std` path in the core, a weakened deny list, a rule module outside the core, a protocol step outside `binder.rs`, and a dependency against the direction above.

## Gates

| Command | Runs |
| --- | --- |
| `cargo xtask check` | rustfmt, taplo, typos, clippy with warnings denied, `structure`, the decision records, the generated repository files and the diagnostics catalog, actionlint and zizmor, nextest, cargo-deny, cargo-audit |
| `cargo xtask check --staged` | The quick subset over the staged files |
| `cargo xtask proofs` | Every verifier of `verification/obligations.toml` with its rejecting probe: Kani, Verus, Z3, Alloy, Quint with the trace replay, the Prolog reference, TLC over the publication model, Miri over the pure crates, every fuzz corpus replay, and the mutation campaign over the pure crates; each verifier also refuses its fixture under `xtask/proofs/` |
| `cargo xtask push-check` | `check` and then `proofs` |

The machine-wide hooks of the owner's dotfiles run first on every commit and push: the assurance shapes, the structural catalog, the comment and prose checks.
This repository's `lefthook.yml` adds only its own gates.
