//! Integration tests for the report enhancements: coverage charts, traceability
//! graphs, Mermaid export, filters, collapsible cards and the sortable table view.

use ovft_core::reporters::{HtmlReporter, PlainTextReporter, ReportFilter, Verbosity};
use ovft_core::{Config, Tracer};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_project() -> TempDir {
    let dir = TempDir::new().expect("temp dir");
    let spec = dir.path().join("docs/requirements");
    let src = dir.path().join("src");
    fs::create_dir_all(&spec).unwrap();
    fs::create_dir_all(&src).unwrap();

    fs::write(
        spec.join("requirements.md"),
        "## req~alpha~1\n\n**Title:** Alpha < requirement & more\n\n**Needs:** dsn\n\n**Status:** approved\n\n\
         ## req~beta~1\n\n**Title:** Beta requirement\n\n**Needs:** dsn\n\n**Status:** draft\n\n\
         ## dsn~alpha-design~1\n\n**Title:** Alpha design\n\n**Covers:** req~alpha~1\n\n**Needs:** impl\n\n",
    )
    .unwrap();
    fs::write(
        src.join("main.rs"),
        "// [impl->dsn~alpha-design~1]\nfn alpha() {}\n",
    )
    .unwrap();

    dir
}

fn trace() -> (TempDir, ovft_core::TraceResult) {
    let dir = setup_project();
    let config = Config::empty()
        .add_source_dir(dir.path().join("src"))
        .add_spec_dir(dir.path().join("docs/requirements"));
    let tracer = Tracer::new(config);
    let result = tracer.trace().expect("trace should succeed");
    (dir, result)
}

#[test]
fn html_report_contains_charts_and_graphs() {
    let (_dir, result) = trace();
    let html = HtmlReporter::new(&Config::empty())
        .render_report_html(&result)
        .expect("render should succeed");

    // Coverage charts
    assert!(
        html.contains("class=\"chart-donut\""),
        "donut chart missing"
    );
    assert!(
        html.contains("class=\"chart-bars\""),
        "coverage bars missing"
    );

    // Traceability section with both graphs
    assert!(html.contains("Traceability Graph"));
    assert!(html.contains("Artifact type flow"));
    assert!(
        html.contains("class=\"chart-flow\""),
        "type flow graph missing"
    );
    assert!(html.contains("class=\"trace-graph\""), "item graph missing");
    // Graph nodes link back to item cards
    assert!(
        html.contains("href=\"#item_req_alpha_1\""),
        "graph node anchor missing"
    );

    // Mermaid source block, HTML-escaped
    assert!(html.contains("id=\"mermaid-source\""));
    assert!(html.contains("flowchart LR"));
    assert!(html.contains("&lt;br/&gt;"), "mermaid must be HTML-escaped");
    assert!(!html.contains("<br/>]:::covered"));

    // Title with special characters must be escaped everywhere
    assert!(
        !html.contains("< requirement &"),
        "unescaped title leaked into HTML"
    );
}

#[test]
fn html_report_has_filters_table_and_collapse() {
    let (_dir, result) = trace();
    let html = HtmlReporter::new(&Config::empty())
        .render_report_html(&result)
        .expect("render should succeed");

    // Type + item status filter selects
    assert!(html.contains("id=\"filter-type\""));
    assert!(html.contains("id=\"filter-istatus\""));
    assert!(html.contains("<option value=\"req\">req</option>"));
    assert!(html.contains("<option value=\"approved\">approved</option>"));

    // View toggle + collapse controls
    assert!(html.contains("data-view=\"cards\""));
    assert!(html.contains("data-view=\"table\""));
    assert!(html.contains("id=\"collapse-all\""));
    assert!(html.contains("id=\"expand-all\""));

    // Sortable table with data attributes on rows and cells
    assert!(html.contains("id=\"items-table\""));
    assert!(html.contains("data-sort=\"cov\""));
    assert!(html.contains("data-val=\"req\""));

    // Item cards carry the filter data attributes
    assert!(html.contains("data-type=\"req\""));
    assert!(html.contains("data-istatus=\"draft\""));
    assert!(
        html.contains("class=\"item-toggle\""),
        "collapse toggle missing"
    );

    // Collapsed bodies and hidden views must actually hide (regression: author
    // display rules beat the hidden attribute)
    assert!(html.contains(".items-grid[hidden]"));
    assert!(html.contains(".item-body.collapsed"));
}

#[test]
fn plain_text_report_verbosity_and_filters() {
    let (_dir, result) = trace();

    let quiet = PlainTextReporter::new(Verbosity::Quiet, ReportFilter::default()).render(&result);
    assert!(
        quiet.starts_with("FAILED"),
        "uncovered req~beta should fail the trace"
    );
    assert_eq!(quiet.lines().count(), 1);

    let all = PlainTextReporter::new(Verbosity::All, ReportFilter::default()).render(&result);
    assert!(all.contains("items:"));
    assert!(all.contains("req~alpha~1"));
    assert!(all.contains("needs coverage by dsn"));

    let filtered = PlainTextReporter::new(
        Verbosity::Summary,
        ReportFilter {
            wanted_types: Some(vec!["req".to_string()]),
            item_status: Some(ovft_core::ItemStatus::Draft),
        },
    )
    .render(&result);
    assert!(filtered.contains("status: draft"));
    assert!(filtered.contains("types: req"));
    assert!(filtered.contains("1 item(s)"));
}

#[test]
fn mermaid_export_renders_flowchart() {
    let (_dir, result) = trace();
    let chart = ovft_core::reporters::visuals::mermaid_flowchart(&result, 100);
    assert!(chart.starts_with("flowchart LR"));
    assert!(chart.contains("-->"));
    assert!(chart.contains("classDef defect"));
}

#[test]
fn large_project_graph_limits_nodes() {
    let dir = TempDir::new().expect("temp dir");
    let spec = dir.path().join("docs");
    fs::create_dir_all(&spec).unwrap();

    // 200 requirements, only a few covered: the graph must switch to the
    // defective-items-plus-neighbours subset and say so.
    let mut md = String::new();
    for i in 0..200 {
        md.push_str(&format!(
            "## req~r{i}~1\n\n**Title:** R{i}\n\n**Needs:** dsn\n\n"
        ));
    }
    for i in 0..3 {
        md.push_str(&format!(
            "## dsn~r{i}~1\n\n**Title:** D{i}\n\n**Covers:** req~r{i}~1\n\n"
        ));
    }
    fs::write(spec.join("spec.md"), md).unwrap();

    let config = Config::empty().add_spec_dir(spec);
    let result = Tracer::new(config).trace().unwrap();
    let html = HtmlReporter::new(&Config::empty())
        .render_report_html(&result)
        .unwrap();
    assert!(
        html.contains("Large project (203 items)"),
        "graph should announce the subset it renders"
    );
}
