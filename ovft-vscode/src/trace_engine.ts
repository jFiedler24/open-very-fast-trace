/**
 * WASM-powered trace engine — runs the Rust ovft-core linker/analyzer
 * inside the VS Code extension host via WebAssembly.
 *
 * Collects all spec/source file contents from the workspace and feeds
 * them to the WASM module, producing a full TraceResult with linked items,
 * defects, and coverage summaries.
 */

import * as vscode from "vscode";
import { loadOvftConfig, buildSourceGlobs, buildSpecGlobs, buildExcludeGlob } from "./config_loader";

// eslint-disable-next-line @typescript-eslint/no-var-requires
const wasm = require("../pkg/ovft_wasm");

// ---------------------------------------------------------------------------
// Types mirroring JsTraceResult from ovft-wasm
// ---------------------------------------------------------------------------

export interface TraceLink {
  source_id: string | null;
  target_id: string;
  status: string;
}

export interface TraceLinkedItem {
  id: string;
  artifact_type: string;
  name: string;
  revision: number;
  title: string | null;
  description: string | null;
  needs: string[];
  covers: string[];
  depends: string[];
  coverage_status: string;
  is_defect: boolean;
  file_path: string | null;
  line: number | null;
  incoming_links: TraceLink[];
  outgoing_links: TraceLink[];
}

export interface TraceDefect {
  defect_type: string;
  description: string;
  item_id: string | null;
  file_path: string | null;
  line: number | null; // 1-based from Rust
}

export interface TraceCoverageSummary {
  total: number;
  covered: number;
  percentage: number;
  status: string;
}

export interface TraceResult {
  total_items: number;
  defect_count: number;
  is_success: boolean;
  coverage_percentage: number;
  defects: TraceDefect[];
  items: TraceLinkedItem[];
  coverage_summary: Record<string, TraceCoverageSummary>;
}

// ---------------------------------------------------------------------------
// Trace engine
// ---------------------------------------------------------------------------

export class TraceEngine {
  private _result: TraceResult | undefined;
  private _onDidUpdate = new vscode.EventEmitter<TraceResult>();
  readonly onDidUpdate = this._onDidUpdate.event;

  get result(): TraceResult | undefined {
    return this._result;
  }

  /**
   * Collect all spec and source file contents from the workspace,
   * using .ovft.toml config if present, falling back to VS Code settings.
   */
  private async collectFiles(): Promise<{
    specFiles: { path: string; content: string }[];
    sourceFiles: { path: string; content: string }[];
  }> {
    const ovftConfig = await loadOvftConfig();
    const specPatterns = buildSpecGlobs(ovftConfig);
    const tagPatterns = buildSourceGlobs(ovftConfig);
    const exclude = buildExcludeGlob(ovftConfig);

    const specFiles: { path: string; content: string }[] = [];
    const sourceFiles: { path: string; content: string }[] = [];

    for (const pattern of specPatterns) {
      const uris = await vscode.workspace.findFiles(pattern, exclude);
      for (const uri of uris) {
        try {
          const bytes = await vscode.workspace.fs.readFile(uri);
          specFiles.push({
            path: uri.fsPath,
            content: Buffer.from(bytes).toString("utf-8"),
          });
        } catch {
          // skip unreadable files
        }
      }
    }

    for (const pattern of tagPatterns) {
      const uris = await vscode.workspace.findFiles(pattern, exclude);
      for (const uri of uris) {
        try {
          const bytes = await vscode.workspace.fs.readFile(uri);
          sourceFiles.push({
            path: uri.fsPath,
            content: Buffer.from(bytes).toString("utf-8"),
          });
        } catch {
          // skip unreadable files
        }
      }
    }

    return { specFiles, sourceFiles };
  }

  /**
   * Run a full trace analysis by reading all workspace files and passing
   * them through the WASM core.
   */
  async runTrace(): Promise<TraceResult> {
    const { specFiles, sourceFiles } = await this.collectFiles();

    const result: TraceResult = wasm.trace_from_contents(
      JSON.stringify(specFiles),
      JSON.stringify(sourceFiles)
    );

    this._result = result;
    this._onDidUpdate.fire(result);
    return result;
  }

  /**
   * Render the HTML trace report by collecting workspace files and
   * running a full trace-and-render pass through the WASM core.
   */
  async renderHtmlReportFromWorkspace(): Promise<string> {
    const { specFiles, sourceFiles } = await this.collectFiles();
    return wasm.trace_and_render_html(
      JSON.stringify(specFiles),
      JSON.stringify(sourceFiles)
    );
  }

  /**
   * Find an item by its ID string (e.g. "req~name~1").
   */
  findItem(id: string): TraceLinkedItem | undefined {
    return this._result?.items.find((i) => i.id === id);
  }

  /**
   * Get the transitive coverage chain starting from the given item ID.
   * Follows outgoing_links (covers) transitively upward, and incoming_links
   * (covered-by) transitively downward.
   */
  transitiveFollow(startId: string, direction: "up" | "down"): TraceLinkedItem[] {
    if (!this._result) return [];

    const itemMap = new Map<string, TraceLinkedItem>();
    for (const item of this._result.items) {
      itemMap.set(item.id, item);
    }

    const visited = new Set<string>();
    const result: TraceLinkedItem[] = [];

    const walk = (id: string) => {
      if (visited.has(id)) return;
      visited.add(id);
      const item = itemMap.get(id);
      if (!item) return;
      result.push(item);

      if (direction === "up") {
        // Follow the items this one covers (outgoing links going up the hierarchy)
        for (const coverId of item.covers) {
          walk(coverId);
        }
      } else {
        // Follow incoming links (items that cover this one)
        for (const link of item.incoming_links) {
          if (link.source_id) {
            walk(link.source_id);
          }
        }
      }
    };

    walk(startId);
    // Remove the start item itself from results
    return result.filter((i) => i.id !== startId);
  }

  dispose(): void {
    this._onDidUpdate.dispose();
  }
}
