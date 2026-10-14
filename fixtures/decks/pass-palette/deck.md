---
theme: themes/seeded
default_master: bullets
title: A seeded palette
lang: ja
---
master: title
---
# 一つの色から育つパレット

seed `#0b57d0`、complementary

---
# 役割ごとの色

- 本文は ink、補足は *muted*、リンクは [accent](https://example.test/)
- ==マーカー== は補色側の明るい帯
- **太字** と `code` はそのまま読める

---
master: code
---
# コードは code_bg の上に

```rust
fn grow(seed: Color) -> Palette {
    palette(Seed::of(seed), Mode::Light, Harmony::Complementary)
}
```
