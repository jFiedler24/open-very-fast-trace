use ovft_core::importers::MarkdownImporter;
use std::path::PathBuf;

#[test]
fn test_duplicate_needs_reproduction() {
    let importer = MarkdownImporter::new();

    // Case 1: Normal needs
    let md1 = "## req~test-normal~1\n\n**Title:** Normal\n\n**Needs:** impl, utest\n";
    let items1 = importer.parse_markdown(md1, &PathBuf::from("test.md")).unwrap();
    for item in &items1 {
        println!("Case 1: {} needs={:?}", item.id, item.needs);
    }
    assert_eq!(items1.len(), 1);
    assert_eq!(items1[0].needs, vec!["impl", "utest"]);

    // Case 2: Needs keyword appears on two separate lines — both get accumulated
    let md2 = "## req~test-double~1\n\n**Title:** Double\n\n**Needs:** impl\n\n**Needs:** utest\n";
    let items2 = importer.parse_markdown(md2, &PathBuf::from("test.md")).unwrap();
    for item in &items2 {
        println!("Case 2: {} needs={:?}", item.id, item.needs);
    }
    assert_eq!(items2.len(), 1);
    // Two separate Needs lines should both be accumulated (non-duplicate)
    assert!(items2[0].needs.contains(&"impl".to_string()));
    assert!(items2[0].needs.contains(&"utest".to_string()));

    // Case 3: Same need listed twice in one line
    let md3 = "## req~test-dup~1\n\n**Title:** Dup\n\n**Needs:** impl, impl, utest\n";
    let items3 = importer.parse_markdown(md3, &PathBuf::from("test.md")).unwrap();
    for item in &items3 {
        println!("Case 3: {} needs={:?}", item.id, item.needs);
    }
    assert_eq!(items3.len(), 1);
    // Duplicates in the same line should be deduplicated
    let impl_count = items3[0].needs.iter().filter(|n| *n == "impl").count();
    assert_eq!(impl_count, 1, "Duplicate 'impl' in needs should be deduplicated");

    // Case 4: Description line that looks like bold Needs keyword
    let md4 = "## req~test-desc~1\n\n**Description:** The system checks **Needs:** impl stuff\n\n**Needs:** dsn, impl\n";
    let items4 = importer.parse_markdown(md4, &PathBuf::from("test.md")).unwrap();
    for item in &items4 {
        println!("Case 4: {} needs={:?}", item.id, item.needs);
    }

    // Case 5: Needs on two separate lines with overlap
    let md5 = "## req~test-overlap~1\n\n**Needs:** impl, dsn\n\n**Needs:** impl, itest\n";
    let items5 = importer.parse_markdown(md5, &PathBuf::from("test.md")).unwrap();
    for item in &items5 {
        println!("Case 5: {} needs={:?}", item.id, item.needs);
    }
    let impl_count5 = items5[0].needs.iter().filter(|n| *n == "impl").count();
    assert_eq!(impl_count5, 1, "Overlapping needs across lines should be deduplicated");
}
