//! Language Server Protocol mode for `cargo ovft ovft --lsp`.
//!
//! Provides diagnostics, hover and go-to-definition for OVFT requirement IDs
//! over stdio so any LSP capable editor (Zed, Neovim, Helix, ...) can use it.

use anyhow::{Context, Result};
use lsp_server::{Connection, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::{GotoDefinition, HoverRequest, Request as _};
use lsp_types::{
    Diagnostic, DiagnosticSeverity, ExecuteCommandOptions, GotoDefinitionResponse, Hover,
    HoverContents, HoverProviderCapability, InitializeParams, Location as LspLocation,
    MarkupContent, MarkupKind, OneOf, Position, PublishDiagnosticsParams, Range,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
};
use ovft_core::core::{LinkStatus, LinkedSpecificationItem, SpecificationItemId};
use ovft_core::{Config, TraceResult, Tracer};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const CMD_REFRESH: &str = "ovft.refresh";
pub const CMD_REPORT: &str = "ovft.generateReport";
pub const CMD_RELEASE_NOTES: &str = "ovft.releaseNotes";

/// Run the LSP server on stdio until the client disconnects.
pub fn run(config_path: Option<String>) -> Result<()> {
    let (connection, io_threads) = Connection::stdio();

    let capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        execute_command_provider: Some(ExecuteCommandOptions {
            commands: vec![
                CMD_REFRESH.to_string(),
                CMD_REPORT.to_string(),
                CMD_RELEASE_NOTES.to_string(),
            ],
            ..Default::default()
        }),
        ..Default::default()
    })?;

    let init_value = connection
        .initialize(capabilities)
        .map_err(|e| anyhow::anyhow!("LSP initialize failed: {e}"))?;
    let init: InitializeParams = serde_json::from_value(init_value)?;

    #[allow(deprecated)]
    let root = init
        .workspace_folders
        .as_ref()
        .and_then(|f| f.first())
        .and_then(|f| uri_to_path(&f.uri))
        .or_else(|| init.root_uri.as_ref().and_then(uri_to_path))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let mut server = Server::new(root, config_path);
    server.main_loop(&connection)?;
    // The writer thread only finishes once the sender is dropped.
    drop(connection);
    io_threads.join()?;
    Ok(())
}

struct Server {
    root: PathBuf,
    config: Config,
    result: Option<TraceResult>,
    /// Open documents (full text), keyed by absolute path.
    docs: HashMap<PathBuf, String>,
    /// Files we published diagnostics for last time.
    published: HashSet<PathBuf>,
}

impl Server {
    fn new(root: PathBuf, config_path: Option<String>) -> Self {
        // Tracer resolves configured directories relative to the cwd.
        let _ = std::env::set_current_dir(&root);
        let config = match config_path {
            Some(p) => Config::from_file(&p).unwrap_or_default(),
            None => Config::find_and_load_config(&root).unwrap_or_default(),
        };
        Self {
            root,
            config,
            result: None,
            docs: HashMap::new(),
            published: HashSet::new(),
        }
    }

    fn main_loop(&mut self, conn: &Connection) -> Result<()> {
        self.refresh(conn);
        for msg in &conn.receiver {
            match msg {
                Message::Request(req) => {
                    if conn
                        .handle_shutdown(&req)
                        .map_err(|e| anyhow::anyhow!("{e}"))?
                    {
                        return Ok(());
                    }
                    self.handle_request(conn, req);
                }
                Message::Notification(note) => self.handle_notification(conn, note),
                Message::Response(_) => {}
            }
        }
        Ok(())
    }

    fn handle_notification(&mut self, conn: &Connection, note: Notification) {
        match note.method.as_str() {
            DidOpenTextDocument::METHOD => {
                if let Ok(p) =
                    serde_json::from_value::<lsp_types::DidOpenTextDocumentParams>(note.params)
                {
                    if let Some(path) = uri_to_path(&p.text_document.uri) {
                        self.docs.insert(path, p.text_document.text);
                    }
                }
            }
            DidChangeTextDocument::METHOD => {
                if let Ok(p) =
                    serde_json::from_value::<lsp_types::DidChangeTextDocumentParams>(note.params)
                {
                    if let (Some(path), Some(change)) = (
                        uri_to_path(&p.text_document.uri),
                        p.content_changes.into_iter().last(),
                    ) {
                        self.docs.insert(path, change.text);
                    }
                }
            }
            DidCloseTextDocument::METHOD => {
                if let Ok(p) =
                    serde_json::from_value::<lsp_types::DidCloseTextDocumentParams>(note.params)
                {
                    if let Some(path) = uri_to_path(&p.text_document.uri) {
                        self.docs.remove(&path);
                    }
                }
            }
            DidSaveTextDocument::METHOD => self.refresh(conn),
            _ => {}
        }
    }

    fn handle_request(&mut self, conn: &Connection, req: Request) {
        let Request { id, method, params } = req;
        let response = match method.as_str() {
            HoverRequest::METHOD => {
                let hover = serde_json::from_value(params)
                    .ok()
                    .and_then(|p: lsp_types::HoverParams| {
                        let tdp = p.text_document_position_params;
                        self.hover(&tdp.text_document.uri, tdp.position)
                    });
                Response::new_ok(id, hover)
            }
            GotoDefinition::METHOD => {
                let locs = serde_json::from_value(params)
                    .ok()
                    .and_then(|p: lsp_types::GotoDefinitionParams| {
                        let tdp = p.text_document_position_params;
                        self.definition(&tdp.text_document.uri, tdp.position)
                    });
                Response::new_ok(id, locs)
            }
            lsp_types::request::ExecuteCommand::METHOD => {
                match serde_json::from_value::<lsp_types::ExecuteCommandParams>(params) {
                    Ok(p) => match self.execute_command(conn, &p.command) {
                        Ok(v) => Response::new_ok(id, v),
                        Err(e) => Response::new_err(id, -32603, format!("{e:#}")),
                    },
                    Err(e) => Response::new_err(id, -32602, e.to_string()),
                }
            }
            _ => Response::new_err(id, -32601, format!("method not found: {method}")),
        };
        let _ = conn.sender.send(Message::Response(response));
    }

    // ---- tracing & diagnostics -------------------------------------------

    fn refresh(&mut self, conn: &Connection) {
        match Tracer::new(self.config.clone()).trace() {
            Ok(result) => self.result = Some(result),
            Err(e) => {
                log::error!("trace failed: {e}");
                return;
            }
        }
        self.publish_diagnostics(conn);
    }

    fn publish_diagnostics(&mut self, conn: &Connection) {
        let Some(result) = &self.result else { return };
        let by_file = collect_diagnostics(result, &self.root, |p| self.read_file(p));

        let current: HashSet<PathBuf> = by_file.keys().cloned().collect();
        for stale in self.published.difference(&current) {
            send_diagnostics(conn, stale, Vec::new());
        }
        for (path, diags) in by_file {
            send_diagnostics(conn, &path, diags);
        }
        self.published = current;
    }

    fn read_file(&self, path: &Path) -> Option<String> {
        self.docs
            .get(path)
            .cloned()
            .or_else(|| std::fs::read_to_string(path).ok())
    }

    // ---- hover & definition ----------------------------------------------

    fn id_at(&self, uri: &Uri, pos: Position) -> Option<(PathBuf, SpecificationItemId)> {
        let path = uri_to_path(uri)?;
        let text = self.read_file(&path)?;
        let line = text.lines().nth(pos.line as usize)?;
        let id = id_at_column(line, pos.character as usize)?;
        Some((path, id))
    }

    fn hover(&self, uri: &Uri, pos: Position) -> Option<Hover> {
        let (_, id) = self.id_at(uri, pos)?;
        let result = self.result.as_ref()?;
        Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: hover_markdown(result, &id),
            }),
            range: None,
        })
    }

    fn definition(&self, uri: &Uri, pos: Position) -> Option<GotoDefinitionResponse> {
        let (path, id) = self.id_at(uri, pos)?;
        let result = self.result.as_ref()?;
        let targets = definition_targets(result, &id, &self.root, &path, pos.line);
        if targets.is_empty() {
            return None;
        }
        let locations: Vec<LspLocation> = targets
            .into_iter()
            .filter_map(|(p, line)| {
                let uri = path_to_uri(&p)?;
                let line = line.saturating_sub(1);
                Some(LspLocation {
                    uri,
                    range: Range::new(Position::new(line, 0), Position::new(line, 0)),
                })
            })
            .collect();
        Some(GotoDefinitionResponse::Array(locations))
    }

    // ---- commands ---------------------------------------------------------

    fn execute_command(&mut self, conn: &Connection, command: &str) -> Result<serde_json::Value> {
        match command {
            CMD_REFRESH => {
                self.refresh(conn);
                Ok(serde_json::Value::Null)
            }
            CMD_REPORT => {
                self.refresh(conn);
                let result = self.result.as_ref().context("no trace result available")?;
                let out = self.root.join("target").join("requirements_report.html");
                if let Some(dir) = out.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                Tracer::new(self.config.clone())
                    .generate_html_report(result, &out)
                    .context("failed to generate HTML report")?;
                Ok(serde_json::json!({ "path": out.to_string_lossy() }))
            }
            CMD_RELEASE_NOTES => {
                let from = ovft_core::release_notes::latest_tag(&self.root)
                    .context("no git tag found to use as baseline")?;
                let baseline =
                    ovft_core::release_notes::trace_at_ref(&self.config, &self.root, &from)?;
                let current = Tracer::new(self.config.clone()).trace()?;
                let entries = ovft_core::release_notes::newly_covered_items(
                    &baseline,
                    &current,
                    &self.config.release_note_types,
                );
                let md = ovft_core::release_notes::render_markdown(&from, &entries);
                let out = self.root.join("target").join("release-notes.md");
                if let Some(dir) = out.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&out, md)?;
                Ok(serde_json::json!({ "path": out.to_string_lossy() }))
            }
            other => anyhow::bail!("unknown command: {other}"),
        }
    }
}

fn send_diagnostics(conn: &Connection, path: &Path, diagnostics: Vec<Diagnostic>) {
    let Some(uri) = path_to_uri(path) else { return };
    let params = PublishDiagnosticsParams {
        uri,
        diagnostics,
        version: None,
    };
    let note = Notification::new(PublishDiagnostics::METHOD.to_string(), params);
    let _ = conn.sender.send(Message::Notification(note));
}

// ---- pure helpers (unit tested) --------------------------------------------

fn abs(root: &Path, p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        root.join(p)
    }
}

fn is_problem_link(status: &LinkStatus) -> bool {
    matches!(
        status,
        LinkStatus::Orphaned
            | LinkStatus::Outdated
            | LinkStatus::Predated
            | LinkStatus::Ambiguous
            | LinkStatus::Unwanted
            | LinkStatus::Duplicate
    )
}

/// Build diagnostics per absolute file path from a trace result.
fn collect_diagnostics(
    result: &TraceResult,
    root: &Path,
    read: impl Fn(&Path) -> Option<String>,
) -> HashMap<PathBuf, Vec<Diagnostic>> {
    let mut out: HashMap<PathBuf, Vec<Diagnostic>> = HashMap::new();
    let mut texts: HashMap<PathBuf, Option<String>> = HashMap::new();

    let mut push = |path: PathBuf, line: u32, severity: DiagnosticSeverity, message: String| {
        let text = texts.entry(path.clone()).or_insert_with(|| read(&path));
        let line0 = line.saturating_sub(1);
        let len = text
            .as_ref()
            .and_then(|t| t.lines().nth(line0 as usize))
            .map(|l| l.encode_utf16().count() as u32)
            .unwrap_or(0);
        out.entry(path).or_default().push(Diagnostic {
            range: Range::new(Position::new(line0, 0), Position::new(line0, len)),
            severity: Some(severity),
            source: Some("ovft".to_string()),
            message,
            ..Default::default()
        });
    };

    for item in &result.items {
        let Some(loc) = &item.item.location else { continue };
        let path = abs(root, &loc.path);

        if item.is_defect {
            let desc = result
                .defects
                .iter()
                .find(|d| d.item_id.as_ref() == Some(item.id()))
                .map(|d| d.description.clone())
                .unwrap_or_else(|| format!("{} is not covered", item.id()));
            push(path.clone(), loc.line, DiagnosticSeverity::WARNING, desc);
        }
        for link in &item.outgoing_links {
            if is_problem_link(&link.status) {
                push(
                    path.clone(),
                    loc.line,
                    DiagnosticSeverity::ERROR,
                    format!("{} covers {}: {}", item.id(), link.target_id, link.status),
                );
            }
        }
    }
    out
}

fn is_id_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '~' | ':')
}

/// Find the OVFT ID (`type~name~revision`) under a UTF-16 column in a line.
fn id_at_column(line: &str, col_utf16: usize) -> Option<SpecificationItemId> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut utf16 = 0;
    let mut idx = chars.len();
    for (i, (_, c)) in chars.iter().enumerate() {
        if utf16 >= col_utf16 {
            idx = i;
            break;
        }
        utf16 += c.len_utf16();
    }
    let at = |i: usize| chars.get(i).map(|(_, c)| is_id_char(*c)).unwrap_or(false);
    // Cursor may sit just past the token end.
    if !at(idx) {
        if idx > 0 && at(idx - 1) {
            idx -= 1;
        } else {
            return None;
        }
    }
    let mut s = idx;
    while s > 0 && at(s - 1) {
        s -= 1;
    }
    let mut e = idx;
    while at(e + 1) {
        e += 1;
    }
    let start = chars[s].0;
    let end = chars[e].0 + chars[e].1.len_utf8();
    let token = line[start..end].trim_matches(|c: char| matches!(c, '.' | ':' | '-'));
    SpecificationItemId::parse(token).ok()
}

fn find_item<'a>(
    result: &'a TraceResult,
    id: &SpecificationItemId,
) -> Option<&'a LinkedSpecificationItem> {
    result.items.iter().find(|i| i.id() == id)
}

fn hover_markdown(result: &TraceResult, id: &SpecificationItemId) -> String {
    let Some(item) = find_item(result, id) else {
        return format!("**`{id}`**\n\n_Unknown item (no definition found)._");
    };
    let mut md = format!("**{}**\n\n`{}`", item.title(), item.id());
    md.push_str(&format!("\n\nCoverage: **{}**", item.coverage_status));
    if item.is_defect {
        md.push_str(" ⚠️");
    }
    if !item.item.needs.is_empty() {
        md.push_str(&format!("\n\nNeeds: {}", item.item.needs.join(", ")));
    }
    if !item.incoming_links.is_empty() {
        md.push_str("\n\nCovered by:");
        for l in &item.incoming_links {
            let src = l
                .source_id
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_default();
            md.push_str(&format!("\n- `{}` ({})", src, l.status));
        }
    }
    if !item.outgoing_links.is_empty() {
        md.push_str("\n\nCovers:");
        for l in &item.outgoing_links {
            md.push_str(&format!("\n- `{}` ({})", l.target_id, l.status));
        }
    }
    md
}

/// Locations (absolute path, 1-based line) for go-to-definition.
///
/// If the cursor is on the definition itself, returns the covering items
/// instead (jump "back"); otherwise returns the definition.
fn definition_targets(
    result: &TraceResult,
    id: &SpecificationItemId,
    root: &Path,
    cur_path: &Path,
    cur_line0: u32,
) -> Vec<(PathBuf, u32)> {
    let Some(item) = find_item(result, id) else {
        return Vec::new();
    };
    let loc_of = |i: &LinkedSpecificationItem| {
        i.item
            .location
            .as_ref()
            .map(|l| (abs(root, &l.path), l.line))
    };
    let own = loc_of(item);
    let on_definition = matches!(&own, Some((p, line))
        if p == cur_path && line.saturating_sub(1) == cur_line0);
    if !on_definition {
        return own.into_iter().collect();
    }
    item.incoming_links
        .iter()
        .filter_map(|l| l.source_id.as_ref())
        .filter_map(|sid| find_item(result, sid))
        .filter_map(loc_of)
        .collect()
}

// ---- URI helpers ------------------------------------------------------------

fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let s = uri.as_str().strip_prefix("file://")?;
    // Strip optional authority (e.g. `localhost`).
    let s = &s[s.find('/')?..];
    let decoded = percent_decode(s);
    #[cfg(windows)]
    let decoded = decoded.trim_start_matches('/').to_string();
    Some(PathBuf::from(decoded))
}

fn path_to_uri(path: &Path) -> Option<Uri> {
    let p = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    if !p.starts_with('/') {
        out.push('/');
    }
    for b in p.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    Uri::from_str(&out).ok()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ovft_core::core::{Linker, Location, SpecificationItem};

    fn item(id: &str, line: u32, path: &str, needs: &[&str], covers: &[&str]) -> SpecificationItem {
        let mut b = SpecificationItem::builder(SpecificationItemId::parse(id).unwrap())
            .title(format!("title of {id}"))
            .location(Location::new(PathBuf::from(path), line))
            .needs_multiple(needs.iter().map(|s| s.to_string()).collect());
        for c in covers {
            b = b.covers(SpecificationItemId::parse(c).unwrap());
        }
        b.build()
    }

    fn trace() -> TraceResult {
        let items = vec![
            item("req~a~1", 3, "docs/spec.md", &["impl"], &[]),
            item("req~lonely~1", 9, "docs/spec.md", &["impl"], &[]),
            item("impl~a~1", 7, "src/lib.rs", &[], &["req~a~1"]),
            item("impl~ghost~1", 12, "src/lib.rs", &[], &["req~missing~1"]),
        ];
        let linked = Linker::new().link_items(items).unwrap();
        Tracer::new(Config::empty()).analyze_trace(&linked)
    }

    #[test]
    fn extracts_id_under_cursor() {
        let line = "/// [impl->req~a-b~1]";
        let col = line.find("req").unwrap() + 2;
        assert_eq!(id_at_column(line, col).unwrap().to_string(), "req~a-b~1");
        assert!(id_at_column("no ids here", 3).is_none());
    }

    #[test]
    fn diagnostics_flag_uncovered_and_orphaned() {
        let r = trace();
        let d = collect_diagnostics(&r, Path::new("/ws"), |_| None);
        let spec = &d[&PathBuf::from("/ws/docs/spec.md")];
        assert!(spec.iter().any(|x| x.range.start.line == 8));
        assert!(!spec.iter().any(|x| x.range.start.line == 2));
        let src = &d[&PathBuf::from("/ws/src/lib.rs")];
        assert!(src
            .iter()
            .any(|x| x.range.start.line == 11 && x.message.contains("orphaned")));
    }

    #[test]
    fn hover_shows_title_and_links() {
        let r = trace();
        let md = hover_markdown(&r, &SpecificationItemId::parse("req~a~1").unwrap());
        assert!(md.contains("title of req~a~1"));
        assert!(md.contains("impl~a~1"));
        let unknown = hover_markdown(&r, &SpecificationItemId::parse("req~nope~1").unwrap());
        assert!(unknown.contains("Unknown"));
    }

    #[test]
    fn definition_goes_both_ways() {
        let r = trace();
        let id = SpecificationItemId::parse("req~a~1").unwrap();
        let root = Path::new("/ws");
        let fwd = definition_targets(&r, &id, root, Path::new("/ws/src/lib.rs"), 6);
        assert_eq!(fwd, vec![(PathBuf::from("/ws/docs/spec.md"), 3)]);
        let back = definition_targets(&r, &id, root, Path::new("/ws/docs/spec.md"), 2);
        assert_eq!(back, vec![(PathBuf::from("/ws/src/lib.rs"), 7)]);
    }

    #[test]
    fn uri_roundtrip_with_spaces() {
        let p = PathBuf::from("/tmp/my project/a.rs");
        let u = path_to_uri(&p).unwrap();
        assert_eq!(u.as_str(), "file:///tmp/my%20project/a.rs");
        assert_eq!(uri_to_path(&u).unwrap(), p);
    }
}
