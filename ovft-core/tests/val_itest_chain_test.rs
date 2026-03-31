use ovft_core::{Config, Tracer};
use std::fs;
use tempfile::TempDir;

/// Test the req -> val -> itest tracing chain.
///
/// Tree:
///   feat~data-validation~1  (needs: req)
///     └── req~validate-field-lengths~1  (covers: feat, needs: val)
///           └── val~field-length-boundary~1  (covers: req, needs: itest)
///                 └── [itest->val~field-length-boundary~1]  (source tag)
///
///   feat~data-validation~1  (needs: req)
///     └── req~validate-field-formats~1  (covers: feat, needs: val)
///           └── val~email-format-patterns~1  (covers: req, needs: itest)
///                 └── [itest->val~email-format-patterns~1]  (source tag)
///
#[test]
fn test_req_val_itest_chain() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let base = temp_dir.path();

    // -- spec files --
    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(
        spec_dir.join("features.md"),
        r#"# Features

## feat~data-validation~1

**Title:** Validate incoming data

**Needs:** req
"#,
    )
    .unwrap();

    fs::write(
        spec_dir.join("requirements.md"),
        r#"# Requirements

## req~validate-field-lengths~1

**Title:** Validate field lengths

**Covers:** feat~data-validation~1

**Needs:** val

---

## req~validate-field-formats~1

**Title:** Validate field formats

**Covers:** feat~data-validation~1

**Needs:** val
"#,
    )
    .unwrap();

    fs::write(
        spec_dir.join("validation_tests.md"),
        r#"# Validation Test Requirements

## val~field-length-boundary~1

**Title:** Test field length boundaries

**Covers:** req~validate-field-lengths~1

**Needs:** itest

---

## val~email-format-patterns~1

**Title:** Test email format patterns

**Covers:** req~validate-field-formats~1

**Needs:** itest
"#,
    )
    .unwrap();

    // -- source files with itest tags --
    let src_dir = base.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    fs::write(
        src_dir.join("validation_integration_tests.rs"),
        r#"// Integration tests covering validation test requirements

// [itest->val~field-length-boundary~1]
fn test_field_length_at_max() {}

// [itest->val~field-length-boundary~1]
fn test_field_length_above_max() {}

// [itest->val~email-format-patterns~1]
fn test_valid_email_accepted() {}

// [itest->val~email-format-patterns~1]
fn test_invalid_email_rejected() {}
"#,
    )
    .unwrap();

    // -- run the tracer --
    let config = Config::empty()
        .add_source_dir(&src_dir)
        .add_spec_dir(&spec_dir);

    let tracer = Tracer::new(config);
    let result = tracer.trace().expect("Tracing should succeed");

    // -- diagnostics: print everything --
    println!("\n=== TRACE RESULT SUMMARY ===");
    println!("total_items: {}", result.total_items);
    println!("defect_count: {}", result.defect_count);
    println!("is_success: {}", result.is_success);
    println!("coverage_percentage: {:.1}%", result.coverage_percentage());

    println!("\n=== ITEMS ===");
    for item in &result.items {
        println!(
            "  {} | coverage={} | is_defect={} | needs={:?} | covers={:?}",
            item.item.id,
            item.coverage_status,
            item.is_defect,
            item.item.needs,
            item.item
                .covers
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>(),
        );
        for link in &item.outgoing_links {
            println!("    OUT -> {} (status: {})", link.target_id, link.status);
        }
        for link in &item.incoming_links {
            println!(
                "    IN  <- {} (status: {})",
                link.source_id
                    .as_ref()
                    .map(|s| s.to_string())
                    .unwrap_or_default(),
                link.status,
            );
        }
    }

    println!("\n=== DEFECTS ===");
    for defect in &result.defects {
        println!(
            "  [{}] {} — {}",
            defect.defect_type,
            defect
                .item_id
                .as_ref()
                .map(|id| id.to_string())
                .unwrap_or_default(),
            defect.description,
        );
    }

    println!("\n=== COVERAGE SUMMARY ===");
    for (artifact_type, summary) in &result.coverage_summary {
        println!(
            "  {}: {}/{} ({:.1}%) — {}",
            artifact_type, summary.covered, summary.total, summary.percentage, summary.status,
        );
    }

    // -- assert the chain should be fully covered --
    assert!(
        result.is_success,
        "Expected full coverage in the req->val->itest chain, but got {} defect(s)",
        result.defect_count,
    );
}
