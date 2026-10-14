# The carried themes

The binary carries four themes, which a deck names under `theme:` or the command line names with `--theme`, and `sivi theme list` prints with their masters.
Every carried theme draws on the bundled fonts alone, so a deck builds offline, and every one holds the same fifteen masters under the same names, so a deck moves between the themes without a change.

| Theme | Faces | Paper and ink | Character |
| --- | --- | --- | --- |
| `plain` | BIZ UDPGothic, JetBrains Mono | white, near-black, a blue accent | The default: quiet, legible, and ready for Japanese and English alike |
| `blank` | BIZ UDPGothic, JetBrains Mono | white, black | A white page and black ink with nothing drawn that the deck did not write: no page numbers, no fields, no rules |
| `editorial` | Shippori Mincho, Playfair Display, JetBrains Mono | warm paper, warm ink, a vermilion accent, a yellow marker | A magazine's page: serifs, wide leading, thin rules, and `gruvbox-light-soft` for code |
| `studio` | Zen Kaku Gothic New, Montserrat, JetBrains Mono | deep navy, near-white, an electric mint accent, a violet glow | A dark stage: heavy sans display, numbers set huge, a violet shadow under every picture, and `tokyo-night-dark` for code |

## The fifteen masters

| Master | Binds | Holds |
| --- | --- | --- |
| `title` | the title and a subtitle | the author and the date beneath |
| `section` | the title | a divider between chapters |
| `bullets` | the title and a body | paragraphs, lists, and tables |
| `comparison` | the title and two sections | two columns, each a `##` |
| `figure` | the title, an image, and a caption | one picture with a line beneath |
| `quote` | a quote and an attribution | one quotation |
| `code` | the title and a code block | one block of up to fourteen lines |
| `blank` | a body | anything, over the whole page |
| `hero` | an image, the title, and a lede | a photograph behind the slide under a scrim, with the title over it |
| `image-left` | the title, an image, and a body | a picture on the left that covers its box, the words on the right |
| `image-right` | the title, a body, and an image | the words on the left, a picture on the right |
| `image-full` | an image and a caption | a photograph over the whole slide with one line at the bottom and no title |
| `grid-2` | the title and two images | two pictures side by side |
| `grid-3` | the title and three images | three pictures in a row |
| `statement` | one paragraph | one sentence or one number, set large and centered |

A slide names its master with `master:`, or takes the deck's `default_master`.
The masters that hold pictures take `![alt](path)` blocks in the order the slots are declared, so a `grid-3` slide lists its three pictures from left to right.

## Choosing and changing

`sivi theme check <NAME>` validates a carried theme by name, and `sivi theme check <DIR>` a theme of your own.
A theme of your own starts best as a copy of a carried one from `themes/` in the source tree, with its `[colors]` grown from a seed by `sivi palette`, its `[code] theme` chosen from `sivi code-themes`, and its faces chosen from `sivi fonts list` and installed by `sivi fonts install`.
The design rules hold every theme alike: text reads at four and a half to one on what lies under it, the colors form at most three hue families, the paper stays near neutral, and a slot over a photograph has a scrim.

## A gallery

`just gallery` renders every showcase fixture under `fixtures/decks/` to PNG files under `target/gallery/`, one per page, so each theme can be seen slide by slide.
