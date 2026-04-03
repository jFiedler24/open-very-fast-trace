# Features

## Tracing
`feat~tracing~1`

The system shall support requirements tracing across multiple artifact types.

Needs: req

## Reporting
`feat~reporting~1`

The system shall generate human-readable reports showing requirement coverage status.

Needs: req

# Requirements

## Forward Coverage
`req~forward-coverage~1`

When a user traces a set of requirements, OVFT shall determine which specification items are covered by items on the next lower level.

Covers:

  * [feat~tracing~1](#tracing)

Needs: dsn

## Backward Coverage
`req~backward-coverage~1`

OVFT shall allow users to navigate from a low-level item to the requirement it covers.

Covers:

  * [feat~tracing~1](#tracing)

Needs: dsn

## Report Generation
`req~report-generation~1`

OVFT shall generate an HTML report that contains the overall coverage status and details for every specification item.

Covers:

  * [feat~reporting~1](#reporting)

Needs: dsn
