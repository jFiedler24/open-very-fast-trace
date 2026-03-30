/**
 * OVFT Requirement Tree — TreeDataProvider for the sidebar view.
 *
 * Displays a hierarchical requirement tree built from trace results.
 * Root nodes are top-level requirements (specs that don't cover anything).
 * Children are items that cover each requirement, recursively.
 */

import * as vscode from "vscode";
import { TraceEngine, TraceLinkedItem, TraceResult } from "./trace_engine";

// ---------------------------------------------------------------------------
// Tree item
// ---------------------------------------------------------------------------

export class RequirementTreeItem extends vscode.TreeItem {
  constructor(
    public readonly traceItem: TraceLinkedItem,
    public readonly hasChildren: boolean
  ) {
    super(
      traceItem.id,
      hasChildren
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None
    );

    // Show title as description next to the ID
    this.description = traceItem.title ?? traceItem.coverage_status;

    // Tooltip with full info
    const lines: string[] = [`**${traceItem.id}**`];
    if (traceItem.title) lines.push(`\n${traceItem.title}`);
    if (traceItem.description) lines.push(`\n${traceItem.description}`);
    lines.push(`\nStatus: ${traceItem.coverage_status}`);
    if (traceItem.needs.length > 0) lines.push(`Needs: ${traceItem.needs.join(", ")}`);
    if (traceItem.covers.length > 0) lines.push(`Covers: ${traceItem.covers.join(", ")}`);
    const tip = new vscode.MarkdownString(lines.join("\n"));
    tip.isTrusted = true;
    this.tooltip = tip;

    // Icon based on coverage status
    this.iconPath = new vscode.ThemeIcon(itemIcon(traceItem), itemColor(traceItem));

    // Click navigates to file
    if (traceItem.file_path && traceItem.line != null) {
      const uri = vscode.Uri.file(traceItem.file_path);
      const line = traceItem.line - 1; // 1-based → 0-based
      this.command = {
        command: "vscode.open",
        title: "Go to definition",
        arguments: [uri, { selection: new vscode.Range(line, 0, line, 0) }],
      };
    }

    this.contextValue = traceItem.is_defect ? "ovftItem-defect" : "ovftItem";
  }
}

function itemIcon(item: TraceLinkedItem): string {
  if (item.is_defect) return "warning";
  switch (item.coverage_status) {
    case "covered":
      return "pass";
    case "partial":
      return "circle-outline";
    default:
      return "circle-slash";
  }
}

function itemColor(item: TraceLinkedItem): vscode.ThemeColor | undefined {
  if (item.is_defect) return new vscode.ThemeColor("list.warningForeground");
  switch (item.coverage_status) {
    case "covered":
      return new vscode.ThemeColor("testing.iconPassed");
    case "partial":
      return new vscode.ThemeColor("list.warningForeground");
    default:
      return new vscode.ThemeColor("list.errorForeground");
  }
}

// ---------------------------------------------------------------------------
// Tree data provider
// ---------------------------------------------------------------------------

export class RequirementTreeProvider
  implements vscode.TreeDataProvider<RequirementTreeItem>
{
  private _onDidChangeTreeData = new vscode.EventEmitter<
    RequirementTreeItem | undefined | void
  >();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  private itemMap = new Map<string, TraceLinkedItem>();
  /** Map from item ID → IDs of items that cover it (incoming). */
  private childrenMap = new Map<string, string[]>();
  /** Root items: specs that don't cover anything else. */
  private rootIds: string[] = [];

  constructor(private traceEngine: TraceEngine) {
    traceEngine.onDidUpdate((result) => {
      this.buildTree(result);
      this._onDidChangeTreeData.fire();
    });

    // Initialize from existing result
    if (traceEngine.result) {
      this.buildTree(traceEngine.result);
    }
  }

  private buildTree(result: TraceResult): void {
    this.itemMap.clear();
    this.childrenMap.clear();
    this.rootIds = [];

    for (const item of result.items) {
      this.itemMap.set(item.id, item);
    }

    // Build children map: for each item, find which items cover it
    for (const item of result.items) {
      for (const coveredId of item.covers) {
        const children = this.childrenMap.get(coveredId) ?? [];
        children.push(item.id);
        this.childrenMap.set(coveredId, children);
      }
    }

    // Root items are those that don't cover anything (top-level specs)
    for (const item of result.items) {
      if (item.covers.length === 0) {
        this.rootIds.push(item.id);
      }
    }

    // Sort roots by ID
    this.rootIds.sort();
  }

  refresh(): void {
    if (this.traceEngine.result) {
      this.buildTree(this.traceEngine.result);
    }
    this._onDidChangeTreeData.fire();
  }

  getTreeItem(element: RequirementTreeItem): vscode.TreeItem {
    return element;
  }

  getChildren(
    element?: RequirementTreeItem
  ): RequirementTreeItem[] {
    if (!element) {
      // Root level
      return this.rootIds
        .map((id) => this.makeTreeItem(id))
        .filter(Boolean) as RequirementTreeItem[];
    }

    // Children of a given item
    const childIds = this.childrenMap.get(element.traceItem.id) ?? [];
    const sorted = [...childIds].sort();
    return sorted
      .map((id) => this.makeTreeItem(id))
      .filter(Boolean) as RequirementTreeItem[];
  }

  private makeTreeItem(id: string): RequirementTreeItem | undefined {
    const item = this.itemMap.get(id);
    if (!item) return undefined;
    const hasChildren = (this.childrenMap.get(id) ?? []).length > 0;
    return new RequirementTreeItem(item, hasChildren);
  }
}
