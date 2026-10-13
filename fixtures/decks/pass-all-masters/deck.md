---
theme: plain
default_master: bullets
title: Every master
author: sivi
date: 2026-10-13
lang: en
---
master: title
id: cover
---
# Every master of the plain theme

A deck that passes through each master once

---
master: section
---
# Section one

---
# Bullets

- The first point
- The second point with *emphasis* and `code`
  - A nested point
- A [link](https://example.test/) and a link to [the end](#end)

---
master: comparison
---
# Comparison

## Left

- One
- Two

## Right

- Three
- Four

---
master: figure
---
# Figure

![A blue square](assets/figure.svg)

The caption under the figure

---
master: quote
---
> A quote with **strong** words and a soft
> break inside

The attribution

---
master: code
---
# Code

```rust
fn main() {
    println!("hello");
}
```

---
master: blank
id: end
---
| Column | Value |
| --- | --- |
| a | 1 |
| b | 2 |
