---
status: accepted
date: 2026-10-14
---
# Offer every Google Fonts family through a catalog pinned by digest, installed on request

## Context

A theme chooses its faces, and people who care about slides care about faces.
Bundling every family people reach for would make the binary gigabytes large, and reading the host's fonts would make the PDF depend on the host.
Typst 0.15.1 selects the weight of a variable font along its axis, so one file of a family serves every weight the theme sets.
Typst lists a family under its plain family record with the style words trimmed, so a theme must name a family exactly as Typst does.

## Decision

`fonts/families.toml` lists the families the catalog carries: every family of Google Fonts that covers Japanese, the Korean and Chinese Noto families, and a curated range of Latin families.
`cargo xtask fonts catalog` fetches each family from google/fonts at one pinned commit and writes `fonts/catalog.toml`: the directory, the category, the writing systems, the license, and for every file its digest, its size, whether it is italic, and the weights it provides.
The catalog names a family as Typst lists it, which the generator reads from the file with Typst's own trimming rule; a file that Typst would list under another name is set aside.
The binary carries the catalog.
`sivi fonts list` shows every family with its state, `sivi fonts install` fetches the files of a family at the pinned commit into the library and refuses any file whose digest differs, and `sivi fonts dir` names the library.
The library lives under `SIVI_HOME`, else under `LOCALAPPDATA` on Windows, else under `XDG_DATA_HOME` or `HOME/.local/share`, as `home_rules` decides from the environment alone.
A theme sets text in any bundled family, any family of its own `[[fonts]]` files, or any family of the catalog, by name, and a text style may list several families so that each glyph falls through to the first family that has it.
A build never reaches the network: a catalog family that is not installed is the error `E0610`, which names the command that installs it, and a family neither bundled, declared, nor cataloged is the error `E0609`, which names the closest known family.
The official themes set text in bundled families alone, so a deck under an official theme builds offline.
The bundled families grow to BIZ UDPGothic, JetBrains Mono, Shippori Mincho, Zen Kaku Gothic New, Playfair Display, and Montserrat.

## Assurance boundary

The network owner `net.rs` is the one place the binary reaches the network, which the lint policy enforces for every other `ureq` entry point.
The digest check keeps an installed file identical to the cataloged one, so a family installed anywhere lays out identically.
`cargo xtask fonts verify` refuses a catalog that names a family the list lacks or lacks a listed family, and `fonts/catalog.toml` carries the commit the list pins.
The Typst boundary tests show that a variable font sets the weight the text asks for and that a font list falls through per glyph.
What stays outside: google/fonts may move or delete a file after the pinned commit, which the installer reports as a fetch failure rather than substituting another file.

## Consequences

The binary grows by about twenty-six megabytes of bundled fonts.
Changing the pinned commit or the family list regenerates the catalog, which takes one run of the generator with the network.
A theme that names a catalog family builds only where the family is installed, and says so with the exact command.
