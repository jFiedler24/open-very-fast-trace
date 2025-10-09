# cargo-ovft

A Cargo plugin for Open Very Fast Trace (OVFT) - requirements traceability for Rust projects.

## Installation

Install the cargo plugin:

```bash
cargo install cargo-ovft
```

Or build from source in this workspace:

```bash
cargo build -p cargo-ovft
```

## Usage

Run requirements traceability analysis on your Rust project:

```bash
# Basic usage - analyze current project
cargo ovft

# Specify custom input directory and output file
cargo ovft --input docs/requirements --output trace_report.html

# Generate JSON output instead of HTML
cargo ovft --format json --output trace_report.json

# Verbose output
cargo ovft --verbose

# Check mode - exit with error code if issues found (useful for CI)
cargo ovft --check
```

## Commands

### `cargo ovft`

Runs requirements traceability analysis on the current Cargo project.

**Options:**
- `-i, --input <DIR>` - Input directory containing requirements files (default: ".")
- `-o, --output <FILE>` - Output HTML report file (default: "requirements_report.html")
- `-f, --format <FORMAT>` - Output format: "html" or "json" (default: "html")
- `-v, --verbose` - Enable verbose output
- `-c, --check` - Check for issues and return non-zero exit code if found

## Integration with CI/CD

Use the `--check` flag in your CI pipeline to fail builds when requirements traceability issues are found:

```yaml
# GitHub Actions example
- name: Check requirements traceability
  run: cargo ovft --check
```

## Requirements Format

The plugin supports the OpenFastTrace format for requirements. Create requirements in Markdown files:

```markdown
# Feature Requirements

## User Authentication
`feat~user-auth~1`

Users shall be able to authenticate using username and password.

Needs: dsn

## Login Process  
`feat~login-process~1`

The system shall provide a login form.

Needs: dsn
Covers: feat~user-auth~1
```

And reference them in your Rust code:

```rust
// [impl->feat~login-process~1]
fn create_login_form() -> LoginForm {
    // Implementation
}
```

## About

This cargo plugin is part of the Open Very Fast Trace project, which brings OpenFastTrace-compatible requirements traceability to Rust projects.
