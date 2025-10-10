use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
}
