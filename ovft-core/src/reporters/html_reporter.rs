use askama::Template;
use std::fs;
use std::path::Path;

use crate::config::Config;
use crate::core::TraceResult;
use crate::Result;

/// HTML reporter that generates OpenFastTrace-compatible HTML reports
/// [impl->dsn~html-reporter-module~1]
pub struct HtmlReporter;

impl HtmlReporter {
    /// Create a new HTML reporter
    pub fn new(_config: &Config) -> Self {
        Self
    }

    /// Generate an HTML report for the trace result
    pub fn generate_report(&self, trace_result: &TraceResult, output_path: &Path) -> Result<()> {
        let template = HtmlReportTemplate {
            trace_result,
            css: include_str!("../assets/report.css"),
        };

        let html = template.render()?;

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(output_path, html)?;
        Ok(())
    }
}

/// Template for generating HTML reports
#[derive(Template)]
#[template(path = "report.html")]
struct HtmlReportTemplate<'a> {
    trace_result: &'a TraceResult,
    css: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{LinkedSpecificationItem, SpecificationItem, SpecificationItemId};
    use std::collections::HashMap;

    #[test]
    fn test_html_reporter_creation() {
        let config = Config::default();
        let _reporter = HtmlReporter::new(&config);
        // Basic creation test - reporter should be created successfully
    }

    #[test]
    fn test_template_rendering() {
        // Create a simple trace result for testing
        let items = vec![LinkedSpecificationItem::new(
            SpecificationItem::builder(SpecificationItemId::new(
                "req".to_string(),
                "test".to_string(),
                1,
            ))
            .title("Test Requirement".to_string())
            .description("A test requirement".to_string())
            .build(),
        )];

        let trace_result = TraceResult {
            items,
            total_items: 1,
            defect_count: 0,
            defects: vec![],
            coverage_summary: HashMap::new(),
            is_success: true,
        };

        let template = HtmlReportTemplate {
            trace_result: &trace_result,
            css: "/* test css */",
        };

        // Test that template has the expected data
        assert_eq!(template.trace_result.total_items, 1);
        assert!(template.trace_result.is_success);
        assert_eq!(template.css, "/* test css */");
    }
}
