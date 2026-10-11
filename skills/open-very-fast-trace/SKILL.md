---
name: open-very-fast-trace
description: Requirements traceability for Rust projects with Open Very Fast Trace (OVFT) — tag code with [impl->req~…~1] links, write OFT-style markdown specs, generate coverage reports, gate CI with cargo ovft --check, and link customer requirements (creq) to derived requirements (feat/req/dsn/impl/utest). Use when a project uses ovft-core, cargo-ovft, .ovft.toml, or asks for requirements tracing, coverage reports, or customer-requirement trace links.
---

# Open Very Fast Trace (OVFT)

Rust-native requirements tracing, compatible with
[OpenFastTrace](https://github.com/itsallcode/openfasttrace) syntax.
Repo: <https://github.com/jFiedler24/open-very-fast-trace> ·
Crates: `ovft-core` (library), `cargo-ovft` (CLI).

The idea: every requirement lives in a markdown spec, every code location
that implements it carries a one-line tag, and the tracer proves that
nothing is orphaned and nothing required is unimplemented. It answers
"where is this requirement implemented?" and "why does this code exist?"
in both directions.

## Setup

```bash
cargo install cargo-ovft        # CLI for reports and CI
```

`.ovft.toml` in the project root:

```toml
source_dirs   = ["src"]                    # scanned for tags
spec_dirs     = ["docs/requirements"]      # markdown spec items
source_patterns = ["*.rs"]
artifact_types = ["creq", "feat", "req", "arch", "dsn",
                  "impl", "utest", "itest", "stest"]
```

Run `cargo ovft` (HTML report, default `requirements_report.html`), or
`cargo ovft --check` which exits non-zero on defects — the CI gate.
`cargo ovft release-notes [--from <git-ref>]` summarizes requirements that
became covered since a tag (default: latest tag) as markdown release notes.
`cargo ovft reqif import <file> [--map "Customer Requirement=creq"]` turns a
ReqIF document (`.reqif` or zipped `.reqifz`) into a generated markdown spec;
`cargo ovft reqif export` writes items and trace links back out as ReqIF
(round-trip preserves IDs).
Config resolution: CLI flags > `--config file` > `.ovft.toml` > defaults.

Optional build-time integration (report refreshed on every build, warning
only; `OVFT_STRICT=1` makes it a hard gate):

```rust,ignore
// build.rs — [build-dependencies] ovft-core = "0.1"
use ovft_core::{Config, Tracer};
let config = Config::find_and_load_config(std::path::Path::new("."))
    .unwrap_or_else(Config::default);
let tracer = Tracer::new(config);
let result = tracer.trace()?;
tracer.generate_html_report(&result, std::path::Path::new("target/requirements_report.html"))?;
```

## The chain: customer requirement → own requirements

This is the discipline that makes tracing valuable. Use the artifact
types as **direction of derivation**, not as decoration:

```
creq  (customer's words — INPUT, owned by the customer)
  ↑ covered by
feat  (a product feature we offer — one or more per creq)
  ↑ covered by
req   (a derived, testable requirement — the engineering refinement)
  ↑ covered by
dsn   (a design decision, when the how is non-obvious)
  ↑ covered by
impl  (a place in the code)
  ↑ verified by
utest / itest / stest
```

Rules that keep the graph honest:

1. **Never rewrite a `creq`.** Quote the customer, cite the source
   (ticket, email, `FeaturesAndBugs.md`). If the customer changes their
   mind: bump the revision (`creq~x~2`) and keep the old one — history
   is traceable too.
2. **Every `creq` is covered by at least one `feat`.** A creq nobody
   covers is an unmet promise; the tracer reports it.
3. **`feat` items say WHAT the product does, in user language. `req`
   items say WHAT MUST HOLD, in testable language** ("shall", measurable,
   no design details). One feat typically refines into 1–3 reqs.
4. **Code never points at `creq` or `feat` directly** — only at `req`
   (or `dsn`). This keeps code comments stable while customer wording
   churns above them.
5. **`Needs:` declares what must cover you; `Covers:` declares what you
   cover.** Both directions are checked — an item that needs `impl` but
   has no `[impl->…]` tag anywhere is a defect, and a `Covers:` pointing
   at a nonexistent ID is a defect.
6. **Revision bumps (`~1` → `~2`)**: when the meaning of an item changes,
   bump it and update everything that covers/needs it. The tracer flags
   stale/outdated references. Pure clarifications of wording (same
   meaning) may keep the revision.

Worked example (from the aetherium project):

```markdown
<!-- docs/requirements/customer.md — owned by the customer -->
## creq~login-without-typing-passwords~1
Connecting must work without typing a password every time — use the
system's SSH keys and agent, like the normal `ssh` command does.
On Windows that means both the OpenSSH agent and Pageant.
```

```markdown
<!-- docs/requirements/connection.md — our derivation -->
## feat~silent-key-auth~1

**Title:** Silent key-based authentication

**Description:** Connecting shall first try key-based authentication
without prompting (ssh-agent, then default key files), falling back to
the stored password only when key auth does not succeed.

**Covers:** creq~login-without-typing-passwords~1
**Needs:** req, impl

## req~agent-auth~1

**Title:** Platform ssh-agent authentication

**Description:** Agent authentication shall support the platform
ssh-agent: Unix domain socket via `SSH_AUTH_SOCK`, and on Windows the
OpenSSH agent named pipe falling back to Pageant.

**Aliases:** SSH-101

**Covers:** feat~silent-key-auth~1
**Needs:** impl
```

```rust
// src/session.rs — [impl->req~agent-auth~1]
async fn connect_agent() -> … { … }
```

Result: customer sentence → feature → requirement → code, all machine-
verified. When the customer says "also support YubiKey PIV", that is a
new `creq` (or a `creq~…~2`), a new/extended feat, new reqs, new tags —
and `cargo ovft --check` stays green only when every link exists.

## Tag syntax

In source files (any comment, any language the regex sees):

```rust
// [impl->req~agent-auth~1]            covers one item
// [utest->req~log-highlighting~1]      a test verifying it
// [impl->dsn~config-system~1]          design-level cover
// [impl->ABC-1234]                     covers via alias (item declares **Aliases:** ABC-1234)
// [[req~standalone-declaration~1]]     declare without link
// [covers:req~legacy-format~1]         OFT-compatible alternate form
```

Aliases resolve case-insensitively to the newest revision; unresolved aliases
are reported as orphaned coverage.

Multiple tags per line/file/function are fine.

In markdown specs:

```markdown
## req~some-requirement~1           ← heading carries the canonical ID

**Title:** Short human-readable title  ← shown as the heading in the HTML
                                          report and in release notes
**Description:** The system shall …   ← the testable "shall" statement
**Aliases:** ABC-1234, PROJ-77        ← short external IDs usable as tag targets
**Needs:** impl, utest               ← artifact types that must cover this
**Covers:** feat~parent-feature~1    ← items this derives from
**Tags:** security, performance      ← free-form filter labels
**Status:** approved                 ← draft | proposed | approved | rejected
**Rationale:** …                     ← why this exists
```

Always give an item a `**Title:**` — reports, release notes and the VS Code
tree lead with the title and render the machine ID as a secondary badge.
If the title is omitted it is derived from the ID name
(`req~agent-auth~1` → "Agent auth"), which reads poorly for longer names.
Declare `**Aliases:**` for IDs the outside world uses (ticket keys, customer
references); tags may then point at the alias instead of the full ID.

An item ends at the next ID-carrying heading. Put `creq` files in their
own document so ownership stays obvious.

### Readable documents: keep the ID out of the heading

The heading does not have to carry the ID. Prefer a human-readable heading
with the ID in backticks on the line directly below — the heading becomes
the item's title, and readers never see the machine ID:

```markdown
## Validate user credentials

`req~validate-credentials~1`

**Description:** The system shall validate username/password …
**Covers:** feat~user-login~1
**Needs:** dsn
```

Rules: the backtick ID line must stand alone; inline mentions like
``[`req~x~1`](other.md)`` in prose are cross-references, not definitions
(so an item is defined exactly once). When the heading does carry the ID
(`## req~validate-credentials~1`), set `**Title:**` explicitly — reports,
release notes and the VS Code tree lead with the title and show the ID as
a badge.

## Defects and how to fix them

`cargo ovft --check` reports these — always fix by editing links, not by
weakening `Needs:` to make the report green:

- **UncoveredItem** — an item's `Needs:` has no covering tag. Fix: add
  the tag in code, or split/merge the requirement.
- **Orphan** / dangling `Covers:` — the covered ID doesn't exist. Fix:
  check spelling/revision; if the target was renamed, update the link.
- **OutdatedRevision** — a link points at `~1` but only `~2` exists.
  Fix: bump the reference (and confirm the new revision still applies).
- **AmbiguousItem** — duplicate ID. Fix: rename or bump one instance.

Anti-pattern: deleting `Needs:` lines to silence a defect you don't
want to satisfy. That silently breaks the contract; either implement,
or mark the item `**Status:** rejected` with a rationale.

## Workflow when adding a feature

1. Customer asks → add/confirm the `creq` in the customer file.
2. Add the `feat` covering it; add `req`s the feat needs.
3. Implement; tag the implementing functions with `[impl->req~…~1]`,
   tests with `[utest->req~…~1]`.
4. `cargo ovft --check` → green → commit specs and code together.

## CI

```yaml
- run: cargo install cargo-ovft
- run: cargo ovft --check --output requirements_report.html
- uses: actions/upload-artifact@v4
  if: always()
  with: { name: requirements-report, path: requirements_report.html }
```

## Reference projects

- aetherium (jFiedler24/aetherium): `.ovft.toml`, `docs/requirements/`
  with `customer.md` + per-area files, tags throughout `src/`, build.rs
  report refresh, `.github/workflows/requirements.yml` — a complete,
  green example of the creq→feat→req→impl discipline.
