//! Core logic for the learning tool: storage, scheduling, sessions, references and the question guide.

pub mod db;
pub mod error;
pub mod settings;

pub use error::{Error, Result};
