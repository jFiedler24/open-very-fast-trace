use ovft_core::{
    Config, CoverageStatus, Defect, DefectType, LinkedSpecificationItem, Location,
    SpecificationItem, SpecificationItemId, TraceResult, Tracer,
};
use ovft_core::core::Linker;
use ovft_core::importers::{MarkdownImporter, TagImporter};
use ovft_core::reporters::HtmlReporter;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use wasm_bindgen::prelude::*;

// ---------------------------------------------------------------------------
// JS-friendly data types (mirroring the Rust core types as serialisable DTOs)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
pub struct JsFileContent {
    pub path: String,
    pub content: String,
}

#[derive(Serialize, Deserialize)]
pub struct JsTraceResult {
    pub total_items: usize,
    pub defect_count: usize,
    pub is_success: bool,
    pub coverage_percentage: f64,
    pub defects: Vec<JsDefect>,
    pub items: Vec<JsLinkedItem>,
    pub coverage_summary: HashMap<String, JsCoverageSummary>,
}

#[derive(Serialize, Deserialize)]
pub struct JsDefect {
    pub defect_type: String,
    pub description: String,
    pub item_id: Option<String>,
    pub file_path: Option<String>,
    pub line: Option<u32>,
}

#[derive(Serialize, Deserialize)]
pub struct JsLinkedItem {
    pub id: String,
    pub artifact_type: String,
    pub name: String,
    pub revision: u32,
    pub title: Option<String>,
    pub description: Option<String>,
    pub needs: Vec<String>,
    pub covers: Vec<String>,
    pub depends: Vec<String>,
    pub coverage_status: String,
    pub is_defect: bool,
    pub file_path: Option<String>,
    pub line: Option<u32>,
    pub incoming_links: Vec<JsLink>,
    pub outgoing_links: Vec<JsLink>,
}

#[derive(Serialize, Deserialize)]
pub struct JsLink {
    pub source_id: Option<String>,
    pub target_id: String,
    pub status: String,
}

#[derive(Serialize, Deserialize)]
pub struct JsCoverageSummary {
    pub total: usize,
    pub covered: usize,
    pub percentage: f64,
    pub status: String,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn to_js_linked_item(item: &LinkedSpecificationItem) -> JsLinkedItem {
    JsLinkedItem {
        id: item.item.id.to_string(),
        artifact_type: item.item.id.artifact_type.clone(),
        name: item.item.id.name.clone(),
        revision: item.item.id.revision,
        title: item.item.title.clone(),
        description: item.item.description.clone(),
        needs: item.item.needs.clone(),
        covers: item.item.covers.iter().map(|c| c.to_string()).collect(),
        depends: item.item.depends.iter().map(|d| d.to_string()).collect(),
        coverage_status: item.coverage_status.to_string(),
        is_defect: item.is_defect,
        file_path: item.item.location.as_ref().map(|l| l.path.to_string_lossy().to_string()),
        line: item.item.location.as_ref().map(|l| l.line),
        incoming_links: item
            .incoming_links
            .iter()
            .map(|l| JsLink {
                source_id: l.source_id.as_ref().map(|id| id.to_string()),
                target_id: l.target_id.to_string(),
                status: l.status.to_string(),
            })
            .collect(),
        outgoing_links: item
            .outgoing_links
            .iter()
            .map(|l| JsLink {
                source_id: l.source_id.as_ref().map(|id| id.to_string()),
                target_id: l.target_id.to_string(),
                status: l.status.to_string(),
            })
            .collect(),
    }
}

fn to_js_defect(defect: &Defect, items: &[LinkedSpecificationItem]) -> JsDefect {
    // Try to find the file/line from the defective item
    let (file_path, line) = if let Some(ref item_id) = defect.item_id {
        items
            .iter()
            .find(|i| i.item.id == *item_id)
            .and_then(|i| i.item.location.as_ref())
            .map(|loc| (Some(loc.path.to_string_lossy().to_string()), Some(loc.line)))
            .unwrap_or((None, None))
    } else {
        (None, None)
    };

    JsDefect {
        defect_type: defect.defect_type.to_string(),
        description: defect.description.clone(),
        item_id: defect.item_id.as_ref().map(|id| id.to_string()),
        file_path,
        line,
    }
}

fn to_js_trace_result(result: &TraceResult) -> JsTraceResult {
    JsTraceResult {
        total_items: result.total_items,
        defect_count: result.defect_count,
        is_success: result.is_success,
        coverage_percentage: result.coverage_percentage(),
        defects: result
            .defects
            .iter()
            .map(|d| to_js_defect(d, &result.items))
            .collect(),
        items: result.items.iter().map(to_js_linked_item).collect(),
        coverage_summary: result
            .coverage_summary
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    JsCoverageSummary {
                        total: v.total,
                        covered: v.covered,
                        percentage: v.percentage,
                        status: v.status.to_string(),
                    },
                )
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// WASM-exported functions
// ---------------------------------------------------------------------------

/// Parse markdown spec files and tag source files, link them, and return
/// a full trace result as JSON.
///
/// `spec_files` and `source_files` are JSON arrays of `{path, content}`.
#[wasm_bindgen]
pub fn trace_from_contents(spec_files_json: &str, source_files_json: &str) -> std::result::Result<JsValue, JsValue> {
    let spec_files: Vec<JsFileContent> =
        serde_json::from_str(spec_files_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let source_files: Vec<JsFileContent> =
        serde_json::from_str(source_files_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let md_importer = MarkdownImporter::new();
    let tag_importer = TagImporter::new();

    let mut items: Vec<SpecificationItem> = Vec::new();

    // Parse spec files
    for file in &spec_files {
        let path = PathBuf::from(&file.path);
        match md_importer.parse_markdown(&file.content, &path) {
            Ok(file_items) => items.extend(file_items),
            Err(e) => {
                // Log but don't fail — partial results are still useful
                web_sys_log(&format!("Warning: failed to parse {}: {}", file.path, e));
            }
        }
    }

    // Parse source files
    for file in &source_files {
        let path = PathBuf::from(&file.path);
        for (line_number, line) in file.content.lines().enumerate() {
            match tag_importer.parse_line(line, &path, line_number as u32 + 1) {
                Ok(line_items) => items.extend(line_items),
                Err(_) => {} // skip unparseable lines
            }
        }
    }

    // Link and analyze
    let linker = Linker::new();
    let linked_items = linker
        .link_items(items)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let config = Config::empty();
    let tracer = Tracer::new(config);
    let trace_result = tracer.analyze_trace(&linked_items);
    let js_result = to_js_trace_result(&trace_result);

    serde_wasm_bindgen::to_value(&js_result).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Render the full HTML report from a trace result (as returned by `trace_from_contents`).
/// Returns the HTML string.
#[wasm_bindgen]
pub fn render_html_report(trace_result_json: &str) -> std::result::Result<String, JsValue> {
    // We need to reconstruct a TraceResult from the JS representation.
    // For simplicity, we re-trace from the items, but the caller should pass
    // the spec_files/source_files again or we can accept the already-linked data.
    //
    // Actually, the most practical approach: accept the same inputs and render.
    // But let's accept the raw result JSON and reconstruct.
    let js_result: JsTraceResult =
        serde_json::from_str(trace_result_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    // Reconstruct TraceResult from JsTraceResult
    let trace_result = reconstruct_trace_result(&js_result);

    let reporter = HtmlReporter::new(&Config::empty());
    reporter
        .render_report_html(&trace_result)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Render HTML report directly from file contents (convenience: combines trace + render).
#[wasm_bindgen]
pub fn trace_and_render_html(spec_files_json: &str, source_files_json: &str) -> std::result::Result<String, JsValue> {
    let spec_files: Vec<JsFileContent> =
        serde_json::from_str(spec_files_json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let source_files: Vec<JsFileContent> =
        serde_json::from_str(source_files_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let md_importer = MarkdownImporter::new();
    let tag_importer = TagImporter::new();

    let mut items: Vec<SpecificationItem> = Vec::new();

    for file in &spec_files {
        let path = PathBuf::from(&file.path);
        if let Ok(file_items) = md_importer.parse_markdown(&file.content, &path) {
            items.extend(file_items);
        }
    }

    for file in &source_files {
        let path = PathBuf::from(&file.path);
        for (line_number, line) in file.content.lines().enumerate() {
            if let Ok(line_items) = tag_importer.parse_line(line, &path, line_number as u32 + 1) {
                items.extend(line_items);
            }
        }
    }

    let linker = Linker::new();
    let linked_items = linker
        .link_items(items)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let config = Config::empty();
    let tracer = Tracer::new(config);
    let trace_result = tracer.analyze_trace(&linked_items);

    let reporter = HtmlReporter::new(&Config::empty());
    reporter
        .render_report_html(&trace_result)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn reconstruct_trace_result(js: &JsTraceResult) -> TraceResult {
    let items: Vec<LinkedSpecificationItem> = js
        .items
        .iter()
        .map(|ji| {
            let id = SpecificationItemId::new(
                ji.artifact_type.clone(),
                ji.name.clone(),
                ji.revision,
            );
            let location = match (&ji.file_path, ji.line) {
                (Some(p), Some(l)) => Some(Location::new(PathBuf::from(p), l)),
                _ => None,
            };
            let covers: Vec<SpecificationItemId> = ji
                .covers
                .iter()
                .filter_map(|s| SpecificationItemId::parse(s).ok())
                .collect();
            let depends: Vec<SpecificationItemId> = ji
                .depends
                .iter()
                .filter_map(|s| SpecificationItemId::parse(s).ok())
                .collect();

            let mut builder = SpecificationItem::builder(id).needs_multiple(ji.needs.clone());
            if let Some(ref t) = ji.title {
                builder = builder.title(t.clone());
            }
            if let Some(ref d) = ji.description {
                builder = builder.description(d.clone());
            }
            if let Some(loc) = location {
                builder = builder.location(loc);
            }
            builder = builder.covers_multiple(covers);
            for dep in depends {
                builder = builder.depends(dep);
            }

            let mut linked = LinkedSpecificationItem::new(builder.build());
            linked.is_defect = ji.is_defect;
            linked.coverage_status = match ji.coverage_status.as_str() {
                "covered" => CoverageStatus::Covered,
                "partial" => CoverageStatus::Partial,
                _ => CoverageStatus::Uncovered,
            };
            // Reconstruct links
            for jl in &ji.outgoing_links {
                if let Ok(target) = SpecificationItemId::parse(&jl.target_id) {
                    linked.add_outgoing_link(target, parse_link_status(&jl.status));
                }
            }
            for jl in &ji.incoming_links {
                if let Ok(target) = SpecificationItemId::parse(&jl.target_id) {
                    let source = jl
                        .source_id
                        .as_ref()
                        .and_then(|s| SpecificationItemId::parse(s).ok());
                    if let Some(src) = source {
                        linked.add_incoming_link(src, parse_link_status(&jl.status));
                    }
                }
            }
            linked
        })
        .collect();

    let defects: Vec<Defect> = js
        .defects
        .iter()
        .map(|jd| Defect {
            defect_type: match jd.defect_type.as_str() {
                "orphaned" => DefectType::OrphanedCoverage,
                "duplicate" => DefectType::DuplicateItem,
                "wrong-revision" => DefectType::WrongRevision,
                "circular-dependency" => DefectType::CircularDependency,
                _ => DefectType::UncoveredItem,
            },
            description: jd.description.clone(),
            item_id: jd
                .item_id
                .as_ref()
                .and_then(|s| SpecificationItemId::parse(s).ok()),
        })
        .collect();

    let coverage_summary: HashMap<String, ovft_core::CoverageSummary> = js
        .coverage_summary
        .iter()
        .map(|(k, v)| {
            (
                k.clone(),
                ovft_core::CoverageSummary {
                    total: v.total,
                    covered: v.covered,
                    percentage: v.percentage,
                    status: match v.status.as_str() {
                        "covered" => CoverageStatus::Covered,
                        "partial" => CoverageStatus::Partial,
                        _ => CoverageStatus::Uncovered,
                    },
                },
            )
        })
        .collect();

    TraceResult {
        items,
        total_items: js.total_items,
        defect_count: js.defect_count,
        defects,
        coverage_summary,
        is_success: js.is_success,
    }
}

fn parse_link_status(s: &str) -> ovft_core::LinkStatus {
    match s {
        "covers" => ovft_core::LinkStatus::Covers,
        "predated" => ovft_core::LinkStatus::Predated,
        "outdated" => ovft_core::LinkStatus::Outdated,
        "ambiguous" => ovft_core::LinkStatus::Ambiguous,
        "unwanted" => ovft_core::LinkStatus::Unwanted,
        "orphaned" => ovft_core::LinkStatus::Orphaned,
        "covered shallow" => ovft_core::LinkStatus::CoveredShallow,
        "covered unwanted" => ovft_core::LinkStatus::CoveredUnwanted,
        "covered predated" => ovft_core::LinkStatus::CoveredPredated,
        "covered outdated" => ovft_core::LinkStatus::CoveredOutdated,
        "duplicate" => ovft_core::LinkStatus::Duplicate,
        _ => ovft_core::LinkStatus::Orphaned,
    }
}

fn web_sys_log(_msg: &str) {
    // In WASM with web-sys, we could use web_sys::console::log_1.
    // For now this is a no-op; the caller handles logging.
}
