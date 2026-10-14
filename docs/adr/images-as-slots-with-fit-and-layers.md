---
status: accepted
date: 2026-10-14
---

# Images as slots with a fit and a layer

## Context

A slide with a photograph wants three things a plain image slot lacks: a picture that fills its box edge to edge, a box with rounded corners under a soft shadow, and a photograph behind the whole slide with the title set over it.
The deck format carries only the image and its alt text, so every one of those is a choice the theme makes.
A picture behind the title overlaps the title's region, which the geometry rule refuses, and its laid-out items would be attributed to the title's region, which the report would read as an overflow.
Text over a photograph reads well or badly with the photograph, which the theme never sees.

## Decision

A slot carries a `layer`: `front` with the content, or `back` as a backdrop the other slots may cover.
A backdrop is a flow slot that accepts exactly one image and holds exactly one block, which `theme_rules::backdrop_fault` decides, so the content over it never measures against it.
The geometry rule refuses two overlapping front regions alone, so a backdrop may lie under anything.
The emitter places a backdrop without a marker and records its box among the expected backdrops; the report leaves every image and shape inside such a box to the backdrop, by `attribute_rules::covered`, while the text over it still answers to its own region.

A slot carries an `image` style for an image that fills it alone: a `fit` of `contain` or `cover`, a corner `radius`, a `shadow`, and a `scrim`.
The emitter builds the box as a clipped, rounded block the size of the region, less the shadow's offset, with the image at the box's full size, and lays the scrim as a linear gradient over the image inside the same clip.
The shadow is three translucent bands of the shadow's color, each a third of the way out and a third as opaque, since Typst draws no blur, and the box gives up the offset so the shadow stays inside the region.
An image in a stack with other blocks keeps the region's width and its own height, as before.

The design lints read a text style on what lies under its slot: on the paper where nothing does, and on the scrim where a backdrop with a scrim does, at the strength the gradient has where the slot ends toward the clear edge, over a mid gray, which is the lightest image the rule assumes.
A slot over a backdrop whose image has no scrim is a warning, since no rule can hold its text readable on an unknown image.

## Assurance boundary

Kani proves the layered geometry rule over three regions in either layer, the backdrop rule over every binding, every set of kinds, and every capacity, the cover rule over three backdrops and every item, and that a channel over another lies between the two.
Alloy holds that a point lies in the interior of at most one front slot under the validators, with a backdrop under a front slot in the populated instance, and refutes the claim without the overlap validator.
The scrim's strength along its gradient is held by tests, since its division lies beyond the solver's budget for wide integers.
That a covering image reaches past its box as an item under a group whose clip is the box, so the walker reads it as the box it shows through, and that a gradient fill reaches the frame as a gradient paint, are assumptions of the Typst boundary, each kept by a boundary test.

## Consequences

A theme can carry a hero master, a picture beside the words, and a grid of pictures, with no new deck syntax.
The resolution warning measures an image at its region's width, so a covering image that Typst scales wider may print coarser than the warning says.
A `contain` fit leaves the box's margins clear, so a radius or a shadow on a letterboxed image frames the box and not the picture; a theme that wants a framed picture sets `cover`.
A scrim cannot promise readability on a photograph lighter than a mid gray; a theme for light photographs deepens the scrim or starts it nearer the text.
