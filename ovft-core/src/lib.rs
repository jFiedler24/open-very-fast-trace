//! # Open Very Fast Trace - Core Library
//!
//! A Rust library for requirements tracing and coverage analysis.
//! Compatible with OpenFastTrace format and methodology.
//!
//! ## Quick Start
//!
//! ```rust
//! use ovft_core::{Tracer, Config};
//! use std::path::Path;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let config = Config::default()
//!     .add_source_dir("src")
//!     .add_spec_dir("docs/requirements");
//!
//! let tracer = Tracer::new(config);
//! let trace_result = tracer.trace()?;
//!
//! // Generate HTML report
//! tracer.generate_html_report(&trace_result, Path::new("target/trace_report.html"))?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod core;
pub mod error;
pub mod importers;
pub mod reporters;

pub use config::Config;
pub use core::{TraceResult, Tracer};
pub use error::{Error, Result};

/// Re-export commonly used types
pub use crate::core::{
    CoverageStatus, CoverageSummary, Defect, DefectType, ItemStatus, LinkStatus,
    LinkedSpecificationItem, Location, SpecificationItem, SpecificationItemId,
};
