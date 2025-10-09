use std::fs;
use std::path::Path;
use regex::Regex;
use walkdir::WalkDir;

use crate::core::{SpecificationItem, SpecificationItemId, Location};
use crate::config::Config;
use crate::Result;

/// Importer for parsing requirement tags from source code files
/// [impl->dsn~tag-importer-module~1]
pub struct TagImporter {
    /// Regex for matching full coverage tags like [impl->dsn~validate-authentication-request~1]
    full_tag_regex: Regex,
    /// Regex for matching short tags like [[req~name~1:impl]]
    short_tag_regex: Regex,
}

impl TagImporter {
    /// Create a new tag importer
    pub fn new() -> Self {
        Self {
            // Full tag format: [artifact_type->covered_id] or [artifact_type~name~revision->covered_id]
            full_tag_regex: Regex::new(
                r"\[\s*([a-zA-Z]+)(?:~([a-zA-Z0-9._-]+)~(\d+))?\s*->\s*([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)\s*(?:>>\s*([a-zA-Z0-9,\s]+))?\s*\]"
            ).unwrap(),
            // Short tag format: [[item_id:artifact_type]]
            short_tag_regex: Regex::new(
                r"\[\[\s*([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)\s*:\s*([a-zA-Z]+)\s*\]\]"
            ).unwrap(),
        }
    }

    /// Import specification items from a directory
    pub fn import_from_directory(&self, dir: &Path) -> Result<Vec<SpecificationItem>> {
        let mut items = Vec::new();
        
        if !dir.exists() {
            log::warn!("Directory does not exist: {}", dir.display());
            return Ok(items);
        }

        for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            
            if path.is_file() && self.should_scan_file(path) {
                let file_items = self.import_from_file(path)?;
                items.extend(file_items);
            }
        }

        Ok(items)
    }

    /// Import specification items from a single file
    pub fn import_from_file(&self, file_path: &Path) -> Result<Vec<SpecificationItem>> {
        let content = fs::read_to_string(file_path)?;
        let mut items = Vec::new();
        
        for (line_number, line) in content.lines().enumerate() {
            let line_items = self.parse_line(line, file_path, line_number as u32 + 1)?;
            items.extend(line_items);
        }

        Ok(items)
    }

    /// Parse a single line for requirement tags
    fn parse_line(&self, line: &str, file_path: &Path, line_number: u32) -> Result<Vec<SpecificationItem>> {
        let mut items = Vec::new();
        let location = Location::new(file_path.to_path_buf(), line_number);

        // Try to match full tag format
        for captures in self.full_tag_regex.captures_iter(line) {
            if let Some(item) = self.parse_full_tag(&captures, &location)? {
                items.push(item);
            }
        }

        // Try to match short tag format
        for captures in self.short_tag_regex.captures_iter(line) {
            if let Some(item) = self.parse_short_tag(&captures, &location)? {
                items.push(item);
            }
        }

        Ok(items)
    }

    /// Parse a full tag like [impl->dsn~validate-authentication-request~1]
    fn parse_full_tag(&self, captures: &regex::Captures, location: &Location) -> Result<Option<SpecificationItem>> {
        let artifact_type = captures.get(1).unwrap().as_str();
        let name = captures.get(2).map(|m| m.as_str());
        let revision = captures.get(3).map(|m| m.as_str());
        let covered_artifact_type = captures.get(4).unwrap().as_str();
        let covered_name = captures.get(5).unwrap().as_str();
        let covered_revision_str = captures.get(6).unwrap().as_str();
        let covered_revision = covered_revision_str.parse::<u32>()
            .map_err(|_| crate::Error::Parse {
                message: format!("Invalid revision number: {}", covered_revision_str),
                location: location.to_string(),
            })?;
        let needs_str = captures.get(7).map(|m| m.as_str());

        // Create the covering item
        let item_name = if let (Some(name), Some(_revision)) = (name, revision) {
            name.to_string()
        } else {
            // Generate a name based on the covered item and location
            format!("{}-{}", covered_name, self.generate_hash(&location.to_string()))
        };

        let item_revision = if let Some(revision) = revision {
            revision.parse::<u32>().map_err(|_| crate::Error::Parse {
                message: format!("Invalid revision number: {}", revision),
                location: location.to_string(),
            })?
        } else {
            0 // Default revision for auto-generated items
        };

        let item_id = SpecificationItemId::new(
            artifact_type.to_string(),
            item_name,
            item_revision,
        );

        let covered_id = SpecificationItemId::new(
            covered_artifact_type.to_string(),
            covered_name.to_string(),
            covered_revision,
        );

        let mut builder = SpecificationItem::builder(item_id)
            .covers(covered_id)
            .location(location.clone());

        // Parse needs if present
        if let Some(needs_str) = needs_str {
            let needs = self.parse_needs_list(needs_str);
            builder = builder.needs_multiple(needs);
        }

        Ok(Some(builder.build()))
    }

    /// Parse a short tag like [[req~name~1:impl]]
    fn parse_short_tag(&self, captures: &regex::Captures, location: &Location) -> Result<Option<SpecificationItem>> {
        let covered_artifact_type = captures.get(1).unwrap().as_str();
        let covered_name = captures.get(2).unwrap().as_str();
        let covered_revision_str = captures.get(3).unwrap().as_str();
        let covered_revision = covered_revision_str.parse::<u32>()
            .map_err(|_| crate::Error::Parse {
                message: format!("Invalid revision number: {}", covered_revision_str),
                location: location.to_string(),
            })?;
        let artifact_type = captures.get(4).unwrap().as_str();

        // Create the covering item
        let item_name = format!("{}-{}", covered_name, self.generate_hash(&location.to_string()));
        let item_id = SpecificationItemId::new(
            artifact_type.to_string(),
            item_name,
            0, // Default revision for auto-generated items
        );

        let covered_id = SpecificationItemId::new(
            covered_artifact_type.to_string(),
            covered_name.to_string(),
            covered_revision,
        );

        let item = SpecificationItem::builder(item_id)
            .covers(covered_id)
            .location(location.clone())
            .build();

        Ok(Some(item))
    }

    /// Parse a comma-separated list of needed artifact types
    fn parse_needs_list(&self, needs_str: &str) -> Vec<String> {
        needs_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Generate a hash for auto-generated item names
    fn generate_hash(&self, input: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        input.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if a file should be scanned for tags
    fn should_scan_file(&self, path: &Path) -> bool {
        // Use default config for now, could be made configurable
        let config = Config::default();
        config.matches_source_pattern(path)
    }
}

impl Default for TagImporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_parse_full_tag() {
        let importer = TagImporter::new();
        let content = "// [impl->dsn~validate-authentication-request~1]";
        let temp_file = NamedTempFile::new().unwrap();
        
        let items = importer.parse_line(content, temp_file.path(), 1).unwrap();
        assert_eq!(items.len(), 1);
        
        let item = &items[0];
        assert_eq!(item.id.artifact_type, "impl");
        assert_eq!(item.covers.len(), 1);
        assert_eq!(item.covers[0].artifact_type, "dsn");
        assert_eq!(item.covers[0].name, "validate-authentication-request");
        assert_eq!(item.covers[0].revision, 1);
    }

    #[test]
    fn test_parse_tag_with_needs() {
        let importer = TagImporter::new();
        let content = "// [dsn->feat~login~1>>impl,test]";
        let temp_file = NamedTempFile::new().unwrap();
        
        let items = importer.parse_line(content, temp_file.path(), 1).unwrap();
        assert_eq!(items.len(), 1);
        
        let item = &items[0];
        assert_eq!(item.id.artifact_type, "dsn");
        assert_eq!(item.needs, vec!["impl", "test"]);
    }

    #[test]
    fn test_parse_short_tag() {
        let importer = TagImporter::new();
        let content = "// [[req~login~1:impl]]";
        let temp_file = NamedTempFile::new().unwrap();
        
        let items = importer.parse_line(content, temp_file.path(), 1).unwrap();
        assert_eq!(items.len(), 1);
        
        let item = &items[0];
        assert_eq!(item.id.artifact_type, "impl");
        assert_eq!(item.covers.len(), 1);
        assert_eq!(item.covers[0].artifact_type, "req");
    }

    #[test]
    fn test_import_from_file() {
        let importer = TagImporter::new();
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "// Test file with requirements").unwrap();
        writeln!(temp_file, "// [impl->dsn~authenticate-user~1]").unwrap();
        writeln!(temp_file, "fn authenticate_user() {{}}").unwrap();
        writeln!(temp_file, "// [utest->dsn~authenticate-user~1]").unwrap();
        writeln!(temp_file, "#[test] fn test_authenticate() {{}}").unwrap();
        
        let items = importer.import_from_file(temp_file.path()).unwrap();
        assert_eq!(items.len(), 2);
        
        let impl_item = items.iter().find(|i| i.id.artifact_type == "impl").unwrap();
        let test_item = items.iter().find(|i| i.id.artifact_type == "utest").unwrap();
        
        assert_eq!(impl_item.covers[0].name, "authenticate-user");
        assert_eq!(test_item.covers[0].name, "authenticate-user");
    }
}
