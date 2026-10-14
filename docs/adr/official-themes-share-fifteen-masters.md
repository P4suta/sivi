---
status: accepted
date: 2026-10-14
---

# Official themes share fifteen masters

## Context

The binary carried one theme, `plain`, with eight masters, none of which placed a photograph behind a slide or a picture beside the words.
A deck written for one theme should move to another without a change, which holds only when every carried theme answers to the same master names with the same grammar.
The theme loader refused two masters whose slots bound alike, which forbade a picture on the left and a picture on the right as two masters.

## Decision

The binary carries four themes: `plain`, `blank`, `editorial`, and `studio`, each drawing on the bundled fonts alone.
Every carried theme holds the same fifteen masters under the same names, with the same bindings and capacities: the eight of before, and `hero`, `image-left`, `image-right`, `image-full`, `grid-2`, `grid-3`, and `statement`.
A theme sets the faces, the colors, the leading, the decor, and the image styles; a deck never does.

Two masters are one master under two names only when their slots, in order, and their decor are the same, so masters that bind alike but lay out apart are distinct.

`sivi theme list` prints the carried themes with their character and their masters, and a build names the carried themes when a deck asks for one the binary lacks.
`just gallery` renders every showcase fixture to PNG pages through the binary's own renderer, so a theme is reviewed slide by slide, and the same renderer will serve a preview on a terminal.

## Assurance boundary

A test loads every carried theme, holds it to the fifteen master names in order, and holds it to the design rules with no finding.
A fixture deck for every carried theme uses each master once and builds without an error to fifteen pages, and the plain fixture builds to the same bytes twice.
The faces each theme names are bundled, which the font manifest pins by digest and the loader checks for every weight a style asks for.

## Consequences

A deck written for `plain` builds under `editorial` or `studio` as it stands.
A theme of one's own that wants the same portability keeps the fifteen names and their grammar.
A master that binds like another is no longer refused for that alone, so a theme may carry mirrored layouts.
The binary grows by the renderer and its glyph rasterizer, which the terminal preview will need as well.
