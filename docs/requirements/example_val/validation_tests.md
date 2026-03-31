# Validation Test Requirements — Data Validation Example

## val~field-length-boundary~1

**Title:** Test field length boundaries

**Description:** Integration tests shall verify that inputs at, below, and above the maximum field length are correctly accepted or rejected.

**Covers:** req~validate-field-lengths~1

**Rationale:** Boundary testing is essential to ensure off-by-one errors are caught.

**Tags:** validation-test, boundary

**Needs:** itest

---

## val~email-format-patterns~1

**Title:** Test email format patterns

**Description:** Integration tests shall verify that valid and invalid email addresses are correctly classified.

**Covers:** req~validate-field-formats~1

**Rationale:** Email format validation is a common source of bugs, requiring dedicated test coverage.

**Tags:** validation-test, email

**Needs:** itest
