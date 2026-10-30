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
    /// [impl->dsn~status-aware-analysis~1]
    pub fn analyze_trace(&self, linked_items: &[LinkedSpecificationItem]) -> TraceResult {
        let total_items = linked_items.len();

        // Stamp whether each item counts toward headline coverage, based on
        // its status. Excluded items stay in the report but never affect
        // coverage statistics or the check outcome.
        let mut items = linked_items.to_vec();
        for item in &mut items {
            item.counted = self.config.status.is_counted(&item.item.status);
        }

        let mut defects = Vec::new();
        let mut coverage_summary = HashMap::new();

        // Group counted items by artifact type for coverage analysis
        let mut artifact_groups: HashMap<String, Vec<&LinkedSpecificationItem>> = HashMap::new();
        for item in &items {
            if !item.counted {
                continue;
            }
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
        for item in &items {
            if item.is_defect {
                let detailed_description = self.generate_detailed_defect_description(item);
                defects.push(Defect {
                    defect_type: crate::core::DefectType::UncoveredItem,
                    description: detailed_description,
                    item_id: Some(item.item.id.clone()),
                });
            }
        }

        let check_defect_count = defects
            .iter()
            .filter(|d| match &d.item_id {
                Some(id) => items
                    .iter()
                    .find(|i| &i.item.id == id)
                    .is_none_or(|i| {
                        self.config.status.check_policy(&i.item.status)
                            == crate::config::StatusCheckPolicy::Fail
                    }),
                // Defects not attributable to an item always fail the check
                None => true,
            })
            .count();

        let is_success = defects.is_empty();

        TraceResult {
            status_summary: TraceResult::compute_status_summary(&items, &self.config.status.count),
            items,
            total_items,
            defect_count: defects.len(),
            defects,
            coverage_summary,
            check_defect_count,
            is_success,
        }
    }

    /// Generate a detailed description of what's wrong with a defective item
    pub fn generate_detailed_defect_description(&self, item: &LinkedSpecificationItem) -> String {
        let mut issues = Vec::new();

        // Check for broken outgoing links
        for link in &item.outgoing_links {
            let target = if link.target_id.artifact_type.is_empty() {
                link.target_id.name.clone()
            } else {
                link.target_id.to_string()
            };
            match link.status {
                crate::core::LinkStatus::Orphaned => {
                    issues.push(format!("covers non-existing item {}", target));
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
    pub fn find_missing_coverage_types(&self, item: &LinkedSpecificationItem) -> Vec<String> {
        let mut missing = Vec::new();
        let mut seen = std::collections::HashSet::new();
        
        for needed_type in &item.item.needs {
            if !seen.insert(needed_type.clone()) {
                continue; // skip duplicates
            }
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
    /// Coverage summary by artifact type (counted items only)
    pub coverage_summary: HashMap<String, CoverageSummary>,
    /// Coverage summary by item status, including excluded statuses
    pub status_summary: std::collections::BTreeMap<crate::core::ItemStatus, crate::core::StatusCoverageSummary>,
    /// Defects that fail the configured per-status check policy
    /// (`cargo ovft --check` exit code); defects of statuses configured as
    /// `ignore` are reported but never fail the check
    pub check_defect_count: usize,
    /// Whether the trace was successful (no defects at all, regardless of status)
    pub is_success: bool,
}

impl TraceResult {
    /// Compute a per-status coverage overview over the given items. Statuses
    /// present in `counted_statuses` contribute to headline coverage.
    pub fn compute_status_summary(
        items: &[LinkedSpecificationItem],
        counted_statuses: &[crate::core::ItemStatus],
    ) -> std::collections::BTreeMap<crate::core::ItemStatus, crate::core::StatusCoverageSummary> {
        use std::collections::BTreeMap;
        let mut summary: BTreeMap<crate::core::ItemStatus, (usize, usize)> = BTreeMap::new();
        for item in items {
            let entry = summary.entry(item.item.status.clone()).or_insert((0, 0));
            entry.0 += 1;
            if item.is_covered() {
                entry.1 += 1;
            }
        }
        summary
            .into_iter()
            .map(|(status, (total, covered))| {
                let percentage = if total > 0 {
                    (covered as f64 / total as f64) * 100.0
                } else {
                    100.0
                };
                (
                    status.clone(),
                    crate::core::StatusCoverageSummary {
                        total,
                        covered,
                        percentage,
                        counted: counted_statuses.contains(&status),
                    },
                )
            })
            .collect()
    }

    /// Check if the trace has no defects
    pub fn has_no_defects(&self) -> bool {
        self.is_success
    }

    /// Whether `cargo ovft --check` passes under the configured status policy
    pub fn check_passes(&self) -> bool {
        self.check_defect_count == 0
    }

    /// Get coverage percentage over counted items only; excluded items
    /// (e.g. draft or rejected requirements) do not lower the percentage
    pub fn coverage_percentage(&self) -> f64 {
        let counted = self.items.iter().filter(|i| i.counted).count();
        if counted == 0 {
            100.0
        } else {
            let defective_counted = self
                .items
                .iter()
                .filter(|i| i.counted && i.is_defect)
                .count();
            ((counted - defective_counted) as f64 / counted as f64) * 100.0
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StatusCheckPolicy;
    use crate::core::{ItemStatus, SpecificationItem, SpecificationItemId};
    use std::path::PathBuf;

    fn mk(id: &str, status: ItemStatus, needs: &[&str], covers: &[&str]) -> SpecificationItem {
        let mut b = SpecificationItem::builder(SpecificationItemId::parse(id).unwrap())
            .title(format!("Title {id}"))
            .status(status)
            .location(crate::core::Location::new(PathBuf::from("f.md"), 1))
            .needs_multiple(needs.iter().map(|s| s.to_string()).collect());
        for c in covers {
            b = b.covers(SpecificationItemId::parse(c).unwrap());
        }
        b.build()
    }

    /// One approved uncovered requirement plus a rejected one with the same problem.
    fn mixed_status_trace() -> TraceResult {
        let items = vec![
            mk("req~counted~1", ItemStatus::Approved, &["dsn"], &[]),
            mk("req~declined~1", ItemStatus::Rejected, &["dsn"], &[]),
        ];
        let linked = Linker::new().link_items(items).unwrap();
        Tracer::new(Config::default()).analyze_trace(&linked)
    }

    #[test]
    fn excluded_statuses_are_stamped_and_not_counted() {
        let result = mixed_status_trace();
        let approved = result
            .items
            .iter()
            .find(|i| i.item.id.name == "counted")
            .unwrap();
        let rejected = result
            .items
            .iter()
            .find(|i| i.item.id.name == "declined")
            .unwrap();
        assert!(approved.counted);
        assert!(!rejected.counted);
        // Only the approved item counts toward artifact coverage
        let req_summary = result.coverage_summary.get("req").unwrap();
        assert_eq!(req_summary.total, 1);
    }

    #[test]
    fn status_summary_covers_all_statuses_with_counted_flag() {
        let result = mixed_status_trace();
        let approved = result.status_summary.get(&ItemStatus::Approved).unwrap();
        assert_eq!(approved.total, 1);
        assert!(approved.counted);
        let rejected = result.status_summary.get(&ItemStatus::Rejected).unwrap();
        assert_eq!(rejected.total, 1);
        assert!(!rejected.counted);
    }

    #[test]
    fn check_policy_ignores_excluded_status_defects() {
        let result = mixed_status_trace();
        // Both items are defective, but rejected is configured as "ignore"
        assert_eq!(result.defect_count, 2);
        assert_eq!(result.check_defect_count, 1);
        assert!(!result.check_passes());
        assert!(!result.is_success);
    }

    #[test]
    fn coverage_percentage_ignores_excluded_items() {
        let result = mixed_status_trace();
        // The only counted item is uncovered
        assert_eq!(result.coverage_percentage(), 0.0);
        let items = vec![mk("req~a~1", ItemStatus::Draft, &["dsn"], &[])];
        let linked = Linker::new().link_items(items).unwrap();
        let draft_only = Tracer::new(Config::default()).analyze_trace(&linked);
        assert_eq!(draft_only.coverage_percentage(), 100.0);
        assert!(draft_only.check_passes());
    }

    #[test]
    fn custom_check_policy_can_fail_drafts() {
        let mut config = Config::default();
        config
            .status
            .check
            .insert(ItemStatus::Draft, StatusCheckPolicy::Fail);
        config
            .status
            .check
            .insert(ItemStatus::Rejected, StatusCheckPolicy::Fail);
        let items = vec![mk("req~a~1", ItemStatus::Draft, &["dsn"], &[])];
        let linked = Linker::new().link_items(items).unwrap();
        let result = Tracer::new(config).analyze_trace(&linked);
        assert_eq!(result.check_defect_count, 1);
    }

    #[test]
    fn custom_count_configuration_adds_statuses() {
        let mut config = Config::default();
        config.status.count.push(ItemStatus::Draft);
        let items = vec![
            mk("req~a~1", ItemStatus::Draft, &["dsn"], &[]),
            mk("req~b~1", ItemStatus::Approved, &["dsn"], &[]),
        ];
        let linked = Linker::new().link_items(items).unwrap();
        let result = Tracer::new(config).analyze_trace(&linked);
        let req_summary = result.coverage_summary.get("req").unwrap();
        assert_eq!(req_summary.total, 2);
        assert!(result.status_summary.get(&ItemStatus::Draft).unwrap().counted);
    }
}
