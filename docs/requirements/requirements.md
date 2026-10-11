# Requirements

This document defines the detailed requirements for the Open Very Fast Trace (OVFT) library.

## req~tag-regex-parsing~1

**Title:** Parse requirement tags using regex patterns

**Description:** The system shall use regex patterns to identify and parse requirement tags in source code files, supporting both full tags `[impl->dsn~validate-request~1]` and short tags `[[req~login~1:impl]]`.

**Covers:** feat~tag-parsing~1

**Rationale:** Regex parsing provides efficient and flexible tag recognition across different file formats.

**Tags:** parsing, regex, performance

**Needs:** dsn

---

## req~markdown-specification-parsing~1

**Title:** Parse OpenFastTrace-compatible markdown syntax

**Description:** The system shall parse markdown files containing requirement specifications using OpenFastTrace syntax including headers like `# req~name~1`, and keywords like `Needs:`, `Covers:`, `Tags:`.

**Covers:** feat~markdown-parsing~1

**Rationale:** Compatibility with OpenFastTrace ensures existing requirement documents can be migrated without modification.

**Tags:** parsing, compatibility, markdown

**Needs:** dsn

---

## req~coverage-analysis~1

**Title:** Analyze requirement coverage and detect defects

**Description:** The system shall analyze the relationship between requirements and their implementations to determine coverage status and identify defects such as orphaned coverage, uncovered requirements, and circular dependencies.

**Covers:** feat~requirement-linking~1

**Rationale:** Automated coverage analysis ensures all requirements are properly implemented and tested.

**Tags:** analysis, coverage, quality

**Needs:** dsn

---

## req~html-template-rendering~1

**Title:** Render HTML reports using templates

**Description:** The system shall use template-based rendering to generate HTML reports that display requirement coverage, defects, and detailed traceability information.

**Covers:** feat~html-reporting~1

**Rationale:** Template-based rendering allows for consistent, professional-looking reports that can be easily customized.

**Tags:** reporting, templates, html

**Needs:** dsn

---

## req~configuration-management~1

**Title:** Manage configuration through files and code

**Description:** The system shall support configuration through both TOML files and programmatic configuration to specify source directories, specification directories, and output settings.

**Covers:** feat~build-integration~1

**Rationale:** Flexible configuration allows the tool to be adapted to different project structures and build processes.

**Tags:** configuration, flexibility, integration

**Needs:** dsn

---

## req~detailed-error-messages~1

**Title:** Provide detailed error messages with location information

**Description:** The system shall provide detailed error messages that include file paths, line numbers, and specific descriptions of parsing or analysis failures.

**Covers:** feat~error-handling~1

**Rationale:** Detailed error information helps developers quickly locate and fix issues in their requirements or code.

**Tags:** error-handling, debugging, usability

**Needs:** dsn

---

## req~file-format-support~1

**Title:** Support multiple source file formats

**Description:** The system shall support parsing requirement tags from Rust (.rs), ATL (.atl), and ADL (.adl) files using appropriate comment syntax for each format.

**Covers:** feat~tag-parsing~1

**Rationale:** Supporting multiple file formats makes the tool useful across different types of projects and toolchains.

**Tags:** compatibility, file-formats, flexibility

**Needs:** dsn

---

## req~html-compliant-anchors~1

**Title:** Generate HTML-compliant anchor IDs without displaying them

**Description:** The system shall generate HTML-compliant anchor IDs from requirement IDs for internal linking purposes while preserving the original requirement ID display in the HTML output. The anchor IDs shall replace problematic characters (like tildes) with HTML-safe alternatives (like underscores) but these transformed IDs shall not be shown to users in the rendered HTML.

**Covers:** feat~html-reporting~1

**Rationale:** HTML anchor IDs must be compliant with HTML standards for proper linking functionality, but users should continue to see the original, meaningful requirement IDs in the interface.

**Tags:** html, anchors, usability, compliance

**Needs:** dsn

---

## req~defect-requirement-linking~1

**Title:** Provide clickable links from defects to related requirements

**Description:** The system shall include clickable links in defect descriptions that navigate to the related requirement items in the HTML report. When a defect references a specific requirement ID, that ID shall be rendered as a hyperlink that scrolls to the corresponding requirement section.

**Covers:** feat~html-reporting~1

**Rationale:** Direct navigation from defects to requirements improves user workflow efficiency and reduces the time needed to understand and resolve traceability issues.

**Tags:** navigation, defects, linking, usability

**Needs:** dsn

---

## req~defect-type-statistics~1

**Title:** Provide detailed statistics breakdown by defect type

**Description:** The system shall extend the summary statistics section to include a breakdown of defects by type, showing counts for each defect category such as "X SpecItems have no coverage of utest", "Y items have orphaned coverage", etc. This shall be displayed in addition to the overall defect count.

**Covers:** feat~html-reporting~1

**Rationale:** Detailed defect statistics help users quickly identify the most common traceability issues and prioritize their resolution efforts effectively.

**Tags:** statistics, defects, analysis, reporting

**Needs:** dsn

---

## req~release-notes-from-requirements~1

**Title:** Generate release notes from newly covered requirements

**Description:** The system shall provide a command that diffs requirement coverage between a baseline git ref (default: latest tag) and the current state, and renders requirements that became covered as markdown grouped by artifact type, showing title, ID, and the first description line.

**Covers:** feat~release-notes~1

**Rationale:** Coverage diffs between releases are exactly the set of delivered features; rendering them is mechanical once tracing exists.

**Tags:** release, reporting, git

**Needs:** impl

---

## req~title-first-report~1

**Title:** Title-first report items with filtering

**Description:** Each item in the HTML report shall display its human-readable title as the heading with the full requirement ID rendered as a badge, and the report shall provide a text and status filter over all items.

**Covers:** feat~readable-html-report~1

**Rationale:** Scanning a report by title is an order of magnitude faster than scanning by machine ID.

**Tags:** reporting, usability, html

**Needs:** impl

---

## req~alias-id-resolution~1

**Title:** Resolve alias IDs in coverage tags

**Description:** Coverage tags may reference an item by an alias declared in its spec via `**Aliases:**` (e.g. a tag pointing at ABC-1234 instead of the full ID). The linker shall resolve aliases case-insensitively, fall back to an unambiguous match on the item name, pick the newest revision when several exist, and report unresolved aliases as orphaned coverage.

**Aliases:** OVFT-42

**Covers:** feat~short-alias-ids~1

**Rationale:** Aliases are only useful if the tracer resolves them transparently to canonical IDs.

**Tags:** usability, ids, linking

**Needs:** impl

---

## req~editor-tag-hiding~1

**Title:** Collapse tag-only lines in VS Code

**Description:** The extension shall hide lines consisting solely of coverage tags (with optional comment markers), rendering an inline badge with the covered IDs instead, and reveal the raw line while it is selected. A command and a setting shall toggle the behavior.

**Covers:** feat~editor-tag-hiding~1

**Rationale:** Readability complaints about tag noise are the most common criticism of the tool.

**Tags:** usability, editor, vscode

**Needs:** impl

---

## req~reqif-import~2

**Title:** Import ReqIF/RIF files as markdown specs

**Description:** The system shall parse ReqIF and RIF XML (accepting the root elements REQ-IF and RIF and rejecting other XML documents), map spec objects to items (artifact type from the object type via a configurable map, stable canonical ID derived from the ReqIF identifier so renames in the source tool do not break trace links, title from the long name, identifier kept as alias, enumeration values resolved to their long names, XHTML text stripped to plain text without double-decoding entities), preserve all remaining attribute values as named item attributes, map relations to covers links honoring the relation type's direction via a configurable map (covers / covered-by / ignore), order items like the source document's specification hierarchy, tolerate unknown elements, and render the result as a generated markdown spec document. Zipped containers (.reqifz/.rifz) shall be unpacked transparently.

**Covers:** feat~reqif-interchange~1

**Rationale:** Imported customer requirements become first-class traceable items while the generated file stays clearly marked as non-editable. Identifier-derived IDs keep coverage tags valid across re-imports after the customer renames headings.

**Tags:** interoperability, reqif, import

**Needs:** impl

---

## req~reqif-export~2

**Title:** Export traced items as ReqIF

**Description:** The system shall export all traced items as a ReqIF 1.1 document with one object type per artifact type, the item text as an XHTML attribute, all named item attributes as string attribute values, covers links as typed spec relations, and a single specification with a flat hierarchy over all objects so importing tools present the export as an ordered module. Re-importing the export shall yield the same canonical IDs, links, and attributes.

**Covers:** feat~reqif-interchange~1

**Rationale:** A lossless round-trip lets teams answer customer tools with live traceability data instead of stale spreadsheets. The specification hierarchy makes the exchange module-oriented, as DOORS and Polarion expect.

**Tags:** interoperability, reqif, export

**Needs:** impl
