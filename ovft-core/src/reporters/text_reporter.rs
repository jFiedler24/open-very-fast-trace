//! Plain-text report for terminals and CI logs, in the style of
//! OpenFastTrace's plain text reporter.
//!
//! Four verbosity levels (quiet, summary, failures, all) and optional filters
//! by artifact type and item status keep the output useful both in a local
//! terminal and in CI log output.

use crate::core::{ItemStatus, TraceResult};
use std::fmt::Write;

/// How much detail the plain-text report contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum Verbosity {
    /// One-line verdict only (`OK` / `FAILED`).
    Quiet,
    /// Verdict plus totals and coverage per artifact type.
    Summary,
    /// Summary plus one line per defect.
    Failures,
    /// Failures plus one line per item (respecting the filter).
    All,
}

impl Verbosity {
    /// Map a `-v` repetition count (0–3+) to a verbosity level.
    pub fn from_count(count: u8) -> Self {
        match count {
            0 => Self::Quiet,
            1 => Self::Summary,
            2 => Self::Failures,
            _ => Self::All,
        }
    }
}

/// Restrict which items the report covers.
#[derive(Debug, Clone, Default)]
pub struct ReportFilter {
    /// Only include these artifact types (e.g. `["req", "dsn"]`). `None` = all.
    pub wanted_types: Option<Vec<String>>,
    /// Only include items with this status. `None` = all.
    pub item_status: Option<ItemStatus>,
}

impl ReportFilter {
    pub fn is_empty(&self) -> bool {
        self.wanted_types.is_none() && self.item_status.is_none()
    }
}

/// Renders a [`TraceResult`] as plain text.
pub struct PlainTextReporter {
    verbosity: Verbosity,
    filter: ReportFilter,
}

impl PlainTextReporter {
    pub fn new(verbosity: Verbosity, filter: ReportFilter) -> Self {
        Self { verbosity, filter }
    }

    /// True when the item passes the wanted-type and status filters.
    fn wanted(&self, item: &crate::core::LinkedSpecificationItem) -> bool {
        if let Some(types) = &self.filter.wanted_types {
            if !types.contains(&item.item.id.artifact_type) {
                return false;
            }
        }
        if let Some(status) = &self.filter.item_status {
            if &item.item.status != status {
                return false;
            }
        }
        true
    }

    /// Render the report to a string.
    pub fn render(&self, result: &TraceResult) -> String {
        let mut out = String::new();

        let items: Vec<_> = result.items.iter().filter(|i| self.wanted(i)).collect();
        let defects: Vec<_> = result
            .defects
            .iter()
            .filter(|d| match &d.item_id {
                Some(id) => match &self.filter.wanted_types {
                    Some(types) => types.contains(&id.artifact_type),
                    None => true,
                },
                None => true,
            })
            .collect();
        let covered = items
            .iter()
            .filter(|i| matches!(i.coverage_status, crate::core::CoverageStatus::Covered))
            .count();

        // Verdict line (always printed).
        let _ = if defects.is_empty() {
            writeln!(out, "OK – {} item(s), no defects", items.len())
        } else {
            writeln!(
                out,
                "FAILED – {} defect(s) in {} item(s)",
                defects.len(),
                items.len()
            )
        };

        if self.verbosity == Verbosity::Quiet {
            return out;
        }

        // Summary.
        let pct = if items.is_empty() {
            100.0
        } else {
            covered as f64 / items.len() as f64 * 100.0
        };
        let _ = writeln!(
            out,
            "{} item(s), {} covered ({:.0}%)",
            items.len(),
            covered,
            pct
        );

        let mut summaries: Vec<_> = result.coverage_summary.iter().collect();
        summaries.sort_by(|a, b| a.0.cmp(b.0));
        for (artifact_type, summary) in summaries {
            if let Some(types) = &self.filter.wanted_types {
                if !types.contains(artifact_type) {
                    continue;
                }
            }
            let _ = writeln!(
                out,
                "  {:<8} {:>3}/{:<3} ({:>5.1}%) {}",
                artifact_type,
                summary.covered,
                summary.total,
                summary.percentage,
                coverage_word(&summary.status),
            );
        }
        if !self.filter.is_empty() {
            let _ = writeln!(out, "  (filtered: {})", self.filter.describe());
        }

        if self.verbosity < Verbosity::Failures {
            return out;
        }

        // Failures.
        if !defects.is_empty() {
            let _ = writeln!(out, "\ndefects:");
            for defect in &defects {
                let _ = writeln!(out, "  - {}", defect.description);
            }
        }

        if self.verbosity < Verbosity::All {
            return out;
        }

        // All items.
        let _ = writeln!(out, "\nitems:");
        for item in items {
            let flag = if item.is_defect {
                "!"
            } else {
                match item.coverage_status {
                    crate::core::CoverageStatus::Covered => " ",
                    crate::core::CoverageStatus::Partial => "~",
                    crate::core::CoverageStatus::Uncovered => "x",
                }
            };
            let _ = writeln!(
                out,
                "  [{}] {:<28} {:<9} {}",
                flag,
                item.item.id.to_string(),
                item.item.status,
                item.title(),
            );
            for link in &item.outgoing_links {
                let _ = writeln!(out, "        covers {} ({})", link.target_id, link.status);
            }
        }

        out
    }
}

fn coverage_word(status: &crate::core::CoverageStatus) -> &'static str {
    match status {
        crate::core::CoverageStatus::Covered => "covered",
        crate::core::CoverageStatus::Partial => "partial",
        crate::core::CoverageStatus::Uncovered => "uncovered",
    }
}

impl ReportFilter {
    fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(types) = &self.wanted_types {
            parts.push(format!("types: {}", types.join(", ")));
        }
        if let Some(status) = &self.item_status {
            parts.push(format!("status: {}", status));
        }
        parts.join("; ")
    }
}

impl std::str::FromStr for ItemStatus {
    type Err = crate::Error;

    fn from_str(s: &str) -> crate::Result<Self> {
        match s.to_lowercase().as_str() {
            "draft" => Ok(Self::Draft),
            "proposed" => Ok(Self::Proposed),
            "approved" => Ok(Self::Approved),
            "rejected" => Ok(Self::Rejected),
            _ => Err(crate::Error::InvalidId(format!(
                "Unknown item status '{}'. Expected: draft, proposed, approved or rejected",
                s
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Linker, Location, SpecificationItem, SpecificationItemId};
    use crate::{Config, Tracer};
    use std::path::PathBuf;

    fn mk(id: &str, status: ItemStatus, needs: &[&str], covers: &[&str]) -> SpecificationItem {
        let mut b = SpecificationItem::builder(SpecificationItemId::parse(id).unwrap())
            .title(format!("Title {id}"))
            .status(status)
            .location(Location::new(PathBuf::from("f.md"), 1))
            .needs_multiple(needs.iter().map(|s| s.to_string()).collect());
        for c in covers {
            b = b.covers(SpecificationItemId::parse(c).unwrap());
        }
        b.build()
    }

    /// Fully traced: dsn covers req.
    fn sample() -> TraceResult {
        let items = vec![
            mk("req~a~1", ItemStatus::Approved, &["dsn"], &[]),
            mk("dsn~a~1", ItemStatus::Draft, &[], &["req~a~1"]),
        ];
        let linked = Linker::new().link_items(items).unwrap();
        Tracer::new(Config::empty()).analyze_trace(&linked)
    }

    /// Untraced: req needs dsn but nothing covers it.
    fn defective() -> TraceResult {
        let items = vec![mk("req~a~1", ItemStatus::Approved, &["dsn"], &[])];
        let linked = Linker::new().link_items(items).unwrap();
        Tracer::new(Config::empty()).analyze_trace(&linked)
    }

    #[test]
    fn quiet_prints_only_verdict() {
        let r = sample();
        let out = PlainTextReporter::new(Verbosity::Quiet, ReportFilter::default()).render(&r);
        assert!(out.starts_with("OK"));
        assert!(!out.contains("items:"));
        assert!(!out.contains("coverage"));
    }

    #[test]
    fn summary_shows_coverage_rows() {
        let r = sample();
        let out = PlainTextReporter::new(Verbosity::Summary, ReportFilter::default()).render(&r);
        assert!(out.contains("req"));
        assert!(out.contains("dsn"));
        assert!(out.contains("covered"));
    }

    #[test]
    fn failures_lists_defects() {
        let r = defective();
        let filter = ReportFilter {
            wanted_types: Some(vec!["req".to_string()]),
            item_status: None,
        };
        let out = PlainTextReporter::new(Verbosity::Failures, filter).render(&r);
        assert!(out.contains("FAILED"));
        assert!(out.contains("defects:"));
        assert!(out.contains("needs coverage by dsn"));
        assert!(out.contains("types: req"));
    }

    #[test]
    fn all_lists_items_and_links() {
        let r = sample();
        let out = PlainTextReporter::new(Verbosity::All, ReportFilter::default()).render(&r);
        assert!(out.contains("items:"));
        assert!(out.contains("dsn~a~1"));
        assert!(out.contains("covers req~a~1 (covers)"));
        assert!(out.contains("draft"));
    }

    #[test]
    fn status_filter_narrows_items() {
        let r = sample();
        let filter = ReportFilter {
            wanted_types: None,
            item_status: Some(ItemStatus::Draft),
        };
        let out = PlainTextReporter::new(Verbosity::All, filter).render(&r);
        assert!(out.contains("dsn~a~1"));
        assert!(!out.contains("] req~a~1"));
        assert!(out.contains("status: draft"));
    }

    #[test]
    fn failed_verdict_when_defects() {
        let out =
            PlainTextReporter::new(Verbosity::Quiet, ReportFilter::default()).render(&defective());
        assert!(out.starts_with("FAILED"));
    }

    #[test]
    fn verbosity_from_count() {
        assert_eq!(Verbosity::from_count(0), Verbosity::Quiet);
        assert_eq!(Verbosity::from_count(1), Verbosity::Summary);
        assert_eq!(Verbosity::from_count(2), Verbosity::Failures);
        assert_eq!(Verbosity::from_count(9), Verbosity::All);
    }

    #[test]
    fn parses_item_status() {
        assert_eq!(
            "approved".parse::<ItemStatus>().unwrap(),
            ItemStatus::Approved
        );
        assert!("nope".parse::<ItemStatus>().is_err());
    }
}
