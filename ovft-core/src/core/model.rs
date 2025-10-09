use std::fmt;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Represents a specification item ID with artifact type, name, and revision
/// [impl->dsn~core-data-models~1]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpecificationItemId {
    /// Artifact type (e.g., "feat", "req", "dsn", "impl", "utest")
    pub artifact_type: String,
    /// Item name (e.g., "user-authentication", "validate-input")
    pub name: String,
    /// Revision number (typically starts at 1)
    pub revision: u32,
}

impl SpecificationItemId {
    /// Create a new specification item ID
    pub fn new(artifact_type: String, name: String, revision: u32) -> Self {
        Self {
            artifact_type,
            name,
            revision,
        }
    }

    /// Parse a specification item ID from string format like "req~user-login~1"
    pub fn parse(id_str: &str) -> crate::Result<Self> {
        let parts: Vec<&str> = id_str.split('~').collect();
        if parts.len() != 3 {
            return Err(crate::Error::InvalidId(format!(
                "Invalid ID format '{}'. Expected format: 'type~name~revision'",
                id_str
            )));
        }

        let artifact_type = parts[0].to_string();
        let name = parts[1].to_string();
        let revision = parts[2].parse::<u32>().map_err(|_| {
            crate::Error::InvalidId(format!(
                "Invalid revision number '{}' in ID '{}'",
                parts[2], id_str
            ))
        })?;

        Ok(Self::new(artifact_type, name, revision))
    }
}

impl fmt::Display for SpecificationItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}~{}~{}", self.artifact_type, self.name, self.revision)
    }
}

/// Status of a specification item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemStatus {
    Draft,
    Proposed,
    Approved,
    Rejected,
}

impl Default for ItemStatus {
    fn default() -> Self {
        Self::Approved
    }
}

impl fmt::Display for ItemStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Draft => write!(f, "draft"),
            Self::Proposed => write!(f, "proposed"),
            Self::Approved => write!(f, "approved"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

/// Source location of a specification item
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Location {
    /// File path where the item is defined
    pub path: PathBuf,
    /// Line number in the file
    pub line: u32,
}

impl Location {
    pub fn new(path: PathBuf, line: u32) -> Self {
        Self { path, line }
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.path.display(), self.line)
    }
}

/// A specification item representing a requirement, design, implementation, or test
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecificationItem {
    /// Unique identifier for this item
    pub id: SpecificationItemId,
    /// Optional title/summary
    pub title: Option<String>,
    /// Description of the item
    pub description: Option<String>,
    /// Rationale for the item
    pub rationale: Option<String>,
    /// Additional comments
    pub comment: Option<String>,
    /// Status of the item
    pub status: ItemStatus,
    /// Tags associated with this item
    pub tags: Vec<String>,
    /// Artifact types that this item needs to be covered by
    pub needs: Vec<String>,
    /// Specification items that this item covers
    pub covers: Vec<SpecificationItemId>,
    /// Dependencies on other specification items
    pub depends: Vec<SpecificationItemId>,
    /// Source location where this item is defined
    pub location: Option<Location>,
}

impl SpecificationItem {
    /// Create a new specification item with minimal required fields
    pub fn new(id: SpecificationItemId) -> Self {
        Self {
            id,
            title: None,
            description: None,
            rationale: None,
            comment: None,
            status: ItemStatus::default(),
            tags: Vec::new(),
            needs: Vec::new(),
            covers: Vec::new(),
            depends: Vec::new(),
            location: None,
        }
    }

    /// Builder pattern for creating specification items
    pub fn builder(id: SpecificationItemId) -> SpecificationItemBuilder {
        SpecificationItemBuilder::new(id)
    }

    /// Get the title or generate one from the ID if not set
    pub fn title_or_fallback(&self) -> String {
        self.title
            .clone()
            .unwrap_or_else(|| self.id.name.replace('-', " ").replace('_', " "))
    }

    /// Check if this item is a terminating item (doesn't need coverage)
    pub fn is_terminating(&self) -> bool {
        self.needs.is_empty()
    }
}

/// Builder for creating specification items
pub struct SpecificationItemBuilder {
    item: SpecificationItem,
}

impl SpecificationItemBuilder {
    pub fn new(id: SpecificationItemId) -> Self {
        Self {
            item: SpecificationItem::new(id),
        }
    }

    pub fn title(mut self, title: String) -> Self {
        self.item.title = Some(title);
        self
    }

    pub fn description(mut self, description: String) -> Self {
        self.item.description = Some(description);
        self
    }

    pub fn rationale(mut self, rationale: String) -> Self {
        self.item.rationale = Some(rationale);
        self
    }

    pub fn comment(mut self, comment: String) -> Self {
        self.item.comment = Some(comment);
        self
    }

    pub fn status(mut self, status: ItemStatus) -> Self {
        self.item.status = status;
        self
    }

    pub fn tag(mut self, tag: String) -> Self {
        self.item.tags.push(tag);
        self
    }

    pub fn tags(mut self, tags: Vec<String>) -> Self {
        self.item.tags.extend(tags);
        self
    }

    pub fn needs(mut self, artifact_type: String) -> Self {
        self.item.needs.push(artifact_type);
        self
    }

    pub fn needs_multiple(mut self, artifact_types: Vec<String>) -> Self {
        self.item.needs.extend(artifact_types);
        self
    }

    pub fn covers(mut self, covered_id: SpecificationItemId) -> Self {
        self.item.covers.push(covered_id);
        self
    }

    pub fn covers_multiple(mut self, covered_ids: Vec<SpecificationItemId>) -> Self {
        self.item.covers.extend(covered_ids);
        self
    }

    pub fn depends(mut self, dependency: SpecificationItemId) -> Self {
        self.item.depends.push(dependency);
        self
    }

    pub fn location(mut self, location: Location) -> Self {
        self.item.location = Some(location);
        self
    }

    pub fn build(self) -> SpecificationItem {
        self.item
    }
}

/// Status of a link between specification items
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkStatus {
    /// Link is valid and current
    Covers,
    /// Item covers a newer revision than expected
    Predated,
    /// Item covers an older revision than expected
    Outdated,
    /// Multiple items with the same ID exist
    Ambiguous,
    /// Coverage is provided but not requested
    Unwanted,
    /// Item covers a non-existing item
    Orphaned,
    /// Item is covered by another item
    CoveredShallow,
    /// Item is covered but coverage is unwanted
    CoveredUnwanted,
    /// Item is covered with wrong revision
    CoveredPredated,
    /// Item is covered with old revision
    CoveredOutdated,
    /// Duplicate item IDs exist
    Duplicate,
}

impl fmt::Display for LinkStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Covers => write!(f, "covers"),
            Self::Predated => write!(f, "predated"),
            Self::Outdated => write!(f, "outdated"),
            Self::Ambiguous => write!(f, "ambiguous"),
            Self::Unwanted => write!(f, "unwanted"),
            Self::Orphaned => write!(f, "orphaned"),
            Self::CoveredShallow => write!(f, "covered shallow"),
            Self::CoveredUnwanted => write!(f, "covered unwanted"),
            Self::CoveredPredated => write!(f, "covered predated"),
            Self::CoveredOutdated => write!(f, "covered outdated"),
            Self::Duplicate => write!(f, "duplicate"),
        }
    }
}

/// Coverage status for specification items
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoverageStatus {
    /// Item is properly covered
    Covered,
    /// Item lacks required coverage
    Uncovered,
    /// Item has partial coverage
    Partial,
}

impl fmt::Display for CoverageStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Covered => write!(f, "covered"),
            Self::Uncovered => write!(f, "uncovered"),
            Self::Partial => write!(f, "partial"),
        }
    }
}

/// Linked specification item with tracing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedSpecificationItem {
    /// The original specification item
    pub item: SpecificationItem,
    /// Items that this item covers (outgoing links)
    pub outgoing_links: Vec<Link>,
    /// Items that cover this item (incoming links)
    pub incoming_links: Vec<Link>,
    /// Coverage status for each needed artifact type
    pub coverage_status: CoverageStatus,
    /// Whether this item has defects
    pub is_defect: bool,
}

/// A link between specification items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// ID of the source item (for incoming links) or target item (for outgoing links)
    pub source_id: Option<SpecificationItemId>,
    /// ID of the target item (for outgoing links) or source item (for incoming links)
    pub target_id: SpecificationItemId,
    /// Status of the link
    pub status: LinkStatus,
}

/// Defect found during tracing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Defect {
    /// Type of defect
    pub defect_type: DefectType,
    /// Description of the defect
    pub description: String,
    /// ID of the item with the defect (if applicable)
    pub item_id: Option<SpecificationItemId>,
}

/// Types of defects that can be found
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DefectType {
    /// Item lacks required coverage
    UncoveredItem,
    /// Item covers a non-existing item
    OrphanedCoverage,
    /// Multiple items with the same ID
    DuplicateItem,
    /// Item covers wrong revision
    WrongRevision,
    /// Circular dependency detected
    CircularDependency,
}

impl fmt::Display for DefectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UncoveredItem => write!(f, "uncovered"),
            Self::OrphanedCoverage => write!(f, "orphaned"),
            Self::DuplicateItem => write!(f, "duplicate"),
            Self::WrongRevision => write!(f, "wrong-revision"),
            Self::CircularDependency => write!(f, "circular-dependency"),
        }
    }
}

/// Coverage summary for an artifact type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageSummary {
    /// Total number of items
    pub total: usize,
    /// Number of covered items
    pub covered: usize,
    /// Coverage percentage
    pub percentage: f64,
    /// Overall status
    pub status: CoverageStatus,
}

impl LinkedSpecificationItem {
    pub fn new(item: SpecificationItem) -> Self {
        Self {
            item,
            outgoing_links: Vec::new(),
            incoming_links: Vec::new(),
            coverage_status: CoverageStatus::Uncovered,
            is_defect: false,
        }
    }

    /// Get the ID of this item
    pub fn id(&self) -> &SpecificationItemId {
        &self.item.id
    }

    /// Get the title with fallback
    pub fn title(&self) -> String {
        self.item.title_or_fallback()
    }

    /// Check if this item is properly covered
    pub fn is_covered(&self) -> bool {
        matches!(self.coverage_status, CoverageStatus::Covered)
    }

    /// Add an outgoing link
    pub fn add_outgoing_link(&mut self, target_id: SpecificationItemId, status: LinkStatus) {
        self.outgoing_links.push(Link {
            source_id: Some(self.item.id.clone()),
            target_id,
            status,
        });
    }

    /// Add an incoming link
    pub fn add_incoming_link(&mut self, source_id: SpecificationItemId, status: LinkStatus) {
        self.incoming_links.push(Link {
            source_id: Some(source_id),
            target_id: self.item.id.clone(),
            status,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_specification_item_id_parse() {
        let id = SpecificationItemId::parse("req~user-login~1").unwrap();
        assert_eq!(id.artifact_type, "req");
        assert_eq!(id.name, "user-login");
        assert_eq!(id.revision, 1);
    }

    #[test]
    fn test_specification_item_id_display() {
        let id = SpecificationItemId::new("dsn".to_string(), "validate-input".to_string(), 2);
        assert_eq!(id.to_string(), "dsn~validate-input~2");
    }

    #[test]
    fn test_specification_item_builder() {
        let id = SpecificationItemId::new("feat".to_string(), "authentication".to_string(), 1);
        let item = SpecificationItem::builder(id.clone())
            .title("User Authentication".to_string())
            .description("The system shall support user authentication".to_string())
            .needs("req".to_string())
            .tag("security".to_string())
            .build();

        assert_eq!(item.id, id);
        assert_eq!(item.title, Some("User Authentication".to_string()));
        assert_eq!(item.needs, vec!["req"]);
        assert_eq!(item.tags, vec!["security"]);
    }
}
