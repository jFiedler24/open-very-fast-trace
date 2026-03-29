/**
 * OVFT Requirement Tracing — Data model and cursor-level detection.
 *
 * The workspace index (RequirementIndex) is populated from the WASM
 * trace engine's TraceResult — all file parsing is done by the Rust core.
 *
 * The cursor-level detection functions (detectAtPosition, detectAllOnLine)
 * use regex patterns to identify requirement IDs under the cursor for
 * navigation providers. These are a UI concern and not duplicating the
 * Rust parsing.
 */

import * as vscode from "vscode";
import type { TraceResult, TraceLinkedItem } from "./trace_engine";

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

export interface RequirementId {
  artifactType: string;
  name: string;
  revision: number;
}

export function reqIdToString(id: RequirementId): string {
  return `${id.artifactType}~${id.name}~${id.revision}`;
}

export function parseReqId(raw: string): RequirementId | undefined {
  const m = raw.match(/^([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)$/);
  if (!m) return undefined;
  return { artifactType: m[1], name: m[2], revision: parseInt(m[3], 10) };
}

/** A requirement specification found in a markdown file. */
export interface SpecLocation {
  id: RequirementId;
  uri: vscode.Uri;
  line: number; // 0-based
  needs: string[];
  covers: RequirementId[];
  depends: RequirementId[];
}

/** A coverage tag found in a source file. */
export interface TagLocation {
  /** The item that is covered by this tag. */
  coveredId: RequirementId;
  /** Artifact type of the covering item (the tag's own type). */
  coveringType: string;
  /** Optional explicit covering item ID. */
  coveringId?: RequirementId;
  uri: vscode.Uri;
  line: number; // 0-based
}

// ---------------------------------------------------------------------------
// Regex patterns — used only for cursor-level detection in the editor,
// NOT for workspace indexing (that comes from the WASM core).
// ---------------------------------------------------------------------------

// Spec ID in markdown (backtick-wrapped)
const SPEC_ID_RE = /`([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)`/g;

// Full tag: [coverType->type~name~rev]  or  [cType~cName~cRev->type~name~rev>>needs]
const FULL_TAG_RE =
  /\[\s*([a-zA-Z]+)(?:~([a-zA-Z0-9._-]+)~(\d+))?\s*->\s*([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)\s*(?:>>\s*([a-zA-Z0-9,\s]+))?\s*\]/g;

// Short tag: [[type~name~rev:coveringType]]
const SHORT_TAG_RE =
  /\[\[\s*([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)\s*:\s*([a-zA-Z]+)\s*\]\]/g;

// Bare ID reference (without backticks) — used in Covers/Depends lists
const BARE_ID_RE = /([a-zA-Z]+)~([a-zA-Z0-9._-]+)~(\d+)/g;

// ---------------------------------------------------------------------------
// Workspace index — populated from WASM TraceResult
// ---------------------------------------------------------------------------

export class RequirementIndex {
  /** spec ID string → locations */
  specs = new Map<string, SpecLocation[]>();

  /** covered ID string → tag locations */
  tags = new Map<string, TagLocation[]>();

  private disposables: vscode.Disposable[] = [];
  private watcher: vscode.FileSystemWatcher | undefined;
  private debounceTimer: ReturnType<typeof setTimeout> | undefined;
  private _onDidUpdate = new vscode.EventEmitter<void>();
  readonly onDidUpdate = this._onDidUpdate.event;

  /** Callback set by extension.ts — triggers a full re-trace on file changes. */
  onFileChanged: (() => void) | undefined;

  /**
   * Populate the index from a WASM TraceResult.
   * All parsing is done by the Rust core; this just maps the result
   * into the SpecLocation / TagLocation data structures used by providers.
   */
  populateFromTrace(result: TraceResult): void {
    this.specs.clear();
    this.tags.clear();

    for (const item of result.items) {
      if (!item.file_path || item.line == null) continue;

      const uri = vscode.Uri.file(item.file_path);
      const line = item.line - 1; // Rust is 1-based, VS Code is 0-based

      const isSpec = /\.(md|markdown)$/i.test(item.file_path);

      if (isSpec) {
        const id = parseReqId(item.id);
        if (!id) continue;
        const spec: SpecLocation = {
          id,
          uri,
          line,
          needs: item.needs,
          covers: item.covers.map((c) => parseReqId(c)).filter(Boolean) as RequirementId[],
          depends: item.depends.map((d) => parseReqId(d)).filter(Boolean) as RequirementId[],
        };
        const key = reqIdToString(id);
        const arr = this.specs.get(key) ?? [];
        arr.push(spec);
        this.specs.set(key, arr);
      } else {
        // Source file item — it's a covering tag
        const id = parseReqId(item.id);
        if (!id) continue;
        for (const coveredStr of item.covers) {
          const coveredId = parseReqId(coveredStr);
          if (!coveredId) continue;
          const tag: TagLocation = {
            coveredId,
            coveringType: item.artifact_type,
            coveringId: id,
            uri,
            line,
          };
          const key = reqIdToString(coveredId);
          const arr = this.tags.get(key) ?? [];
          arr.push(tag);
          this.tags.set(key, arr);
        }
      }
    }

    this._onDidUpdate.fire();
  }

  startWatching(): void {
    this.watcher = vscode.workspace.createFileSystemWatcher("**/*");
    const debouncedChange = () => {
      if (this.debounceTimer) clearTimeout(this.debounceTimer);
      this.debounceTimer = setTimeout(() => {
        this.onFileChanged?.();
      }, 500);
    };
    this.disposables.push(
      this.watcher.onDidChange(debouncedChange),
      this.watcher.onDidCreate(debouncedChange),
      this.watcher.onDidDelete(debouncedChange),
      this.watcher
    );
  }

  dispose(): void {
    if (this.debounceTimer) clearTimeout(this.debounceTimer);
    this.disposables.forEach((d) => d.dispose());
    this._onDidUpdate.dispose();
  }

  // -----------------------------------------------------------------------
  // Queries
  // -----------------------------------------------------------------------

  findSpec(id: RequirementId): SpecLocation[] {
    return this.specs.get(reqIdToString(id)) ?? [];
  }

  findCoverageTags(id: RequirementId): TagLocation[] {
    return this.tags.get(reqIdToString(id)) ?? [];
  }

  findRelated(id: RequirementId): { specs: SpecLocation[]; tags: TagLocation[] } {
    return {
      specs: this.findSpec(id),
      tags: this.findCoverageTags(id),
    };
  }
}

// ---------------------------------------------------------------------------
// Utility: detect a requirement ID or tag at a given position in text
// ---------------------------------------------------------------------------

export interface DetectedItem {
  /** The requirement ID found at the cursor. */
  id: RequirementId;
  /** Was this inside a tag (true) or a spec ID (false)? */
  isTag: boolean;
  /** Range of the full match in the document. */
  range: vscode.Range;
}

/**
 * Try to detect a requirement ID or coverage tag at the given position.
 * Returns undefined if no requirement reference is found at the cursor.
 */
export function detectAtPosition(
  document: vscode.TextDocument,
  position: vscode.Position
): DetectedItem | undefined {
  const line = document.lineAt(position.line).text;

  // Try full tag first
  FULL_TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = FULL_TAG_RE.exec(line)) !== null) {
    const start = m.index;
    const end = start + m[0].length;
    if (position.character >= start && position.character <= end) {
      return {
        id: { artifactType: m[4], name: m[5], revision: parseInt(m[6], 10) },
        isTag: true,
        range: new vscode.Range(position.line, start, position.line, end),
      };
    }
  }

  // Try short tag
  SHORT_TAG_RE.lastIndex = 0;
  while ((m = SHORT_TAG_RE.exec(line)) !== null) {
    const start = m.index;
    const end = start + m[0].length;
    if (position.character >= start && position.character <= end) {
      return {
        id: { artifactType: m[1], name: m[2], revision: parseInt(m[3], 10) },
        isTag: true,
        range: new vscode.Range(position.line, start, position.line, end),
      };
    }
  }

  // Try backtick-wrapped spec ID
  SPEC_ID_RE.lastIndex = 0;
  while ((m = SPEC_ID_RE.exec(line)) !== null) {
    const start = m.index;
    const end = start + m[0].length;
    if (position.character >= start && position.character <= end) {
      return {
        id: { artifactType: m[1], name: m[2], revision: parseInt(m[3], 10) },
        isTag: false,
        range: new vscode.Range(position.line, start, position.line, end),
      };
    }
  }

  // Try bare ID (in Covers/Depends lists, comments, etc.)
  BARE_ID_RE.lastIndex = 0;
  while ((m = BARE_ID_RE.exec(line)) !== null) {
    const start = m.index;
    const end = start + m[0].length;
    if (position.character >= start && position.character <= end) {
      return {
        id: { artifactType: m[1], name: m[2], revision: parseInt(m[3], 10) },
        isTag: false,
        range: new vscode.Range(position.line, start, position.line, end),
      };
    }
  }

  return undefined;
}

/**
 * Find all requirement IDs and tags on a given line.
 * Used by CodeLens to enumerate all items on a line.
 */
export function detectAllOnLine(
  document: vscode.TextDocument,
  lineNumber: number
): DetectedItem[] {
  const items: DetectedItem[] = [];
  const line = document.lineAt(lineNumber).text;
  const seen = new Set<string>();

  const add = (id: RequirementId, isTag: boolean, start: number, end: number) => {
    const key = `${reqIdToString(id)}@${start}`;
    if (seen.has(key)) return;
    seen.add(key);
    items.push({
      id,
      isTag,
      range: new vscode.Range(lineNumber, start, lineNumber, end),
    });
  };

  FULL_TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = FULL_TAG_RE.exec(line)) !== null) {
    add({ artifactType: m[4], name: m[5], revision: parseInt(m[6], 10) }, true, m.index, m.index + m[0].length);
  }

  SHORT_TAG_RE.lastIndex = 0;
  while ((m = SHORT_TAG_RE.exec(line)) !== null) {
    add({ artifactType: m[1], name: m[2], revision: parseInt(m[3], 10) }, true, m.index, m.index + m[0].length);
  }

  SPEC_ID_RE.lastIndex = 0;
  while ((m = SPEC_ID_RE.exec(line)) !== null) {
    add({ artifactType: m[1], name: m[2], revision: parseInt(m[3], 10) }, false, m.index, m.index + m[0].length);
  }

  // Bare IDs (e.g. in markdown headings like "## req~name~1s")
  BARE_ID_RE.lastIndex = 0;
  while ((m = BARE_ID_RE.exec(line)) !== null) {
    add({ artifactType: m[1], name: m[2], revision: parseInt(m[3], 10) }, false, m.index, m.index + m[0].length);
  }

  return items;
}
