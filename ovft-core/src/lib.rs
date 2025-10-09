//! # Open Very Fast Trace - Core Library
//! 
//! A Rust library for requirements tracing and coverage analysis.
//! Compatible with OpenFastTrace format and methodology.
//! 
//! ## Quick Start
//! 
//! ```rust
//! use ovft_core::{Tracer, Config};
//! 
//! let config = Config::default()
//!     .add_source_dir("src")
//!     .add_spec_dir("docs/requirements");
//! 
//! let tracer = Tracer::new(config);
//! let trace_result = tracer.trace()?;
//! 
//! // Generate HTML report
//! tracer.generate_html_report(&trace_result, "target/trace_report.html")?;
//! ```

pub mod config;
pub mod core;
pub mod importers;
pub mod reporters;
pub mod error;

pub use config::Config;
pub use core::{Tracer, TraceResult};
pub use error::{Error, Result};

/// Re-export commonly used types
pub use crate::core::{
    SpecificationItem, SpecificationItemId, LinkedSpecificationItem,
    LinkStatus, CoverageStatus, ItemStatus, Location, Defect, DefectType, CoverageSummary
};
