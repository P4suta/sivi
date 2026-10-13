---
status: accepted
date: 2026-10-13
---
# Record each advisory inside the pinned Typst with its exposure and the condition that removes it

## Context

`cargo audit --deny warnings` refuses the lockfile of `typst 0.15.1`.
Two advisories name `quick-xml 0.38`, which `citationberg` uses to parse citation styles.
Six warnings name unmaintained crates: `bincode 1` and `yaml-rust` under `syntect`, `paste` under `biblatex`, `rustybuzz` and `ttf-parser` under the shaping and font stack, and `unsafe-libyaml` under `serde_yaml` inside `hayagriva`.
No release of Typst without them exists, and sivi pins Typst exactly.

## Decision

`.cargo/audit.toml` ignores each advisory by identifier, with the path through the dependency tree and the reason sivi is not exposed.
A deck carries no bibliography, so the citation stack never runs.
The syntax definitions `syntect` loads ship inside the binary, so no input reaches `bincode` or `yaml-rust`.
`paste` runs at build time only.
The fonts the shaping stack reads are the bundled ones and the ones a theme pins by digest.
The gate keeps `--deny warnings`, so any advisory outside this list still fails the check.
Every Typst upgrade re-reads this list: an entry whose crate left the tree is removed, and a new advisory is assessed before it is ignored.

## Alternatives

Dropping `--deny warnings` would hide every future unmaintained crate as well.
Patching the dependency tree would fork Typst.

## Consequences

The audit stays a required gate with an explicit, reasoned exception list.
A future advisory against a path sivi does feed, such as the SVG decoder or the font parser on theme fonts, is not covered by this record and fails the gate.
