use crate::config::Config;
use crate::core::{ItemStatus, Location, SpecificationItem, SpecificationItemId};
use crate::Result;
use regex::Regex;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Importer for parsing requirement specifications from markdown files
/// [impl->dsn~markdown-importer-module~1]
pub struct MarkdownImporter {
    /// Regex for matching specification item IDs like `req~user-login~1`
    id_regex: Regex,
    /// Regex for matching needs lines like "Needs: impl, utest"
    needs_regex: Regex,
    /// Regex for matching covers lines like "Covers:" followed by bullet points
    covers_regex: Regex,
    /// Regex for matching inline covers like "Covers: req~user~1, dsn~auth~1"
    covers_inline_regex: Regex,
    /// Regex for matching depends lines like "Depends:" followed by bullet points
    depends_regex: Regex,
    /// Regex for matching tags lines like "Tags: security, authentication"
    tags_regex: Regex,
    /// Regex for matching status lines like "Status: approved"
    status_regex: Regex,
    /// Regex for matching rationale sections
    rationale_regex: Regex,
    /// Regex for matching comment sections
    comment_regex: Regex,
    /// Regex for matching specification item references in lists
    item_ref_regex: Regex,
}

impl MarkdownImporter {
    /// Create a new markdown importer
    pub fn new() -> Self {
        Self {
            id_regex: Regex::new(r"`([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)`").unwrap(),
            needs_regex: Regex::new(r"(?i)^\*?\*?Needs:\*?\*?\s*(.+)$").unwrap(),
            covers_regex: Regex::new(r"(?i)^\*?\*?Covers:\*?\*?\s*$").unwrap(),
            covers_inline_regex: Regex::new(r"(?i)^\*?\*?Covers:\*?\*?\s*(.+)$").unwrap(),
            depends_regex: Regex::new(r"(?i)^\*?\*?Depends:\*?\*?\s*$").unwrap(),
            tags_regex: Regex::new(r"(?i)^\*?\*?Tags:\*?\*?\s*(.+)$").unwrap(),
            status_regex: Regex::new(
                r"(?i)^\*?\*?Status:\*?\*?\s*(draft|proposed|approved|rejected)\s*$",
            )
            .unwrap(),
            rationale_regex: Regex::new(r"(?i)^\*?\*?Rationale:\*?\*?\s*$").unwrap(),
            comment_regex: Regex::new(r"(?i)^\*?\*?Comment:\*?\*?\s*$").unwrap(),
            item_ref_regex: Regex::new(r"([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)").unwrap(),
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

            if path.is_file() && self.is_markdown_file(path) {
                let file_items = self.import_from_file(path)?;
                items.extend(file_items);
            }
        }

        Ok(items)
    }

    /// Import specification items from a single markdown file
    pub fn import_from_file(&self, file_path: &Path) -> Result<Vec<SpecificationItem>> {
        let content = fs::read_to_string(file_path)?;
        self.parse_markdown(&content, file_path)
    }

    /// Parse markdown content for specification items
    fn parse_markdown(&self, content: &str, file_path: &Path) -> Result<Vec<SpecificationItem>> {
        let mut items = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        let mut line_number = 0;

        while line_number < lines.len() {
            let line = lines[line_number];

            // Look for specification item IDs in regular text (backticks)
            if let Some(captures) = self.id_regex.captures(line) {
                if let Some(item) =
                    self.parse_specification_item(&lines, &mut line_number, file_path, &captures)?
                {
                    items.push(item);
                }
            }
            // Also look for specification item IDs in headings
            else if self.is_heading(line) {
                let heading_text = self.extract_heading_text(line);
                if let Some(captures) = self.item_ref_regex.captures(&heading_text) {
                    if let Some(item) = self.parse_specification_item(
                        &lines,
                        &mut line_number,
                        file_path,
                        &captures,
                    )? {
                        items.push(item);
                    }
                }
            }

            line_number += 1;
        }

        Ok(items)
    }

    /// Parse a complete specification item starting from the ID line
    fn parse_specification_item(
        &self,
        lines: &[&str],
        line_number: &mut usize,
        file_path: &Path,
        id_captures: &regex::Captures,
    ) -> Result<Option<SpecificationItem>> {
        let artifact_type = id_captures.get(1).unwrap().as_str();
        let name = id_captures.get(2).unwrap().as_str();
        let revision_str = id_captures.get(3).unwrap().as_str();
        let revision = revision_str
            .parse::<u32>()
            .map_err(|_| crate::Error::Parse {
                message: format!("Invalid revision number: {}", revision_str),
                location: format!("{}:{}", file_path.display(), *line_number + 1),
            })?;

        let id = SpecificationItemId::new(artifact_type.to_string(), name.to_string(), revision);

        let location = Location::new(file_path.to_path_buf(), (*line_number + 1) as u32);
        let mut builder = SpecificationItem::builder(id).location(location);

        // Look for title (if the ID is preceded by a heading, or extract from heading if ID is in heading)
        if *line_number > 0 {
            let prev_line = lines[*line_number - 1];
            if self.is_heading(prev_line) {
                let title = self.extract_heading_text(prev_line);
                builder = builder.title(title);
            }
        }

        // If ID is in a heading line itself, extract title from that line
        let current_line = lines[*line_number];
        if self.is_heading(current_line) {
            let heading_text = self.extract_heading_text(current_line);
            // Remove the ID part from the heading to get title
            if let Some(pos) =
                heading_text.find(&format!("{}~{}~{}", artifact_type, name, revision))
            {
                let title_part = heading_text
                    [pos + format!("{}~{}~{}", artifact_type, name, revision).len()..]
                    .trim();
                if !title_part.is_empty() {
                    builder = builder.title(title_part.to_string());
                } else {
                    // Use the ID as title if no additional text
                    builder = builder.title(format!("{}~{}~{}", artifact_type, name, revision));
                }
            } else {
                builder = builder.title(heading_text);
            }
        }

        // Parse the specification item content
        *line_number += 1;
        let mut current_section = Section::Description;
        let mut description = String::new();
        let mut rationale = String::new();
        let mut comment = String::new();
        let mut covers_list = Vec::new();
        let mut depends_list = Vec::new();

        while *line_number < lines.len() {
            let line = lines[*line_number];

            // Check if we've reached another specification item
            if self.id_regex.is_match(line) {
                *line_number -= 1; // Back up so the outer loop can process this
                break;
            }
            // Also check for specification items in headings
            if self.is_heading(line) {
                let heading_text = self.extract_heading_text(line);
                if self.item_ref_regex.is_match(&heading_text) {
                    *line_number -= 1; // Back up so the outer loop can process this
                    break;
                }
            }

            // Check for section keywords
            if let Some(captures) = self.needs_regex.captures(line) {
                let needs_str = captures.get(1).unwrap().as_str();
                let needs = self.parse_list(needs_str);
                builder = builder.needs_multiple(needs);
            } else if let Some(captures) = self.covers_inline_regex.captures(line) {
                // Handle inline covers like "Covers: req~user~1, dsn~auth~1"
                let covers_str = captures.get(1).unwrap().as_str();
                let covers_list = self.parse_covers_list(covers_str);
                for cover_id in covers_list {
                    builder = builder.covers(cover_id);
                }
            } else if self.covers_regex.is_match(line) {
                current_section = Section::Covers;
            } else if self.depends_regex.is_match(line) {
                current_section = Section::Depends;
            } else if let Some(captures) = self.tags_regex.captures(line) {
                let tags_str = captures.get(1).unwrap().as_str();
                let tags = self.parse_list(tags_str);
                builder = builder.tags(tags);
            } else if let Some(captures) = self.status_regex.captures(line) {
                let status_str = captures.get(1).unwrap().as_str();
                let status = match status_str.to_lowercase().as_str() {
                    "draft" => ItemStatus::Draft,
                    "proposed" => ItemStatus::Proposed,
                    "approved" => ItemStatus::Approved,
                    "rejected" => ItemStatus::Rejected,
                    _ => ItemStatus::Approved,
                };
                builder = builder.status(status);
            } else if self.rationale_regex.is_match(line) {
                current_section = Section::Rationale;
            } else if self.comment_regex.is_match(line) {
                current_section = Section::Comment;
            } else if line.trim().starts_with('-')
                || line.trim().starts_with('*')
                || line.trim().starts_with('+')
            {
                // Handle bullet point lists
                match current_section {
                    Section::Covers => {
                        if let Some(item_id) = self.extract_item_reference(line) {
                            covers_list.push(item_id);
                        }
                    }
                    Section::Depends => {
                        if let Some(item_id) = self.extract_item_reference(line) {
                            depends_list.push(item_id);
                        }
                    }
                    _ => {
                        self.append_to_section(
                            &mut description,
                            &mut rationale,
                            &mut comment,
                            current_section,
                            line,
                        );
                    }
                }
            } else if !line.trim().is_empty() {
                // Regular content line
                self.append_to_section(
                    &mut description,
                    &mut rationale,
                    &mut comment,
                    current_section,
                    line,
                );
            }

            *line_number += 1;
        }

        // Build the final specification item
        if !description.trim().is_empty() {
            builder = builder.description(description.trim().to_string());
        }
        if !rationale.trim().is_empty() {
            builder = builder.rationale(rationale.trim().to_string());
        }
        if !comment.trim().is_empty() {
            builder = builder.comment(comment.trim().to_string());
        }
        if !covers_list.is_empty() {
            builder = builder.covers_multiple(covers_list);
        }
        if !depends_list.is_empty() {
            for dep in depends_list {
                builder = builder.depends(dep);
            }
        }

        Ok(Some(builder.build()))
    }

    /// Append text to the appropriate section
    fn append_to_section(
        &self,
        description: &mut String,
        rationale: &mut String,
        comment: &mut String,
        section: Section,
        line: &str,
    ) {
        let text = if line.trim().is_empty() {
            "\n".to_string()
        } else {
            format!("{}\n", line)
        };

        match section {
            Section::Description => description.push_str(&text),
            Section::Rationale => rationale.push_str(&text),
            Section::Comment => comment.push_str(&text),
            _ => description.push_str(&text), // Default to description
        }
    }

    /// Check if a line is a heading
    fn is_heading(&self, line: &str) -> bool {
        line.trim_start().starts_with('#')
    }

    /// Extract text from a heading line
    fn extract_heading_text(&self, line: &str) -> String {
        line.trim_start().trim_start_matches('#').trim().to_string()
    }

    /// Parse a comma-separated list of covers
    fn parse_covers_list(&self, covers_str: &str) -> Vec<SpecificationItemId> {
        covers_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .filter_map(|s| {
                if let Some(captures) = self.item_ref_regex.captures(s) {
                    let artifact_type = captures.get(1).unwrap().as_str();
                    let name = captures.get(2).unwrap().as_str();
                    let revision = captures.get(3).unwrap().as_str().parse::<u32>().ok()?;
                    Some(SpecificationItemId::new(
                        artifact_type.to_string(),
                        name.to_string(),
                        revision,
                    ))
                } else {
                    None
                }
            })
            .collect()
    }

    /// Parse a comma-separated list
    fn parse_list(&self, list_str: &str) -> Vec<String> {
        list_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Extract a specification item reference from a line
    fn extract_item_reference(&self, line: &str) -> Option<SpecificationItemId> {
        if let Some(captures) = self.item_ref_regex.captures(line) {
            let artifact_type = captures.get(1)?.as_str();
            let name = captures.get(2)?.as_str();
            let revision = captures.get(3)?.as_str().parse::<u32>().ok()?;

            Some(SpecificationItemId::new(
                artifact_type.to_string(),
                name.to_string(),
                revision,
            ))
        } else {
            None
        }
    }

    /// Check if a file is a markdown file
    fn is_markdown_file(&self, path: &Path) -> bool {
        let config = Config::default();
        config.is_spec_file(path)
    }
}

/// Current section being parsed
#[derive(Debug, Clone, Copy)]
enum Section {
    Description,
    Rationale,
    Comment,
    Covers,
    Depends,
}

impl Default for MarkdownImporter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_simple_requirement() {
        let importer = MarkdownImporter::new();
        let content = r#"
# User Authentication
`req~user-authentication~1`

The system shall support user authentication with username and password.

Needs: dsn, impl, utest
Tags: security, login
Status: approved
"#;

        let temp_file = NamedTempFile::new().unwrap();
        let items = importer.parse_markdown(content, temp_file.path()).unwrap();
        assert_eq!(items.len(), 1);

        let item = &items[0];
        assert_eq!(item.id.artifact_type, "req");
        assert_eq!(item.id.name, "user-authentication");
        assert_eq!(item.id.revision, 1);
        assert_eq!(item.title, Some("User Authentication".to_string()));
        assert!(item.description.is_some());
        assert_eq!(item.needs, vec!["dsn", "impl", "utest"]);
        assert_eq!(item.tags, vec!["security", "login"]);
        assert_eq!(item.status, ItemStatus::Approved);
    }

    #[test]
    fn test_parse_requirement_with_covers() {
        let importer = MarkdownImporter::new();
        let content = r#"
`dsn~authentication-service~1`

The authentication service validates user credentials.

Covers:
- req~user-authentication~1
- req~password-validation~1

Needs: impl, utest
"#;

        let temp_file = NamedTempFile::new().unwrap();
        let items = importer.parse_markdown(content, temp_file.path()).unwrap();
        assert_eq!(items.len(), 1);

        let item = &items[0];
        assert_eq!(item.covers.len(), 2);
        assert_eq!(item.covers[0].name, "user-authentication");
        assert_eq!(item.covers[1].name, "password-validation");
    }

    #[test]
    fn test_parse_requirement_with_rationale() {
        let importer = MarkdownImporter::new();
        let content = r#"
`req~secure-password~1`

Passwords must be at least 8 characters long.

Rationale:
This requirement ensures basic password security and reduces the risk
of brute force attacks.

Comment:
Consider implementing additional password complexity requirements
in future versions.

Needs: dsn
"#;

        let temp_file = NamedTempFile::new().unwrap();
        let items = importer.parse_markdown(content, temp_file.path()).unwrap();
        assert_eq!(items.len(), 1);

        let item = &items[0];
        assert!(item.rationale.is_some());
        assert!(item.comment.is_some());
        assert!(item
            .rationale
            .as_ref()
            .unwrap()
            .contains("password security"));
        assert!(item.comment.as_ref().unwrap().contains("future versions"));
    }

    #[test]
    fn test_import_from_file() {
        let importer = MarkdownImporter::new();
        let mut temp_file = NamedTempFile::with_suffix(".md").unwrap();
        writeln!(temp_file, "# Requirements Document").unwrap();
        writeln!(temp_file).unwrap();
        writeln!(temp_file, "## Authentication").unwrap();
        writeln!(temp_file, "`req~auth~1`").unwrap();
        writeln!(temp_file).unwrap();
        writeln!(temp_file, "User authentication is required.").unwrap();
        writeln!(temp_file).unwrap();
        writeln!(temp_file, "Needs: dsn").unwrap();

        let items = importer.import_from_file(temp_file.path()).unwrap();
        assert_eq!(items.len(), 1);

        let item = &items[0];
        assert_eq!(item.id.name, "auth");
        assert_eq!(item.title, Some("Authentication".to_string()));
        assert_eq!(item.needs, vec!["dsn"]);
    }
}
