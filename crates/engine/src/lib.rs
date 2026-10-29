//! Backup Leve sync engine.
//!
//! The behaviour is specified in `docs/REGRAS-DE-SINCRONISMO.md`.

pub mod copy;
pub mod decision;
pub mod error;
pub mod index;
pub mod job;
pub mod paths;
pub mod report;
pub mod scan;

pub use error::{Error, Result};
pub use job::{run, Event, Options};
pub use report::Report;
