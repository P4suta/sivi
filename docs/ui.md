# The terminal interface

`sivi ui deck.md` takes the terminal, builds the deck, and shows it: a header with the theme and the last verdict, the slides on the left with a mark for each, the selected slide's page on the right, the diagnostics of that slide beneath, and the keys at the foot.
`sivi ui` alone opens the one `.md` file of the working directory.
The interface writes the PDF beside the deck on every build, as `sivi build` does, and never touches the deck or the theme.

## Keys

| Key | What it asks |
| --- | --- |
| `q`, `ctrl-c` | quit |
| `b` | build the deck and write the PDF beside it |
| `w` | watch the deck's directory and build on every change of a deck, a theme, a picture, or a font |
| `j`, `k`, `↓`, `↑` | move among the slides, or among the entries of a picker |
| `1`, `2`, `3` | the overview, the slides with a larger preview, the diagnostics |
| `t` | pick a carried theme, which builds the deck under it |
| `f` | browse the font catalog; enter installs the family into the library |
| `p` | browse the carried palettes with their swatches, with the `[palette]` text to paste |
| `c` | browse the code themes with their swatches, with the `[code]` text to paste |
| `enter` | choose the entry |
| `esc` | leave the picker or the help |
| `o` | open the PDF in the host's viewer |
| `?` | the keys |

A key for a build while one runs leaves it alone and says so; a change that arrives during a build starts the next one once it ends.
Quitting is one key away from every screen, and nothing runs after it but the build that was already running, which writes its PDF and ends.

## The preview

When the terminal is taken, the interface asks it whether it speaks the kitty graphics protocol, as kitty, Ghostty, and WezTerm do, and shows the page as a picture through it when the terminal says so, whatever the variables of the shell say.
A terminal that does not answer in kind but whose `TERM_PROGRAM` or `LC_TERMINAL` names iTerm2 shows the page through its inline images.
Every other terminal, and a multiplexer such as tmux that passes no picture through, shows the page as a mosaic in true color: two picture columns and two picture rows to a cell, each cell split over the two colors closest to its four quarters behind a quadrant glyph, which keeps the slide's aspect.
A page is rendered when it is first shown and kept until the next build, and a kitty terminal receives it once per build and per frame size, so a page shown again appears at once.

## The machine behind it

Every key, build result, file change, and tick goes through `ui_rules::step` in the core, which decides the screen, the selection, whether a build runs, and which effect the shell carries out: a build, the viewer, an installation, or a note.
The shell owns the terminal, the watcher, the build thread, and the viewer, and does nothing the machine did not ask.
`specs/ui.qnt` models the machine, the model checker holds that quitting is always one key away, that one build runs at a time, and that every selection stays within its count, and the committed traces replay through the core's step.
