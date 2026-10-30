use crate::core::ItemStatus;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// How `cargo ovft --check` treats defects of items with a given status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatusCheckPolicy {
    /// Defects of items with this status fail the check
    Fail,
    /// Defects of items with this status are reported but never fail the check
    Ignore,
}

/// Status-based coverage and check configuration (`[status]` in .ovft.toml)
/// [impl->dsn~status-check-policy-config~1]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StatusConfig {
    /// Statuses whose items count toward headline coverage statistics
    #[serde(with = "status_list_serde")]
    pub count: Vec<ItemStatus>,
    /// Per-status policy deciding whether defects fail `cargo ovft --check`
    #[serde(with = "status_check_map_serde")]
    pub check: BTreeMap<ItemStatus, StatusCheckPolicy>,
}

impl Default for StatusConfig {
    fn default() -> Self {
        let mut check = BTreeMap::new();
        check.insert(ItemStatus::Draft, StatusCheckPolicy::Ignore);
        check.insert(ItemStatus::Proposed, StatusCheckPolicy::Fail);
        check.insert(ItemStatus::Approved, StatusCheckPolicy::Fail);
        check.insert(ItemStatus::Rejected, StatusCheckPolicy::Ignore);
        Self {
            count: vec![ItemStatus::Proposed, ItemStatus::Approved],
            check,
        }
    }
}

impl StatusConfig {
    /// Whether items with the given status count toward headline coverage
    pub fn is_counted(&self, status: &ItemStatus) -> bool {
        self.count.contains(status)
    }

    /// The check policy for the given status (defaults to `Fail` for unlisted statuses)
    pub fn check_policy(&self, status: &ItemStatus) -> StatusCheckPolicy {
        self.check.get(status).copied().unwrap_or(StatusCheckPolicy::Fail)
    }
}

/// Serde helpers representing `ItemStatus` as its lowercase name (as in markdown specs)
mod status_list_serde {
    use crate::core::ItemStatus;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(statuses: &[ItemStatus], s: S) -> Result<S::Ok, S::Error> {
        let names: Vec<String> = statuses.iter().map(ToString::to_string).collect();
        names.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<ItemStatus>, D::Error> {
        let names = Vec::<String>::deserialize(d)?;
        names
            .iter()
            .map(|n| {
                n.parse::<ItemStatus>().map_err(serde::de::Error::custom)
            })
            .collect()
    }
}

/// Serde helpers representing the check policy map with lowercase status keys
mod status_check_map_serde {
    use crate::core::ItemStatus;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    pub fn serialize<S: Serializer>(
        map: &BTreeMap<ItemStatus, super::StatusCheckPolicy>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let names: BTreeMap<String, super::StatusCheckPolicy> = map
            .iter()
            .map(|(k, v)| (k.to_string(), *v))
            .collect();
        names.serialize(s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<BTreeMap<ItemStatus, super::StatusCheckPolicy>, D::Error> {
        let names = BTreeMap::<String, super::StatusCheckPolicy>::deserialize(d)?;
        names
            .into_iter()
            .map(|(k, v)| k.parse::<ItemStatus>().map(|s| (s, v)).map_err(serde::de::Error::custom))
            .collect()
    }
}

/// Configuration for the requirements tracing process
/// [impl->dsn~configuration-system~1]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Directories containing source code files to scan for tags
    pub source_dirs: Vec<PathBuf>,
    /// Directories containing specification files (markdown)
    pub spec_dirs: Vec<PathBuf>,
    /// File patterns to include when scanning source directories
    pub source_patterns: Vec<String>,
    /// File patterns to exclude when scanning
    pub exclude_patterns: Vec<String>,
    /// Additional artifact types to recognize
    pub artifact_types: Vec<String>,
    /// Whether to generate detailed reports
    pub verbose: bool,
    /// Output directory for reports
    pub output_dir: Option<PathBuf>,
    /// Artifact types included in release notes (cargo ovft release-notes)
    #[serde(default = "crate::release_notes::default_release_note_types")]
    pub release_note_types: Vec<String>,
    /// Status-based coverage counting and check policy
    #[serde(default)]
    pub status: StatusConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            source_dirs: vec![PathBuf::from("src")],
            spec_dirs: vec![PathBuf::from("docs")],
            source_patterns: vec![
                // Rust files
                "*.rs".to_string(),
                // Architecture Description Language files
                "*.adl".to_string(),
                "*.atl".to_string(),
                // Other common source file extensions
                "*.java".to_string(),
                "*.c".to_string(),
                "*.cpp".to_string(),
                "*.h".to_string(),
                "*.hpp".to_string(),
                "*.py".to_string(),
                "*.js".to_string(),
                "*.ts".to_string(),
                "*.go".to_string(),
                "*.rb".to_string(),
                "*.php".to_string(),
                "*.sh".to_string(),
                "*.sql".to_string(),
            ],
            exclude_patterns: vec![
                "target/**".to_string(),
                "node_modules/**".to_string(),
                ".git/**".to_string(),
                "*.tmp".to_string(),
                "*.bak".to_string(),
            ],
            artifact_types: vec![
                "feat".to_string(),
                "req".to_string(),
                "arch".to_string(),
                "dsn".to_string(),
                "impl".to_string(),
                "utest".to_string(),
                "itest".to_string(),
                "stest".to_string(),
                "uman".to_string(),
                "oman".to_string(),
            ],
            verbose: false,
            output_dir: Some(PathBuf::from("target")),
            release_note_types: crate::release_notes::default_release_note_types(),
            status: StatusConfig::default(),
        }
    }
}

impl Config {
    /// Create a new configuration with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a completely empty configuration
    pub fn empty() -> Self {
        Self {
            source_dirs: vec![],
            spec_dirs: vec![],
            source_patterns: vec!["*.rs".to_string(), "*.adl".to_string(), "*.atl".to_string()],
            exclude_patterns: vec!["target/**".to_string(), ".git/**".to_string()],
            artifact_types: vec![
                "feat".to_string(),
                "req".to_string(),
                "dsn".to_string(),
                "impl".to_string(),
                "utest".to_string(),
                "itest".to_string(),
            ],
            verbose: false,
            output_dir: Some(PathBuf::from("target")),
            release_note_types: crate::release_notes::default_release_note_types(),
            status: StatusConfig::default(),
        }
    }

    /// Add a source directory to scan for tags
    pub fn add_source_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.source_dirs.push(dir.into());
        self
    }

    /// Add a specification directory to scan for requirements
    pub fn add_spec_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.spec_dirs.push(dir.into());
        self
    }

    /// Add a file pattern to include when scanning
    pub fn add_source_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.source_patterns.push(pattern.into());
        self
    }

    /// Add a file pattern to exclude when scanning
    pub fn add_exclude_pattern<S: Into<String>>(mut self, pattern: S) -> Self {
        self.exclude_patterns.push(pattern.into());
        self
    }

    /// Add an artifact type to recognize
    pub fn add_artifact_type<S: Into<String>>(mut self, artifact_type: S) -> Self {
        self.artifact_types.push(artifact_type.into());
        self
    }

    /// Set whether to generate verbose output
    pub fn verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Set the output directory for reports
    pub fn output_dir<P: Into<PathBuf>>(mut self, dir: P) -> Self {
        self.output_dir = Some(dir.into());
        self
    }

    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load configuration from .ovft.toml file if it exists, otherwise return default
    pub fn load_or_default() -> Self {
        Self::load_from_current_dir().unwrap_or_else(|| Self::default())
    }

    /// Try to load configuration from .ovft.toml in current directory or parent directories
    pub fn load_from_current_dir() -> Option<Self> {
        let current_dir = std::env::current_dir().ok()?;
        Self::find_and_load_config(&current_dir)
    }

    /// Search for .ovft.toml file starting from the given directory and walking up parent directories
    pub fn find_and_load_config(start_dir: &std::path::Path) -> Option<Self> {
        let mut current = start_dir.to_path_buf();

        loop {
            let config_path = current.join(".ovft.toml");
            if config_path.exists() {
                if let Ok(config) = Self::from_file(&config_path) {
                    return Some(config);
                }
            }

            if !current.pop() {
                break;
            }
        }

        None
    }

    /// Save configuration to a TOML file
    pub fn save_to_file<P: AsRef<std::path::Path>>(&self, path: P) -> crate::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Check if a file path matches the source patterns
    pub fn matches_source_pattern(&self, path: &std::path::Path) -> bool {
        let path_str = path.to_string_lossy();

        // Check if excluded
        for exclude_pattern in &self.exclude_patterns {
            if glob::Pattern::new(exclude_pattern)
                .map(|p| p.matches(&path_str))
                .unwrap_or(false)
            {
                return false;
            }
        }

        // Check if included
        for include_pattern in &self.source_patterns {
            if glob::Pattern::new(include_pattern)
                .map(|p| p.matches(&path_str))
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    /// Check if a file is a markdown specification file
    pub fn is_spec_file(&self, path: &std::path::Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.source_dirs.is_empty());
        assert!(!config.spec_dirs.is_empty());
        assert!(!config.artifact_types.is_empty());
    }

    #[test]
    fn test_config_builder() {
        let config = Config::new()
            .add_source_dir("src")
            .add_spec_dir("requirements")
            .add_artifact_type("custom")
            .verbose(true);

        assert!(config.verbose);
        assert!(config.artifact_types.contains(&"custom".to_string()));
    }

    #[test]
    fn test_source_pattern_matching() {
        let config = Config::default();

        assert!(config.matches_source_pattern(Path::new("src/main.rs")));
        assert!(config.matches_source_pattern(Path::new("test.java")));
        assert!(!config.matches_source_pattern(Path::new("target/debug/main")));
        assert!(!config.matches_source_pattern(Path::new("README.md")));
    }

    #[test]
    fn test_spec_file_detection() {
        let config = Config::default();

        assert!(config.is_spec_file(Path::new("requirements.md")));
        assert!(config.is_spec_file(Path::new("spec.markdown")));
        assert!(!config.is_spec_file(Path::new("main.rs")));
        assert!(!config.is_spec_file(Path::new("config.toml")));
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        
        assert_eq!(config.source_dirs, deserialized.source_dirs);
        assert_eq!(config.spec_dirs, deserialized.spec_dirs);
        assert_eq!(config.source_patterns, deserialized.source_patterns);
        assert_eq!(config.artifact_types, deserialized.artifact_types);
    }

    #[test]
    fn test_load_or_default() {
        // This should not panic and return a valid config
        let config = Config::load_or_default();
        assert!(!config.artifact_types.is_empty());
    }

    #[test]
    fn test_status_config_defaults() {
        let config = Config::default();
        assert!(config.status.is_counted(&crate::core::ItemStatus::Proposed));
        assert!(config.status.is_counted(&crate::core::ItemStatus::Approved));
        assert!(!config.status.is_counted(&crate::core::ItemStatus::Draft));
        assert!(!config.status.is_counted(&crate::core::ItemStatus::Rejected));
        assert_eq!(
            config.status.check_policy(&crate::core::ItemStatus::Draft),
            StatusCheckPolicy::Ignore
        );
        assert_eq!(
            config.status.check_policy(&crate::core::ItemStatus::Approved),
            StatusCheckPolicy::Fail
        );
    }

    #[test]
    fn test_status_config_from_toml() {
        let toml_str = r#"
            source_dirs = ["src"]
            spec_dirs = ["docs"]
            source_patterns = ["*.rs"]
            exclude_patterns = ["target/**"]
            artifact_types = ["req"]
            verbose = false

            [status]
            count = ["approved"]

            [status.check]
            draft = "fail"
            approved = "fail"
        "#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert!(config.status.is_counted(&crate::core::ItemStatus::Approved));
        assert!(!config.status.is_counted(&crate::core::ItemStatus::Proposed));
        assert_eq!(
            config.status.check_policy(&crate::core::ItemStatus::Draft),
            StatusCheckPolicy::Fail
        );
        // Unlisted statuses keep the conservative default
        assert_eq!(
            config.status.check_policy(&crate::core::ItemStatus::Rejected),
            StatusCheckPolicy::Fail
        );
    }

    #[test]
    fn test_status_config_rejects_unknown_status() {
        let toml_str = r#"
            [status]
            count = ["wip"]
        "#;
        assert!(toml::from_str::<Config>(toml_str).is_err());
    }

    #[test]
    fn test_status_config_serialization_roundtrip() {
        let config = Config::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.status.count, deserialized.status.count);
        assert_eq!(config.status.check, deserialized.status.check);
    }
}
