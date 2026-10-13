---
status: accepted
date: 2026-10-13
---
# Represent every length in the core as a whole number of millipoints

## Context

Typst measures in floating-point points.
A floating-point comparison can differ between hosts by rounding, and a proof over floating point needs a verifier that models it.
The core must decide whether content fits with the same answer on every machine, and the decision must be provable.

## Decision

The core's `Length` is a signed 64-bit count of millipoints, one thousandth of a point.
Every arithmetic operation on it is checked or saturating, so no rule can overflow or panic.
The tolerance `EPSILON` is ten millipoints, one hundredth of a point, which absorbs the rounding of a measurement to the nearest millipoint while staying far below any visible difference.
The adapter converts a Typst length once, in `sivi-typst::convert`, after checking that it is finite and within a billion points, and that is the one `as` conversion in the workspace.
A length prints as points with three decimals, such as `151.118pt`.

## Assurance boundary

`fit_rules` proves over the whole admitted domain that the verdict is `Fits` exactly when the excess is at most the tolerance.
The Z3 lemma in `specs/smt/millipoints.smt2` states, for the same constants, that the fixed-point verdict agrees with the real-valued one whenever the real excess is more than one millipoint away from the tolerance.

## Alternatives

Floating point in the core would need a verifier with floating-point semantics and would leave host-dependent rounding.
Rational numbers would be exact but need an allocation-free implementation the core would have to own.

## Consequences

A measurement is exact to one millipoint, and every rule is an integer rule.
The core cannot express sub-millipoint differences, which no layout needs.
