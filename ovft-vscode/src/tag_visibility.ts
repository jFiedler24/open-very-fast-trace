/**
 * Tag visibility manager — hides raw OVFT tag lines and shows a subtle
 * inline badge (e.g. `⇄ req~agent-auth~1`) instead.
 *
 * A line is hidden only when its entire content (ignoring leading
 * whitespace and comment markers like `//`, `#`, `/*`, `*`, `--`, `;`)
 * consists of one or more OVFT tags. Code lines that merely end with a
 * tag are never hidden.
 *
 * Hidden lines are temporarily revealed while the cursor/selection
 * intersects them, so they remain editable.
 */

import * as vscode from "vscode";

// Full tag shape: artifact -> type~name~rev, optionally with >>needs,list
const FULL_TAG = String.raw`\[\s*[A-Za-z][\w-]*\s*->\s*[A-Za-z][\w-]*~[\w.-]+~\d+\s*(?:>>\s*[A-Za-z][\w-]*(?:\s*,\s*[A-Za-z][\w-]*)*)?\s*\]`;
// Short tag shape: [[ type~name~rev ]] or [[ type~name~rev:artifact ]]
const SHORT_TAG = String.raw`\[\[\s*[A-Za-z][\w-]*~[\w.-]+~\d+\s*(?::\s*[A-Za-z][\w-]*)?\s*\]\]`;
const TAG = String.raw`(?:${FULL_TAG}|${SHORT_TAG})`;
// Comment markers allowed before/around tags: // //! # /* ** * -- ;
const COMMENT_MARK = String.raw`(?:\/\/!?|\/\*+|\*+|--|;|#)`;

/** Matches a line whose entire content is comment syntax + OVFT tags. */
const TAG_LINE_RE = new RegExp(
  String.raw`^\s*(?:${COMMENT_MARK}\s*)*(?:${TAG}\s*)+(?:\*\/\s*)?$`
);

/** Extracts covered requirement IDs (`type~name~version`) from a line. */
const COVERED_ID_RE = /[A-Za-z][\w-]*~[\w.-]+~\d+/g;

const REFRESH_DEBOUNCE_MS = 150;

// [impl->req~editor-tag-hiding~1]
export class OvftTagVisibilityManager implements vscode.Disposable {
  private hiddenType: vscode.TextEditorDecorationType;
  private badgeType: vscode.TextEditorDecorationType;
  private disposables: vscode.Disposable[] = [];
  private refreshTimer: ReturnType<typeof setTimeout> | undefined;

  constructor() {
    this.hiddenType = vscode.window.createTextEditorDecorationType({
      textDecoration: "none; display: none;",
    });
    this.badgeType = vscode.window.createTextEditorDecorationType({
      before: {
        color: new vscode.ThemeColor("descriptionForeground"),
        fontStyle: "italic",
      },
    });

    this.disposables.push(
      vscode.window.onDidChangeActiveTextEditor(() => this.scheduleRefresh()),
      vscode.window.onDidChangeVisibleTextEditors(() => this.scheduleRefresh()),
      vscode.window.onDidChangeTextEditorSelection(() => this.scheduleRefresh()),
      vscode.workspace.onDidChangeTextDocument((e) => {
        if (vscode.window.visibleTextEditors.some((ed) => ed.document === e.document)) {
          this.scheduleRefresh();
        }
      }),
      vscode.workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration("ovft.hideTags")) {
          this.refresh();
        }
      })
    );

    this.refresh();
  }

  /** Trigger decoration refresh on all visible editors. */
  refresh(): void {
    for (const editor of vscode.window.visibleTextEditors) {
      this.updateDecorations(editor);
    }
  }

  private scheduleRefresh(): void {
    if (this.refreshTimer) clearTimeout(this.refreshTimer);
    this.refreshTimer = setTimeout(() => {
      this.refreshTimer = undefined;
      this.refresh();
    }, REFRESH_DEBOUNCE_MS);
  }

  private isEnabled(): boolean {
    return vscode.workspace.getConfiguration("ovft").get("hideTags", true);
  }

  private updateDecorations(editor: vscode.TextEditor): void {
    if (!this.isEnabled()) {
      editor.setDecorations(this.hiddenType, []);
      editor.setDecorations(this.badgeType, []);
      return;
    }

    const doc = editor.document;
    const hiddenRanges: vscode.DecorationOptions[] = [];
    const badges: vscode.DecorationOptions[] = [];

    for (let i = 0; i < doc.lineCount; i++) {
      const text = doc.lineAt(i).text;
      if (!text.trim()) continue;
      if (!TAG_LINE_RE.test(text)) continue;

      // Reveal-on-edit: keep lines intersecting the cursor/selection visible
      const touched = editor.selections.some(
        (sel) => i >= sel.start.line && i <= sel.end.line
      );
      if (touched) continue;

      hiddenRanges.push({ range: new vscode.Range(i, 0, i, text.length) });

      COVERED_ID_RE.lastIndex = 0;
      const ids = [...new Set(
        Array.from(text.matchAll(COVERED_ID_RE), (m) => m[0])
      )];
      if (ids.length > 0) {
        badges.push({
          range: new vscode.Range(i, 0, i, 0),
          renderOptions: {
            before: { contentText: `⇄ ${ids.join(", ")}` },
          },
        });
      }
    }

    editor.setDecorations(this.hiddenType, hiddenRanges);
    editor.setDecorations(this.badgeType, badges);
  }

  dispose(): void {
    if (this.refreshTimer) clearTimeout(this.refreshTimer);
    this.hiddenType.dispose();
    this.badgeType.dispose();
    this.disposables.forEach((d) => d.dispose());
  }
}
