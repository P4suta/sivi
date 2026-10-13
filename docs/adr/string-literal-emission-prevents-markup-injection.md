---
status: accepted
date: 2026-10-13
---
# Emit author text only as Typst string literals so no deck can inject markup

## Context

The emitter turns a deck into Typst source.
If author text were written into Typst markup, a sentence containing `#`, `*`, `_`, `[`, or `<` would change the layout, call functions, or break the compile.
Escaping the whole markup grammar would mean tracking every construct Typst adds.

## Decision

Author text reaches the generated source only inside Typst string literals in code mode, such as `#"text"`, `#emph[#"text"]`, or `#raw("text", block: true)`.
`escape_rules::string_literal` writes the literal: it escapes the backslash and the double quote, writes newline, carriage return, and tab by name, and writes every other control character as a `\u{..}` escape.
Nothing else is escaped, because inside a string literal nothing else has meaning.

## Assurance boundary

`escape_rules` proves for every text of up to three bytes that the literal is delimited, holds no raw control character, and decodes back to the text; the fuzz target `typst_string` replays the same round trip on longer inputs.
The boundary test `a_family_outside_the_bundle_is_a_warning_the_adapter_surfaces` and the deck fixtures show the compiler reads the literals as intended.

## Alternatives

Escaping markup characters one by one leaves a grammar the emitter must track with every Typst release.
Typst's `raw` function alone would lose emphasis and links.

## Consequences

The escape rule is small enough to prove, and a Typst grammar change cannot turn text into markup.
Emphasis, links, and code spans are built by calling Typst functions on the literals rather than by writing markup.
