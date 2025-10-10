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

    /// Generate an HTML report for the trace result
    pub fn generate_report(&self, trace_result: &TraceResult, output_path: &Path) -> Result<()> {
        // Convert markdown descriptions to HTML
        let processed_trace_result = self.process_markdown_content(trace_result);
        
        let template = HtmlReportTemplate {
            trace_result: &processed_trace_result,
            css: include_str!("../assets/report.css"),
        };

        let mut html = template.render()?;
        
        // Post-process HTML to fix ID links by replacing tilde characters with underscores
        html = self.fix_html_ids(html);

        // Ensure output directory exists
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(output_path, html)?;
        Ok(())
    }
    
    /// Fix HTML IDs and links by replacing problematic characters
    fn fix_html_ids(&self, html: String) -> String {
        // Replace tilde characters in both ID attributes and href links
        html.replace("id=\"item-", "id=\"item_")
            .replace("href=\"#item-", "href=\"#item_")
            .replace('~', "_")
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
    
    /// Create a safe HTML ID from a specification item ID by replacing problematic characters
    fn safe_html_id(&self, id: &str) -> String {
        id.replace('~', "_")
          .replace(':', "_")
          .replace(' ', "_")
          .replace('-', "_")
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
