---
status: accepted
date: 2026-10-14
---

# Code themes from base16 schemes

## Context

A code block on a slide reads as the editor theme its audience knows, and presenters ask for the famous ones by name: ayu, Dracula, Catppuccin, Gruvbox, Nord, Solarized, Tokyo Night, and dozens more, each in a dark and a light variant.
Typst highlights code through syntect and takes any theme in the `tmTheme` format, but ships one theme of its own.
Writing a `tmTheme` file per editor theme by hand would drift from the sources and would carry each file's own scope rules.
The tinted-theming schemes collection holds every one of those themes as a base16 palette of sixteen colors with fixed roles, under one license, at a commit the repository can pin.

## Decision

The binary carries every base16 scheme of tinted-theming/schemes at one pinned commit, in `themes/code/schemes.toml`, with the collection's license beside it.
`cargo xtask code-themes fetch` reads the repository's tree and every scheme at that commit, keeps each scheme's slug, name, author, variant, and palette, and writes the file; `cargo xtask code-themes verify` holds the file to its rules on every check without the network.

A theme names a scheme under `[code] theme` by its slug.
The core turns the palette into a `tmTheme` file through one fixed table of scope rules that follows the base16 styling guide, and the emitter hands the file to the compiler's world as a generated file beside the deck's assets.
With a scheme, the scheme's background and foreground replace the theme's code background and text color, since a palette only reads as its editor theme on its own background.
`sivi code-themes` lists every scheme with its variant, with the palette as swatches on a color terminal, and as JSON for a program.
A slug the binary lacks is `E0512`, which names the nearest slug.

## Assurance boundary

The generator refuses a scheme file without a name, a variant of dark or light, or sixteen colors, and the verify task refuses a schemes file out of slug order, with a slug outside its grammar, or whose license digest differs.
The core's theme generator is a pure function with tests over a counting palette, so every role reaches its own color and every name is escaped.
That syntect reads the generated theme and paints a keyword with the scheme's keyword color is an assumption of the Typst boundary, which a boundary test keeps over every carried scheme.

## Consequences

Three hundred and more schemes cost about one hundred and forty kilobytes of text in the binary and no font or image.
A new scheme or a fix upstream reaches the binary by moving the pinned commit and running the fetch task, which the check then holds to its rules.
The scope table is the one place a highlighting choice lives; a scheme cannot refine it, which keeps every scheme consistent and rules out per-theme scope rules.
Code set in a light scheme on a dark theme keeps the scheme's light background, which is the scheme author's intent.
