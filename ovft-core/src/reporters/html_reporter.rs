use askama::Template;
use pulldown_cmark::{html, Options, Parser};
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

    /// Render an HTML report to a String without writing to disk.
    /// This is the pure function used by WASM and the webview.
    /// [impl->req~title-first-report~1]
    pub fn render_report_html(&self, trace_result: &TraceResult) -> Result<String> {
        let processed_trace_result = self.process_markdown_content(trace_result);

        let mut coverage_strip: Vec<(String, crate::core::CoverageSummary)> = trace_result
            .coverage_summary
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        coverage_strip.sort_by(|a, b| a.0.cmp(&b.0));

        let artifact_types: Vec<String> = coverage_strip.iter().map(|(k, _)| k.clone()).collect();

        let mut item_statuses: Vec<String> = processed_trace_result
            .items
            .iter()
            .map(|i| i.item.status.to_string())
            .collect();
        item_statuses.sort();
        item_statuses.dedup();

        let (trace_graph, trace_note) = crate::reporters::visuals::trace_graph_svg(
            &processed_trace_result,
            MAX_GRAPH_NODES,
        );
        let coverage_bars_html = crate::reporters::visuals::coverage_bars(&coverage_strip);

        let template = HtmlReportTemplate {
            trace_result: &processed_trace_result,
            coverage_strip,
            artifact_types,
            item_statuses,
            donut_html: crate::reporters::visuals::status_donut(&processed_trace_result),
            coverage_bars_html,
            defect_bars_html: crate::reporters::visuals::defect_bars(&processed_trace_result),
            type_flow_html: crate::reporters::visuals::type_flow_svg(&processed_trace_result),
            trace_graph_html: trace_graph,
            trace_graph_note: trace_note,
            mermaid: crate::reporters::visuals::mermaid_flowchart(
                &processed_trace_result,
                MAX_GRAPH_NODES * 2,
            ),
            css: include_str!("../assets/report.css"),
        };

        Ok(template.render()?)
    }

    /// Generate an HTML report for the trace result
    /// [impl->req~html-compliant-anchors~1]
    pub fn generate_report(&self, trace_result: &TraceResult, output_path: &Path) -> Result<()> {
        let html = self.render_report_html(trace_result)?;

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(output_path, html)?;
        Ok(())
    }
    
    /// Process markdown content in descriptions and convert to HTML
    fn process_markdown_content(&self, trace_result: &TraceResult) -> TraceResult {
        let processed_items = trace_result.items.iter().map(|linked_item| {
            let mut processed_item = linked_item.clone();
            
            // Convert markdown in description to HTML
            if let Some(ref description) = processed_item.item.description {
                processed_item.item.description = Some(self.markdown_to_html(description));
            }
            
            processed_item
        }).collect();
        
        // Sort items: those with incoming links first, then those without incoming links
        let mut sorted_items: Vec<_> = processed_items;
        sorted_items.sort_by(|a, b| {
            let a_has_incoming = !a.incoming_links.is_empty();
            let b_has_incoming = !b.incoming_links.is_empty();
            
            // First sort by incoming links (items with incoming links first)
            match (a_has_incoming, b_has_incoming) {
                (true, false) => std::cmp::Ordering::Less,    // a has incoming, b doesn't -> a first
                (false, true) => std::cmp::Ordering::Greater, // a doesn't have incoming, b does -> b first
                _ => a.item.id.to_string().cmp(&b.item.id.to_string()), // same incoming status -> sort by ID
            }
        });
        
        TraceResult {
            items: sorted_items,
            total_items: trace_result.total_items,
            defect_count: trace_result.defect_count,
            defects: trace_result.defects.clone(),
            coverage_summary: trace_result.coverage_summary.clone(),
            is_success: trace_result.is_success,
        }
    }
    
    /// Convert markdown text to HTML
    fn markdown_to_html(&self, markdown: &str) -> String {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_TASKLISTS);
        
        let parser = Parser::new_ext(markdown, options);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);
        html_output
    }
}

/// Maximum number of item-graph nodes rendered in the HTML report.
const MAX_GRAPH_NODES: usize = 150;

/// Template for generating HTML reports
#[derive(Template)]
#[template(path = "report.html")]
struct HtmlReportTemplate<'a> {
    trace_result: &'a TraceResult,
    coverage_strip: Vec<(String, crate::core::CoverageSummary)>,
    artifact_types: Vec<String>,
    item_statuses: Vec<String>,
    donut_html: String,
    coverage_bars_html: String,
    defect_bars_html: String,
    type_flow_html: String,
    trace_graph_html: String,
    trace_graph_note: Option<String>,
    mermaid: String,
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
            coverage_strip: vec![],
            artifact_types: vec![],
            item_statuses: vec![],
            donut_html: String::new(),
            coverage_bars_html: String::new(),
            defect_bars_html: String::new(),
            type_flow_html: String::new(),
            trace_graph_html: String::new(),
            trace_graph_note: None,
            mermaid: String::new(),
            css: "/* test css */",
        };

        // Test that template has the expected data
        assert_eq!(template.trace_result.total_items, 1);
        assert!(template.trace_result.is_success);
        assert_eq!(template.css, "/* test css */");
    }
}
