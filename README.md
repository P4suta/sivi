# Sivi

A Markdown to PDF slide compiler that refuses any slide its master cannot hold.

## Installation

```sh
git clone https://github.com/P4suta/sivi.git
cd sivi
cargo install --locked --path crates/sivi
```

## Usage

```sh
sivi build deck.md
sivi check deck.md --format json
sivi theme check themes/plain
```

## License

Licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or the [Massachusetts Institute of Technology (MIT) license](LICENSE-MIT), at your option.
You license each contribution you submit under the same two licenses, without other terms, unless you state otherwise.
