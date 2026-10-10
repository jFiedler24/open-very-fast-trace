//! Release notes generation from requirement coverage changes.
//!
//! Compares the current trace against a baseline at a git ref (typically the
//! latest tag) and summarizes requirements that became covered as markdown
//! release notes.

use crate::config::Config;
use crate::core::{CoverageStatus, TraceResult, Tracer};
use crate::{Error, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// One newly covered requirement for the release notes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseNoteEntry {
    /// Artifact type (e.g. "feat", "req")
    pub artifact_type: String,
    /// Canonical item ID (e.g. "req~agent-auth~1")
    pub id: String,
    /// Human-readable title
    pub title: String,
    /// First line of the description, if any
    pub summary: Option<String>,
}

/// Default artifact types included in release notes
pub fn default_release_note_types() -> Vec<String> {
    vec!["feat".to_string(), "req".to_string()]
}

/// Find items that are covered now but were not covered at the baseline.
///
/// "Newly covered" means: the item's coverage status is `Covered` in the
/// current trace, and at the baseline it was either absent or not covered.
/// Only items whose artifact type is listed in `types` are included.
/// [impl->req~release-notes-from-requirements~1]
pub fn newly_covered_items(
    baseline: &TraceResult,
    current: &TraceResult,
    types: &[String],
) -> Vec<ReleaseNoteEntry> {
    let mut entries = Vec::new();

    for item in &current.items {
        let id = &item.item.id;
        if !types.iter().any(|t| t == &id.artifact_type) {
            continue;
        }
        if item.coverage_status != CoverageStatus::Covered {
            continue;
        }

        let was_covered = baseline.items.iter().any(|b| {
            b.item.id == *id && b.coverage_status == CoverageStatus::Covered
        });
        if was_covered {
            continue;
        }

        let summary = item
            .item
            .description
            .as_ref()
            .and_then(|d| d.lines().map(str::trim).find(|l| !l.is_empty()))
            .map(|l| l.to_string());

        entries.push(ReleaseNoteEntry {
            artifact_type: id.artifact_type.clone(),
            id: id.to_string(),
            title: item.item.title_or_fallback(),
            summary,
        });
    }

    entries.sort_by(|a, b| {
        a.artifact_type
            .cmp(&b.artifact_type)
            .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
    });
    entries
}

/// Render release notes entries as markdown, grouped by artifact type.
pub fn render_markdown(from_ref: &str, entries: &[ReleaseNoteEntry]) -> String {
    let mut out = format!("# Release notes ({} → HEAD)\n", from_ref);

    if entries.is_empty() {
        out.push_str("\nNo newly covered requirements.\n");
        return out;
    }

    let mut current_type = String::new();
    for entry in entries {
        if entry.artifact_type != current_type {
            current_type = entry.artifact_type.clone();
            out.push_str(&format!("\n## {}\n\n", current_type));
        }
        match &entry.summary {
            Some(summary) => out.push_str(&format!(
                "- **{}** (`{}`) — {}\n",
                entry.title, entry.id, summary
            )),
            None => out.push_str(&format!("- **{}** (`{}`)\n", entry.title, entry.id)),
        }
    }

    out
}

/// Determine the most recent git tag in the repository containing `repo_root`.
pub fn latest_tag(repo_root: &Path) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .map_err(|e| Error::Git(format!("failed to run git: {}", e)))?;

    if !output.status.success() {
        return Err(Error::Git(format!(
            "no git tag found in {}: {}",
            repo_root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// A temporary checkout of a git ref; removed on drop.
pub struct TempCheckout {
    path: PathBuf,
}

impl TempCheckout {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempCheckout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Extract the repository state at `git_ref` into a fresh temporary directory
/// using `git archive`.
pub fn checkout_ref(repo_root: &Path, git_ref: &str) -> Result<TempCheckout> {
    let mut dir = std::env::temp_dir();
    dir.push(format!(
        "ovft-release-notes-{}-{}",
        std::process::id(),
        git_ref.replace(['/', '~', '^'], "-")
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;

    let mut archive = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["archive", "--format=tar", git_ref])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Git(format!("failed to run git archive: {}", e)))?;

    let mut tar = Command::new("tar")
        .args(["-x", "-C"])
        .arg(&dir)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Git(format!("failed to run tar: {}", e)))?;

    if let (Some(mut archive_out), Some(mut tar_in)) = (archive.stdout.take(), tar.stdin.take()) {
        std::io::copy(&mut archive_out, &mut tar_in)?;
        tar_in.flush()?;
    }

    let archive_status = archive.wait()?;
    let tar_status = tar.wait()?;

    if !archive_status.success() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(Error::Git(format!(
            "git archive failed for ref '{}' (is it a valid git ref?)",
            git_ref
        )));
    }
    if !tar_status.success() {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(Error::Git("failed to extract git archive".to_string()));
    }

    Ok(TempCheckout { path: dir })
}

/// Run the tracer against the repository state at `git_ref`.
///
/// The config's source and spec dirs are reinterpreted relative to the
/// temporary checkout of that ref.
pub fn trace_at_ref(config: &Config, repo_root: &Path, git_ref: &str) -> Result<TraceResult> {
    let checkout = checkout_ref(repo_root, git_ref)?;

    let mut baseline_config = config.clone();
    baseline_config.source_dirs = config
        .source_dirs
        .iter()
        .map(|d| checkout.path().join(d))
        .collect();
    baseline_config.spec_dirs = config
        .spec_dirs
        .iter()
        .map(|d| checkout.path().join(d))
        .collect();

    Tracer::new(baseline_config).trace()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        CoverageStatus, LinkedSpecificationItem, SpecificationItem, SpecificationItemId,
    };
    use std::collections::HashMap;

    fn covered_item(artifact_type: &str, name: &str) -> LinkedSpecificationItem {
        let mut linked = LinkedSpecificationItem::new(
            SpecificationItem::builder(SpecificationItemId::new(
                artifact_type.to_string(),
                name.to_string(),
                1,
            ))
            .build(),
        );
        linked.coverage_status = CoverageStatus::Covered;
        linked
    }

    fn uncovered_item(artifact_type: &str, name: &str) -> LinkedSpecificationItem {
        let mut linked = covered_item(artifact_type, name);
        linked.coverage_status = CoverageStatus::Uncovered;
        linked
    }

    fn trace_result(items: Vec<LinkedSpecificationItem>) -> TraceResult {
        TraceResult {
            total_items: items.len(),
            items,
            defect_count: 0,
            defects: vec![],
            coverage_summary: HashMap::new(),
            is_success: true,
        }
    }

    #[test]
    fn test_newly_covered_items_diff() {
        let baseline = trace_result(vec![
            covered_item("req", "old-feature"),
            uncovered_item("req", "new-feature"),
        ]);
        let current = trace_result(vec![
            covered_item("req", "old-feature"),
            covered_item("req", "new-feature"),
            covered_item("req", "brand-new"),
            covered_item("impl", "some-tag"),
            uncovered_item("req", "still-open"),
        ]);

        let types = vec!["req".to_string()];
        let entries = newly_covered_items(&baseline, &current, &types);
        let names: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();

        assert_eq!(names, vec!["req~brand-new~1", "req~new-feature~1"]);
    }

    #[test]
    fn test_render_markdown_groups_by_type() {
        let entries = vec![
            ReleaseNoteEntry {
                artifact_type: "feat".to_string(),
                id: "feat~login~1".to_string(),
                title: "Login".to_string(),
                summary: Some("Users can log in.".to_string()),
            },
            ReleaseNoteEntry {
                artifact_type: "req".to_string(),
                id: "req~agent-auth~1".to_string(),
                title: "Agent auth".to_string(),
                summary: None,
            },
        ];

        let markdown = render_markdown("v0.1.0", &entries);
        assert!(markdown.contains("# Release notes (v0.1.0 → HEAD)"));
        assert!(markdown.contains("## feat"));
        assert!(markdown.contains("## req"));
        assert!(markdown.contains("- **Login** (`feat~login~1`) — Users can log in."));
        assert!(markdown.contains("- **Agent auth** (`req~agent-auth~1`)"));
    }

    #[test]
    fn test_render_markdown_empty() {
        let markdown = render_markdown("v0.1.0", &[]);
        assert!(markdown.contains("No newly covered requirements."));
    }
}
