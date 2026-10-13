# The deck format

A deck is one UTF-8 file, `deck.md`, of at most four mebibytes and at most one thousand slides.
It holds content only; every look comes from the theme.

## Grammar

```text
deck        = [headmatter] slide *( separator slide )
headmatter  = fence NL *kvline fence NL        ; only at the very start of the file
fence       = "---" *SP                        ; a whole line
separator   = fence NL                         ; outside a fenced code block
slide       = [frontmatter] *line
frontmatter = 1*kvline fence NL                ; right after the head or a separator
kvline      = key ":" 1*SP value *SP NL
key         = 1*( %x61-7A / "_" )
value       = 1*VCHAR *( 1*SP 1*VCHAR )
```

A line that is exactly `---` with optional trailing blanks is a fence.
A line with up to three leading spaces and three or more backticks or tildes opens a fenced code block, and a fence inside it is content.
A `---` that follows a text line without a blank line reports `E0204`, since CommonMark would read a heading underline.
A deck whose first line is `---` opens its head there, and the head holds only `key: value` lines until its closing fence.
A slide starts with front matter when `key: value` lines follow the head's fence or a separator and a fence closes them; lines that start with a slide key and never close report `E0102`.
Front matter lines are replaced by spaces of the same width before the slide is parsed, so every position the parser reports is a position of the deck.

## Keys

| Where | Key | Value |
| --- | --- | --- |
| head | `theme` | `plain`, or a relative path with a slash to a theme directory |
| head | `default_master` | an identifier: a lowercase letter, then up to thirty-one lowercase letters, digits, or dashes |
| head | `title`, `author` | one to two hundred characters |
| head | `date` | `YYYY-MM-DD` |
| head | `lang` | `[a-z]{2,3}(-[A-Z]{2})?`; `ja`, `zh`, and `ko` lay text out as East Asian |
| slide | `master` | an identifier; required unless the head sets `default_master` |
| slide | `id` | an identifier, unique in the deck, which `[text](#id)` links reach |

An unknown key or a key in the wrong place reports `E0104`, a value outside its grammar `E0105`, a repeated key `E0106`, and a repeated slide id `E0107`.

## Content

| Markdown | Reading | Refusal |
| --- | --- | --- |
| `#` | the slide's title, as the first block only | `E0307` when elsewhere |
| `##` | opens a section | `E0305` when the master holds fewer |
| `###` and deeper, `===` or `---` underlines, empty headings or items | refused | `E0205`, `E0204`, `E0220` |
| paragraph | inlines; a soft break is a space, or nothing between East Asian characters | |
| `*em*` `**strong**` `` `code` `` `~~gone~~` | inline marks the theme styles | |
| `[text](dest)` | `https:`, `http:`, `mailto:`, or `#slide-id` | `E0210`, `E0308` for an unknown id |
| `[text][ref]` without a definition | refused | `E0221` |
| a backslash or two spaces at a line end | refused | `E0214` |
| `-` and `1.` lists | items of one paragraph and an optional nested list; nesting to the slot's depth | `E0218`, `E0306` |
| `- [ ]` | refused | `E0207` |
| `![alt](path)` | alone in its paragraph; a relative path beside the deck ending in `.png`, `.jpg`, `.jpeg`, or `.svg` | `E0212`; `W0105` without alt |
| fenced code | the first word of the info string names the language; `notes` makes speaker notes | `E0215` for a second notes block; `E0402` when a line is too wide |
| indented code | refused | `E0209` |
| GFM table | a header row and rows with as many cells | `E0219` |
| `>` | one paragraph | `E0213` |
| footnotes, math | refused | `E0206`, `E0208` |
| raw HTML, inline HTML, comments | refused | `E0201`, `E0202`, `E0203` |
| `***`, `___`, `- - -` | refused: not a separator | `E0216` |
