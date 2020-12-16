//! Common types used by other Holochain crates.
//!
//! This crate is a complement to the
//! [holochain_zome_types crate](https://crates.io/crates/holochain_zome_types),
//! which contains only the essential types which are used in Holochain DNA
//! code. This crate expands on those types to include all types which Holochain
//! itself depends on.

#![deny(missing_docs)]

pub mod activity;
pub mod app;
pub mod autonomic;
pub mod chain;
pub mod db;
pub mod dht_op;
pub mod dna;
pub mod element;
pub mod entry;
pub mod fixt;
pub mod header;
pub mod link;
mod macros;
pub mod metadata;
pub mod prelude;
pub mod signal;
pub mod timestamp;
pub mod validate;

// #[cfg(test)]
pub mod test_utils;

#[doc(inline)]
pub use entry::{Entry, EntryHashed};

#[doc(inline)]
pub use header::HeaderHashed;

pub use activity::*;
pub use app::*;
pub use autonomic::*;
pub use chain::*;
pub use db::*;
pub use dht_op::*;
pub use dna::*;
pub use element::*;
pub use entry::*;
pub use header::*;
pub use link::*;
pub use metadata::*;
pub use prelude::*;
pub use signal::*;
pub use timestamp::*;
pub use validate::*;
