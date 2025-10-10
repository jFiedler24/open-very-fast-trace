use crate::config::Config;
use crate::core::Linker;
use crate::core::{CoverageStatus, CoverageSummary, Defect, LinkedSpecificationItem};
use crate::importers::{MarkdownImporter, TagImporter};
use crate::Result;
use std::collections::HashMap;
use std::path::Path;

/// Main tracer that orchestrates the requirement tracing process
pub struct Tracer {
    config: Config,
    tag_importer: TagImporter,
    markdown_importer: MarkdownImporter,
}

impl Tracer {
    /// Create a new tracer with the given configuration
    pub fn new(config: Config) -> Self {
        Self {
            tag_importer: TagImporter::new(),
            markdown_importer: MarkdownImporter::new(),
            config,
        }
    }

    /// Run the complete tracing process
    pub fn trace(&self) -> Result<TraceResult> {
        // 1. Import specification items from all sources
        let mut items = Vec::new();

        // Import from source code files
        for source_dir in &self.config.source_dirs {
            let source_items = self.tag_importer.import_from_directory(source_dir)?;
            items.extend(source_items);
        }

        // Import from specification files
        for spec_dir in &self.config.spec_dirs {
            let spec_items = self.markdown_importer.import_from_directory(spec_dir)?;
            items.extend(spec_items);
        }

        // 2. Link items together
        let linker = Linker::new();
        let linked_items = linker.link_items(items)?;

        // 3. Analyze coverage and defects
        let trace_result = self.analyze_trace(&linked_items);

        Ok(trace_result)
    }

    /// Generate an HTML report for the trace result
    pub fn generate_html_report(
        &self,
        trace_result: &TraceResult,
        output_path: &Path,
    ) -> Result<()> {
        let reporter = crate::reporters::HtmlReporter::new(&self.config);
        reporter.generate_report(trace_result, output_path)
    }

    /// Analyze the linked items to determine coverage and defects
    fn analyze_trace(&self, linked_items: &[LinkedSpecificationItem]) -> TraceResult {
        let total_items = linked_items.len();
        let mut defects = Vec::new();
        let mut coverage_summary = HashMap::new();

        // Group items by artifact type for coverage analysis
        let mut artifact_groups: HashMap<String, Vec<&LinkedSpecificationItem>> = HashMap::new();
        for item in linked_items {
            artifact_groups
                .entry(item.item.id.artifact_type.clone())
                .or_default()
                .push(item);
        }

        // Analyze coverage for each artifact type
        for (artifact_type, items) in artifact_groups {
            let total = items.len();
            let covered = items.iter().filter(|item| item.is_covered()).count();
            let percentage = if total > 0 {
                (covered as f64 / total as f64) * 100.0
            } else {
                100.0
            };

            let status = if covered == total {
                CoverageStatus::Covered
            } else if covered > 0 {
                CoverageStatus::Partial
            } else {
                CoverageStatus::Uncovered
            };

            coverage_summary.insert(
                artifact_type,
                CoverageSummary {
                    total,
                    covered,
                    percentage,
                    status,
                },
            );
        }

        // Collect defective items
        for item in linked_items {
            if item.is_defect {
                defects.push(Defect {
                    defect_type: crate::core::DefectType::UncoveredItem,
                    description: format!("Item {} has defects", item.item.id),
                    item_id: Some(item.item.id.clone()),
                });
            }
        }

        let is_success = defects.is_empty();

        TraceResult {
            items: linked_items.to_vec(),
            total_items,
            defect_count: defects.len(),
            defects,
            coverage_summary,
            is_success,
        }
    }
}

/// Result of a tracing operation
#[derive(Debug, Clone)]
pub struct TraceResult {
    /// All linked specification items
    pub items: Vec<LinkedSpecificationItem>,
    /// Total number of items processed
    pub total_items: usize,
    /// Number of items with defects
    pub defect_count: usize,
    /// Defects found during tracing
    pub defects: Vec<Defect>,
    /// Coverage summary by artifact type
    pub coverage_summary: HashMap<String, CoverageSummary>,
    /// Whether the trace was successful (no defects)
    pub is_success: bool,
}

impl TraceResult {
    /// Check if the trace has no defects
    pub fn has_no_defects(&self) -> bool {
        self.is_success
    }

    /// Get coverage percentage
    pub fn coverage_percentage(&self) -> f64 {
        if self.total_items == 0 {
            100.0
        } else {
            let covered_items = self.total_items - self.defect_count;
            (covered_items as f64 / self.total_items as f64) * 100.0
        }
    }

    /// Get items by artifact type
    pub fn items_by_artifact_type(&self) -> HashMap<String, Vec<&LinkedSpecificationItem>> {
        let mut result = HashMap::new();
        for item in &self.items {
            let artifact_type = &item.item.id.artifact_type;
            result
                .entry(artifact_type.clone())
                .or_insert_with(Vec::new)
                .push(item);
        }
        result
    }
}
