//! The pure decision core of sivi.
//!
//! Every fact arrives as a value and every decision leaves as a value.
//! The crate reaches no file, process, network, clock, or environment, and `cargo xtask structure` refuses a dependency, a `std` path, or a weakened deny list.
#![no_std]
#![cfg_attr(verus_keep_ghost, feature(proc_macro_hygiene))]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::todo,
    clippy::unimplemented,
    clippy::unreachable,
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::exit,
    clippy::mem_forget
)]

extern crate alloc;

pub mod attribute_rules;
pub mod binder;
pub mod deck_rules;
pub mod diagnostic;
pub mod escape_rules;
pub mod fit_rules;
pub mod front_matter;
pub mod front_matter_rules;
pub mod geometry_rules;
pub mod ir;
pub mod length;
pub mod master_rules;
pub mod measure;
pub mod protocol;
pub mod slot_rules;
pub mod softbreak_rules;
pub mod span;
pub mod split_rules;
pub mod subset_rules;
pub mod text;
pub mod theme;
pub mod theme_rules;
