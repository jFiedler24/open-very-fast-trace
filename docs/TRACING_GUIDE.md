# Requirement Tracing Patterns

This guide documents the two standard requirement tracing chains supported by OVFT and shows the correct Open Fast Trace (OFT) notation for each.

---

## Chain 1: Feature → Requirement → Design → Implementation + Unit Test

Use this chain when requirements are satisfied directly by source code.

```
feat ──needs──▸ req ──needs──▸ dsn ──needs──▸ impl
                                         └──needs──▸ utest
```

### Feature (features.md)

```markdown
## feat~user-login~1

**Title:** User login

**Description:** The system shall allow users to log in with username and password.

**Needs:** req
```

A feature only needs `req`. The requirement is the next layer down.

### Requirement (requirements.md)

```markdown
## req~validate-credentials~1

**Title:** Validate user credentials

**Description:** The system shall validate username/password against the user database.

**Covers:** feat~user-login~1

**Needs:** dsn
```

- **Covers** points up to the feature it satisfies.
- **Needs** points down — a requirement needs a design (`dsn`).

### Design (design.md)

```markdown
## dsn~credential-checker~1

**Title:** Credential checking module

**Description:** Implement a credential checker that hashes the input password and compares it to the stored hash.

**Covers:** req~validate-credentials~1

**Needs:** impl, utest
```

- **Covers** points up to the requirement.
- **Needs** `impl` (source code implementation) and `utest` (unit test).

### Implementation tag (in source code, e.g. auth.rs)

```rust
/// Validate user credentials against the database.
/// [impl->dsn~credential-checker~1]
fn check_credentials(user: &str, pass: &str) -> bool {
    // ...
}
```

### Unit test tag (in source code)

```rust
/// [utest->dsn~credential-checker~1]
#[test]
fn test_check_credentials_valid() {
    assert!(check_credentials("alice", "correct_password"));
}
```

Both `impl` and `utest` tags **cover** the design spec. The design spec declared it **needs** both, so both must be present for the chain to be defect-free.

---

## Chain 2: Feature → Requirement → Validation → Integration Test

Use this chain when requirements are verified through integration/acceptance tests defined as specification items (not direct source code coverage).

```
feat ──needs──▸ req ──needs──▸ val ──needs──▸ itest
```

### Feature (features.md)

```markdown
## feat~data-validation~1

**Title:** Validate incoming data

**Description:** The system shall validate all incoming data against defined schemas.

**Needs:** req
```

### Requirement (requirements.md)

```markdown
## req~validate-field-lengths~1

**Title:** Validate field lengths

**Description:** The system shall reject input where string fields exceed their maximum length.

**Covers:** feat~data-validation~1

**Needs:** val
```

- **Needs** `val` instead of `dsn` — this requirement is verified by a validation test requirement, not a design spec.

### Validation test requirement (validation_tests.md)

```markdown
## val~field-length-boundary~1

**Title:** Test field length boundaries

**Description:** Integration tests shall verify inputs at, below, and above the maximum field length.

**Covers:** req~validate-field-lengths~1

**Needs:** itest
```

- A `val` item is a **specification item** that describes *what* must be tested.
- It **covers** the requirement and **needs** `itest` (integration test tags in source code).

### Integration test tag (in source code, e.g. integration_tests.rs)

```rust
// [itest->val~field-length-boundary~1]
fn test_field_length_at_max() {
    // Test that a field at exactly max length is accepted
}

// [itest->val~field-length-boundary~1]
fn test_field_length_above_max() {
    // Test that a field exceeding max length is rejected
}
```

Multiple test functions can cover the same `val` item.

---

## Quick Reference

| Layer | Artifact Type | Covers (points up) | Needs (points down) |
|-------|--------------|--------------------|--------------------|
| Feature | `feat` | — | `req` |
| Requirement | `req` | `feat` | `dsn` or `val` |
| Design | `dsn` | `req` | `impl`, `utest` |
| Validation | `val` | `req` | `itest` |
| Implementation | `impl` | `dsn` | — (source tag) |
| Unit test | `utest` | `dsn` | — (source tag) |
| Integration test | `itest` | `val` | — (source tag) |

### Rules

1. **Covers points up.** A child item lists its parent in `Covers:`.
2. **Needs points down.** A parent item lists the artifact type(s) it requires in `Needs:`.
3. **Source tags are leaf nodes.** `impl`, `utest`, and `itest` appear only as tags in source code — they don't have their own markdown spec files.
4. **One Needs per role.** A `req` should need either `dsn` (implementation chain) or `val` (validation chain), not both, unless the requirement genuinely demands both paths.
5. **Custom types must be registered.** Add any custom artifact types (like `val`) to `artifact_types` in `.ovft.toml`.

### Tag Syntax in Source Code

**Full tag** — the tag declares its own type and what it covers:
```
[impl->dsn~credential-checker~1]
[itest->val~field-length-boundary~1]
```

**Short tag** — the covered item is listed first, with the tag type after the colon:
```
[[dsn~credential-checker~1:impl]]
[[val~field-length-boundary~1:itest]]
```

Both forms are equivalent. Place them in comments appropriate for the language (`//`, `#`, `--`, etc.).

---

## Configuration (.ovft.toml)

Ensure your config includes all artifact types used in your chains:

```toml
artifact_types = [
    "feat", "req", "dsn", "impl", "utest",
    "val", "itest", "stest",
]
```

And that both spec and source directories are listed:

```toml
spec_dirs = ["docs/requirements"]
source_dirs = ["src", "tests"]
```

`spec_dirs` are scanned recursively for `.md` files. `source_dirs` are scanned recursively for source files matching `source_patterns`.
