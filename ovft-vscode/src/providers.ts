/**
 * VS Code providers for OVFT requirement tracing.
 *
 * Registers as a DefinitionProvider and ReferenceProvider but scoped to
 * requirement IDs/tags only — returns nothing when the cursor isn't on
 * a requirement, so it doesn't interfere with language-specific LSPs.
 *
 * Also provides CodeLens annotations and document link support.
 */

import * as vscode from "vscode";
import {
  RequirementIndex,
  detectAtPosition,
  detectAllOnLine,
  reqIdToString,
  parseReqId,
  RequirementId,
  SpecLocation,
  TagLocation,
} from "./index";
import type { TraceEngine } from "./trace_engine";

// ---------------------------------------------------------------------------
// Definition provider  —  from tag → go to spec, from spec → go to spec
// ---------------------------------------------------------------------------

export class OvftDefinitionProvider implements vscode.DefinitionProvider {
  constructor(private index: RequirementIndex) {}

  provideDefinition(
    document: vscode.TextDocument,
    position: vscode.Position,
    _token: vscode.CancellationToken
  ): vscode.Definition | undefined {
    const detected = detectAtPosition(document, position);
    if (!detected) return undefined;

    const specs = this.index.findSpec(detected.id);
    if (specs.length === 0) return undefined;

    return specs.map(
      (s) => new vscode.Location(s.uri, new vscode.Position(s.line, 0))
    );
  }
}

// ---------------------------------------------------------------------------
// Reference provider  —  find all tags & specs referencing a requirement
// ---------------------------------------------------------------------------

export class OvftReferenceProvider implements vscode.ReferenceProvider {
  constructor(private index: RequirementIndex) {}

  provideReferences(
    document: vscode.TextDocument,
    position: vscode.Position,
    _context: vscode.ReferenceContext,
    _token: vscode.CancellationToken
  ): vscode.Location[] | undefined {
    const detected = detectAtPosition(document, position);
    if (!detected) return undefined;

    const { specs, tags } = this.index.findRelated(detected.id);
    const locations: vscode.Location[] = [];

    for (const s of specs) {
      locations.push(new vscode.Location(s.uri, new vscode.Position(s.line, 0)));
    }
    for (const t of tags) {
      locations.push(new vscode.Location(t.uri, new vscode.Position(t.line, 0)));
    }

    // Also find specs where this ID appears in Covers/Depends lists
    for (const [, specList] of this.index.specs) {
      for (const spec of specList) {
        const idStr = reqIdToString(detected.id);
        const coversMatch = spec.covers.some((c) => reqIdToString(c) === idStr);
        const dependsMatch = spec.depends.some((d) => reqIdToString(d) === idStr);
        if (coversMatch || dependsMatch) {
          const already = locations.some(
            (l) => l.uri.toString() === spec.uri.toString() && l.range.start.line === spec.line
          );
          if (!already) {
            locations.push(new vscode.Location(spec.uri, new vscode.Position(spec.line, 0)));
          }
        }
      }
    }

    return locations.length > 0 ? locations : undefined;
  }
}

// ---------------------------------------------------------------------------
// CodeLens provider  —  show "N coverage tags | Go to spec" above IDs
// ---------------------------------------------------------------------------

export class OvftCodeLensProvider implements vscode.CodeLensProvider {
  private _onDidChange = new vscode.EventEmitter<void>();
  readonly onDidChangeCodeLenses = this._onDidChange.event;

  constructor(private index: RequirementIndex, private traceEngine: TraceEngine) {
    index.onDidUpdate(() => this._onDidChange.fire());
  }

  provideCodeLenses(
    document: vscode.TextDocument,
    _token: vscode.CancellationToken
  ): vscode.CodeLens[] {
    const config = vscode.workspace.getConfiguration("ovft");
    if (!config.get("codeLensEnabled", true)) return [];

    const lenses: vscode.CodeLens[] = [];

    for (let i = 0; i < document.lineCount; i++) {
      const items = detectAllOnLine(document, i);
      for (const item of items) {
        const specs = this.index.findSpec(item.id);
        const tags = this.index.findCoverageTags(item.id);
        const idStr = reqIdToString(item.id);

        if (item.isTag) {
          // On a coverage tag: show link to spec definition
          if (specs.length > 0) {
            lenses.push(
              new vscode.CodeLens(item.range, {
                title: `⇒ ${idStr} (${specs.length} spec${specs.length > 1 ? "s" : ""})`,
                command: "ovft.followRequirement",
                arguments: [document.uri, new vscode.Position(i, item.range.start.character)],
              })
            );
          } else {
            lenses.push(
              new vscode.CodeLens(item.range, {
                title: `⚠ ${idStr} — spec not found`,
                command: "",
              })
            );
          }
        } else {
          // On a spec ID: show coverage summary using trace engine data
          const traceItem = this.traceEngine.findItem(idStr);
          const needsList = specs.flatMap((s) => s.needs);
          const uniqueNeeds = [...new Set(needsList)];

          // Collect coverage from both source tags AND spec-to-spec (Covers:) links
          const tagCoveringTypes = [...new Set(tags.map((t) => t.coveringType))];
          const specCoveringTypes: string[] = [];
          if (traceItem) {
            for (const link of traceItem.incoming_links) {
              if (link.source_id) {
                const parsed = parseReqId(link.source_id);
                if (parsed && !tagCoveringTypes.includes(parsed.artifactType)) {
                  specCoveringTypes.push(parsed.artifactType);
                }
              }
            }
          }
          const allCoveringTypes = [...new Set([...tagCoveringTypes, ...specCoveringTypes])];
          const missingTypes = uniqueNeeds.filter((n) => !allCoveringTypes.includes(n));
          const totalCoverageCount = tags.length + (traceItem ? traceItem.incoming_links.filter((l) => {
            if (!l.source_id) return false;
            const p = parseReqId(l.source_id);
            return p && !tags.some((t) => t.coveringId && reqIdToString(t.coveringId) === l.source_id);
          }).length : 0);

          // Use trace engine's authoritative coverage status when available
          const coverageStatus = traceItem?.coverage_status;

          let title: string;
          if (coverageStatus === "covered") {
            // Build a summary of what's providing coverage
            const parts: string[] = [];
            if (tags.length > 0) {
              parts.push(`${tags.length} tag${tags.length !== 1 ? "s" : ""}`);
            }
            if (specCoveringTypes.length > 0) {
              parts.push(`${specCoveringTypes.length} spec${specCoveringTypes.length !== 1 ? "s" : ""}`);
            }
            title = `✓ ${idStr} — covered (${parts.join(", ")})`;
          } else if (coverageStatus === "partial" || missingTypes.length > 0) {
            const parts: string[] = [];
            if (tags.length > 0) parts.push(`${tags.length} tag${tags.length !== 1 ? "s" : ""}`);
            if (specCoveringTypes.length > 0) parts.push(`${specCoveringTypes.length} spec${specCoveringTypes.length !== 1 ? "s" : ""}`);
            const covPart = parts.length > 0 ? ` (${parts.join(", ")})` : "";
            title = `◐ ${idStr} — partial${covPart}, missing: ${missingTypes.join(", ")}`;
          } else if (uniqueNeeds.length > 0 && totalCoverageCount === 0) {
            title = `⚠ ${idStr} — uncovered (needs: ${uniqueNeeds.join(", ")})`;
          } else if (totalCoverageCount > 0) {
            title = `✓ ${idStr} — ${totalCoverageCount} coverage link${totalCoverageCount !== 1 ? "s" : ""}`;
          } else {
            title = `${idStr}`;
          }

          lenses.push(
            new vscode.CodeLens(item.range, {
              title,
              command: "ovft.findCoverage",
              arguments: [document.uri, new vscode.Position(i, item.range.start.character)],
            })
          );
        }
      }
    }

    return lenses;
  }
}

// ---------------------------------------------------------------------------
// Document highlight provider — highlight matching IDs in the same file
// ---------------------------------------------------------------------------

export class OvftDocumentHighlightProvider implements vscode.DocumentHighlightProvider {
  constructor(private index: RequirementIndex) {}

  provideDocumentHighlights(
    document: vscode.TextDocument,
    position: vscode.Position,
    _token: vscode.CancellationToken
  ): vscode.DocumentHighlight[] | undefined {
    const detected = detectAtPosition(document, position);
    if (!detected) return undefined;

    const idStr = reqIdToString(detected.id);
    const highlights: vscode.DocumentHighlight[] = [];

    for (let i = 0; i < document.lineCount; i++) {
      const items = detectAllOnLine(document, i);
      for (const item of items) {
        if (reqIdToString(item.id) === idStr) {
          highlights.push(
            new vscode.DocumentHighlight(
              item.range,
              item.isTag
                ? vscode.DocumentHighlightKind.Write
                : vscode.DocumentHighlightKind.Read
            )
          );
        }
      }
    }

    return highlights.length > 0 ? highlights : undefined;
  }
}

// ---------------------------------------------------------------------------
// Hover provider — show requirement info on hover
// ---------------------------------------------------------------------------

export class OvftHoverProvider implements vscode.HoverProvider {
  constructor(private index: RequirementIndex, private traceEngine: TraceEngine) {}

  provideHover(
    document: vscode.TextDocument,
    position: vscode.Position,
    _token: vscode.CancellationToken
  ): vscode.Hover | undefined {
    const detected = detectAtPosition(document, position);
    if (!detected) return undefined;

    const idStr = reqIdToString(detected.id);
    const specs = this.index.findSpec(detected.id);
    const tags = this.index.findCoverageTags(detected.id);
    const traceItem = this.traceEngine.findItem(idStr);

    const md = new vscode.MarkdownString();
    md.isTrusted = true;
    md.supportThemeIcons = true;

    md.appendMarkdown(`**OVFT Requirement: \`${idStr}\`**\n\n`);

    // Show title and description from the trace result
    if (traceItem?.title) {
      md.appendMarkdown(`### ${traceItem.title}\n\n`);
    }
    if (traceItem?.description) {
      md.appendMarkdown(`${traceItem.description}\n\n`);
    }

    if (specs.length > 0) {
      md.appendMarkdown(`**Defined in:**\n`);
      for (const s of specs) {
        const relPath = vscode.workspace.asRelativePath(s.uri);
        md.appendMarkdown(`- ${relPath}:${s.line + 1}\n`);
        if (s.needs.length > 0) {
          md.appendMarkdown(`  - Needs: ${s.needs.join(", ")}\n`);
        }
        if (s.covers.length > 0) {
          md.appendMarkdown(`  - Covers: ${s.covers.map(reqIdToString).join(", ")}\n`);
        }
      }
    } else {
      md.appendMarkdown(`*No specification found*\n`);
    }

    if (tags.length > 0) {
      md.appendMarkdown(`\n**Coverage tags (${tags.length}):**\n`);
      for (const t of tags) {
        const relPath = vscode.workspace.asRelativePath(t.uri);
        md.appendMarkdown(`- \`${t.coveringType}\` in ${relPath}:${t.line + 1}\n`);
      }
    } else {
      md.appendMarkdown(`\n*No coverage tags found*\n`);
    }

    md.appendMarkdown(
      `\n[Follow Requirement](command:ovft.followRequirement?${encodeURIComponent(
        JSON.stringify([document.uri.toString(), { line: position.line, character: position.character }])
      )}) | [Find Coverage](command:ovft.findCoverage?${encodeURIComponent(
        JSON.stringify([document.uri.toString(), { line: position.line, character: position.character }])
      )})`
    );

    return new vscode.Hover(md, detected.range);
  }
}

// ---------------------------------------------------------------------------
// Helper: build quick-pick items for navigation
// ---------------------------------------------------------------------------

export function specsToLocations(specs: SpecLocation[]): vscode.Location[] {
  return specs.map((s) => new vscode.Location(s.uri, new vscode.Position(s.line, 0)));
}

export function tagsToLocations(tags: TagLocation[]): vscode.Location[] {
  return tags.map((t) => new vscode.Location(t.uri, new vscode.Position(t.line, 0)));
}

export async function navigateToLocations(
  locations: vscode.Location[],
  label: string
): Promise<void> {
  if (locations.length === 0) {
    vscode.window.showInformationMessage(`OVFT: No ${label} found.`);
    return;
  }

  if (locations.length === 1) {
    const loc = locations[0];
    const doc = await vscode.workspace.openTextDocument(loc.uri);
    const editor = await vscode.window.showTextDocument(doc);
    editor.selection = new vscode.Selection(loc.range.start, loc.range.start);
    editor.revealRange(loc.range, vscode.TextEditorRevealType.InCenter);
    return;
  }

  // Multiple results — use peek view
  const activeEditor = vscode.window.activeTextEditor;
  if (activeEditor) {
    await vscode.commands.executeCommand(
      "editor.action.peekLocations",
      activeEditor.document.uri,
      activeEditor.selection.active,
      locations,
      "peek"
    );
  }
}
