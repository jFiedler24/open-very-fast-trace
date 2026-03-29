/**
 * OVFT Diagnostics Provider — shows requirement tracing defects
 * (uncovered items, orphaned coverage, duplicates, wrong revisions)
 * in the VS Code Problems panel.
 */

import * as vscode from "vscode";
import { TraceResult, TraceDefect } from "./trace_engine";

export class OvftDiagnosticsProvider implements vscode.Disposable {
  private collection: vscode.DiagnosticCollection;

  constructor() {
    this.collection = vscode.languages.createDiagnosticCollection("ovft");
  }

  /**
   * Update diagnostics from a trace result.
   */
  update(result: TraceResult): void {
    this.collection.clear();

    // Group defects by file
    const byFile = new Map<string, TraceDefect[]>();
    for (const defect of result.defects) {
      const path = defect.file_path ?? "__unknown__";
      const arr = byFile.get(path) ?? [];
      arr.push(defect);
      byFile.set(path, arr);
    }

    for (const [filePath, defects] of byFile) {
      if (filePath === "__unknown__") continue;

      const uri = vscode.Uri.file(filePath);
      const diagnostics: vscode.Diagnostic[] = defects.map((d) => {
        // Line is 1-based from Rust; VS Code expects 0-based
        const line = d.line ? d.line - 1 : 0;
        const range = new vscode.Range(line, 0, line, Number.MAX_SAFE_INTEGER);
        const severity = defectSeverity(d.defect_type);
        const diag = new vscode.Diagnostic(range, d.description, severity);
        diag.source = "OVFT";
        diag.code = d.defect_type;
        return diag;
      });

      this.collection.set(uri, diagnostics);
    }
  }

  clear(): void {
    this.collection.clear();
  }

  dispose(): void {
    this.collection.dispose();
  }
}

function defectSeverity(defectType: string): vscode.DiagnosticSeverity {
  switch (defectType) {
    case "uncovered":
      return vscode.DiagnosticSeverity.Warning;
    case "orphaned":
      return vscode.DiagnosticSeverity.Warning;
    case "duplicate":
      return vscode.DiagnosticSeverity.Error;
    case "wrong-revision":
      return vscode.DiagnosticSeverity.Error;
    case "circular-dependency":
      return vscode.DiagnosticSeverity.Error;
    default:
      return vscode.DiagnosticSeverity.Information;
  }
}
