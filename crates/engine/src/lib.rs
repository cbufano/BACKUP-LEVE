//! Backup Leve sync engine.
//!
//! The behaviour is specified in `docs/REGRAS-DE-SINCRONISMO.md`.

pub mod copy;
pub mod decision;
pub mod error;
pub mod exclude;
pub mod index;
pub mod job;
pub mod paths;
pub mod profile;
pub mod report;
pub mod resolve;
pub mod scan;

pub use error::{Error, Result};
pub use exclude::Exclusions;
pub use job::{run, Event, Options, Side};
pub use report::Report;
