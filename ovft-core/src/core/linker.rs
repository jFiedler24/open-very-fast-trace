use crate::core::{
    CoverageStatus, LinkStatus, LinkedSpecificationItem, SpecificationItem, SpecificationItemId,
};
use crate::Result;
use std::collections::HashMap;

/// Linker that creates relationships between specification items
/// [impl->dsn~linker-module~1]
pub struct Linker {}

impl Linker {
    pub fn new() -> Self {
        Self {}
    }

    /// Link specification items together and analyze coverage
    pub fn link_items(
        &self,
        items: Vec<SpecificationItem>,
    ) -> Result<Vec<LinkedSpecificationItem>> {
        // First, build the lookup map and check for duplicates
        let mut items_by_id = HashMap::new();
        let mut duplicate_ids = Vec::new();

        for item in &items {
            if items_by_id.contains_key(&item.id) {
                duplicate_ids.push(item.id.clone());
            } else {
                items_by_id.insert(item.id.clone(), item.clone());
            }
        }

        // Create linked items
        let mut linked_items = Vec::new();
        for item in items {
            let mut linked_item = LinkedSpecificationItem::new(item.clone());

            // Mark duplicates as defects
            if duplicate_ids.contains(&item.id) {
                linked_item.is_defect = true;
                linked_item.add_outgoing_link(item.id.clone(), LinkStatus::Duplicate);
            }

            linked_items.push(linked_item);
        }

        // Process links between items
        self.process_coverage_links(&mut linked_items, &items_by_id)?;
        self.analyze_coverage(&mut linked_items);

        Ok(linked_items)
    }

    /// Process coverage relationships between items
    fn process_coverage_links(
        &self,
        linked_items: &mut [LinkedSpecificationItem],
        items_by_id: &HashMap<SpecificationItemId, SpecificationItem>,
    ) -> Result<()> {
        // Process outgoing links for each item
        for item in linked_items.iter_mut() {
            let covers = item.item.covers.clone();
            for covered_id in &covers {
                let link_status = self.determine_link_status(covered_id, items_by_id);
                item.add_outgoing_link(covered_id.clone(), link_status);
            }
        }

        // Process incoming links
        let items_clone: Vec<_> = linked_items.iter().map(|li| li.item.clone()).collect();
        for item in linked_items.iter_mut() {
            let item_id = item.item.id.clone();
            for other_item in &items_clone {
                if other_item.covers.contains(&item_id) {
                    let link_status = self.determine_incoming_link_status(&item_id, &other_item.id);
                    item.add_incoming_link(other_item.id.clone(), link_status);
                }
            }
        }

        Ok(())
    }

    /// Determine the status of an outgoing link
    fn determine_link_status(
        &self,
        covered_id: &SpecificationItemId,
        items_by_id: &HashMap<SpecificationItemId, SpecificationItem>,
    ) -> LinkStatus {
        match items_by_id.get(covered_id) {
            Some(covered_item) => {
                // Check if coverage is requested
                if covered_item.needs.is_empty() {
                    LinkStatus::Unwanted
                } else {
                    LinkStatus::Covers
                }
            }
            None => {
                // Check for items with same name but different revision
                let matching_items: Vec<_> = items_by_id
                    .keys()
                    .filter(|id| {
                        id.artifact_type == covered_id.artifact_type && id.name == covered_id.name
                    })
                    .collect();

                if matching_items.is_empty() {
                    LinkStatus::Orphaned
                } else if matching_items.len() > 1 {
                    LinkStatus::Ambiguous
                } else {
                    let existing_item = matching_items[0];
                    if existing_item.revision > covered_id.revision {
                        LinkStatus::Outdated
                    } else {
                        LinkStatus::Predated
                    }
                }
            }
        }
    }

    /// Determine the status of an incoming link
    fn determine_incoming_link_status(
        &self,
        _item_id: &SpecificationItemId,
        _covering_id: &SpecificationItemId,
    ) -> LinkStatus {
        // For now, assume all incoming links are valid
        // In a more sophisticated implementation, we would check revision compatibility
        LinkStatus::CoveredShallow
    }

    /// Analyze coverage status for each item
    fn analyze_coverage(&self, linked_items: &mut [LinkedSpecificationItem]) {
        // Clone the data we need to avoid borrowing issues
        let items_data: Vec<_> = linked_items
            .iter()
            .map(|li| (li.item.clone(), li.outgoing_links.clone()))
            .collect();

        for linked_item in linked_items.iter_mut() {
            // If item has no requirements, it's considered covered (terminating item)
            if linked_item.item.needs.is_empty() {
                linked_item.coverage_status = CoverageStatus::Covered;
                // Still need to check for broken links even if no coverage requirements
            } else {
                // Check if all needed artifact types are covered
                let mut all_covered = true;
                let mut any_covered = false;

                for needed_type in &linked_item.item.needs.clone() {
                    let is_covered = self.is_artifact_type_covered_static(
                        &linked_item.item.id,
                        needed_type,
                        &items_data,
                    );
                    if is_covered {
                        any_covered = true;
                    } else {
                        all_covered = false;
                    }
                }

                // Determine overall coverage status
                linked_item.coverage_status = if all_covered {
                    CoverageStatus::Covered
                } else if any_covered {
                    CoverageStatus::Partial
                } else {
                    CoverageStatus::Uncovered
                };
            }

            // Mark as defect if not properly covered or has broken links (check for ALL items)
            let not_covered = !matches!(linked_item.coverage_status, CoverageStatus::Covered);
            let has_broken_links = linked_item.outgoing_links.iter().any(|link| {
                matches!(
                    link.status,
                    LinkStatus::Orphaned
                        | LinkStatus::Ambiguous
                        | LinkStatus::Outdated
                        | LinkStatus::Predated
                        | LinkStatus::Duplicate
                )
            });

            linked_item.is_defect = not_covered || has_broken_links;
        }
    }

    /// Check if a specific artifact type is covered for an item (static version to avoid borrowing issues)
    fn is_artifact_type_covered_static(
        &self,
        item_id: &SpecificationItemId,
        artifact_type: &str,
        items_data: &[(SpecificationItem, Vec<crate::core::Link>)],
    ) -> bool {
        items_data.iter().any(|(item, outgoing_links)| {
            item.id.artifact_type == artifact_type
                && item.covers.contains(item_id)
                && outgoing_links.iter().any(|link| {
                    link.target_id == *item_id && matches!(link.status, LinkStatus::Covers)
                })
        })
    }
}

impl Default for Linker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{SpecificationItem, SpecificationItemId};

    #[test]
    fn test_simple_linking() {
        let linker = Linker::new();

        // Create a feature and a requirement that covers it
        let feat_id = SpecificationItemId::new("feat".to_string(), "login".to_string(), 1);
        let req_id = SpecificationItemId::new("req".to_string(), "login".to_string(), 1);

        let feat = SpecificationItem::builder(feat_id.clone())
            .needs("req".to_string())
            .build();

        let req = SpecificationItem::builder(req_id.clone())
            .covers(feat_id.clone())
            .build();

        let items = vec![feat, req];
        let linked_items = linker.link_items(items).unwrap();

        assert_eq!(linked_items.len(), 2);

        // Check that feature is covered
        let feat_linked = linked_items
            .iter()
            .find(|li| li.item.id == feat_id)
            .unwrap();
        assert!(feat_linked.is_covered());

        // Check that requirement has outgoing link
        let req_linked = linked_items.iter().find(|li| li.item.id == req_id).unwrap();
        assert!(req_linked
            .outgoing_links
            .iter()
            .any(|link| link.target_id == feat_id));
    }
}
