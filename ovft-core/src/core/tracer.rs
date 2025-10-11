use crate::config::Config;
use crate::core::Linker;
use crate::core::{CoverageStatus, CoverageSummary, Defect, DefectType, LinkedSpecificationItem};
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
                let detailed_description = self.generate_detailed_defect_description(&item);
                defects.push(Defect {
                    defect_type: crate::core::DefectType::UncoveredItem,
                    description: detailed_description,
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

    /// Generate a detailed description of what's wrong with a defective item
    fn generate_detailed_defect_description(&self, item: &LinkedSpecificationItem) -> String {
        let mut issues = Vec::new();

        // Check for broken outgoing links
        for link in &item.outgoing_links {
            match link.status {
                crate::core::LinkStatus::Orphaned => {
                    issues.push(format!("covers non-existing item {}", link.target_id));
                }
                crate::core::LinkStatus::Duplicate => {
                    issues.push(format!("has duplicate ID {}", item.item.id));
                }
                crate::core::LinkStatus::Outdated => {
                    issues.push(format!("covers outdated revision of {}", link.target_id));
                }
                crate::core::LinkStatus::Predated => {
                    issues.push(format!("covers newer revision of {}", link.target_id));
                }
                crate::core::LinkStatus::Ambiguous => {
                    issues.push(format!("has ambiguous reference to {}", link.target_id));
                }
                _ => {}
            }
        }

        // Check for missing coverage
        if !matches!(item.coverage_status, CoverageStatus::Covered) {
            let missing_coverage = self.find_missing_coverage_types(item);
            if !missing_coverage.is_empty() {
                let coverage_list = missing_coverage.join(", ");
                issues.push(format!("needs coverage by {}", coverage_list));
            }
        }

        if issues.is_empty() {
            format!("Item {} has unspecified defects", item.item.id)
        } else if issues.len() == 1 {
            format!("Item {} {}", item.item.id, issues[0])
        } else {
            format!("Item {} has multiple issues: {}", item.item.id, issues.join("; "))
        }
    }

    /// Find which artifact types are missing coverage for an item
    fn find_missing_coverage_types(&self, item: &LinkedSpecificationItem) -> Vec<String> {
        let mut missing = Vec::new();
        
        for needed_type in &item.item.needs {
            // Check if this artifact type has any incoming coverage
            let has_coverage = item.incoming_links.iter().any(|link| {
                if let Some(source_id) = &link.source_id {
                    source_id.artifact_type == *needed_type
                } else {
                    false
                }
            });
            
            if !has_coverage {
                missing.push(needed_type.clone());
            }
        }
        
        missing
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

    /// Get defect statistics grouped by defect type
    /// [impl->req~defect-type-statistics~1]
    pub fn defect_statistics(&self) -> HashMap<DefectType, usize> {
        let mut stats = HashMap::new();
        for defect in &self.defects {
            *stats.entry(defect.defect_type.clone()).or_insert(0) += 1;
        }
        stats
    }

    /// Get human-readable defect statistics with detailed breakdown
    /// [impl->req~defect-type-statistics~1]
    pub fn defect_statistics_messages(&self) -> Vec<String> {
        let mut messages = Vec::new();
        
        // Count defects by needed coverage type
        let mut coverage_needs: HashMap<String, usize> = HashMap::new();
        let mut orphaned_count = 0;
        let mut duplicate_count = 0;
        let mut wrong_revision_count = 0;
        let mut circular_count = 0;
        
        for defect in &self.defects {
            match defect.defect_type {
                DefectType::UncoveredItem => {
                    // Parse the description to find what coverage is needed
                    if defect.description.contains("needs coverage by") {
                        // Extract the coverage types from descriptions like "needs coverage by dsn" or "needs coverage by impl, test"
                        if let Some(start) = defect.description.find("needs coverage by ") {
                            let coverage_part = &defect.description[start + 18..];
                            // Take until semicolon or end of string
                            let coverage_str = coverage_part.split(';').next().unwrap_or(coverage_part).trim();
                            
                            // Split by comma and count each type
                            for coverage_type in coverage_str.split(',') {
                                let trimmed = coverage_type.trim();
                                *coverage_needs.entry(trimmed.to_string()).or_insert(0) += 1;
                            }
                        }
                    } else if defect.description.contains("covers non-existing") {
                        orphaned_count += 1;
                    }
                }
                DefectType::OrphanedCoverage => orphaned_count += 1,
                DefectType::DuplicateItem => duplicate_count += 1,
                DefectType::WrongRevision => wrong_revision_count += 1,
                DefectType::CircularDependency => circular_count += 1,
            }
        }
        
        // Generate messages for coverage needs (sorted by artifact type)
        let mut coverage_types: Vec<_> = coverage_needs.into_iter().collect();
        coverage_types.sort_by(|a, b| a.0.cmp(&b.0));
        for (coverage_type, count) in coverage_types {
            messages.push(format!("{} item(s) need coverage by {}", count, coverage_type));
        }
        
        // Add other defect types
        if orphaned_count > 0 {
            messages.push(format!("{} item(s) have orphaned coverage", orphaned_count));
        }
        if duplicate_count > 0 {
            messages.push(format!("{} duplicate item(s) found", duplicate_count));
        }
        if wrong_revision_count > 0 {
            messages.push(format!("{} item(s) cover wrong revision", wrong_revision_count));
        }
        if circular_count > 0 {
            messages.push(format!("{} circular dependenc(ies) detected", circular_count));
        }

        messages
    }
}
