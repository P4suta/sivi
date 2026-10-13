---
theme: plain
default_master: code
title: Wide code
lang: en
---
# A line too wide

```rust
fn main() {
    let configuration = Configuration::builder().with_region("eu-west-1").with_timeout(Duration::from_secs(30)).with_retries(3).build();
}
```
