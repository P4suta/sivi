---
status: accepted
date: 2026-10-13
---
# Leave line breaking to Typst's Unicode line breaking and join soft breaks between East Asian characters

## Context

Japanese text wraps anywhere between characters, with the rules of JIS X 4051 keeping closing punctuation and small kana off the start of a line.
Typst 0.15.1 breaks lines with the Unicode line breaking algorithm through ICU4X, which keeps those basic rules and lacks the hanging punctuation and table rules of JIS X 4051.
A soft line break in Markdown source becomes a space under CommonMark, which is wrong between two Japanese characters.

## Decision

The compiler leaves line breaking inside a paragraph to Typst and never inserts a break of its own.
A soft break between two East Asian characters vanishes, and between any other pair becomes a space, as `softbreak_rules::joiner` decides for every pair of characters.
The deck's `lang` names the language Typst lays out with, and a deck with East Asian text and no East Asian language reports `W0107`.

## Assurance boundary

Kani proves the joiner over every pair of characters.
The fixture `pass-japanese` keeps a paragraph that wraps, and its geometry dump is checked against the line-start rules of JIS X 4051 once the layout gate exists.

## Alternatives

Breaking lines in the compiler would duplicate the shaping engine's work and disagree with it.
Keeping every soft break as a space would put spaces inside Japanese sentences.

## Consequences

Hanging punctuation and the full JIS X 4051 rules wait on Typst.
