# Requirements — Data Validation Example

## req~validate-field-lengths~1

**Title:** Validate field lengths

**Description:** The system shall reject any input where string fields exceed their maximum length constraints.

**Covers:** feat~data-validation~1

**Rationale:** Length validation prevents buffer overflows and database truncation errors.

**Tags:** validation, fields, length

**Needs:** val

---

## req~validate-field-formats~1

**Title:** Validate field formats

**Description:** The system shall verify that fields like email, date, and phone match their expected format patterns.

**Covers:** feat~data-validation~1

**Rationale:** Format validation catches typos and malformed data early.

**Tags:** validation, fields, format

**Needs:** val
