# Features

This document defines the high-level features for the Open Very Fast Trace (OVFT) library.

## feat~tag-parsing~1

**Title:** Parse requirement tags from source code files

**Description:** The system shall be able to parse requirement tags from various source code file formats including Rust (.rs), ATL (.atl), and ADL (.adl) files.

**Rationale:** This enables developers to embed traceability information directly in their source code, maintaining consistency between implementation and requirements.

**Tags:** parsing, source-code, traceability

**Needs:** req

---

## feat~markdown-parsing~1

**Title:** Parse requirements from markdown files

**Description:** The system shall be able to parse requirement specifications from markdown files using OpenFastTrace-compatible syntax.

**Rationale:** Markdown provides a human-readable format for documenting requirements that can be easily version-controlled and reviewed.

**Tags:** parsing, markdown, documentation

**Needs:** req

---

## feat~requirement-linking~1

**Title:** Link requirements to implementations and tests

**Description:** The system shall be able to automatically link requirements to their implementations and tests by analyzing coverage relationships.

**Rationale:** Automated linking ensures that all requirements are properly covered and helps identify orphaned or uncovered requirements.

**Tags:** linking, coverage, analysis

**Needs:** req

---

## feat~html-reporting~1

**Title:** Generate HTML tracing reports

**Description:** The system shall generate comprehensive HTML reports showing requirement coverage, defects, and traceability matrices.

**Rationale:** HTML reports provide an accessible way to visualize and share requirements tracing results with stakeholders.

**Tags:** reporting, html, visualization

**Needs:** req

---

## feat~build-integration~1

**Title:** Simple build.rs integration

**Description:** The system shall provide a simple API that can be easily integrated into Rust build.rs files for automated requirements checking.

**Rationale:** Build-time integration ensures that requirements tracing is part of the regular build process and prevents incomplete tracing from being committed.

**Tags:** integration, build, automation

**Needs:** req

---

## feat~error-handling~1

**Title:** Comprehensive error handling and reporting

**Description:** The system shall provide clear, actionable error messages when parsing fails or requirements tracing issues are detected.

**Rationale:** Good error handling helps developers quickly identify and fix requirements tracing issues.

**Tags:** error-handling, usability

**Needs:** req

---

## feat~release-notes~1

**Title:** Release notes from requirement coverage

**Description:** The system shall generate release notes by comparing requirement coverage between a git tag and the current state, summarizing all newly covered requirements.

**Rationale:** Release notes written by hand drift from what was actually delivered; deriving them from verified coverage keeps them truthful by construction.

**Tags:** release, reporting, git

**Needs:** req

---

## feat~readable-html-report~1

**Title:** Human-readable HTML report

**Description:** The HTML report shall lead with human-readable titles, show requirement IDs as secondary badges, summarize coverage per artifact type, and allow filtering items by text and status.

**Rationale:** Reports are read by humans under time pressure; the requirement's meaning matters more than its machine ID.

**Tags:** reporting, usability, html

**Needs:** req

---

## feat~short-alias-ids~1

**Title:** Short alias IDs for requirements

**Description:** The system shall allow requirements to declare short external aliases (e.g. ticket keys like ABC-1234) that can be used as coverage tag targets in place of the full type~name~revision ID.

**Rationale:** Teams tracking requirements in external tools (Jira, DOORS) need concise, stable references without giving up the canonical OVFT ID scheme.

**Tags:** usability, interoperability, ids

**Needs:** req

---

## feat~editor-tag-hiding~1

**Title:** Hideable coverage tags in the editor

**Description:** The VS Code extension shall collapse lines that contain only coverage tags into a subtle inline badge, revealing the raw tag while the cursor is on the line.

**Rationale:** Raw tags are machine-readable metadata; hiding them by default keeps source files readable without giving up local traceability.

**Tags:** usability, editor, vscode

**Needs:** req

---

## feat~reqif-interchange~1

**Title:** ReqIF import and export

**Description:** The system shall import requirements from ReqIF files (the OMG interchange format used by DOORS, Polarion and similar tools) into markdown specs, and export its own items with their trace links as ReqIF for exchange with such tools.

**Rationale:** Customers in regulated industries manage their requirements in ReqIF-based tools; interchange lets OVFT fit into that ecosystem without giving up markdown-native authoring.

**Tags:** interoperability, reqif, import, export

**Needs:** req
