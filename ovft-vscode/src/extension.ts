/**
 * OVFT Requirement Tracing — VS Code extension entry point.
 *
 * Provides navigation between requirement specifications (in markdown)
 * and coverage tags (in source code) without interfering with
 * language-specific LSPs.
 *
 * Navigation:
 *   Cmd+Shift+R  — Follow Requirement (go to spec definition)
 *   Cmd+Shift+T  — Find Coverage Tags (find all implementing tags)
 *   Cmd+Shift+U  — Transitive Follow (up the coverage chain)
 *   Cmd+Shift+D  — Transitive Follow (down the coverage chain)
 *   Right-click   — context menu entries
 *   Ctrl+Click   — go-to-definition works on requirement IDs
 *   Shift+F12    — find-references works on requirement IDs
 *   Hover        — shows coverage summary
 *   CodeLens     — inline coverage status above IDs/tags
 */

import * as vscode from "vscode";
import { RequirementIndex, detectAtPosition, reqIdToString, parseReqId } from "./index";
import {
  OvftDefinitionProvider,
  OvftReferenceProvider,
  OvftCodeLensProvider,
  OvftDocumentHighlightProvider,
  OvftHoverProvider,
  OvftNeedsLinkProvider,
  OvftNeedsDecorationManager,
  specsToLocations,
  tagsToLocations,
  navigateToLocations,
} from "./providers";
import { TraceEngine, TraceLinkedItem } from "./trace_engine";
import { OvftDiagnosticsProvider } from "./diagnostics";
import { RequirementTreeProvider } from "./tree_provider";

// All file types we operate on — we register broadly but our providers
// return undefined when the cursor isn't on a requirement, so they
// don't interfere with other LSPs.
const ALL_LANGUAGES: vscode.DocumentSelector = { scheme: "file" };

let index: RequirementIndex;
let traceEngine: TraceEngine;
let diagnostics: OvftDiagnosticsProvider;
let statusBarItem: vscode.StatusBarItem;
let reportPanel: vscode.WebviewPanel | undefined;
let output: vscode.OutputChannel;

/**
 * Run a full trace via WASM, populate the index, update diagnostics.
 * This is the single entry point for all (re-)analysis.
 */
async function runFullTrace(): Promise<void> {
  statusBarItem.text = "$(loading~spin) OVFT: analyzing…";
  try {
    const result = await traceEngine.runTrace();
    index.populateFromTrace(result);
    output.appendLine(
      `Trace: ${result.total_items} items, ${result.defect_count} defects, ` +
      `${result.coverage_percentage.toFixed(1)}% coverage`
    );
  } catch (e) {
    output.appendLine(`Trace analysis failed: ${e}`);
  }
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  output = vscode.window.createOutputChannel("OVFT Tracing");
  output.appendLine("OVFT Requirement Tracing extension activating…");

  // Trace engine (WASM-powered parsing + linking + analysis)
  traceEngine = new TraceEngine();
  context.subscriptions.push(traceEngine);

  // Index (populated from trace result, used by providers for navigation)
  index = new RequirementIndex();
  context.subscriptions.push(index);

  // Diagnostics provider (Problems panel)
  diagnostics = new OvftDiagnosticsProvider();
  context.subscriptions.push(diagnostics);

  // Update diagnostics whenever the trace result changes
  traceEngine.onDidUpdate((result) => {
    diagnostics.update(result);
    // Auto-refresh the report webview if it's open
    if (reportPanel) {
      refreshReportPanel();
    }
  });

  // Status bar item
  statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
  statusBarItem.command = "ovft.refreshIndex";
  statusBarItem.tooltip = "OVFT: Click to refresh requirement index";
  context.subscriptions.push(statusBarItem);

  const updateStatus = () => {
    const specCount = [...index.specs.values()].reduce((n, a) => n + a.length, 0);
    const tagCount = [...index.tags.values()].reduce((n, a) => n + a.length, 0);
    const result = traceEngine.result;
    if (result) {
      const defectStr = result.defect_count > 0 ? `, ${result.defect_count} defects` : "";
      statusBarItem.text = `$(checklist) OVFT: ${specCount} specs, ${tagCount} tags${defectStr}`;
    } else {
      statusBarItem.text = `$(checklist) OVFT: ${specCount} specs, ${tagCount} tags`;
    }
    statusBarItem.show();
  };

  index.onDidUpdate(updateStatus);
  traceEngine.onDidUpdate(updateStatus);

  // Initial trace — single WASM pass does parsing + linking + analysis
  statusBarItem.text = "$(loading~spin) OVFT: analyzing…";
  statusBarItem.show();
  await runFullTrace();

  // File watcher — debounced re-trace on any file change
  index.onFileChanged = () => runFullTrace();
  index.startWatching();

  // Register providers — they return undefined when cursor isn't on a
  // requirement so they coexist peacefully with language LSPs.
  context.subscriptions.push(
    vscode.languages.registerDefinitionProvider(ALL_LANGUAGES, new OvftDefinitionProvider(index)),
    vscode.languages.registerReferenceProvider(ALL_LANGUAGES, new OvftReferenceProvider(index)),
    vscode.languages.registerCodeLensProvider(ALL_LANGUAGES, new OvftCodeLensProvider(index, traceEngine)),
    vscode.languages.registerDocumentHighlightProvider(ALL_LANGUAGES, new OvftDocumentHighlightProvider(index)),
    vscode.languages.registerHoverProvider(ALL_LANGUAGES, new OvftHoverProvider(index, traceEngine)),
    vscode.languages.registerDocumentLinkProvider({ scheme: "file", pattern: "**/*.{md,markdown}" }, new OvftNeedsLinkProvider(traceEngine))
  );

  // Needs coverage decorations (green underline = covered, wavy orange = uncovered)
  const needsDecorations = new OvftNeedsDecorationManager(traceEngine);
  context.subscriptions.push(needsDecorations);

  // Refresh decorations when the index updates (after trace completes)
  index.onDidUpdate(() => needsDecorations.refresh());

  // Register requirement tree view in the sidebar
  const treeProvider = new RequirementTreeProvider(traceEngine);
  context.subscriptions.push(
    vscode.window.createTreeView("ovftRequirementTree", {
      treeDataProvider: treeProvider,
      showCollapseAll: true,
    })
  );

  // -----------------------------------------------------------------------
  // Commands
  // -----------------------------------------------------------------------

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "ovft.followRequirement",
      async (uriArg?: vscode.Uri | string, posArg?: vscode.Position | { line: number; character: number }) => {
        const editor = vscode.window.activeTextEditor;
        let document: vscode.TextDocument;
        let position: vscode.Position;

        if (uriArg && posArg) {
          const uri = typeof uriArg === "string" ? vscode.Uri.parse(uriArg) : uriArg;
          document = await vscode.workspace.openTextDocument(uri);
          position = posArg instanceof vscode.Position
            ? posArg
            : new vscode.Position(posArg.line, posArg.character);
        } else if (editor) {
          document = editor.document;
          position = editor.selection.active;
        } else {
          vscode.window.showWarningMessage("OVFT: No active editor.");
          return;
        }

        const detected = detectAtPosition(document, position);
        if (!detected) {
          vscode.window.showInformationMessage("OVFT: No requirement ID at cursor.");
          return;
        }

        const specs = index.findSpec(detected.id);
        await navigateToLocations(
          specsToLocations(specs),
          `specification for ${reqIdToString(detected.id)}`
        );
      }
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "ovft.findCoverage",
      async (uriArg?: vscode.Uri | string, posArg?: vscode.Position | { line: number; character: number }) => {
        const editor = vscode.window.activeTextEditor;
        let document: vscode.TextDocument;
        let position: vscode.Position;

        if (uriArg && posArg) {
          const uri = typeof uriArg === "string" ? vscode.Uri.parse(uriArg) : uriArg;
          document = await vscode.workspace.openTextDocument(uri);
          position = posArg instanceof vscode.Position
            ? posArg
            : new vscode.Position(posArg.line, posArg.character);
        } else if (editor) {
          document = editor.document;
          position = editor.selection.active;
        } else {
          vscode.window.showWarningMessage("OVFT: No active editor.");
          return;
        }

        const detected = detectAtPosition(document, position);
        if (!detected) {
          vscode.window.showInformationMessage("OVFT: No requirement ID at cursor.");
          return;
        }

        const tags = index.findCoverageTags(detected.id);
        await navigateToLocations(
          tagsToLocations(tags),
          `coverage tags for ${reqIdToString(detected.id)}`
        );
      }
    )
  );

  // Transitive follow commands
  const transitiveFollow = async (direction: "up" | "down") => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) {
      vscode.window.showWarningMessage("OVFT: No active editor.");
      return;
    }

    const detected = detectAtPosition(editor.document, editor.selection.active);
    if (!detected) {
      vscode.window.showInformationMessage("OVFT: No requirement ID at cursor.");
      return;
    }

    const idStr = reqIdToString(detected.id);
    const chain = traceEngine.transitiveFollow(idStr, direction);

    if (chain.length === 0) {
      vscode.window.showInformationMessage(
        `OVFT: No ${direction === "up" ? "covered" : "covering"} items found for ${idStr}.`
      );
      return;
    }

    // Show as a quick-pick with navigation
    const picks = chain.map((item) => ({
      label: `$(${itemIcon(item)}) ${item.id}`,
      description: item.coverage_status,
      detail: item.file_path
        ? `${vscode.workspace.asRelativePath(item.file_path)}:${item.line ?? "?"}`
        : undefined,
      item,
    }));

    const selected = await vscode.window.showQuickPick(picks, {
      title: `OVFT: Transitive ${direction === "up" ? "Covers" : "Covered-by"} chain from ${idStr}`,
      placeHolder: "Select an item to navigate to",
    });

    if (selected?.item.file_path && selected.item.line) {
      const uri = vscode.Uri.file(selected.item.file_path);
      const doc = await vscode.workspace.openTextDocument(uri);
      const ed = await vscode.window.showTextDocument(doc);
      const pos = new vscode.Position(selected.item.line - 1, 0); // 1-based → 0-based
      ed.selection = new vscode.Selection(pos, pos);
      ed.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
    }
  };

  context.subscriptions.push(
    vscode.commands.registerCommand("ovft.transitiveFollowUp", () => transitiveFollow("up")),
    vscode.commands.registerCommand("ovft.transitiveFollowDown", () => transitiveFollow("down"))
  );

  // Navigate to covering items for a specific needs type
  context.subscriptions.push(
    vscode.commands.registerCommand(
      "ovft.navigateNeedCoverage",
      async (specId: string, needType: string) => {
        const traceItem = traceEngine.findItem(specId);
        if (!traceItem) {
          vscode.window.showInformationMessage(`OVFT: Item ${specId} not found in trace.`);
          return;
        }

        const coveringLinks = traceItem.incoming_links.filter((l) => {
          if (!l.source_id) return false;
          const parsed = parseReqId(l.source_id);
          return parsed && parsed.artifactType === needType;
        });

        if (coveringLinks.length === 0) {
          vscode.window.showWarningMessage(
            `OVFT: No ${needType} coverage found for ${specId}`
          );
          return;
        }

        const locations: vscode.Location[] = [];
        for (const link of coveringLinks) {
          if (!link.source_id) continue;
          const sourceItem = traceEngine.findItem(link.source_id);
          if (sourceItem?.file_path && sourceItem?.line) {
            locations.push(
              new vscode.Location(
                vscode.Uri.file(sourceItem.file_path),
                new vscode.Position(sourceItem.line - 1, 0)
              )
            );
          }
        }

        await navigateToLocations(locations, `${needType} coverage for ${specId}`);
      }
    )
  );

  // Show trace report in webview
  context.subscriptions.push(
    vscode.commands.registerCommand("ovft.showTraceReport", async () => {
      if (reportPanel) {
        reportPanel.reveal(vscode.ViewColumn.Beside);
        return;
      }

      reportPanel = vscode.window.createWebviewPanel(
        "ovftTraceReport",
        "OVFT Trace Report",
        vscode.ViewColumn.Beside,
        { enableScripts: false }
      );

      reportPanel.onDidDispose(() => {
        reportPanel = undefined;
      });

      await refreshReportPanel();
    })
  );

  // Refresh index + trace
  context.subscriptions.push(
    vscode.commands.registerCommand("ovft.refreshIndex", async () => {
      await runFullTrace();

      // Refresh webview if open
      if (reportPanel) {
        await refreshReportPanel();
      }

      vscode.window.showInformationMessage("OVFT: Requirement index refreshed.");
    })
  );

  output.appendLine("OVFT Requirement Tracing extension activated.");
}

async function refreshReportPanel(): Promise<void> {
  if (!reportPanel) return;

  reportPanel.webview.html = "<html><body><p>Generating trace report…</p></body></html>";

  try {
    const html = await traceEngine.renderHtmlReportFromWorkspace();
    reportPanel.webview.html = html;
  } catch (e) {
    output.appendLine(`Report generation failed: ${e}`);
    reportPanel.webview.html = `<html><body><h2>Report generation failed</h2><pre>${escapeHtml(String(e))}</pre></body></html>`;
  }
}

function escapeHtml(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

function itemIcon(item: TraceLinkedItem): string {
  if (item.is_defect) return "warning";
  switch (item.coverage_status) {
    case "covered": return "pass";
    case "partial": return "circle-outline";
    default: return "circle-slash";
  }
}

export function deactivate(): void {
  // Cleanup handled by disposables
}
