//! Core logic for the learning tool: storage, scheduling, sessions, references and the question guide.

pub mod clock;
pub mod db;
pub mod error;
pub mod grading;
pub mod questions;
pub mod queue;
pub mod scheduler;
pub mod session;
pub mod settings;
pub mod stats;
pub mod topics;
pub mod types;

pub use error::{Error, Result};

#[cfg(test)]
mod testutil;
