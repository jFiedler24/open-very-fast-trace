//! ReqIF (Requirements Interchange Format, OMG standard) import and export.
//!
//! ReqIF 1.x is the XML exchange format used by tools like DOORS and Polarion.
//! This module maps ReqIF SPEC-OBJECTs to [`SpecificationItem`]s and back.
//!
//! Dialect notes / limitations:
//! - Both dialect roots are accepted: `REQ-IF` (OMG ReqIF 1.0.1/1.1/1.2) and
//!   `RIF` (pre-OMG RIF 1.1a/1.2). Any other root element is an error.
//! - Only `ATTRIBUTE-VALUE-*` elements with a `THE-VALUE` / `THE-VALUE-XHTML`
//!   payload are read; embedded binary blobs (`ATTRIBUTE-VALUE-EMBEDDED-DOCUMENT`)
//!   are skipped. Enumeration values (`ENUM-VALUE-REF`) are resolved to the
//!   enum value's `LONG-NAME`.
//! - The requirement text is located by resolving each value's attribute
//!   definition reference to its `LONG-NAME` and matching, in priority order:
//!   "ReqIF.Text", "Description", "Text" (case-insensitive). If none matches,
//!   the first XHTML value is used, then the first plain value.
//! - Canonical item ids are derived from the ReqIF `IDENTIFIER` (stable when
//!   the source tool renames headings), never from the `LONG-NAME`; the
//!   `LONG-NAME` becomes the item title.
//! - `SPEC-RELATION`s become coverage links honoring the relation type:
//!   "covers" (SOURCE covers TARGET, the default), "covered-by" (inverted)
//!   or "ignore", configurable per relation type LONG-NAME.
//! - All attribute values beyond the picked text survive as named item
//!   attributes (e.g. Status, Priority).
//! - Items are ordered like the first SPECIFICATION's hierarchy;
//!   chapter nesting itself and SPEC-RELATION-GROUPs are not preserved.
//! - Unknown elements and attributes are silently skipped, never an error.

use crate::core::model::{SpecificationItem, SpecificationItemId};
use crate::{Error, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

/// Well-known LONG-NAMEs for the requirement text attribute, in priority order.
const TEXT_ATTRIBUTE_NAMES: [&str; 3] = ["ReqIF.Text", "Description", "Text"];

/// XHTML element names that imply a paragraph/line break when stripping markup.
const BLOCK_TAGS: [&str; 10] = ["p", "div", "br", "li", "tr", "h1", "h2", "h3", "h4", "table"];

/// Local name of an element, ignoring any namespace prefix.
fn local_name(raw: &[u8]) -> &str {
    let s = std::str::from_utf8(raw).unwrap_or("");
    s.rsplit(':').next().unwrap_or(s)
}

/// Read an attribute of a start element by local name, with entities decoded.
fn get_attr(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| local_name(a.key.as_ref()) == name)
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

/// Lowercase, non-alphanumeric runs replaced by a single dash, dashes trimmed.
fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true; // prevents a leading dash
    for c in s.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Internal line-break marker used while stripping XHTML, so that real
/// newlines in the source (pure formatting) can collapse as whitespace.
const LINE_BREAK: char = '\u{1}';

/// Strip XHTML markup to plain text: block-level tags become newlines and
/// whitespace runs collapse to single spaces. The input must already be
/// entity-decoded (quick_xml unescapes text events before they reach us);
/// decoding again here would corrupt literal text like `&lt;`.
fn xhtml_to_text(xhtml: &str) -> String {
    // Replace block-level tags with break markers, drop all other tags.
    let mut marked = String::with_capacity(xhtml.len());
    let mut rest = xhtml;
    while let Some(pos) = rest.find('<') {
        marked.push_str(&rest[..pos]);
        rest = &rest[pos..];
        let Some(end) = rest.find('>') else { break };
        let tag = rest[1..end]
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_lowercase();
        if BLOCK_TAGS.contains(&tag.as_str()) {
            marked.push(LINE_BREAK);
        }
        rest = &rest[end + 1..];
    }
    marked.push_str(rest);

    marked
        .split(LINE_BREAK)
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Default)]
struct RawAttrDef {
    id: String,
    long_name: String,
}

#[derive(Debug, Default)]
struct RawObjectType {
    id: String,
    long_name: String,
    attr_defs: Vec<RawAttrDef>,
}

#[derive(Debug, Default)]
struct RawValue {
    attr_def_ref: Option<String>,
    text: String,
    is_xhtml: bool,
}

#[derive(Debug, Default)]
struct RawObject {
    id: String,
    long_name: String,
    type_ref: String,
    values: Vec<RawValue>,
}

#[derive(Debug, Default)]
struct RawRelation {
    source: String,
    target: String,
    type_ref: String,
}

#[derive(Debug, Default)]
struct RawDocument {
    object_types: Vec<RawObjectType>,
    objects: Vec<RawObject>,
    relations: Vec<RawRelation>,
    /// ENUM-VALUE IDENTIFIER → LONG-NAME, for resolving ENUM-VALUE-REFs.
    enum_values: HashMap<String, String>,
    /// SPEC-RELATION-TYPE IDENTIFIER → LONG-NAME.
    relation_types: HashMap<String, String>,
    /// Object identifiers in SPEC-HIERARCHY order (first SPECIFICATION),
    /// used to order the imported items like the source document.
    hierarchy_order: Vec<String>,
    /// Accumulator for the current hierarchy object reference text.
    scratch: String,
}

/// Where simple text content of the current element should go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextTarget {
    ObjectTypeRef,
    RelationSource,
    RelationTarget,
    RelationTypeRef,
    HierarchyObjectRef,
    AttrDefRef,
    PlainValue,
    EnumValueRef,
}

struct ParseState {
    stack: Vec<String>,
    doc: RawDocument,
    current_type: Option<RawObjectType>,
    current_object: Option<RawObject>,
    current_relation: Option<RawRelation>,
    current_value: Option<RawValue>,
    text_target: Option<TextTarget>,
    xhtml_depth: Option<usize>,
    xhtml_buf: String,
}

impl ParseState {
    fn new() -> Self {
        Self {
            stack: Vec::new(),
            doc: RawDocument::default(),
            current_type: None,
            current_object: None,
            current_relation: None,
            current_value: None,
            text_target: None,
            xhtml_depth: None,
            xhtml_buf: String::new(),
        }
    }

    fn parent(&self) -> Option<&str> {
        self.stack.last().map(String::as_str)
    }

    fn handle_start(&mut self, e: &BytesStart) {
        let name = local_name(e.name().as_ref()).to_string();
        let parent = self.parent().unwrap_or("").to_string();

        match name.as_str() {
            "SPEC-OBJECT-TYPE" => {
                self.current_type = Some(RawObjectType {
                    id: get_attr(e, "IDENTIFIER").unwrap_or_default(),
                    long_name: get_attr(e, "LONG-NAME").unwrap_or_default(),
                    attr_defs: Vec::new(),
                });
            }
            "SPEC-OBJECT" => {
                self.current_object = Some(RawObject {
                    id: get_attr(e, "IDENTIFIER").unwrap_or_default(),
                    long_name: get_attr(e, "LONG-NAME").unwrap_or_default(),
                    type_ref: String::new(),
                    values: Vec::new(),
                });
            }
            "SPEC-RELATION" => {
                self.current_relation = Some(RawRelation::default());
            }
            "SPEC-RELATION-TYPE" => {
                let id = get_attr(e, "IDENTIFIER").unwrap_or_default();
                let long_name = get_attr(e, "LONG-NAME").unwrap_or_default();
                if !id.is_empty() {
                    self.doc.relation_types.insert(id, long_name);
                }
            }
            "THE-VALUE-XHTML" => {
                self.xhtml_depth = Some(1);
                self.xhtml_buf.clear();
            }
            // ReqIF 1.0.1 style: THE-VALUE inside ATTRIBUTE-VALUE-XHTML carries
            // the XHTML payload (1.1 renamed the container to THE-VALUE-XHTML).
            "THE-VALUE"
                if self
                    .current_value
                    .as_ref()
                    .map_or(false, |v| v.is_xhtml) =>
            {
                self.xhtml_depth = Some(1);
                self.xhtml_buf.clear();
            }
            "SPEC-OBJECT-TYPE-REF" => {
                if self.current_object.is_some() {
                    self.text_target = Some(TextTarget::ObjectTypeRef);
                }
            }
            "SPEC-OBJECT-REF" => {
                if parent == "OBJECT" {
                    // SPEC-HIERARCHY entry: remember document order.
                    self.text_target = Some(TextTarget::HierarchyObjectRef);
                    self.doc.scratch.clear();
                } else if self.current_relation.is_some() {
                    self.text_target = match parent.as_str() {
                        "SOURCE" => Some(TextTarget::RelationSource),
                        "TARGET" => Some(TextTarget::RelationTarget),
                        _ => None,
                    };
                }
            }
            "SPEC-RELATION-TYPE-REF" => {
                if self.current_relation.is_some() {
                    self.text_target = Some(TextTarget::RelationTypeRef);
                }
            }
            _ => {
                if parent == "SPEC-ATTRIBUTES" && name.starts_with("ATTRIBUTE-DEFINITION-") {
                    if let Some(t) = self.current_type.as_mut() {
                        t.attr_defs.push(RawAttrDef {
                            id: get_attr(e, "IDENTIFIER").unwrap_or_default(),
                            long_name: get_attr(e, "LONG-NAME").unwrap_or_default(),
                        });
                    }
                } else if parent == "VALUES" && name.starts_with("ATTRIBUTE-VALUE-") {
                    self.current_value = Some(RawValue {
                        attr_def_ref: None,
                        text: get_attr(e, "THE-VALUE").unwrap_or_default(),
                        is_xhtml: name == "ATTRIBUTE-VALUE-XHTML",
                    });
                } else if parent == "DEFINITION"
                    && name.starts_with("ATTRIBUTE-DEFINITION-")
                    && name.ends_with("-REF")
                    && self.current_value.is_some()
                {
                    self.text_target = Some(TextTarget::AttrDefRef);
                } else if name == "THE-VALUE"
                    && parent.starts_with("ATTRIBUTE-VALUE-")
                    && self.current_value.is_some()
                {
                    self.text_target = Some(TextTarget::PlainValue);
                } else if name == "ENUM-VALUE-REF" && self.current_value.is_some() {
                    self.text_target = Some(TextTarget::EnumValueRef);
                } else if name == "ENUM-VALUE" {
                    let id = get_attr(e, "IDENTIFIER").unwrap_or_default();
                    let long_name = get_attr(e, "LONG-NAME").unwrap_or_default();
                    if !id.is_empty() {
                        self.doc.enum_values.insert(id, long_name);
                    }
                }
            }
        }

        if let Some(depth) = self.xhtml_depth.as_mut() {
            // Nested XHTML content: block-level tags mark line breaks.
            if name != "THE-VALUE-XHTML" && name != "THE-VALUE" {
                *depth += 1;
                if BLOCK_TAGS.contains(&name.to_lowercase().as_str()) {
                    self.xhtml_buf.push(LINE_BREAK);
                }
            }
        }

        self.stack.push(name);
    }

    fn handle_empty(&mut self, e: &BytesStart) {
        let name = local_name(e.name().as_ref()).to_string();
        let parent = self.parent().unwrap_or("").to_string();

        if parent == "SPEC-ATTRIBUTES" && name.starts_with("ATTRIBUTE-DEFINITION-") {
            if let Some(t) = self.current_type.as_mut() {
                t.attr_defs.push(RawAttrDef {
                    id: get_attr(e, "IDENTIFIER").unwrap_or_default(),
                    long_name: get_attr(e, "LONG-NAME").unwrap_or_default(),
                });
            }
        } else if name == "SPEC-OBJECT" {
            // Empty-element objects (no VALUES/TYPE children) still import.
            self.doc.objects.push(RawObject {
                id: get_attr(e, "IDENTIFIER").unwrap_or_default(),
                long_name: get_attr(e, "LONG-NAME").unwrap_or_default(),
                type_ref: String::new(),
                values: Vec::new(),
            });
        } else if name == "SPEC-OBJECT-TYPE" {
            self.doc.object_types.push(RawObjectType {
                id: get_attr(e, "IDENTIFIER").unwrap_or_default(),
                long_name: get_attr(e, "LONG-NAME").unwrap_or_default(),
                attr_defs: Vec::new(),
            });
        } else if name == "SPEC-RELATION-TYPE" {
            let id = get_attr(e, "IDENTIFIER").unwrap_or_default();
            let long_name = get_attr(e, "LONG-NAME").unwrap_or_default();
            if !id.is_empty() {
                self.doc.relation_types.insert(id, long_name);
            }
        } else if parent == "VALUES" && name.starts_with("ATTRIBUTE-VALUE-") {
            let value = RawValue {
                attr_def_ref: None,
                text: get_attr(e, "THE-VALUE").unwrap_or_default(),
                is_xhtml: name == "ATTRIBUTE-VALUE-XHTML",
            };
            if let Some(obj) = self.current_object.as_mut() {
                obj.values.push(value);
            }
        } else if name == "ENUM-VALUE" {
            let id = get_attr(e, "IDENTIFIER").unwrap_or_default();
            let long_name = get_attr(e, "LONG-NAME").unwrap_or_default();
            if !id.is_empty() {
                self.doc.enum_values.insert(id, long_name);
            }
        } else if self.xhtml_depth.is_some()
            && BLOCK_TAGS.contains(&name.to_lowercase().as_str())
        {
            self.xhtml_buf.push(LINE_BREAK);
        }
    }

    fn handle_text(&mut self, text: &str) {
        if self.xhtml_depth.is_some() {
            self.xhtml_buf.push_str(text);
            return;
        }
        match self.text_target {
            Some(TextTarget::ObjectTypeRef) => {
                if let Some(obj) = self.current_object.as_mut() {
                    obj.type_ref.push_str(text);
                }
            }
            Some(TextTarget::RelationSource) => {
                if let Some(rel) = self.current_relation.as_mut() {
                    rel.source.push_str(text);
                }
            }
            Some(TextTarget::RelationTarget) => {
                if let Some(rel) = self.current_relation.as_mut() {
                    rel.target.push_str(text);
                }
            }
            Some(TextTarget::RelationTypeRef) => {
                if let Some(rel) = self.current_relation.as_mut() {
                    rel.type_ref.push_str(text);
                }
            }
            Some(TextTarget::HierarchyObjectRef) => {
                self.doc.scratch.push_str(text);
            }
            Some(TextTarget::AttrDefRef) => {
                if let Some(v) = self.current_value.as_mut() {
                    v.attr_def_ref.get_or_insert_with(String::new).push_str(text);
                }
            }
            Some(TextTarget::PlainValue) => {
                if let Some(v) = self.current_value.as_mut() {
                    v.text.push_str(text);
                }
            }
            Some(TextTarget::EnumValueRef) => {
                // Resolve the enumeration value reference to its LONG-NAME;
                // keep the raw ref when the datatype definition is unknown.
                let resolved = self
                    .doc
                    .enum_values
                    .get(text.trim())
                    .cloned()
                    .unwrap_or_else(|| text.trim().to_string());
                if resolved.is_empty() {
                    return;
                }
                if let Some(v) = self.current_value.as_mut() {
                    if !v.text.is_empty() {
                        v.text.push_str(", ");
                    }
                    v.text.push_str(&resolved);
                }
            }
            None => {}
        }
    }

    fn handle_end(&mut self, raw_name: &[u8]) {
        let name = local_name(raw_name).to_string();

        // Pop first so parent() refers to the enclosing element.
        if self.stack.last().map(String::as_str) == Some(name.as_str()) {
            self.stack.pop();
        }

        if let Some(depth) = self.xhtml_depth.as_mut() {
            if *depth == 1 && (name == "THE-VALUE-XHTML" || name == "THE-VALUE") {
                let text = xhtml_to_text(&self.xhtml_buf);
                if let Some(v) = self.current_value.as_mut() {
                    v.text = text;
                }
                self.xhtml_depth = None;
                self.xhtml_buf.clear();
            } else {
                *depth -= 1;
                if BLOCK_TAGS.contains(&name.to_lowercase().as_str()) {
                    self.xhtml_buf.push(LINE_BREAK);
                }
            }
        }

        match name.as_str() {
            "SPEC-OBJECT-TYPE" => {
                if let Some(t) = self.current_type.take() {
                    self.doc.object_types.push(t);
                }
            }
            "SPEC-OBJECT" => {
                if let Some(mut obj) = self.current_object.take() {
                    if let Some(v) = self.current_value.take() {
                        obj.values.push(v);
                    }
                    obj.type_ref = obj.type_ref.trim().to_string();
                    self.doc.objects.push(obj);
                }
            }
            "SPEC-RELATION" => {
                if let Some(rel) = self.current_relation.take() {
                    self.doc.relations.push(rel);
                }
            }
            _ => {
                if name.starts_with("ATTRIBUTE-VALUE-") && self.parent() == Some("VALUES") {
                    if let (Some(obj), Some(v)) =
                        (self.current_object.as_mut(), self.current_value.take())
                    {
                        obj.values.push(v);
                    }
                }
            }
        }

        // Flush a completed hierarchy object reference.
        if self.text_target == Some(TextTarget::HierarchyObjectRef) {
            let id = self.doc.scratch.trim().to_string();
            if !id.is_empty() {
                self.doc.hierarchy_order.push(id);
            }
            self.doc.scratch.clear();
        }

        // Text capture ends with the element that started it.
        if self.text_target.is_some() {
            self.text_target = None;
        }
    }
}

/// Import a ReqIF 1.x XML document into specification items.
///
/// `type_map` maps SPEC-OBJECT-TYPE `LONG-NAME`s to artifact types
/// (e.g. "Customer Requirement" to "creq"); types not in the map get their
/// `LONG-NAME` slugified (lowercase, non-alphanumeric runs become dashes).
///
/// `relation_map` maps SPEC-RELATION-TYPE `LONG-NAME`s to a direction:
/// "covers" (the SOURCE item covers the TARGET item — the default for
/// unmapped types), "covered-by" (inverted), or "ignore" (dropped).
///
/// The root element must be `REQ-IF` (OMG ReqIF 1.0.1/1.1/1.2) or `RIF`
/// (pre-OMG RIF 1.1a/1.2); any other XML document is rejected. Objects whose
/// `IDENTIFIER` is itself a canonical item id (as written by
/// [`export_reqif`]) keep that id verbatim, which makes export followed by
/// import a lossless round-trip for ids. Unknown elements are skipped.
/// [impl->req~reqif-import~2]
pub fn import_reqif(
    xml: &str,
    type_map: &[(String, String)],
    relation_map: &[(String, String)],
) -> Result<Vec<SpecificationItem>> {
    let mut reader = Reader::from_str(xml);
    let mut state = ParseState::new();
    let mut root_checked = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                check_root(&e, &mut root_checked)?;
                state.handle_start(&e);
            }
            Ok(Event::End(e)) => state.handle_end(e.name().as_ref()),
            Ok(Event::Empty(e)) => {
                check_root(&e, &mut root_checked)?;
                state.handle_empty(&e);
            }
            Ok(Event::Text(t)) => {
                let text = t
                    .unescape()
                    .map_err(|e| Error::ReqIf(format!("XML entity decode error: {}", e)))?;
                state.handle_text(&text);
            }
            Ok(Event::CData(t)) => {
                state.handle_text(&String::from_utf8_lossy(t.as_ref()));
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(Error::ReqIf(format!(
                    "XML parse error at byte {}: {}",
                    reader.buffer_position(),
                    e
                )))
            }
            _ => {}
        }
    }

    if !root_checked {
        return Err(Error::ReqIf("Not a ReqIF/RIF document: empty input".to_string()));
    }

    Ok(build_items(&state.doc, type_map, relation_map))
}

/// The first element of the document must be a ReqIF or RIF root.
fn check_root(e: &BytesStart, root_checked: &mut bool) -> Result<()> {
    if *root_checked {
        return Ok(());
    }
    *root_checked = true;
    let name = local_name(e.name().as_ref()).to_string();
    if name != "REQ-IF" && name != "RIF" {
        return Err(Error::ReqIf(format!(
            "Not a ReqIF/RIF document: root element is <{}>",
            name
        )));
    }
    Ok(())
}

/// Turn the parsed raw document into specification items.
fn build_items(
    doc: &RawDocument,
    type_map: &[(String, String)],
    relation_map: &[(String, String)],
) -> Vec<SpecificationItem> {
    // Global attribute definition id to LONG-NAME lookup (all spec object types).
    let attr_names: HashMap<&str, &str> = doc
        .object_types
        .iter()
        .flat_map(|t| t.attr_defs.iter().map(|d| (d.id.as_str(), d.long_name.as_str())))
        .collect();

    let type_by_id: HashMap<&str, &RawObjectType> = doc
        .object_types
        .iter()
        .map(|t| (t.id.as_str(), t))
        .collect();

    let mut items = Vec::with_capacity(doc.objects.len());
    for obj in &doc.objects {
        let type_long_name = type_by_id
            .get(obj.type_ref.as_str())
            .map(|t| t.long_name.clone())
            .unwrap_or_default();

        // Objects carrying a canonical id in the OVFT.ID attribute (our own
        // exports, whose IDENTIFIER must be an NCName) round-trip verbatim;
        // older exports carry the canonical id as IDENTIFIER directly;
        // everything else derives the id from the type and the slugified
        // ReqIF IDENTIFIER — stable when the source tool renames a heading,
        // unlike a LONG-NAME-derived id.
        let ovft_id = obj.values.iter().find_map(|v| {
            let name = v
                .attr_def_ref
                .as_deref()
                .and_then(|r| attr_names.get(r));
            if name == Some(&"OVFT.ID") {
                SpecificationItemId::parse(v.text.trim()).ok()
            } else {
                None
            }
        });
        let parsed_id = ovft_id.or_else(|| SpecificationItemId::parse(&obj.id).ok());
        let id = match parsed_id {
            Some(id) => id,
            None => {
                let artifact_type = type_map
                    .iter()
                    .find(|(from, _)| from.eq_ignore_ascii_case(type_long_name.trim()))
                    .map(|(_, to)| to.clone())
                    .unwrap_or_else(|| {
                        if type_long_name.is_empty() {
                            slugify(&obj.id)
                        } else {
                            slugify(&type_long_name)
                        }
                    });
                let name_source = if obj.id.trim().is_empty() {
                    obj.long_name.trim()
                } else {
                    obj.id.trim()
                };
                SpecificationItemId::new(artifact_type, slugify(name_source), 1)
            }
        };

        let picked = pick_text_value(&obj.values, &attr_names);
        let description = picked.map(|v| {
            // Text is already entity-decoded by quick_xml; just trim.
            v.text.trim().to_string()
        });

        // All other named values survive as attributes (Status, Priority …).
        let picked_ptr = picked.map(|v| v as *const RawValue);
        let mut attributes: Vec<(String, String)> = Vec::new();
        for v in &obj.values {
            let name = v
                .attr_def_ref
                .as_deref()
                .and_then(|r| attr_names.get(r.trim()))
                .map(|n| n.trim().to_string())
                .unwrap_or_default();
            let text = v.text.trim();
            if name.is_empty() || name == "OVFT.ID" || text.is_empty() {
                continue;
            }
            if picked_ptr == Some(v as *const RawValue) {
                continue;
            }
            if !attributes.iter().any(|(n, _)| *n == name) {
                attributes.push((name, text.to_string()));
            }
        }

        let mut builder = SpecificationItem::builder(id)
            .aliases(vec![obj.id.clone()])
            .attributes(attributes);
        if !obj.long_name.trim().is_empty() {
            builder = builder.title(obj.long_name.trim().to_string());
        }
        if let Some(desc) = description.filter(|d| !d.is_empty()) {
            builder = builder.description(desc);
        }
        items.push(builder.build());
    }

    // Resolve relation endpoints (ReqIF identifiers) to canonical ids.
    let id_by_identifier: HashMap<String, SpecificationItemId> = doc
        .objects
        .iter()
        .zip(items.iter())
        .map(|(obj, item)| (obj.id.clone(), item.id.clone()))
        .collect();

    for rel in &doc.relations {
        // Relation direction from the relation type's LONG-NAME: the map may
        // declare "covers" (default), "covered-by" (inverted) or "ignore".
        let type_name = doc
            .relation_types
            .get(rel.type_ref.trim())
            .map(String::as_str)
            .unwrap_or("");
        let direction = relation_map
            .iter()
            .find(|(from, _)| from.eq_ignore_ascii_case(type_name.trim()))
            .map(|(_, to)| to.trim().to_lowercase())
            .unwrap_or_else(|| "covers".to_string());
        if direction == "ignore" {
            continue;
        }
        let (Some(mut covering), Some(mut covered)) = (
            id_by_identifier.get(rel.source.trim()),
            id_by_identifier.get(rel.target.trim()),
        ) else {
            continue;
        };
        if direction == "covered-by" {
            std::mem::swap(&mut covering, &mut covered);
        }
        let (covering_id, covered_id) = (covering.clone(), covered.clone());
        if let Some(source) = items.iter_mut().find(|i| i.id == covering_id) {
            if !source.covers.contains(&covered_id) {
                source.covers.push(covered_id);
            }
        }
    }

    // Order items like the source document's first SPECIFICATION hierarchy;
    // objects not referenced there keep their document order at the end.
    if !doc.hierarchy_order.is_empty() {
        let order: HashMap<&str, usize> = doc
            .hierarchy_order
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let mut indexed: Vec<(usize, SpecificationItem)> = items.into_iter().enumerate().collect();
        indexed.sort_by_key(|(original, item)| {
            let identifier = &item.aliases.first().cloned().unwrap_or_default();
            (
                order.get(identifier.as_str()).copied().unwrap_or(usize::MAX),
                *original,
            )
        });
        items = indexed.into_iter().map(|(_, item)| item).collect();
    }

    items
}

/// Pick the requirement text value: prefer well-known attribute names
/// ("ReqIF.Text", "Description", "Text"), then the first XHTML value,
/// then the first plain value.
fn pick_text_value<'a>(
    values: &'a [RawValue],
    attr_names: &HashMap<&str, &str>,
) -> Option<&'a RawValue> {
    for wanted in TEXT_ATTRIBUTE_NAMES {
        if let Some(v) = values.iter().find(|v| {
            v.attr_def_ref
                .as_deref()
                .and_then(|r| attr_names.get(r.trim()))
                .map_or(false, |n| n.trim().eq_ignore_ascii_case(wanted))
        }) {
            return Some(v);
        }
    }
    values
        .iter()
        .filter(|v| {
            // Never promote the round-trip id attribute to the description.
            v.attr_def_ref
                .as_deref()
                .and_then(|r| attr_names.get(r.trim()))
                .map_or(true, |n| n.trim() != "OVFT.ID")
        })
        .find(|v| v.is_xhtml && !v.text.trim().is_empty())
        .or_else(|| {
            values
                .iter()
                .filter(|v| {
                    v.attr_def_ref
                        .as_deref()
                        .and_then(|r| attr_names.get(r.trim()))
                        .map_or(true, |n| n.trim() != "OVFT.ID")
                })
                .find(|v| !v.text.trim().is_empty())
        })
}

/// Escape text for XML output (both element content and attribute values).
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Seconds since the Unix epoch; on wasm32 the browser clock is used instead
/// of the (unavailable) system clock.
fn unix_epoch_secs() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

/// Current time as an ISO 8601 UTC timestamp (xs:dateTime compatible).
fn iso8601_now() -> String {
    epoch_to_iso8601(unix_epoch_secs())
}

fn epoch_to_iso8601(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let secs_of_day = secs % 86400;
    let hour = secs_of_day / 3600;
    let min = (secs_of_day % 3600) / 60;
    let sec = secs_of_day % 60;
    // civil calendar date from days since epoch (Howard Hinnant's algorithm)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    if m <= 2 {
        y += 1;
    }
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hour, min, sec
    )
}

/// The ReqIF identifier used for an item on export: the first alias that is
/// not itself a canonical item id (e.g. a tool UUID), else the canonical id
/// with `~` replaced by `_` (ReqIF identifiers must be NCNames).
fn export_identifier(item: &SpecificationItem) -> String {
    item.aliases
        .iter()
        .find(|a| SpecificationItemId::parse(a).is_err())
        .cloned()
        .unwrap_or_else(|| item.id.to_string().replace('~', "_"))
}

/// Identifier of the attribute definition for a named item attribute within
/// an artifact type's SPEC-OBJECT-TYPE.
fn attr_def_id(name: &str, artifact_type: &str) -> String {
    format!("ad-attr-{}-{}", slugify(name), artifact_type)
}

/// Export specification items as a ReqIF 1.1 XML document.
///
/// Each artifact type becomes one SPEC-OBJECT-TYPE whose `LONG-NAME` is the
/// artifact type verbatim, with an XHTML attribute definition named
/// "ReqIF.Text" plus one string attribute definition per named item
/// attribute. Covers links become SPEC-RELATIONs (SOURCE covers TARGET), and
/// all objects are listed in a single SPECIFICATION with a flat
/// SPEC-HIERARCHY so importing tools show them as a module in order.
/// Re-importing the output with [`import_reqif`] yields the same canonical
/// ids, covers links and attributes.
/// [impl->req~reqif-export~2]
pub fn export_reqif(items: &[SpecificationItem]) -> Result<String> {
    let now = iso8601_now();
    let export_id = format!("ovft-export-{}", unix_epoch_secs());

    // Artifact types in order of first appearance.
    let mut artifact_types: Vec<&str> = Vec::new();
    for item in items {
        if !artifact_types.contains(&item.id.artifact_type.as_str()) {
            artifact_types.push(&item.id.artifact_type);
        }
    }

    let identifier_by_id: HashMap<String, String> = items
        .iter()
        .map(|i| (i.id.to_string(), export_identifier(i)))
        .collect();

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<REQ-IF xmlns=\"http://www.omg.org/spec/ReqIF/20110401/reqif.xsd\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n");
    xml.push_str("  <THE-HEADER>\n");
    xml.push_str(&format!(
        "    <REQ-IF-HEADER IDENTIFIER=\"{}\">\n",
        escape_xml(&export_id)
    ));
    xml.push_str(&format!("      <CREATION-TIME>{}</CREATION-TIME>\n", now));
    xml.push_str("      <REQ-IF-TOOL-ID>open-very-fast-trace</REQ-IF-TOOL-ID>\n");
    xml.push_str("      <REQ-IF-VERSION>1.1</REQ-IF-VERSION>\n");
    xml.push_str("      <SOURCE-TOOL-ID>open-very-fast-trace</SOURCE-TOOL-ID>\n");
    xml.push_str("      <TITLE>Open Very Fast Trace export</TITLE>\n");
    xml.push_str("    </REQ-IF-HEADER>\n");
    xml.push_str("  </THE-HEADER>\n");
    xml.push_str("  <CORE-CONTENT>\n");
    xml.push_str("    <REQ-IF-CONTENT>\n");

    xml.push_str("      <DATATYPES>\n");
    xml.push_str(&format!(
        "        <DATATYPE-DEFINITION-STRING IDENTIFIER=\"dt-string\" LONG-NAME=\"String\" MAX-LENGTH=\"32000\" LAST-CHANGE=\"{}\"/>\n",
        now
    ));
    xml.push_str(&format!(
        "        <DATATYPE-DEFINITION-XHTML IDENTIFIER=\"dt-xhtml\" LONG-NAME=\"XHTML\" LAST-CHANGE=\"{}\"/>\n",
        now
    ));
    xml.push_str("      </DATATYPES>\n");

    xml.push_str("      <SPEC-TYPES>\n");
    for artifact_type in &artifact_types {
        xml.push_str(&format!(
            "        <SPEC-OBJECT-TYPE IDENTIFIER=\"sot-{}\" LONG-NAME=\"{}\" LAST-CHANGE=\"{}\">\n",
            escape_xml(artifact_type),
            escape_xml(artifact_type),
            now
        ));
        xml.push_str("          <SPEC-ATTRIBUTES>\n");
        xml.push_str(&format!(
            "            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER=\"ad-ovftid-{}\" LONG-NAME=\"OVFT.ID\" LAST-CHANGE=\"{}\">\n",
            escape_xml(artifact_type),
            now
        ));
        xml.push_str("              <TYPE>\n");
        xml.push_str("                <DATATYPE-DEFINITION-STRING-REF>dt-string</DATATYPE-DEFINITION-STRING-REF>\n");
        xml.push_str("              </TYPE>\n");
        xml.push_str("            </ATTRIBUTE-DEFINITION-STRING>\n");
        xml.push_str(&format!(
            "            <ATTRIBUTE-DEFINITION-XHTML IDENTIFIER=\"ad-text-{}\" LONG-NAME=\"ReqIF.Text\" LAST-CHANGE=\"{}\">\n",
            escape_xml(artifact_type),
            now
        ));
        xml.push_str("              <TYPE>\n");
        xml.push_str("                <DATATYPE-DEFINITION-XHTML-REF>dt-xhtml</DATATYPE-DEFINITION-XHTML-REF>\n");
        xml.push_str("              </TYPE>\n");
        xml.push_str("            </ATTRIBUTE-DEFINITION-XHTML>\n");
        // One string attribute definition per named item attribute.
        let mut attr_names: Vec<&str> = Vec::new();
        for item in items
            .iter()
            .filter(|i| i.id.artifact_type == *artifact_type)
        {
            for (name, _) in &item.attributes {
                if !attr_names.contains(&name.as_str()) {
                    attr_names.push(name);
                }
            }
        }
        for name in attr_names {
            xml.push_str(&format!(
                "            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER=\"{}\" LONG-NAME=\"{}\" LAST-CHANGE=\"{}\">\n",
                escape_xml(&attr_def_id(name, artifact_type)),
                escape_xml(name),
                now
            ));
            xml.push_str("              <TYPE>\n");
            xml.push_str("                <DATATYPE-DEFINITION-STRING-REF>dt-string</DATATYPE-DEFINITION-STRING-REF>\n");
            xml.push_str("              </TYPE>\n");
            xml.push_str("            </ATTRIBUTE-DEFINITION-STRING>\n");
        }
        xml.push_str("          </SPEC-ATTRIBUTES>\n");
        xml.push_str("        </SPEC-OBJECT-TYPE>\n");
    }
    xml.push_str(&format!(
        "        <SPEC-RELATION-TYPE IDENTIFIER=\"srt-covers\" LONG-NAME=\"covers\" LAST-CHANGE=\"{}\"/>\n",
        now
    ));
    xml.push_str(&format!(
        "        <SPECIFICATION-TYPE IDENTIFIER=\"spt-1\" LONG-NAME=\"OVFT Specification\" LAST-CHANGE=\"{}\"/>\n",
        now
    ));
    xml.push_str("      </SPEC-TYPES>\n");

    xml.push_str("      <SPEC-OBJECTS>\n");
    for item in items {
        let identifier = export_identifier(item);
        xml.push_str(&format!(
            "        <SPEC-OBJECT IDENTIFIER=\"{}\" LONG-NAME=\"{}\" LAST-CHANGE=\"{}\">\n",
            escape_xml(&identifier),
            escape_xml(&item.title_or_fallback()),
            now
        ));
        xml.push_str("          <VALUES>\n");
        xml.push_str(&format!(
            "            <ATTRIBUTE-VALUE-STRING THE-VALUE=\"{}\">\n",
            escape_xml(&item.id.to_string())
        ));
        xml.push_str("              <DEFINITION>\n");
        xml.push_str(&format!(
            "                <ATTRIBUTE-DEFINITION-STRING-REF>ad-ovftid-{}</ATTRIBUTE-DEFINITION-STRING-REF>\n",
            escape_xml(&item.id.artifact_type)
        ));
        xml.push_str("              </DEFINITION>\n");
        xml.push_str("            </ATTRIBUTE-VALUE-STRING>\n");
        if let Some(description) = &item.description {
            xml.push_str("            <ATTRIBUTE-VALUE-XHTML>\n");
            xml.push_str("              <DEFINITION>\n");
            xml.push_str(&format!(
                "                <ATTRIBUTE-DEFINITION-XHTML-REF>ad-text-{}</ATTRIBUTE-DEFINITION-XHTML-REF>\n",
                escape_xml(&item.id.artifact_type)
            ));
            xml.push_str("              </DEFINITION>\n");
            // ReqIF 1.1 container; line breaks become <xhtml:br/> so
            // paragraph structure survives the round-trip.
            let xhtml = description
                .split('\n')
                .map(escape_xml)
                .collect::<Vec<_>>()
                .join("<xhtml:br/>");
            xml.push_str(&format!(
                "              <THE-VALUE-XHTML><xhtml:div>{}</xhtml:div></THE-VALUE-XHTML>\n",
                xhtml
            ));
            xml.push_str("            </ATTRIBUTE-VALUE-XHTML>\n");
        }
        for (name, value) in &item.attributes {
            xml.push_str(&format!(
                "            <ATTRIBUTE-VALUE-STRING THE-VALUE=\"{}\">\n",
                escape_xml(value)
            ));
            xml.push_str("              <DEFINITION>\n");
            xml.push_str(&format!(
                "                <ATTRIBUTE-DEFINITION-STRING-REF>{}</ATTRIBUTE-DEFINITION-STRING-REF>\n",
                escape_xml(&attr_def_id(name, &item.id.artifact_type))
            ));
            xml.push_str("              </DEFINITION>\n");
            xml.push_str("            </ATTRIBUTE-VALUE-STRING>\n");
        }
        xml.push_str("          </VALUES>\n");
        xml.push_str("          <TYPE>\n");
        xml.push_str(&format!(
            "            <SPEC-OBJECT-TYPE-REF>sot-{}</SPEC-OBJECT-TYPE-REF>\n",
            escape_xml(&item.id.artifact_type)
        ));
        xml.push_str("          </TYPE>\n");
        xml.push_str("        </SPEC-OBJECT>\n");
    }
    xml.push_str("      </SPEC-OBJECTS>\n");

    xml.push_str("      <SPEC-RELATIONS>\n");
    let mut rel_count = 0usize;
    for item in items {
        for covered in &item.covers {
            let Some(target_identifier) = identifier_by_id.get(&covered.to_string()) else {
                continue;
            };
            rel_count += 1;
            xml.push_str(&format!(
                "        <SPEC-RELATION IDENTIFIER=\"rel-{}\" LAST-CHANGE=\"{}\">\n",
                rel_count, now
            ));
            xml.push_str("          <SOURCE>\n");
            xml.push_str(&format!(
                "            <SPEC-OBJECT-REF>{}</SPEC-OBJECT-REF>\n",
                escape_xml(&export_identifier(item))
            ));
            xml.push_str("          </SOURCE>\n");
            xml.push_str("          <TARGET>\n");
            xml.push_str(&format!(
                "            <SPEC-OBJECT-REF>{}</SPEC-OBJECT-REF>\n",
                escape_xml(target_identifier)
            ));
            xml.push_str("          </TARGET>\n");
            xml.push_str("          <TYPE>\n");
            xml.push_str("            <SPEC-RELATION-TYPE-REF>srt-covers</SPEC-RELATION-TYPE-REF>\n");
            xml.push_str("          </TYPE>\n");
            xml.push_str("        </SPEC-RELATION>\n");
        }
    }
    xml.push_str("      </SPEC-RELATIONS>\n");

    // One SPECIFICATION with a flat hierarchy of all objects, so importing
    // tools (DOORS, Polarion) present the export as a module in item order.
    xml.push_str("      <SPEC-RELATION-GROUPS/>\n");
    xml.push_str("      <SPECIFICATIONS>\n");
    xml.push_str(&format!(
        "        <SPECIFICATION IDENTIFIER=\"spec-1\" LONG-NAME=\"OVFT Trace\" LAST-CHANGE=\"{}\">\n",
        now
    ));
    xml.push_str("          <CHILDREN>\n");
    for (index, item) in items.iter().enumerate() {
        xml.push_str(&format!(
            "            <SPEC-HIERARCHY IDENTIFIER=\"h-{}\" LAST-CHANGE=\"{}\">\n",
            index + 1,
            now
        ));
        xml.push_str("              <OBJECT>\n");
        xml.push_str(&format!(
            "                <SPEC-OBJECT-REF>{}</SPEC-OBJECT-REF>\n",
            escape_xml(&export_identifier(item))
        ));
        xml.push_str("              </OBJECT>\n");
        xml.push_str("            </SPEC-HIERARCHY>\n");
    }
    xml.push_str("          </CHILDREN>\n");
    xml.push_str("          <TYPE>\n");
    xml.push_str("            <SPECIFICATION-TYPE-REF>spt-1</SPECIFICATION-TYPE-REF>\n");
    xml.push_str("          </TYPE>\n");
    xml.push_str("        </SPECIFICATION>\n");
    xml.push_str("      </SPECIFICATIONS>\n");

    xml.push_str("    </REQ-IF-CONTENT>\n");
    xml.push_str("  </CORE-CONTENT>\n");
    xml.push_str("  <TOOL-EXTENSIONS/>\n");
    xml.push_str("</REQ-IF>\n");

    Ok(xml)
}

/// Render imported items as an OVFT markdown specification document.
///
/// Each item becomes a `## <id>` section with its title, description, aliases
/// and covers links, separated by horizontal rules. The output carries a
/// generated-file header naming the ReqIF source file.
pub fn render_imported_markdown(source_file: &str, items: &[SpecificationItem]) -> String {
    let mut out = format!(
        "<!-- generated from {} by cargo ovft reqif import — do not edit -->\n\n",
        source_file
    );
    for (index, item) in items.iter().enumerate() {
        out.push_str(&format!("## {}\n\n", item.id));
        if let Some(title) = &item.title {
            out.push_str(&format!("**Title:** {}\n\n", title));
        }
        if let Some(description) = &item.description {
            out.push_str(description);
            out.push_str("\n\n");
        }
        for (name, value) in &item.attributes {
            out.push_str(&format!("**{}:** {}\n\n", name, value));
        }
        if !item.aliases.is_empty() {
            out.push_str(&format!("**Aliases:** {}\n\n", item.aliases.join(", ")));
        }
        if !item.covers.is_empty() {
            let covers = item
                .covers
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("**Covers:** {}\n\n", covers));
        }
        if index + 1 < items.len() {
            out.push_str("---\n\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small DOORS-flavoured ReqIF document: two object types, three
    /// objects (one with XHTML text, one with a plain string value) and one
    /// relation.
    const SAMPLE_REQIF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <THE-HEADER>
    <REQ-IF-HEADER IDENTIFIER="_d9f8a0-header">
      <CREATION-TIME>2024-01-15T10:30:00Z</CREATION-TIME>
      <SOURCE-TOOL-ID>DOORS</SOURCE-TOOL-ID>
    </REQ-IF-HEADER>
  </THE-HEADER>
  <CORE-CONTENT>
    <REQ-IF-CONTENT>
      <DATATYPES>
        <DATATYPE-DEFINITION-STRING IDENTIFIER="_dt-string" LONG-NAME="String" MAX-LENGTH="32000"/>
        <DATATYPE-DEFINITION-XHTML IDENTIFIER="_dt-xhtml" LONG-NAME="XHTML"/>
      </DATATYPES>
      <SPEC-TYPES>
        <SPEC-OBJECT-TYPE IDENTIFIER="_sot-customer" LONG-NAME="Customer Requirement">
          <SPEC-ATTRIBUTES>
            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="_ad-status" LONG-NAME="Status">
              <TYPE>
                <DATATYPE-DEFINITION-STRING-REF>_dt-string</DATATYPE-DEFINITION-STRING-REF>
              </TYPE>
            </ATTRIBUTE-DEFINITION-STRING>
            <ATTRIBUTE-DEFINITION-XHTML IDENTIFIER="_ad-text-customer" LONG-NAME="ReqIF.Text">
              <TYPE>
                <DATATYPE-DEFINITION-XHTML-REF>_dt-xhtml</DATATYPE-DEFINITION-XHTML-REF>
              </TYPE>
            </ATTRIBUTE-DEFINITION-XHTML>
          </SPEC-ATTRIBUTES>
        </SPEC-OBJECT-TYPE>
        <SPEC-OBJECT-TYPE IDENTIFIER="_sot-system" LONG-NAME="System Requirement">
          <SPEC-ATTRIBUTES>
            <ATTRIBUTE-DEFINITION-XHTML IDENTIFIER="_ad-text-system" LONG-NAME="ReqIF.Text">
              <TYPE>
                <DATATYPE-DEFINITION-XHTML-REF>_dt-xhtml</DATATYPE-DEFINITION-XHTML-REF>
              </TYPE>
            </ATTRIBUTE-DEFINITION-XHTML>
            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="_ad-desc" LONG-NAME="Description">
              <TYPE>
                <DATATYPE-DEFINITION-STRING-REF>_dt-string</DATATYPE-DEFINITION-STRING-REF>
              </TYPE>
            </ATTRIBUTE-DEFINITION-STRING>
          </SPEC-ATTRIBUTES>
        </SPEC-OBJECT-TYPE>
        <SPEC-RELATION-TYPE IDENTIFIER="_srt" LONG-NAME="satisfies"/>
      </SPEC-TYPES>
      <SPEC-OBJECTS>
        <SPEC-OBJECT IDENTIFIER="_uuid-0001" LONG-NAME="Fast Login">
          <VALUES>
            <ATTRIBUTE-VALUE-STRING THE-VALUE="approved">
              <DEFINITION>
                <ATTRIBUTE-DEFINITION-STRING-REF>_ad-status</ATTRIBUTE-DEFINITION-STRING-REF>
              </DEFINITION>
            </ATTRIBUTE-VALUE-STRING>
            <ATTRIBUTE-VALUE-XHTML>
              <DEFINITION>
                <ATTRIBUTE-DEFINITION-XHTML-REF>_ad-text-customer</ATTRIBUTE-DEFINITION-XHTML-REF>
              </DEFINITION>
              <THE-VALUE-XHTML><div xmlns="http://www.w3.org/1999/xhtml"><p>The user shall log in
                <b>quickly</b> &amp; securely.</p><p>Second paragraph.</p></div></THE-VALUE-XHTML>
            </ATTRIBUTE-VALUE-XHTML>
          </VALUES>
          <TYPE>
            <SPEC-OBJECT-TYPE-REF>_sot-customer</SPEC-OBJECT-TYPE-REF>
          </TYPE>
        </SPEC-OBJECT>
        <SPEC-OBJECT IDENTIFIER="_uuid-0002" LONG-NAME="Auth Service">
          <VALUES>
            <ATTRIBUTE-VALUE-XHTML>
              <DEFINITION>
                <ATTRIBUTE-DEFINITION-XHTML-REF>_ad-text-system</ATTRIBUTE-DEFINITION-XHTML-REF>
              </DEFINITION>
              <THE-VALUE-XHTML><div xmlns="http://www.w3.org/1999/xhtml">Provide token based auth.</div></THE-VALUE-XHTML>
            </ATTRIBUTE-VALUE-XHTML>
          </VALUES>
          <TYPE>
            <SPEC-OBJECT-TYPE-REF>_sot-system</SPEC-OBJECT-TYPE-REF>
          </TYPE>
        </SPEC-OBJECT>
        <SPEC-OBJECT IDENTIFIER="_uuid-0003" LONG-NAME="Plain Described">
          <VALUES>
            <ATTRIBUTE-VALUE-STRING>
              <DEFINITION>
                <ATTRIBUTE-DEFINITION-STRING-REF>_ad-desc</ATTRIBUTE-DEFINITION-STRING-REF>
              </DEFINITION>
              <THE-VALUE>A plain text description.</THE-VALUE>
            </ATTRIBUTE-VALUE-STRING>
          </VALUES>
          <TYPE>
            <SPEC-OBJECT-TYPE-REF>_sot-system</SPEC-OBJECT-TYPE-REF>
          </TYPE>
        </SPEC-OBJECT>
      </SPEC-OBJECTS>
      <SPEC-RELATIONS>
        <SPEC-RELATION IDENTIFIER="_rel-1">
          <SOURCE>
            <SPEC-OBJECT-REF>_uuid-0002</SPEC-OBJECT-REF>
          </SOURCE>
          <TARGET>
            <SPEC-OBJECT-REF>_uuid-0001</SPEC-OBJECT-REF>
          </TARGET>
          <TYPE>
            <SPEC-RELATION-TYPE-REF>_srt</SPEC-RELATION-TYPE-REF>
          </TYPE>
        </SPEC-RELATION>
        <SPEC-RELATION IDENTIFIER="_rel-dangling">
          <SOURCE>
            <SPEC-OBJECT-REF>_uuid-missing</SPEC-OBJECT-REF>
          </SOURCE>
          <TARGET>
            <SPEC-OBJECT-REF>_uuid-0001</SPEC-OBJECT-REF>
          </TARGET>
        </SPEC-RELATION>
      </SPEC-RELATIONS>
    </REQ-IF-CONTENT>
  </CORE-CONTENT>
</REQ-IF>"#;

    #[test]
    fn test_import_reqif_basic() {
        let items = import_reqif(
            SAMPLE_REQIF,
            &[("Customer Requirement".to_string(), "creq".to_string())],
            &[],
        )
        .unwrap();
        assert_eq!(items.len(), 3);

        let customer = &items[0];
        // Canonical id derives from the stable ReqIF IDENTIFIER, not the name.
        assert_eq!(customer.id.to_string(), "creq~uuid-0001~1");
        assert_eq!(customer.title.as_deref(), Some("Fast Login"));
        assert_eq!(customer.aliases, vec!["_uuid-0001".to_string()]);
        let desc = customer.description.as_deref().unwrap();
        assert!(desc.contains("The user shall log in quickly & securely."), "desc: {:?}", desc);
        assert!(desc.contains('\n'), "paragraph break kept: {:?}", desc);
        assert!(desc.contains("Second paragraph."), "desc: {:?}", desc);
        assert!(!desc.contains('<'), "no markup left: {:?}", desc);
        assert!(customer.covers.is_empty());

        let system = &items[1];
        assert_eq!(system.id.to_string(), "system-requirement~uuid-0002~1");
        assert_eq!(system.description.as_deref(), Some("Provide token based auth."));
        // The relation source covers the relation target.
        assert_eq!(
            system.covers,
            vec![SpecificationItemId::parse("creq~uuid-0001~1").unwrap()]
        );

        let plain = &items[2];
        assert_eq!(plain.description.as_deref(), Some("A plain text description."));
        // Dangling relation endpoint was dropped.
        assert!(plain.covers.is_empty());
    }

    #[test]
    fn test_import_reqif_tolerates_unknown_content() {
        let xml = r#"<?xml version="1.0"?>
<reqif:REQ-IF xmlns:reqif="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <reqif:THE-HEADER>
    <reqif:REQ-IF-HEADER IDENTIFIER="_h" SOME-UNKNOWN-ATTR="x">
      <reqif:COMMENT>anything</reqif:COMMENT>
    </reqif:REQ-IF-HEADER>
  </reqif:THE-HEADER>
  <reqif:CORE-CONTENT>
    <reqif:REQ-IF-CONTENT>
      <UNKNOWN-WRAPPER frobnicate="yes">
        <reqif:SPEC-OBJECTS>
          <reqif:SPEC-OBJECT IDENTIFIER="_u1" LONG-NAME="Only Object" EXTRA="1">
            <reqif:TYPE>
              <reqif:SPEC-OBJECT-TYPE-REF>_unknown-type</reqif:SPEC-OBJECT-TYPE-REF>
            </reqif:TYPE>
            <reqif:VALUES>
              <reqif:SOMETHING-ELSE/>
            </reqif:VALUES>
          </reqif:SPEC-OBJECT>
        </reqif:SPEC-OBJECTS>
      </UNKNOWN-WRAPPER>
    </reqif:REQ-IF-CONTENT>
  </reqif:CORE-CONTENT>
</reqif:REQ-IF>"#;
        let items = import_reqif(xml, &[], &[]).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title.as_deref(), Some("Only Object"));
        // Unknown type falls back to a slug; name slugified from IDENTIFIER.
        assert_eq!(items[0].id.name, "u1");
        assert_eq!(items[0].id.revision, 1);
    }

    #[test]
    fn test_import_rif_dialect_root() {
        // Pre-OMG RIF 1.1a/1.2: root element is RIF, XHTML rides in THE-VALUE.
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<RIF xmlns="http://www.omg.org/spec/ReqIF/20100401/rif.xsd">
  <CORE-CONTENT>
    <REQ-IF-CONTENT>
      <SPEC-OBJECTS>
        <SPEC-OBJECT IDENTIFIER="_rif-7" LONG-NAME="Old Format">
          <VALUES>
            <ATTRIBUTE-VALUE-XHTML>
              <THE-VALUE><div xmlns="http://www.w3.org/1999/xhtml"><p>Legacy payload.</p></div></THE-VALUE>
            </ATTRIBUTE-VALUE-XHTML>
          </VALUES>
        </SPEC-OBJECT>
        <SPEC-OBJECT IDENTIFIER="_rif-8" LONG-NAME="Empty Object"/>
      </SPEC-OBJECTS>
    </REQ-IF-CONTENT>
  </CORE-CONTENT>
</RIF>"#;
        let items = import_reqif(xml, &[], &[]).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].id.to_string(), "rif-7~rif-7~1");
        assert_eq!(items[0].description.as_deref(), Some("Legacy payload."));
        // Empty-element objects import too.
        assert_eq!(items[1].id.to_string(), "rif-8~rif-8~1");
        assert_eq!(items[1].title.as_deref(), Some("Empty Object"));
    }

    #[test]
    fn test_import_rejects_non_reqif_documents() {
        let err = import_reqif("<html><body>not reqif</body></html>", &[], &[]).unwrap_err();
        assert!(err.to_string().contains("Not a ReqIF/RIF document"), "err: {}", err);
        assert!(import_reqif("", &[], &[]).is_err());
    }

    #[test]
    fn test_import_does_not_double_decode_entities() {
        // The source encodes a literal "&lt;" (written &amp;lt;). quick_xml
        // decodes once; a second decode would corrupt it into "<".
        let xml = r#"<?xml version="1.0"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <CORE-CONTENT><REQ-IF-CONTENT><SPEC-OBJECTS>
    <SPEC-OBJECT IDENTIFIER="_e1" LONG-NAME="Entities">
      <VALUES>
        <ATTRIBUTE-VALUE-XHTML>
          <THE-VALUE-XHTML><div xmlns="http://www.w3.org/1999/xhtml">Fish &amp;lt;Chips&gt; &amp; ale</div></THE-VALUE-XHTML>
        </ATTRIBUTE-VALUE-XHTML>
      </VALUES>
    </SPEC-OBJECT>
  </SPEC-OBJECTS></REQ-IF-CONTENT></CORE-CONTENT>
</REQ-IF>"#;
        let items = import_reqif(xml, &[], &[]).unwrap();
        assert_eq!(
            items[0].description.as_deref(),
            Some("Fish &lt;Chips> & ale")
        );
    }

    #[test]
    fn test_import_resolves_enumeration_values() {
        let xml = r#"<?xml version="1.0"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <CORE-CONTENT><REQ-IF-CONTENT>
    <DATATYPES>
      <DATATYPE-DEFINITION-ENUMERATION IDENTIFIER="_dt-status" LONG-NAME="Status">
        <SPECIFIED-VALUES>
          <ENUM-VALUE IDENTIFIER="_ev-draft" LONG-NAME="Draft"/>
          <ENUM-VALUE IDENTIFIER="_ev-approved" LONG-NAME="Approved"/>
        </SPECIFIED-VALUES>
      </DATATYPE-DEFINITION-ENUMERATION>
    </DATATYPES>
    <SPEC-TYPES>
      <SPEC-OBJECT-TYPE IDENTIFIER="_sot" LONG-NAME="Requirement">
        <SPEC-ATTRIBUTES>
          <ATTRIBUTE-DEFINITION-ENUMERATION IDENTIFIER="_ad-status" LONG-NAME="Status"/>
        </SPEC-ATTRIBUTES>
      </SPEC-OBJECT-TYPE>
    </SPEC-TYPES>
    <SPEC-OBJECTS>
      <SPEC-OBJECT IDENTIFIER="_s1" LONG-NAME="Enum Only">
        <VALUES>
          <ATTRIBUTE-VALUE-ENUMERATION>
            <THE-VALUES>
              <ENUM-VALUE-REF>_ev-approved</ENUM-VALUE-REF>
            </THE-VALUES>
            <DEFINITION>
              <ATTRIBUTE-DEFINITION-ENUMERATION-REF>_ad-status</ATTRIBUTE-DEFINITION-ENUMERATION-REF>
            </DEFINITION>
          </ATTRIBUTE-VALUE-ENUMERATION>
        </VALUES>
        <TYPE><SPEC-OBJECT-TYPE-REF>_sot</SPEC-OBJECT-TYPE-REF></TYPE>
      </SPEC-OBJECT>
    </SPEC-OBJECTS>
  </REQ-IF-CONTENT></CORE-CONTENT>
</REQ-IF>"#;
        let items = import_reqif(xml, &[("Requirement".to_string(), "req".to_string())], &[]).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id.to_string(), "req~s1~1");
        // The enum value resolves to its LONG-NAME and surfaces as the text
        // fallback when no text attribute exists.
        assert_eq!(items[0].description.as_deref(), Some("Approved"));
    }

    #[test]
    fn test_import_relation_direction_mapping() {
        let xml = r#"<?xml version="1.0"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <CORE-CONTENT><REQ-IF-CONTENT>
    <SPEC-TYPES>
      <SPEC-RELATION-TYPE IDENTIFIER="_srt-sat" LONG-NAME="satisfied-by"/>
      <SPEC-RELATION-TYPE IDENTIFIER="_srt-dep" LONG-NAME="depends-on"/>
    </SPEC-TYPES>
    <SPEC-OBJECTS>
      <SPEC-OBJECT IDENTIFIER="_a" LONG-NAME="A"/>
      <SPEC-OBJECT IDENTIFIER="_b" LONG-NAME="B"/>
      <SPEC-OBJECT IDENTIFIER="_c" LONG-NAME="C"/>
    </SPEC-OBJECTS>
    <SPEC-RELATIONS>
      <SPEC-RELATION IDENTIFIER="_r1">
        <SOURCE><SPEC-OBJECT-REF>_a</SPEC-OBJECT-REF></SOURCE>
        <TARGET><SPEC-OBJECT-REF>_b</SPEC-OBJECT-REF></TARGET>
        <TYPE><SPEC-RELATION-TYPE-REF>_srt-sat</SPEC-RELATION-TYPE-REF></TYPE>
      </SPEC-RELATION>
      <SPEC-RELATION IDENTIFIER="_r2">
        <SOURCE><SPEC-OBJECT-REF>_a</SPEC-OBJECT-REF></SOURCE>
        <TARGET><SPEC-OBJECT-REF>_c</SPEC-OBJECT-REF></TARGET>
        <TYPE><SPEC-RELATION-TYPE-REF>_srt-dep</SPEC-RELATION-TYPE-REF></TYPE>
      </SPEC-RELATION>
    </SPEC-RELATIONS>
  </REQ-IF-CONTENT></CORE-CONTENT>
</REQ-IF>"#;
        let map = [
            ("satisfied-by".to_string(), "covered-by".to_string()),
            ("depends-on".to_string(), "ignore".to_string()),
        ];
        let items = import_reqif(xml, &[], &map).unwrap();
        let a = &items[0];
        let b = &items[1];
        let c = &items[2];
        // satisfied-by is inverted: the target covers the source.
        assert_eq!(b.covers, vec![a.id.clone()]);
        // depends-on is ignored entirely.
        assert!(a.covers.is_empty());
        assert!(c.covers.is_empty());
    }

    #[test]
    fn test_import_preserves_attributes() {
        let items = import_reqif(
            SAMPLE_REQIF,
            &[("Customer Requirement".to_string(), "creq".to_string())],
            &[],
        )
        .unwrap();
        // The Status attribute of the first object survives alongside the
        // picked ReqIF.Text description.
        assert_eq!(
            items[0].attributes,
            vec![("Status".to_string(), "approved".to_string())]
        );
    }

    #[test]
    fn test_import_orders_by_specification_hierarchy() {
        let xml = r#"<?xml version="1.0"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd">
  <CORE-CONTENT><REQ-IF-CONTENT>
    <SPEC-OBJECTS>
      <SPEC-OBJECT IDENTIFIER="_a" LONG-NAME="First Declared"/>
      <SPEC-OBJECT IDENTIFIER="_b" LONG-NAME="Second Declared"/>
      <SPEC-OBJECT IDENTIFIER="_c" LONG-NAME="Not In Hierarchy"/>
    </SPEC-OBJECTS>
    <SPECIFICATIONS>
      <SPECIFICATION IDENTIFIER="_spec" LONG-NAME="Doc">
        <CHILDREN>
          <SPEC-HIERARCHY IDENTIFIER="_h1">
            <OBJECT><SPEC-OBJECT-REF>_b</SPEC-OBJECT-REF></OBJECT>
          </SPEC-HIERARCHY>
          <SPEC-HIERARCHY IDENTIFIER="_h2">
            <OBJECT><SPEC-OBJECT-REF>_a</SPEC-OBJECT-REF></OBJECT>
          </SPEC-HIERARCHY>
        </CHILDREN>
        <TYPE><SPECIFICATION-TYPE-REF>_spt</SPECIFICATION-TYPE-REF></TYPE>
      </SPECIFICATION>
    </SPECIFICATIONS>
  </REQ-IF-CONTENT></CORE-CONTENT>
</REQ-IF>"#;
        let items = import_reqif(xml, &[], &[]).unwrap();
        let order: Vec<String> = items.iter().map(|i| i.id.to_string()).collect();
        assert_eq!(order, vec!["b~b~1", "a~a~1", "c~c~1"]);
    }

    #[test]
    fn test_export_import_round_trip_preserves_attributes() {
        let req_id = SpecificationItemId::parse("req~auth~1").unwrap();
        let items = vec![SpecificationItem::builder(req_id)
            .description("Do auth.".to_string())
            .attribute("Status".to_string(), "approved".to_string())
            .attribute("Priority".to_string(), "high".to_string())
            .build()];

        let xml = export_reqif(&items).unwrap();
        assert!(xml.contains("<SPECIFICATIONS>"), "module structure present");
        assert!(xml.contains("SPEC-HIERARCHY"), "hierarchy present");

        let reimported = import_reqif(&xml, &[], &[]).unwrap();
        assert_eq!(reimported.len(), 1);
        assert_eq!(
            reimported[0].attributes,
            vec![
                ("Status".to_string(), "approved".to_string()),
                ("Priority".to_string(), "high".to_string()),
            ]
        );
        assert_eq!(reimported[0].description.as_deref(), Some("Do auth."));
    }

    #[test]
    fn test_import_reqif_invalid_xml_errors() {
        assert!(import_reqif("<REQ-IF><a></b></REQ-IF>", &[], &[]).is_err());
    }

    #[test]
    fn test_export_import_round_trip() {
        let creq_id = SpecificationItemId::parse("creq~fast-login~1").unwrap();
        let req_id = SpecificationItemId::parse("req~auth-service~1").unwrap();
        let dsn_id = SpecificationItemId::parse("dsn~token-flow~2").unwrap();

        let items = vec![
            SpecificationItem::builder(creq_id.clone())
                .title("Fast Login".to_string())
                .description(
                    "The user shall log in quickly & securely.\nSecond paragraph.".to_string(),
                )
                .aliases(vec!["_uuid-0001".to_string()])
                .build(),
            SpecificationItem::builder(req_id.clone())
                .title("Auth Service".to_string())
                .description("Provide token based auth.".to_string())
                .covers(creq_id.clone())
                .build(),
            SpecificationItem::builder(dsn_id.clone())
                .covers(req_id.clone())
                .covers(creq_id.clone())
                .build(),
        ];

        let xml = export_reqif(&items).unwrap();
        assert!(xml.contains("xmlns=\"http://www.omg.org/spec/ReqIF/20110401/reqif.xsd\""));
        assert!(xml.contains("<REQ-IF-VERSION>1.1</REQ-IF-VERSION>"));
        assert!(xml.contains("<THE-VALUE-XHTML>"), "ReqIF 1.1 XHTML container");

        let reimported = import_reqif(&xml, &[], &[]).unwrap();
        let original_ids: Vec<_> = items.iter().map(|i| i.id.to_string()).collect();
        let roundtrip_ids: Vec<_> = reimported.iter().map(|i| i.id.to_string()).collect();
        assert_eq!(original_ids, roundtrip_ids);

        let original_covers: Vec<Vec<String>> = items
            .iter()
            .map(|i| i.covers.iter().map(|c| c.to_string()).collect())
            .collect();
        let roundtrip_covers: Vec<Vec<String>> = reimported
            .iter()
            .map(|i| i.covers.iter().map(|c| c.to_string()).collect())
            .collect();
        assert_eq!(original_covers, roundtrip_covers);

        // Descriptions survive the XHTML stripping, paragraph breaks included.
        assert_eq!(
            reimported[0].description.as_deref(),
            Some("The user shall log in quickly & securely.\nSecond paragraph.")
        );
        // The ReqIF UUID alias is preserved.
        assert_eq!(reimported[0].aliases, vec!["_uuid-0001".to_string()]);
        assert_eq!(reimported[0].id, creq_id);
    }

    #[test]
    fn test_export_skips_relations_to_unknown_targets() {
        let req_id = SpecificationItemId::parse("req~lonely~1").unwrap();
        let ghost = SpecificationItemId::parse("req~ghost~1").unwrap();
        let items = vec![SpecificationItem::builder(req_id)
            .covers(ghost)
            .build()];
        let xml = export_reqif(&items).unwrap();
        assert!(!xml.contains("<SPEC-RELATION "));
    }

    #[test]
    fn test_xhtml_to_text() {
        assert_eq!(
            xhtml_to_text("<div><p>Hello <b>world</b></p><p>Again & more</p></div>"),
            "Hello world\nAgain & more"
        );
        assert_eq!(xhtml_to_text("no markup"), "no markup");
        assert_eq!(xhtml_to_text("<p>  spaced   out </p>"), "spaced out");
        // Input arrives entity-decoded from quick_xml; entities must survive.
        assert_eq!(xhtml_to_text("<p>Fish &lt;Chips></p>"), "Fish &lt;Chips>");
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Customer Requirement"), "customer-requirement");
        assert_eq!(slugify("  Weird  Name!! "), "weird-name");
        assert_eq!(slugify(""), "");
    }
}
