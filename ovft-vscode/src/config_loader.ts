/**
 * Loads the .ovft.toml configuration file from the workspace root
 * and converts it into VS Code glob patterns for file discovery.
 */

import * as vscode from "vscode";
import { parse } from "smol-toml";

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
      const parsed = parse(text);
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
