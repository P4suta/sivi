# The Typst boundary

Typst lays out the slides, so its behavior is an external semantic boundary of sivi.
No proof covers it.
Instead, every assumption sivi makes about Typst is written down here and re-checked by a test in `crates/sivi-typst/tests/boundary.rs` on every run, against the pinned `typst 0.15.1`.
A Typst upgrade that breaks an assumption fails that test before it can change a verdict.

| Assumption | Test |
| --- | --- |
| `measure(width: W, body)` inside a placed block returns the height the content needs at width W, and measuring each prefix of the parts gives cumulative heights that never decrease and end at the full height | `measure_at_the_region_width_reports_the_required_height_and_rising_prefixes` |
| Japanese text without spaces wraps at the region width, and the bundled fonts show every character of it | `japanese_text_wraps_at_the_region_width_without_spaces` |
| Content placed with `place` never adds a page, however far it overflows; the overflow shows only as items whose boxes lie below the page | `a_placed_region_that_overflows_the_page_never_adds_a_page` |
| A `metadata` marker's location reports the page it landed on, and consecutive slides separated by `pagebreak()` land on consecutive pages | `each_slide_marker_lands_on_its_own_page` |
| Walking the frames with the group transforms accumulated puts every text box of a placed region inside that region when the text edges are `bounds` | `text_leaves_land_inside_the_region_that_placed_them` |
| A code line that cannot break keeps its natural width and its text box leaves the region to the right, which `measure` does not report because it clamps the width | `an_unbreakable_code_line_leaves_the_region_to_the_right` |
| A character no bundled font covers is shaped with glyph 0, which the frame walk reports as a missing glyph | `a_character_no_bundled_font_has_is_reported_as_a_missing_glyph` |
| A font family outside the bundle compiles with an `unknown font family` warning, which the adapter surfaces instead of hiding | `a_family_outside_the_bundle_is_a_warning_the_adapter_surfaces` |
| A marker the emitter promised but the document lacks, or holds twice, is an error and never a verdict | `a_missing_marker_is_an_error_and_never_a_verdict` |
| Two exports of the same document with the same identity and timestamp are byte-identical, and the timestamp is the only date in the PDF | `the_pdf_bytes_depend_on_the_inputs_alone` |
| A `raw` element sets its own font, which no bundled file provides, so only a `show raw: set text(font: ...)` rule gives it a bundled font; with `fallback: false` a plain `set text(font: ...)` leaves it without glyphs | `raw_takes_its_font_only_from_a_show_set_rule` |
| A `heading` element adds its own block spacing and text size, which `show heading: set ...` rules remove, so a title measures as its lines alone | `a_heading_under_show_set_rules_measures_as_its_lines` |
| A variable font sets the weight the text asks for along its axis, so one file serves every weight a theme sets | `a_variable_font_sets_the_weight_the_text_asks_for` |
| A font list falls through per glyph to the first family that has the glyph, with `fallback: false` keeping the host's fonts out | `a_font_stack_falls_through_to_the_family_that_has_the_glyph` |
| A `highlight` fill and an `underline` stroke with an alpha reach the frame as solid paints with that alpha, and as background decorations they come before the text they decorate | `a_translucent_decoration_keeps_its_alpha_and_lies_behind_the_text` |

## What the adapter trusts and what it checks

- The adapter trusts Typst's line breaking, shaping, and the `measure` function.
- The adapter checks every marker it reads: a missing or duplicated marker, a value of the wrong type, a length with an `em` part, a cumulative height that decreases, and a length beyond the core's range each become a `ReadBackError`.
- The adapter never reads a clock: `World::today` returns `None`, so a theme that asks for the date fails to compile.
- The adapter never reads the host's fonts: the font book holds the bundled fonts and the theme's files alone.

## Known limits of the pinned Typst

- Line breaking follows UAX #14 through ICU4X; JIS X 4051 tables and hanging punctuation are not implemented.
- A newline between two CJK characters in Typst markup becomes a space in 0.15.1, so the emitter joins soft breaks itself and never relies on markup newlines.
- `measure` clamps the width to the region, so horizontal overflow is detected only by the frame walk.
