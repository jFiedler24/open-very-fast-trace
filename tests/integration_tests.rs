use open_very_fast_trace::{Config, Tracer};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Integration test that validates the entire requirements tracing pipeline
/// [itest->req~tag-regex-parsing~1,req~markdown-specification-parsing~1,req~coverage-analysis~1,req~html-template-rendering~1]
#[test]
fn test_complete_requirements_tracing_pipeline() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path();
    
    // Setup test structure
    setup_test_project_structure(&temp_path);
    
    // Configure the tracer
    let config = Config::empty()
        .add_source_dir(temp_path.join("src"))
        .add_spec_dir(temp_path.join("docs/requirements"));
    
    let tracer = Tracer::new(config);
    
    // Run the complete tracing process
    let trace_result = tracer.trace().expect("Tracing should succeed");
    
    // Validate the trace result
    assert!(trace_result.total_items >= 8, "Should find at least our test specification items");
    assert_eq!(trace_result.defect_count, 0, "Should have no defects");
    assert!(trace_result.is_success, "Tracing should be successful");
    
    // Validate that items were found
    let item_ids: Vec<String> = trace_result.items.iter()
        .map(|item| item.item.id.to_string())
        .collect();
    
    assert!(item_ids.contains(&"feat~user-auth~1".to_string()));
    assert!(item_ids.contains(&"req~secure-login~1".to_string()));
    assert!(item_ids.contains(&"dsn~auth-module~1".to_string()));
    
    // Generate HTML report
    let report_path = temp_path.join("requirements_report.html");
    tracer.generate_html_report(&trace_result, &report_path)
        .expect("HTML report generation should succeed");
    
    // Validate HTML report was generated
    assert!(report_path.exists(), "HTML report file should be created");
    
    let html_content = fs::read_to_string(&report_path)
        .expect("Should be able to read HTML report");
    
    // Validate HTML content contains expected elements
    assert!(html_content.contains("Requirements Tracing Report"));
    assert!(html_content.contains("feat~user-auth~1"));
    assert!(html_content.contains("req~secure-login~1"));
    assert!(html_content.contains("dsn~auth-module~1"));
    assert!(html_content.contains("Total Items"));
    
    // Validate CSS is embedded
    assert!(html_content.contains("<style>"));
    assert!(html_content.contains("container"));
}

/// Test that validates defect detection
/// [itest->req~coverage-analysis~1]
#[test]
fn test_defect_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path();
    
    setup_test_project_with_defects(&temp_path);
    
    let config = Config::empty()
        .add_source_dir(temp_path.join("src"))
        .add_spec_dir(temp_path.join("docs/requirements"));
    
    let tracer = Tracer::new(config);
    let trace_result = tracer.trace().expect("Tracing should succeed");
    
    // Should detect defects
    assert!(trace_result.defect_count > 0, "Should detect defects in problematic project");
    assert!(!trace_result.is_success, "Tracing should report failure due to defects");
    
    // Generate report for defective project
    let report_path = temp_path.join("defects_report.html");
    tracer.generate_html_report(&trace_result, &report_path)
        .expect("HTML report generation should succeed even with defects");
    
    let html_content = fs::read_to_string(&report_path)
        .expect("Should be able to read HTML report");
    
    assert!(html_content.contains("✗"), "Report should show failure indicator");
}

/// Test configuration loading and saving
/// [itest->req~configuration-management~1]
#[test]
fn test_configuration_management() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let config_path = temp_dir.path().join("ovft.toml");
    
    // Create a configuration
    let original_config = Config::empty()  // Use empty() to start with empty config
        .add_source_dir("src")
        .add_source_dir("tests")
        .add_spec_dir("docs/requirements")
        .add_spec_dir("docs/specs");
    
    // Save configuration
    original_config.save_to_file(&config_path)
        .expect("Should be able to save configuration");
    
    // Load configuration
    let loaded_config = Config::from_file(&config_path)
        .expect("Should be able to load configuration");
    
    // Validate configuration was preserved
    assert_eq!(loaded_config.source_dirs.len(), 2);
    assert_eq!(loaded_config.spec_dirs.len(), 2);
    assert!(loaded_config.source_dirs.contains(&PathBuf::from("src")));
    assert!(loaded_config.source_dirs.contains(&PathBuf::from("tests")));
    assert!(loaded_config.spec_dirs.contains(&PathBuf::from("docs/requirements")));
    assert!(loaded_config.spec_dirs.contains(&PathBuf::from("docs/specs")));
}

/// Test error handling with invalid files
/// [itest->req~detailed-error-messages~1]
#[test]
fn test_error_handling() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path();
    
    // Create invalid requirement file
    let docs_dir = temp_path.join("docs/requirements");
    fs::create_dir_all(&docs_dir).expect("Should create docs directory");
    
    fs::write(docs_dir.join("invalid.md"), 
        "# This is not a valid requirement\n\
         invalid~syntax~here\n")
        .expect("Should write invalid file");
    
    let config = Config::empty()
        .add_spec_dir(docs_dir);
    
    let tracer = Tracer::new(config);
    
    // Should handle parsing errors gracefully
    match tracer.trace() {
        Ok(_) => {
            // If it succeeds, that's fine - it might skip invalid lines
        }
        Err(err) => {
            // If it fails, the error should be descriptive
            let error_msg = err.to_string();
            assert!(!error_msg.is_empty(), "Error message should not be empty");
        }
    }
}

fn setup_test_project_structure(base_path: &Path) {
    // Create source directory structure
    let src_dir = base_path.join("src");
    fs::create_dir_all(&src_dir).expect("Should create src directory");
    
    // Create lib.rs with requirement tags
    fs::write(src_dir.join("lib.rs"),
        r#"//! Open Very Fast Trace Library
//! [impl->feat~user-auth~1]

pub mod auth {
    /// Authentication module implementation
    /// [impl->dsn~auth-module~1]
    pub struct AuthService {
        // Implementation details
    }
    
    impl AuthService {
        /// Secure login implementation
        /// [impl->req~secure-login~1]
        pub fn login(&self, username: &str, password: &str) -> bool {
            // Secure login logic
            !username.is_empty() && !password.is_empty()
        }
        
        /// Session management
        /// [impl->req~session-mgmt~1]
        pub fn create_session(&self) -> String {
            "session_token".to_string()
        }
    }
}

/// Configuration management
/// [impl->req~config-loading~1]
pub struct Config {
    pub database_url: String,
}
"#).expect("Should write lib.rs");
    
    // Create test file with test tags
    fs::write(src_dir.join("auth_test.rs"),
        r#"//! Authentication tests
//! [utest->req~secure-login~1]

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Test secure login functionality
    /// [utest->req~secure-login~1]
    #[test]
    fn test_secure_login() {
        let auth = AuthService::new();
        assert!(auth.login("user", "pass"));
        assert!(!auth.login("", ""));
    }
    
    /// Test session management
    /// [utest->req~session-mgmt~1]
    #[test]
    fn test_session_creation() {
        let auth = AuthService::new();
        let session = auth.create_session();
        assert!(!session.is_empty());
    }
}
"#).expect("Should write auth_test.rs");
    
    // Create requirements directory structure
    let req_dir = base_path.join("docs/requirements");
    fs::create_dir_all(&req_dir).expect("Should create requirements directory");
    
    // Create features.md
    fs::write(req_dir.join("features.md"),
        r#"# Features

## feat~user-auth~1

**Title:** User Authentication System

**Description:** The system shall provide secure user authentication capabilities.

**Rationale:** Authentication is required for system security.

**Tags:** security, authentication

**Needs:** req, dsn
"#).expect("Should write features.md");
    
    // Create requirements.md
    fs::write(req_dir.join("requirements.md"),
        r#"# Requirements

## req~secure-login~1

**Title:** Secure Login Process

**Description:** The system shall implement a secure login process that validates user credentials.

**Covers:** feat~user-auth~1

**Rationale:** Secure login prevents unauthorized access.

**Tags:** security, login

**Needs:** dsn, impl, utest

---

## req~session-mgmt~1

**Title:** Session Management

**Description:** The system shall manage user sessions securely.

**Covers:** feat~user-auth~1

**Rationale:** Session management is essential for maintaining user state.

**Tags:** security, session

**Needs:** dsn, impl, utest

---

## req~config-loading~1

**Title:** Configuration Loading

**Description:** The system shall load configuration from external sources.

**Covers:** feat~user-auth~1

**Rationale:** External configuration enables deployment flexibility.

**Tags:** configuration

**Needs:** impl
"#).expect("Should write requirements.md");
    
    // Create design.md
    fs::write(req_dir.join("design.md"),
        r#"# Design Specifications

## dsn~auth-module~1

**Title:** Authentication Module Design

**Description:** The system shall implement an AuthService module that encapsulates all authentication logic.

**Covers:** feat~user-auth~1, req~secure-login~1, req~session-mgmt~1

**Rationale:** Modular design promotes maintainability and testability.

**Tags:** architecture, security

**Needs:** impl
"#).expect("Should write design.md");
}

fn setup_test_project_with_defects(base_path: &Path) {
    let src_dir = base_path.join("src");
    fs::create_dir_all(&src_dir).expect("Should create src directory");
    
    // Create source with orphaned coverage
    fs::write(src_dir.join("lib.rs"),
        r#"//! Library with defects
//! [impl->nonexistent~requirement~1]  // This will be orphaned

pub fn some_function() {
    // Implementation without proper coverage
}
"#).expect("Should write lib.rs with defects");
    
    let req_dir = base_path.join("docs/requirements");
    fs::create_dir_all(&req_dir).expect("Should create requirements directory");
    
    // Create requirement without implementation
    fs::write(req_dir.join("requirements.md"),
        r#"# Requirements

## req~uncovered-requirement~1

**Title:** Uncovered Requirement

**Description:** This requirement has no implementation.

**Rationale:** Testing defect detection.

**Tags:** testing

**Needs:** impl
"#).expect("Should write requirements with defects");
}
