//! Memory-management tools for real-time audio and other latency-critical scenarios.
//!
//! - [`Owned`] and [`Shared`] are smart pointers analogous to `Box` and `Arc`
//! which add their contents to a queue for deferred collection when dropped.
//! - [`Collector`] is used to process the drop queue.
//! - [`Node`] provides a lower-level interface for implementing custom smart
//!   pointers or data structures.
//! - [`SharedCell`] implements a mutable memory location holding a [`Shared`]
//!   pointer that can be used by multiple readers and writers in a thread-safe
//!   manner.
//!
//! [`Owned`]: crate::Owned
//! [`Shared`]: crate::Shared
//! [`Collector`]: crate::Collector
//! [`Node`]: crate::Node
//! [`SharedCell`]: crate::SharedCell

#![no_std]

mod collector;
mod owned;
mod shared;
mod shared_cell;

pub use collector::*;
pub use owned::*;
pub use shared::*;
pub use shared_cell::*;
