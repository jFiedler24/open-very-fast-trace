/**
 * Loads the .ovft.toml configuration file from the workspace root
 * and converts it into VS Code glob patterns for file discovery.
 */

import * as vscode from "vscode";

export interface OvftConfig {
  source_dirs: string[];
  spec_dirs: string[];
  source_patterns: string[];
  exclude_patterns: string[];
  artifact_types: string[];
  verbose: boolean;
  output_dir: string;
}

const DEFAULT_CONFIG: OvftConfig = {
  source_dirs: ["src"],
  spec_dirs: ["docs"],
  source_patterns: [
    "*.rs", "*.adl", "*.atl", "*.java", "*.c", "*.cpp", "*.h", "*.hpp",
    "*.py", "*.js", "*.ts", "*.go", "*.rb", "*.php", "*.sh", "*.sql",
  ],
  exclude_patterns: [
    "target/**", "node_modules/**", ".git/**", "*.tmp", "*.bak",
  ],
  artifact_types: [
    "feat", "req", "arch", "dsn", "impl", "utest", "itest", "stest", "uman", "oman",
  ],
  verbose: false,
  output_dir: "target",
};

/**
 * Minimal TOML parser for flat key = value files (strings, booleans, arrays of strings).
 * Handles comments (#), quoted strings, and multi-line arrays with inline comments.
 */
function parseSimpleToml(text: string): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  // Strip comments (outside of strings) and join continuation lines
  const lines = text.split("\n");
  let buffer = "";

  for (const raw of lines) {
    // Remove inline comments: find # that is outside quotes
    let line = "";
    let inString = false;
    for (let i = 0; i < raw.length; i++) {
      const ch = raw[i];
      if (ch === '"') inString = !inString;
      if (ch === "#" && !inString) break;
      line += ch;
    }
    buffer += line.trim() + " ";

    // If we have an unclosed bracket, keep accumulating
    const open = (buffer.match(/\[/g) || []).length;
    const close = (buffer.match(/\]/g) || []).length;
    if (open > close) continue;

    const trimmed = buffer.trim();
    buffer = "";
    if (!trimmed || !trimmed.includes("=")) continue;

    const eqIdx = trimmed.indexOf("=");
    const key = trimmed.slice(0, eqIdx).trim();
    const val = trimmed.slice(eqIdx + 1).trim();

    if (val === "true") {
      result[key] = true;
    } else if (val === "false") {
      result[key] = false;
    } else if (val.startsWith('"') && val.endsWith('"')) {
      result[key] = val.slice(1, -1);
    } else if (val.startsWith("[")) {
      // Parse array of strings
      const inner = val.slice(1, val.lastIndexOf("]")).trim();
      if (!inner) {
        result[key] = [];
      } else {
        const items: string[] = [];
        const re = /"([^"]*?)"/g;
        let m;
        while ((m = re.exec(inner)) !== null) {
          items.push(m[1]);
        }
        result[key] = items;
      }
    }
  }

  return result;
}

/**
 * Try to load .ovft.toml from the workspace root.
 * Returns the parsed config merged with defaults, or just defaults if no file found.
 */
export async function loadOvftConfig(): Promise<OvftConfig> {
  const workspaceFolders = vscode.workspace.workspaceFolders;
  if (!workspaceFolders || workspaceFolders.length === 0) {
    return { ...DEFAULT_CONFIG };
  }

  // Search for .ovft.toml in each workspace folder, use the first found
  for (const folder of workspaceFolders) {
    const configUri = vscode.Uri.joinPath(folder.uri, ".ovft.toml");
    try {
      const bytes = await vscode.workspace.fs.readFile(configUri);
      const text = Buffer.from(bytes).toString("utf-8");
      const parsed = parseSimpleToml(text);
      return {
        source_dirs: asStringArray(parsed.source_dirs) ?? DEFAULT_CONFIG.source_dirs,
        spec_dirs: asStringArray(parsed.spec_dirs) ?? DEFAULT_CONFIG.spec_dirs,
        source_patterns: asStringArray(parsed.source_patterns) ?? DEFAULT_CONFIG.source_patterns,
        exclude_patterns: asStringArray(parsed.exclude_patterns) ?? DEFAULT_CONFIG.exclude_patterns,
        artifact_types: asStringArray(parsed.artifact_types) ?? DEFAULT_CONFIG.artifact_types,
        verbose: typeof parsed.verbose === "boolean" ? parsed.verbose : DEFAULT_CONFIG.verbose,
        output_dir: typeof parsed.output_dir === "string" ? parsed.output_dir : DEFAULT_CONFIG.output_dir,
      };
    } catch {
      // File not found or parse error — continue to next folder or use defaults
    }
  }

  return { ...DEFAULT_CONFIG };
}

function asStringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  if (value.every((v) => typeof v === "string")) return value as string[];
  return undefined;
}

/**
 * Convert source_dirs x source_patterns into glob patterns for
 * vscode.workspace.findFiles.
 *
 * Example: dirs=["src"] + patterns=["*.rs"] => ["src/ ** / *.rs"]
 */
export function buildSourceGlobs(config: OvftConfig): string[] {
  const globs: string[] = [];
  for (const dir of config.source_dirs) {
    for (const pattern of config.source_patterns) {
      globs.push(`${dir}/**/${pattern}`);
    }
  }
  return globs;
}

/**
 * Convert spec_dirs into glob patterns for markdown files.
 */
export function buildSpecGlobs(config: OvftConfig): string[] {
  return config.spec_dirs.map((dir) => `${dir}/**/*.md`);
}

/**
 * Convert exclude_patterns into a single VS Code exclusion glob string.
 */
export function buildExcludeGlob(config: OvftConfig): string {
  const patterns = config.exclude_patterns.map((p) =>
    p.startsWith("**/") ? p : `**/${p}`
  );
  return `{${patterns.join(",")}}`;
}
