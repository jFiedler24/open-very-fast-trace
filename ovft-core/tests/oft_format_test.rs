use ovft_core::{Config, Tracer};
use ovft_core::core::Linker;
use ovft_core::importers::MarkdownImporter;
use std::path::{Path, PathBuf};

/// Test that the OFT-format multi-level requirement chain (feat→req→dsn→impl)
/// is parsed, linked, and fully covered.
#[test]
fn test_oft_format_full_coverage_chain() {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/oft_testdata");

    let config = Config::empty()
        .add_source_dir(testdata.join("src"))
        .add_spec_dir(testdata.clone());

    let tracer = Tracer::new(config);
    let result = tracer.trace().expect("OFT tracing should succeed");

    // Verify we found all items
    let ids: Vec<String> = result.items.iter().map(|i| i.item.id.to_string()).collect();

    // Feature level
    assert!(ids.contains(&"feat~tracing~1".to_string()), "Missing feat~tracing~1, got: {:?}", ids);
    assert!(ids.contains(&"feat~reporting~1".to_string()), "Missing feat~reporting~1, got: {:?}", ids);

    // Requirement level
    assert!(ids.contains(&"req~forward-coverage~1".to_string()), "Missing req~forward-coverage~1");
    assert!(ids.contains(&"req~backward-coverage~1".to_string()), "Missing req~backward-coverage~1");
    assert!(ids.contains(&"req~report-generation~1".to_string()), "Missing req~report-generation~1");
    assert!(ids.contains(&"req~c-source~1".to_string()), "Missing req~c-source~1");
    assert!(ids.contains(&"req~java-source~1".to_string()), "Missing req~java-source~1");
    assert!(ids.contains(&"req~cpp-source~1".to_string()), "Missing req~cpp-source~1");

    // Design level
    assert!(ids.contains(&"dsn~linker~1".to_string()), "Missing dsn~linker~1");
    assert!(ids.contains(&"dsn~html-reporter~1".to_string()), "Missing dsn~html-reporter~1");

    // Source tag items (impl and src types)
    let has_impl_items = ids.iter().any(|id| id.starts_with("impl~"));
    assert!(has_impl_items, "Should have impl~ items from source tags, got: {:?}", ids);
    let has_src_items = ids.iter().any(|id| id.starts_with("src~"));
    assert!(has_src_items, "Should have src~ items from source tags, got: {:?}", ids);

    // Verify the full chain is covered — no defects
    assert_eq!(result.defect_count, 0,
        "Expected no defects but found: {:?}",
        result.defects.iter().map(|d| &d.description).collect::<Vec<_>>()
    );
    assert!(result.is_success, "Trace should be successful");
}

/// Test that each coverage link is correctly resolved between levels.
#[test]
fn test_oft_format_link_resolution() {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/oft_testdata");

    let config = Config::empty()
        .add_source_dir(testdata.join("src"))
        .add_spec_dir(testdata.clone());

    let tracer = Tracer::new(config);
    let result = tracer.trace().expect("OFT tracing should succeed");

    let find_item = |id_str: &str| {
        result.items.iter().find(|i| i.item.id.to_string() == id_str)
            .unwrap_or_else(|| panic!("Item {} not found", id_str))
    };

    // feat~tracing~1 should have incoming links from req items that cover it
    let feat_tracing = find_item("feat~tracing~1");
    assert!(!feat_tracing.incoming_links.is_empty(),
        "feat~tracing~1 should have incoming links (covered by req items)");
    let incoming_sources: Vec<String> = feat_tracing.incoming_links.iter()
        .filter_map(|l| l.source_id.as_ref().map(|id| id.to_string()))
        .collect();
    assert!(incoming_sources.contains(&"req~forward-coverage~1".to_string()),
        "feat~tracing~1 should be covered by req~forward-coverage~1, got: {:?}", incoming_sources);

    // req~forward-coverage~1 should have outgoing link to feat~tracing~1
    let req_fwd = find_item("req~forward-coverage~1");
    let outgoing_targets: Vec<String> = req_fwd.outgoing_links.iter()
        .map(|l| l.target_id.to_string())
        .collect();
    assert!(outgoing_targets.contains(&"feat~tracing~1".to_string()),
        "req~forward-coverage~1 should cover feat~tracing~1, got: {:?}", outgoing_targets);

    // req~forward-coverage~1 needs dsn, and dsn~linker~1 covers it
    assert!(req_fwd.item.needs.contains(&"dsn".to_string()),
        "req~forward-coverage~1 should need dsn");
    let req_fwd_incoming: Vec<String> = req_fwd.incoming_links.iter()
        .filter_map(|l| l.source_id.as_ref().map(|id| id.to_string()))
        .collect();
    assert!(req_fwd_incoming.contains(&"dsn~linker~1".to_string()),
        "req~forward-coverage~1 should be covered by dsn~linker~1, got: {:?}", req_fwd_incoming);

    // dsn~linker~1 needs impl, should have incoming from impl tag
    let dsn_linker = find_item("dsn~linker~1");
    assert!(dsn_linker.item.needs.contains(&"impl".to_string()),
        "dsn~linker~1 should need impl");
    assert!(!dsn_linker.incoming_links.is_empty(),
        "dsn~linker~1 should have incoming impl coverage");

    // req~c-source~1 needs src, should be covered by src tag from test.c
    let req_c = find_item("req~c-source~1");
    assert!(req_c.item.needs.contains(&"src".to_string()),
        "req~c-source~1 should need src");
    assert!(!req_c.incoming_links.is_empty(),
        "req~c-source~1 should have incoming src coverage from test.c");

    // All items should be covered
    for item in &result.items {
        assert!(item.is_covered(),
            "Item {} should be covered but has status {:?}",
            item.item.id, item.coverage_status);
    }
}

/// Test that missing coverage is correctly detected.
#[test]
fn test_oft_format_missing_coverage_detection() {
    let md = MarkdownImporter::new();
    let content = r#"# Incomplete Spec

## Missing Feature
`feat~incomplete~1`

This feature has no requirement covering it.

Needs: req
"#;
    let items = md.parse_markdown(content, Path::new("incomplete.md")).unwrap();
    let linker = Linker::new();
    let linked = linker.link_items(items).unwrap();

    let tracer = Tracer::new(Config::empty());
    let result = tracer.analyze_trace(&linked);

    assert!(!result.is_success, "Should detect missing coverage");
    assert!(result.defect_count > 0, "Should have defect for uncovered feat");

    let feat_item = result.items.iter()
        .find(|i| i.item.id.to_string() == "feat~incomplete~1")
        .expect("Should find feat~incomplete~1");
    assert!(!feat_item.is_covered(), "feat~incomplete~1 should be uncovered");
    assert!(feat_item.is_defect, "feat~incomplete~1 should be marked as defect");
}
