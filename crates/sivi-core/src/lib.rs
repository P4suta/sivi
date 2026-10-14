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
pub mod bind;
pub mod binder;
pub mod calendar;
pub mod color;
pub mod color_rules;
pub mod color_tables;
pub mod deck_rules;
pub mod diagnostic;
pub mod emit;
pub mod escape_rules;
pub mod fit_rules;
pub mod front_matter;
pub mod front_matter_rules;
pub mod geometry_rules;
pub mod home_rules;
pub mod image;
pub mod ir;
pub mod length;
pub mod lints;
pub mod mark_rules;
pub mod master_rules;
pub mod measure;
pub mod palette_rules;
pub mod protocol;
pub mod publish_rules;
pub mod report;
pub mod slot_rules;
pub mod softbreak_rules;
pub mod span;
pub mod split_rules;
pub mod subset_rules;
pub mod syntax_theme;
pub mod text;
pub mod theme;
pub mod theme_rules;
