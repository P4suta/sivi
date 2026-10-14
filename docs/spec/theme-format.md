# The theme format

A theme is one `theme.toml` of at most one mebibyte, with `schema = 1`.
Every key is closed: an unknown key reports `E0502` with its position.
Every value outside its grammar reports `E0503` with its path, such as `masters.bullets.slots.body.region.x`.

## Units and names

| Value | Grammar |
| --- | --- |
| length | a decimal with up to three digits after the point and `pt` |
| measure | a length, or a decimal and `em` for a multiple of the font size |
| position or extent | a length, or a decimal and `%` of the page side |
| ratio | a decimal number, such as `1.45` |
| color | `#rrggbb` |
| weight | one hundred to nine hundred |
| style name | a lowercase letter, then lowercase letters, digits, or underscores |
| family | a font family as Typst lists it: the plain family record of its files with the style words trimmed, such as `Shippori Mincho` |
| font list | a family, or a list of families; text is set in the first and each glyph it lacks falls through to the next |
| identifier | a lowercase letter, then up to thirty-one lowercase letters, digits, or dashes |

## Tables

| Table | Keys |
| --- | --- |
| `theme` | `name`, an identifier |
| `page` | `width`, `height`, `background`, a color name |
| `colors` | color names to colors, which a theme with a `palette` may leave out |
| `palette` | `seed`, a color, or `name`, a carried palette; `mode`, `light` or `dark`; `harmony`, one of `mono`, `analogous`, `complementary`, `split`, `triadic` |
| `typography` | `min_size`, `paragraph_gap`, `fallback`, a list of families |
| `text.<name>` | `font`, a font list, `size`, `weight`, `color`, `line_height`, `align` |
| `inline` | `emphasis`, `strong`, `code`, `link`, `mark`, each a mark |
| `lists` | `bullets`, `ordered`, `indent`, `gap` |
| `code` | `font`, `size`, `line_height`, `background`, `padding`, `highlight`, `tab_width`, `theme`, the slug of a carried scheme |
| `tables` | `text`, `header_weight`, `rule`, `padding` |
| `quotes` | `bar`, `bar_width`, `indent` |
| `fonts` | an array of `family`, `path`, `sha256`, the files the theme ships beside itself |
| `masters.<name>.slots.<slot>` | a slot |
| `masters.<name>.decor` | an array of decor |

A mark sets any of `style`, which is `italic` or `color`, `color`, `weight`, `font`, a font list, `background`, `marker`, and `underline`.
`mark` styles `==text==`, and every theme names it.

A `marker` is a translucent band behind the text: `color`, `opacity` from 0 for clear to 1 for solid, `top` among `ascender`, `cap-height`, `x-height`, `baseline`, and `bounds`, `bottom` among `descender`, `baseline`, and `bounds`, `extent` past the text on each side, and `radius`.
The band is solid, from the ascender to the descender, flush, and square unless a key says otherwise.
Typst paints the band once per shaped run, and a run ends where the script changes, as between kanji and kana, so an `extent` or a `radius` shows a seam there; a theme for Japanese text keeps both at zero.

An `underline` is `true` for the plain line in the text color, or a table of `color`, `opacity`, `thickness`, `offset` below the baseline, `extent`, and `behind`, which lays the line behind the glyphs and across their descenders.
An `opacity` needs a `color` beside it, since the plain line takes the text color.

A style sets text in a bundled family, a family of the theme's own `fonts`, or a family of the font catalog, which `sivi fonts list` shows and `sivi fonts install` places in the library.

A `palette` grows ten colors from one seed in the OKLCH space: `paper`, `ink`, `muted`, `rule`, `code_bg`, `accent`, `accent2`, `accent3`, `highlight`, and `soft`, each at a lightness and a chroma the mode sets, on hues the harmony sets, so the ink reads on the paper at seven to one, every accent at three to one, and the palette holds at most three hue families.
A color named under `colors` wins over the role of the same name.
`seed` is a color as `#rrggbb`; `name` is one of `ink`, `indigo`, `sakura`, `matcha`, `sunset`, `ocean`, `wine`, `sand`, `slate-night`, `neon-night`, `forest`, and `coral`, each carrying its own mode and harmony, which `mode` and `harmony` beside it replace.
Without a name, the mode is `light` and the harmony `complementary`.
`sivi palette` prints the colors a seed or a name grows, as this table, and lists the carried palettes.

Every theme is held to three design rules: a text color that reads below four and a half to one on the paper is `E0513`; more than three hue families among the colors is `W0503`; and a paper with a chroma above six hundredths is `W0504`.

`code.theme` names one of the base16 schemes the binary carries by its slug, such as `dracula` or `catppuccin-latte`, which `sivi code-themes` lists with their variants.
With a scheme, the scheme's background and foreground replace `code.background` and the color of the code's text style, and every token takes the scheme's color for its role.
A slug the binary lacks reports `E0512` with the nearest slug.

## Slots

| Key | Value |
| --- | --- |
| `binds` | `title`, `subtitle`, `section`, or `flow` |
| `section` | the number of the `##` the slot binds, from one; only with `binds = "section"` |
| `accepts` | the block kinds a flow or section slot holds: `paragraph`, `list`, `code`, `quote`, `image`, `table`, `heading` |
| `required` | whether a slide must fill the slot |
| `follows` | the slot whose content this slot's content must follow directly |
| `region` | `x`, `y`, `width`, `height` |
| `text` | a text style name |
| `valign` | `top`, `middle`, or `bottom` |
| `align` | `left`, `center`, or `right` |
| `max_blocks`, `max_items`, `max_depth`, `max_rows`, `max_columns`, `max_lines` | capacities, each a whole number |

Slots keep their declaration order, which the binder reads when more than one slot accepts a block.

## Decor

| `kind` | Keys |
| --- | --- |
| `rect` | `region`, `color` |
| `image` | `region`, `path` |
| `page_number` | `region`, `text`, `align`, `format` with `{n}` |
| `field` | `region`, `text`, `align`, `field` of `title`, `author`, or `date` |

## What the loader refuses

| Code | Fault |
| --- | --- |
| `E0501` | the file is not TOML |
| `E0502` | the schema is not one, or a key is unknown or missing |
| `E0503` | a value leaves its grammar |
| `E0504` | a region leaves the page |
| `E0505` | two slot regions overlap |
| `E0506` | a text size below `typography.min_size` |
| `E0507` | a reference to a color or text style the theme does not define |
| `E0508` | two masters with the same grammar, section numbers with a gap, a `follows` that leaves the master or loops, or two title slots |
| `E0509` | no master, or a master without a slot |
| `E0510` | a face the styles need that no font provides |
| `E0511` | a line height below its face's natural line box |
