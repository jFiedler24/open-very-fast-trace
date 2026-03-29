# OVFT Requirement Tracing — VS Code Extension

Navigate between requirement specifications (in markdown) and their coverage tags (in source code) for [Open Very Fast Trace](https://github.com/open-very-fast-trace) projects.

## Features

| Feature | Description |
|---------|-------------|
| **Go to Definition** | `Ctrl+Click` / `F12` on a coverage tag jumps to the requirement spec in markdown |
| **Find References** | `Shift+F12` on a requirement ID shows all coverage tags and related specs |
| **Follow Requirement** | `Cmd+Shift+R` (context menu) — navigate to the spec definition |
| **Find Coverage** | `Cmd+Shift+T` (context menu) — find all tags covering a requirement |
| **CodeLens** | Inline annotations showing coverage status above requirement IDs and tags |
| **Hover** | Hover over any requirement ID to see coverage summary with clickable links |
| **Highlight** | All occurrences of the same requirement ID are highlighted in the current file |

## How It Works

The extension indexes your workspace on activation:

- **Specification files** (`*.md`): Scans for requirement IDs in backticks (`` `req~name~1` ``) and parses their `Needs:`, `Covers:`, and `Depends:` metadata.
- **Source files** (`*.rs`, `*.java`, `*.py`, `*.ts`, etc.): Scans for coverage tags in two formats:
  - Full tag: `[impl->dsn~validate-request~1]`
  - Short tag: `[[req~login~1:impl]]`

The index is kept up-to-date via file watchers.

### Non-interference with other LSPs

The extension registers VS Code providers (Definition, References, Hover, etc.) that **return `undefined`** when the cursor is not on a requirement ID or tag. VS Code merges results from all providers, so language-specific LSPs continue to work normally for regular code symbols.

## Requirement ID Format

```
type~name~revision
```

Examples: `req~user-auth~1`, `dsn~login-module~2`, `impl~validate~1`

## Tag Formats

### Full Tag
```
[impl->dsn~validate-authentication-request~1]
[dsn~my-design~1->req~user-login~1>>impl,utest]
```

### Short Tag
```
[[req~login~1:impl]]
```

## Configuration

| Setting | Default | Description |
|---------|---------|-------------|
| `ovft.specFilePatterns` | `["**/*.md"]` | Glob patterns for spec files |
| `ovft.tagFilePatterns` | `["**/*.rs", "**/*.java", ...]` | Glob patterns for source files |
| `ovft.excludePatterns` | `["**/target/**", ...]` | Patterns to exclude |
| `ovft.codeLensEnabled` | `true` | Show CodeLens annotations |

## Development

```bash
cd ovft-vscode
npm install
npm run compile
```

Press `F5` in VS Code to launch an Extension Development Host for testing.

## Building a VSIX

```bash
npm install -g @vscode/vsce
cd ovft-vscode
vsce package
```

This produces `ovft-tracing-0.1.0.vsix` which can be installed via:
```
code --install-extension ovft-tracing-0.1.0.vsix
```
