# Design

## Linker Component
`dsn~linker~1`

The linker component shall match items that declare coverage (via Covers) with items that need coverage (via Needs) and establish links between them.

Covers:

  * `req~forward-coverage~1`
  * `req~backward-coverage~1`

Needs: impl

## HTML Reporter
`dsn~html-reporter~1`

The HTML reporter shall render a trace report containing all specification items, their coverage status, and any defects found.

Covers:

  * `req~report-generation~1`

Needs: impl
