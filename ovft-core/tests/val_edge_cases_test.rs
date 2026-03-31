use ovft_core::core::{Linker, LinkStatus};
use ovft_core::core::{SpecificationItem, SpecificationItemId};
use ovft_core::{Config, Tracer};
use std::fs;
use tempfile::TempDir;

/// EDGE CASE 1: Unwanted coverage not detected.
///
/// An `impl` tag covers a `req` that only needs `val`.
/// The impl coverage is unwanted, but the current linker marks it as `Covers`
/// because the covered item has non-empty needs (just not `impl`).
///
/// Expected: outgoing link from impl -> req should be `Unwanted`
/// Actual:   outgoing link from impl -> req is `Covers` (BUG)
#[test]
fn test_unwanted_coverage_not_detected() {
    let linker = Linker::new();

    let req_id = SpecificationItemId::new("req".into(), "needs-only-val".into(), 1);
    let val_id = SpecificationItemId::new("val".into(), "covers-req".into(), 1);
    let impl_id = SpecificationItemId::new("impl".into(), "wrongly-covers-req".into(), 1);

    // req needs `val` only
    let req = SpecificationItem::builder(req_id.clone())
        .needs("val".into())
        .build();

    // val correctly covers req and is a terminating item
    let val = SpecificationItem::builder(val_id.clone())
        .covers(req_id.clone())
        .build();

    // impl INCORRECTLY covers req (req only needs val, not impl)
    let bad_impl = SpecificationItem::builder(impl_id.clone())
        .covers(req_id.clone())
        .build();

    let items = vec![req, val, bad_impl];
    let linked = linker.link_items(items).unwrap();

    let impl_linked = linked.iter().find(|li| li.item.id == impl_id).unwrap();

    println!("\n=== EDGE CASE 1: Unwanted coverage ===");
    println!("impl outgoing links:");
    for link in &impl_linked.outgoing_links {
        println!("  -> {} status={}", link.target_id, link.status);
    }
    println!("impl is_defect: {}", impl_linked.is_defect);

    // BUG: this link should be `Unwanted` since req only needs `val`, not `impl`
    let link_to_req = impl_linked
        .outgoing_links
        .iter()
        .find(|l| l.target_id == req_id)
        .unwrap();
    println!(
        "Link status from impl -> req: {} (should be 'unwanted')",
        link_to_req.status
    );

    // This assertion will FAIL, revealing the bug:
    assert_eq!(
        link_to_req.status,
        LinkStatus::Unwanted,
        "impl covering req~needs-only-val~1 should be Unwanted because req only needs val"
    );
}

/// EDGE CASE 2: When req needs both val AND dsn, but only val covers it.
/// Should be Partial coverage.
#[test]
fn test_partial_coverage_with_val_and_dsn() {
    let temp_dir = TempDir::new().unwrap();
    let base = temp_dir.path();

    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(
        spec_dir.join("requirements.md"),
        r#"# Requirements

## req~multi-need~1

**Title:** Requirement needing val and dsn

**Needs:** val, dsn
"#,
    )
    .unwrap();

    fs::write(
        spec_dir.join("validation.md"),
        r#"# Validation Tests

## val~multi-need-test~1

**Title:** Validation test for multi-need

**Covers:** req~multi-need~1

**Needs:** itest
"#,
    )
    .unwrap();

    // No dsn covers req~multi-need~1 → should be Partial
    let src_dir = base.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("tests.rs"),
        "// [itest->val~multi-need-test~1]\nfn test_it() {}\n",
    )
    .unwrap();

    let config = Config::empty()
        .add_source_dir(&src_dir)
        .add_spec_dir(&spec_dir);
    let tracer = Tracer::new(config);
    let result = tracer.trace().unwrap();

    println!("\n=== EDGE CASE 2: Partial coverage ===");
    for item in &result.items {
        println!(
            "  {} coverage={} defect={}",
            item.item.id, item.coverage_status, item.is_defect,
        );
    }
    for defect in &result.defects {
        println!(
            "  DEFECT: [{}] {} — {}",
            defect.defect_type,
            defect.item_id.as_ref().map(|id| id.to_string()).unwrap_or_default(),
            defect.description,
        );
    }

    let req_item = result
        .items
        .iter()
        .find(|i| i.item.id.name == "multi-need")
        .unwrap();
    assert_eq!(
        req_item.coverage_status.to_string(),
        "partial",
        "req~multi-need~1 should be partially covered (val but not dsn)"
    );
    assert!(req_item.is_defect, "Partially covered item should be a defect");
}

/// EDGE CASE 3: Source code tag with `impl` type covers a `req` that only needs `val`.
/// The impl item should not have a valid `Covers` link — it's unwanted.
/// But currently it gets `Covers` and is not flagged.
#[test]
fn test_impl_tag_covering_val_only_req_from_source() {
    let temp_dir = TempDir::new().unwrap();
    let base = temp_dir.path();

    let spec_dir = base.join("specs");
    fs::create_dir_all(&spec_dir).unwrap();

    fs::write(
        spec_dir.join("requirements.md"),
        r#"# Requirements

## req~val-only~1

**Title:** Requirement that only needs val coverage

**Needs:** val
"#,
    )
    .unwrap();

    // Source file has an impl tag covering the req — this is unwanted!
    let src_dir = base.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("lib.rs"),
        "// [impl->req~val-only~1]\nfn do_stuff() {}\n",
    )
    .unwrap();

    let config = Config::empty()
        .add_source_dir(&src_dir)
        .add_spec_dir(&spec_dir);
    let tracer = Tracer::new(config);
    let result = tracer.trace().unwrap();

    println!("\n=== EDGE CASE 3: impl wrongly covering val-only req ===");
    for item in &result.items {
        println!(
            "  {} coverage={} defect={} needs={:?}",
            item.item.id, item.coverage_status, item.is_defect, item.item.needs,
        );
        for link in &item.outgoing_links {
            println!("    OUT -> {} ({})", link.target_id, link.status);
        }
    }
    for defect in &result.defects {
        println!(
            "  DEFECT: {}",
            defect.description,
        );
    }

    // The req should NOT be covered (it needs val, not impl)
    let req_item = result
        .items
        .iter()
        .find(|i| i.item.id.name == "val-only")
        .unwrap();
    assert!(
        req_item.is_defect,
        "req~val-only~1 should be a defect — it needs val coverage but only has impl"
    );

    // The impl link should be Unwanted, not Covers
    let impl_item = result
        .items
        .iter()
        .find(|i| i.item.id.artifact_type == "impl")
        .unwrap();
    let link = impl_item
        .outgoing_links
        .iter()
        .find(|l| l.target_id.name == "val-only")
        .unwrap();
    println!(
        "\nimpl -> req~val-only~1 link status: {} (should be 'unwanted')",
        link.status
    );

    // BUG: The impl item's outgoing link is `covers` when it should be `unwanted`
    assert_eq!(
        link.status,
        LinkStatus::Unwanted,
        "impl covering a req that only needs val should be Unwanted"
    );
}
