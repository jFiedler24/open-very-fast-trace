//! Dependency-free visualisations for the HTML report.
//!
//! Everything here renders to plain inline SVG/HTML strings so the report
//! stays a single self-contained file that works offline (no JS charting
//! library, no CDN, no Mermaid runtime).

use crate::core::{
    CoverageStatus, CoverageSummary, LinkStatus, LinkedSpecificationItem, TraceResult,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

const COVERED: &str = "#16a34a";
const PARTIAL: &str = "#f59e0b";
const UNCOVERED: &str = "#ef4444";
const TRACK: &str = "#e5e7eb";
const INK: &str = "#1f2937";
const MUTED: &str = "#6b7280";

const PALETTE: [&str; 8] = [
    "#6366f1", "#0ea5e9", "#14b8a6", "#8b5cf6", "#ec4899", "#f97316", "#84cc16", "#64748b",
];

/// Escape text for use inside SVG/HTML.
pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn short(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut o: String = s.chars().take(n.saturating_sub(1)).collect();
        o.push('…');
        o
    }
}

fn status_color(status: &CoverageStatus) -> &'static str {
    match status {
        CoverageStatus::Covered => COVERED,
        CoverageStatus::Partial => PARTIAL,
        CoverageStatus::Uncovered => UNCOVERED,
    }
}

/// Distinct, stable colour per artifact type (by sorted position).
struct TypeColors(Vec<String>);

impl TypeColors {
    fn new(result: &TraceResult) -> Self {
        let set: BTreeSet<String> = result
            .items
            .iter()
            .map(|i| i.item.id.artifact_type.clone())
            .collect();
        Self(set.into_iter().collect())
    }
    fn get(&self, t: &str) -> &'static str {
        let idx = self.0.iter().position(|x| x == t).unwrap_or(0);
        PALETTE[idx % PALETTE.len()]
    }
}

// ---------------------------------------------------------------------------
// Charts
// ---------------------------------------------------------------------------

/// Donut chart of item coverage status with legend (HTML fragment).
pub fn status_donut(result: &TraceResult) -> String {
    let count = |s: CoverageStatus| {
        result
            .items
            .iter()
            .filter(|i| i.coverage_status == s)
            .count()
    };
    let covered = count(CoverageStatus::Covered);
    let partial = count(CoverageStatus::Partial);
    let uncovered = count(CoverageStatus::Uncovered);
    let total = covered + partial + uncovered;

    let r = 70.0_f64;
    let c = 2.0 * std::f64::consts::PI * r;
    let mut svg = String::new();
    let _ = write!(
        svg,
        r##"<svg viewBox="0 0 200 200" class="chart-donut" role="img" aria-label="Coverage status of items"><circle cx="100" cy="100" r="{r}" fill="none" stroke="{TRACK}" stroke-width="22"/>"##
    );
    let mut offset = 0.0;
    for (n, color, label) in [
        (covered, COVERED, "Covered"),
        (partial, PARTIAL, "Partial"),
        (uncovered, UNCOVERED, "Uncovered"),
    ] {
        if n == 0 || total == 0 {
            continue;
        }
        let len = c * n as f64 / total as f64;
        let _ = write!(
            svg,
            r##"<circle cx="100" cy="100" r="{r}" fill="none" stroke="{color}" stroke-width="22" stroke-dasharray="{len:.2} {gap:.2}" stroke-dashoffset="{off:.2}" transform="rotate(-90 100 100)"><title>{label}: {n}</title></circle>"##,
            gap = c - len,
            off = -offset
        );
        offset += len;
    }
    let pct = if total == 0 {
        "–".to_string()
    } else {
        format!("{:.0}%", covered as f64 / total as f64 * 100.0)
    };
    let _ = write!(
        svg,
        r##"<text x="100" y="104" text-anchor="middle" font-size="34" font-weight="700" fill="{INK}">{pct}</text><text x="100" y="126" text-anchor="middle" font-size="12" fill="{MUTED}">covered</text></svg>"##
    );

    let mut html = format!(r#"<div class="donut-wrap">{svg}<ul class="legend">"#);
    for (n, color, label) in [
        (covered, COVERED, "Covered"),
        (partial, PARTIAL, "Partial"),
        (uncovered, UNCOVERED, "Uncovered"),
    ] {
        let _ = write!(
            html,
            r#"<li><span class="dot" style="background:{color}"></span>{label}<b>{n}</b></li>"#
        );
    }
    html.push_str("</ul></div>");
    html
}

/// Horizontal coverage bars per artifact type.
pub fn coverage_bars(summaries: &[(String, CoverageSummary)]) -> String {
    if summaries.is_empty() {
        return String::new();
    }
    let row_h = 36;
    let h = summaries.len() * row_h;
    let mut svg = format!(
        r##"<svg viewBox="0 0 620 {h}" class="chart-bars" role="img" aria-label="Coverage by artifact type" preserveAspectRatio="xMinYMin meet">"##
    );
    for (i, (name, s)) in summaries.iter().enumerate() {
        let y = i * row_h;
        let pct = s.percentage.clamp(0.0, 100.0);
        let fill_w = 360.0 * pct / 100.0;
        let color = status_color(&s.status);
        let _ = write!(
            svg,
            r##"<g><title>{name}: {cov}/{tot} covered ({pct:.0}%)</title><text x="0" y="{ty}" font-size="13" font-weight="600" fill="{INK}">{label}</text><rect x="90" y="{ry}" width="360" height="16" rx="8" fill="{TRACK}"/><rect x="90" y="{ry}" width="{fill_w:.1}" height="16" rx="8" fill="{color}"/><text x="466" y="{ty}" font-size="12" fill="{MUTED}">{cov}/{tot} · {pct:.0}%</text></g>"##,
            name = esc(name),
            label = esc(&short(name, 11)),
            cov = s.covered,
            tot = s.total,
            ty = y + 20,
            ry = y + 8,
        );
    }
    svg.push_str("</svg>");
    svg
}

/// Horizontal bars of defects grouped by type. Empty string if no defects.
pub fn defect_bars(result: &TraceResult) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for d in &result.defects {
        *counts.entry(d.defect_type.to_string()).or_default() += 1;
    }
    if counts.is_empty() {
        return String::new();
    }
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let max = rows.iter().map(|r| r.1).max().unwrap_or(1) as f64;
    let row_h = 36;
    let mut svg = format!(
        r##"<svg viewBox="0 0 620 {}" class="chart-bars" role="img" aria-label="Defects by type" preserveAspectRatio="xMinYMin meet">"##,
        rows.len() * row_h
    );
    for (i, (name, n)) in rows.iter().enumerate() {
        let y = i * row_h;
        let w = 360.0 * *n as f64 / max;
        let _ = write!(
            svg,
            r##"<g><title>{name}: {n}</title><text x="0" y="{ty}" font-size="13" font-weight="600" fill="{INK}">{label}</text><rect x="120" y="{ry}" width="330" height="16" rx="8" fill="{TRACK}"/><rect x="120" y="{ry}" width="{w2:.1}" height="16" rx="8" fill="{UNCOVERED}"/><text x="466" y="{ty}" font-size="12" fill="{MUTED}">{n}</text></g>"##,
            name = esc(name),
            label = esc(&short(name, 15)),
            ty = y + 20,
            ry = y + 8,
            w2 = 330.0 * *n as f64 / max,
        );
        let _ = w;
    }
    svg.push_str("</svg>");
    svg
}

// ---------------------------------------------------------------------------
// Graph helpers
// ---------------------------------------------------------------------------

/// Resolves a link target ID to an item index, tolerating revision mismatch.
struct Resolver {
    exact: HashMap<String, usize>,
    loose: HashMap<(String, String), usize>,
}

impl Resolver {
    fn new(items: &[&LinkedSpecificationItem]) -> Self {
        let mut exact = HashMap::new();
        let mut loose = HashMap::new();
        for (i, it) in items.iter().enumerate() {
            let id = &it.item.id;
            exact.entry(id.to_string()).or_insert(i);
            loose
                .entry((id.artifact_type.clone(), id.name.clone()))
                .or_insert(i);
        }
        Self { exact, loose }
    }
    fn resolve(&self, id: &crate::core::SpecificationItemId) -> Option<usize> {
        self.exact
            .get(&id.to_string())
            .or_else(|| self.loose.get(&(id.artifact_type.clone(), id.name.clone())))
            .copied()
    }
}

/// Edges as (coverer index, covered index, link status).
fn collect_edges(
    items: &[&LinkedSpecificationItem],
    resolver: &Resolver,
) -> Vec<(usize, usize, LinkStatus)> {
    let mut edges = Vec::new();
    let mut seen = BTreeSet::new();
    for (i, it) in items.iter().enumerate() {
        for l in &it.outgoing_links {
            if let Some(j) = resolver.resolve(&l.target_id) {
                if i != j && seen.insert((i, j)) {
                    edges.push((i, j, l.status.clone()));
                }
            }
        }
    }
    edges
}

/// Layer artifact types left→right: covered types left of covering types.
fn type_ranks(types: &[String], edges: &[(String, String)]) -> HashMap<String, usize> {
    let mut rank: HashMap<String, usize> = types.iter().map(|t| (t.clone(), 0)).collect();
    for _ in 0..=types.len() {
        let mut changed = false;
        for (coverer, covered) in edges {
            if coverer == covered {
                continue;
            }
            let want = rank[covered] + 1;
            if want <= types.len() && rank[coverer] < want {
                rank.insert(coverer.clone(), want);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Compact empty ranks.
    let used: BTreeSet<usize> = rank.values().copied().collect();
    let remap: HashMap<usize, usize> = used.into_iter().enumerate().map(|(n, r)| (r, n)).collect();
    rank.into_iter().map(|(t, r)| (t, remap[&r])).collect()
}

fn is_ok(status: &LinkStatus) -> bool {
    matches!(status, LinkStatus::Covers)
}

fn edge_path(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
    if x2 > x1 {
        let dx = (x2 - x1) / 2.0;
        format!(
            "M{x1:.1},{y1:.1} C{:.1},{y1:.1} {:.1},{y2:.1} {x2:.1},{y2:.1}",
            x1 + dx,
            x2 - dx
        )
    } else {
        // Same-column or backwards link: loop out to the right.
        format!(
            "M{x1:.1},{y1:.1} C{:.1},{y1:.1} {:.1},{y2:.1} {x2:.1},{y2:.1}",
            x1 + 50.0,
            x2 - 50.0
        )
    }
}

const ARROW_DEFS: &str = r##"<defs><marker id="ovft-arrow-ok" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="#94a3b8"/></marker><marker id="ovft-arrow-warn" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="#f59e0b"/></marker></defs>"##;

// ---------------------------------------------------------------------------
// Type flow overview
// ---------------------------------------------------------------------------

/// High-level flow between artifact types with coverage per type and link counts.
pub fn type_flow_svg(result: &TraceResult) -> String {
    let items: Vec<&LinkedSpecificationItem> = result.items.iter().collect();
    if items.is_empty() {
        return String::new();
    }
    let colors = TypeColors::new(result);
    let types = colors.0.clone();
    let resolver = Resolver::new(&items);

    let mut agg: BTreeMap<(String, String), usize> = BTreeMap::new();
    for it in &items {
        for l in &it.outgoing_links {
            if let Some(j) = resolver.resolve(&l.target_id) {
                let a = it.item.id.artifact_type.clone();
                let b = items[j].item.id.artifact_type.clone();
                *agg.entry((a, b)).or_default() += 1;
            }
        }
    }
    if agg.is_empty() {
        return String::new();
    }
    let type_edges: Vec<(String, String)> = agg.keys().cloned().collect();
    let ranks = type_ranks(&types, &type_edges);
    let ncols = ranks.values().max().copied().unwrap_or(0) + 1;
    let mut cols: Vec<Vec<&String>> = vec![Vec::new(); ncols];
    for t in &types {
        cols[ranks[t]].push(t);
    }

    let (nw, nh, gx, gy, m) = (170.0, 62.0, 120.0, 26.0, 16.0);
    let rows = cols.iter().map(|c| c.len()).max().unwrap_or(1);
    let width = m * 2.0 + ncols as f64 * nw + (ncols as f64 - 1.0) * gx;
    let height = m * 2.0 + rows as f64 * nh + (rows as f64 - 1.0) * gy;
    let mut pos: HashMap<&String, (f64, f64)> = HashMap::new();
    for (ci, col) in cols.iter().enumerate() {
        let col_h = col.len() as f64 * nh + (col.len() as f64 - 1.0).max(0.0) * gy;
        let y0 = m + (height - 2.0 * m - col_h) / 2.0;
        for (ri, t) in col.iter().enumerate() {
            pos.insert(*t, (m + ci as f64 * (nw + gx), y0 + ri as f64 * (nh + gy)));
        }
    }

    let max_count = agg.values().copied().max().unwrap_or(1) as f64;
    let mut svg = format!(
        r##"<svg viewBox="0 0 {width:.0} {height:.0}" class="chart-flow" role="img" aria-label="Traceability flow between artifact types">{ARROW_DEFS}"##
    );
    for ((coverer, covered), n) in &agg {
        let (Some(&(cx, cy)), Some(&(dx, dy))) = (pos.get(coverer), pos.get(covered)) else {
            continue;
        };
        if coverer == covered {
            continue;
        }
        let (x1, y1) = (dx + nw, dy + nh / 2.0);
        let (x2, y2) = (cx, cy + nh / 2.0);
        let sw = 1.5 + 5.0 * (*n as f64 / max_count);
        let _ = write!(
            svg,
            r##"<path d="{}" fill="none" stroke="#cbd5e1" stroke-width="{sw:.1}" stroke-linecap="round" opacity="0.9"><title>{} → {}: {n} link(s)</title></path>"##,
            edge_path(x1, y1, x2, y2),
            esc(coverer),
            esc(covered)
        );
        let (mx, my) = ((x1 + x2) / 2.0, (y1 + y2) / 2.0);
        let _ = write!(
            svg,
            r##"<g><rect x="{:.1}" y="{:.1}" width="30" height="18" rx="9" fill="white" stroke="#cbd5e1"/><text x="{mx:.1}" y="{:.1}" text-anchor="middle" font-size="11" font-weight="600" fill="{MUTED}">{n}</text></g>"##,
            mx - 15.0,
            my - 9.0,
            my + 4.0
        );
    }
    for t in &types {
        let (x, y) = pos[t];
        let color = colors.get(t);
        let (cov, tot, status) = result
            .coverage_summary
            .get(t)
            .map(|s| (s.covered, s.total, status_color(&s.status)))
            .unwrap_or((0, 0, MUTED));
        let frac = if tot == 0 {
            0.0
        } else {
            cov as f64 / tot as f64
        };
        let _ = write!(
            svg,
            r##"<g><title>{name}: {cov}/{tot} covered</title><rect x="{x:.1}" y="{y:.1}" width="{nw}" height="{nh}" rx="12" fill="white" stroke="#e2e8f0" stroke-width="1.5"/><rect x="{x:.1}" y="{y:.1}" width="6" height="{nh}" rx="3" fill="{color}"/><text x="{tx:.1}" y="{ty:.1}" font-size="14" font-weight="700" fill="{INK}">{label}</text><text x="{tx:.1}" y="{ty2:.1}" font-size="11" fill="{MUTED}">{cov}/{tot} covered</text><rect x="{tx:.1}" y="{by:.1}" width="{bw}" height="6" rx="3" fill="{TRACK}"/><rect x="{tx:.1}" y="{by:.1}" width="{fw:.1}" height="6" rx="3" fill="{status}"/></g>"##,
            name = esc(t),
            label = esc(&short(t, 16)),
            tx = x + 18.0,
            ty = y + 22.0,
            ty2 = y + 38.0,
            by = y + 46.0,
            bw = nw - 36.0,
            fw = (nw - 36.0) * frac,
        );
    }
    svg.push_str("</svg>");
    svg
}

// ---------------------------------------------------------------------------
// Item-level traceability graph
// ---------------------------------------------------------------------------

/// Layered item-level graph: one column per artifact-type layer, edges follow
/// "covered by". Returns `(svg, note)`; `svg` is empty when there is nothing
/// to draw. Nodes link to the item cards via `#anchor`.
pub fn trace_graph_svg(result: &TraceResult, max_nodes: usize) -> (String, Option<String>) {
    let all: Vec<&LinkedSpecificationItem> = result.items.iter().collect();
    let resolver = Resolver::new(&all);
    let all_edges = collect_edges(&all, &resolver);
    if all_edges.is_empty() {
        return (String::new(), None);
    }

    // Select nodes: all, or a subset when too large. The subset keeps
    // edge-incident nodes first (so links stay visible), then defective
    // items, then everything else.
    let mut keep: Vec<usize> = (0..all.len()).collect();
    let mut note = None;
    if all.len() > max_nodes {
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let mut priority: Vec<usize> = Vec::new();
        let push = |n: usize, seen: &mut BTreeSet<usize>, priority: &mut Vec<usize>| {
            if seen.insert(n) {
                priority.push(n);
            }
        };
        for (a, b, _) in &all_edges {
            push(*a, &mut seen, &mut priority);
            push(*b, &mut seen, &mut priority);
        }
        for (n, it) in all.iter().enumerate() {
            if it.is_defect {
                push(n, &mut seen, &mut priority);
            }
        }
        for n in 0..all.len() {
            push(n, &mut seen, &mut priority);
        }
        keep = priority.into_iter().take(max_nodes).collect();
        note = Some(format!(
            "Large project ({} items): showing defective items and their direct neighbours ({} nodes).",
            all.len(),
            keep.len()
        ));
    }
    let index_of: HashMap<usize, usize> = keep.iter().enumerate().map(|(n, &o)| (o, n)).collect();
    let nodes: Vec<&LinkedSpecificationItem> = keep.iter().map(|&i| all[i]).collect();
    let edges: Vec<(usize, usize, LinkStatus)> = all_edges
        .iter()
        .filter_map(|(a, b, s)| Some((*index_of.get(a)?, *index_of.get(b)?, s.clone())))
        .collect();
    if edges.is_empty() {
        return (String::new(), note);
    }

    let colors = TypeColors::new(result);
    let types: Vec<String> = colors.0.clone();
    let type_edges: Vec<(String, String)> = edges
        .iter()
        .map(|(a, b, _)| {
            (
                nodes[*a].item.id.artifact_type.clone(),
                nodes[*b].item.id.artifact_type.clone(),
            )
        })
        .collect();
    let ranks = type_ranks(&types, &type_edges);
    let ncols = ranks.values().max().copied().unwrap_or(0) + 1;

    // Initial columns sorted by (type, id).
    let mut cols: Vec<Vec<usize>> = vec![Vec::new(); ncols];
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|&n| {
        (
            nodes[n].item.id.artifact_type.clone(),
            nodes[n].item.id.to_string(),
        )
    });
    for n in order {
        cols[ranks[&nodes[n].item.id.artifact_type]].push(n);
    }

    // Barycenter sweeps to reduce edge crossings.
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for (a, b, _) in &edges {
        neighbours[*a].push(*b);
        neighbours[*b].push(*a);
    }
    let rank_of = |n: usize| ranks[&nodes[n].item.id.artifact_type];
    for sweep in 0..6 {
        let forward = sweep % 2 == 0;
        let range: Vec<usize> = if forward {
            (1..ncols).collect()
        } else {
            (0..ncols.saturating_sub(1)).rev().collect()
        };
        for c in range {
            let ref_col = if forward { c - 1 } else { c + 1 };
            let row_of: HashMap<usize, usize> = cols[ref_col]
                .iter()
                .enumerate()
                .map(|(r, &n)| (n, r))
                .collect();
            let own: HashMap<usize, usize> =
                cols[c].iter().enumerate().map(|(r, &n)| (n, r)).collect();
            let mut keyed: Vec<(f64, usize)> = cols[c]
                .iter()
                .map(|&n| {
                    let rows: Vec<f64> = neighbours[n]
                        .iter()
                        .filter(|&&m| rank_of(m) == ref_col)
                        .filter_map(|m| row_of.get(m).map(|&r| r as f64))
                        .collect();
                    let key = if rows.is_empty() {
                        own[&n] as f64
                    } else {
                        rows.iter().sum::<f64>() / rows.len() as f64
                    };
                    (key, n)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            cols[c] = keyed.into_iter().map(|(_, n)| n).collect();
        }
    }

    let (nw, nh, gx, gy, m, top) = (196.0, 46.0, 110.0, 12.0, 16.0, 38.0);
    let rows = cols.iter().map(|c| c.len()).max().unwrap_or(1);
    let width = m * 2.0 + ncols as f64 * nw + (ncols as f64 - 1.0) * gx;
    let height = top + m + rows as f64 * (nh + gy);
    let mut pos: HashMap<usize, (f64, f64)> = HashMap::new();
    for (ci, col) in cols.iter().enumerate() {
        for (ri, &n) in col.iter().enumerate() {
            pos.insert(n, (m + ci as f64 * (nw + gx), top + ri as f64 * (nh + gy)));
        }
    }

    let mut svg = format!(
        r##"<svg viewBox="0 0 {width:.0} {height:.0}" width="{width:.0}" class="trace-graph" role="img" aria-label="Traceability graph">{ARROW_DEFS}"##
    );

    // Column headers.
    for (ci, col) in cols.iter().enumerate() {
        let label: BTreeSet<&str> = col
            .iter()
            .map(|&n| nodes[n].item.id.artifact_type.as_str())
            .collect();
        let label = label.into_iter().collect::<Vec<_>>().join(" / ");
        let _ = write!(
            svg,
            r##"<text x="{:.1}" y="22" font-size="12" font-weight="700" letter-spacing="1" fill="{MUTED}">{}</text>"##,
            m + ci as f64 * (nw + gx),
            esc(&label.to_uppercase())
        );
    }

    // Edges first so nodes sit on top.
    for (a, b, status) in &edges {
        let (ax, ay) = pos[a];
        let (bx, by) = pos[b];
        let (x1, y1, x2, y2) = if ax >= bx {
            (bx + nw, by + nh / 2.0, ax, ay + nh / 2.0)
        } else {
            (ax + nw, ay + nh / 2.0, bx, by + nh / 2.0)
        };
        let ok = is_ok(status);
        let _ = write!(
            svg,
            r##"<path class="gedge" data-s="{a}" data-t="{b}" d="{}" fill="none" stroke="{}" stroke-width="1.6"{} marker-end="url(#ovft-arrow-{})"><title>{} covers {} ({})</title></path>"##,
            edge_path(x1, y1, x2, y2),
            if ok { "#94a3b8" } else { PARTIAL },
            if ok { "" } else { r#" stroke-dasharray="5 4""# },
            if ok { "ok" } else { "warn" },
            esc(&nodes[*a].item.id.to_string()),
            esc(&nodes[*b].item.id.to_string()),
            status
        );
    }

    for (n, it) in nodes.iter().enumerate() {
        let (x, y) = pos[&n];
        let id = &it.item.id;
        let (fill, stroke) = match it.coverage_status {
            CoverageStatus::Covered => ("#f0fdf4", "#86efac"),
            CoverageStatus::Partial => ("#fffbeb", "#fcd34d"),
            CoverageStatus::Uncovered => ("#fef2f2", "#fca5a5"),
        };
        let (stroke, sw) = if it.is_defect {
            (UNCOVERED, 2.0)
        } else {
            (stroke, 1.2)
        };
        let _ = write!(
            svg,
            r##"<a href="#{anchor}"><g class="gnode" data-n="{n}"><title>{full}&#10;{title}&#10;coverage: {cov}</title><rect class="box" x="{x:.1}" y="{y:.1}" width="{nw}" height="{nh}" rx="9" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/><rect x="{x:.1}" y="{y:.1}" width="5" height="{nh}" rx="2.5" fill="{tc}"/><text x="{tx:.1}" y="{t1:.1}" font-size="12.5" font-weight="600" fill="{INK}">{name}</text><text x="{tx:.1}" y="{t2:.1}" font-size="10.5" fill="{MUTED}">{ty}~{rev} · {cov}</text></g></a>"##,
            anchor = id.to_html_id(),
            full = esc(&id.to_string()),
            title = esc(&it.title()),
            cov = it.coverage_status,
            tc = colors.get(&id.artifact_type),
            tx = x + 14.0,
            t1 = y + 19.0,
            t2 = y + 34.0,
            name = esc(&short(&id.name, 24)),
            ty = esc(&id.artifact_type),
            rev = id.revision,
        );
    }
    svg.push_str("</svg>");
    (svg, note)
}

// ---------------------------------------------------------------------------
// Mermaid export
// ---------------------------------------------------------------------------

/// Mermaid `flowchart LR` source of the traceability graph (covered → coverer).
/// Paste into GitHub/GitLab markdown or any Mermaid renderer.
pub fn mermaid_flowchart(result: &TraceResult, max_nodes: usize) -> String {
    let all: Vec<&LinkedSpecificationItem> = result.items.iter().collect();
    let resolver = Resolver::new(&all);
    let edges = collect_edges(&all, &resolver);

    let mut connected: BTreeSet<usize> = BTreeSet::new();
    for (a, b, _) in &edges {
        connected.insert(*a);
        connected.insert(*b);
    }
    for (i, it) in all.iter().enumerate() {
        if it.is_defect {
            connected.insert(i);
        }
    }
    let nodes: Vec<usize> = connected.into_iter().take(max_nodes).collect();
    let in_graph: BTreeSet<usize> = nodes.iter().copied().collect();

    let mut out = String::from("flowchart LR\n");
    let mut by_type: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for &n in &nodes {
        by_type
            .entry(all[n].item.id.artifact_type.as_str())
            .or_default()
            .push(n);
    }
    for (t, ns) in &by_type {
        let _ = writeln!(out, "  subgraph {t}_grp [\"{}\"]", t.replace('"', ""));
        for &n in ns {
            let id = &all[n].item.id;
            let _ = writeln!(
                out,
                "    n{n}[\"{}<br/>{}\"]:::{}",
                id.name.replace('"', "'"),
                format_args!("{}~{}", id.artifact_type, id.revision),
                if all[n].is_defect {
                    "defect".to_string()
                } else {
                    all[n].coverage_status.to_string()
                }
            );
        }
        out.push_str("  end\n");
    }
    for (a, b, s) in &edges {
        if in_graph.contains(a) && in_graph.contains(b) {
            if is_ok(s) {
                let _ = writeln!(out, "  n{b} --> n{a}");
            } else {
                let _ = writeln!(out, "  n{b} -. \"{s}\" .-> n{a}");
            }
        }
    }
    out.push_str("  classDef covered fill:#f0fdf4,stroke:#16a34a,color:#14532d;\n");
    out.push_str("  classDef partial fill:#fffbeb,stroke:#f59e0b,color:#78350f;\n");
    out.push_str("  classDef uncovered fill:#fef2f2,stroke:#ef4444,color:#7f1d1d;\n");
    out.push_str("  classDef defect fill:#fef2f2,stroke:#dc2626,stroke-width:3px,color:#7f1d1d;\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Linker, Location, SpecificationItem, SpecificationItemId};
    use crate::{Config, Tracer};
    use std::path::PathBuf;

    fn sample() -> TraceResult {
        let mk = |id: &str, needs: &[&str], covers: &[&str]| {
            let mut b = SpecificationItem::builder(SpecificationItemId::parse(id).unwrap())
                .title(format!("Title <{id}>"))
                .location(Location::new(PathBuf::from("f.md"), 1))
                .needs_multiple(needs.iter().map(|s| s.to_string()).collect());
            for c in covers {
                b = b.covers(SpecificationItemId::parse(c).unwrap());
            }
            b.build()
        };
        let items = vec![
            mk("req~a~1", &["dsn"], &[]),
            mk("req~b~1", &["dsn"], &[]),
            mk("dsn~a~1", &["impl"], &["req~a~1"]),
            mk("impl~a~1", &[], &["dsn~a~1"]),
        ];
        let linked = Linker::new().link_items(items).unwrap();
        Tracer::new(Config::empty()).analyze_trace(&linked)
    }

    #[test]
    fn donut_and_bars_render() {
        let r = sample();
        assert!(status_donut(&r).contains("<svg"));
        let mut s: Vec<_> = r.coverage_summary.clone().into_iter().collect();
        s.sort_by(|a, b| a.0.cmp(&b.0));
        assert!(coverage_bars(&s).contains("req"));
        assert!(defect_bars(&r).contains("uncovered"));
    }

    #[test]
    fn graph_has_links_and_escapes() {
        let r = sample();
        let (svg, note) = trace_graph_svg(&r, 100);
        assert!(note.is_none());
        assert!(svg.contains("href=\"#"));
        assert!(svg.contains("&lt;req~a~1&gt;"));
        assert!(!svg.contains("<req~a~1>"));
        assert!(svg.matches("class=\"gedge\"").count() == 2);
    }

    #[test]
    fn graph_limits_large_projects() {
        let r = sample();
        let (svg, note) = trace_graph_svg(&r, 2);
        assert!(note.is_some());
        let _ = svg;
    }

    #[test]
    fn flow_and_mermaid() {
        let r = sample();
        assert!(type_flow_svg(&r).contains("covered"));
        let m = mermaid_flowchart(&r, 100);
        assert!(m.starts_with("flowchart LR"));
        assert!(m.contains("-->"));
    }

    #[test]
    fn ranks_follow_coverage_direction() {
        let types: Vec<String> = ["req", "dsn", "impl"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let edges = vec![
            ("dsn".to_string(), "req".to_string()),
            ("impl".to_string(), "dsn".to_string()),
        ];
        let r = type_ranks(&types, &edges);
        assert!(r["req"] < r["dsn"] && r["dsn"] < r["impl"]);
    }
}
