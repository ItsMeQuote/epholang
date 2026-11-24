# Epholang handbook · 0.3.0 Beta

To run your first program, start with [installation](Installation.md), then work through the [tutorial](Tutorial.md). You will check and run a file, call a typed function and validate input. If you already have a working 0.2 program, use the [migration guide](../MIGRATION.md) first.

## Find a chapter by task

| I want to… | Chapter | Worked example |
| --- | --- | --- |
| Look up valid syntax | [Language reference](Language-Reference.md) | Declarations, precedence, block indentation and interpolation |
| Choose a variable or parameter type | [Types and scope](Types-and-Scope.md) | Represent a missing selection, accept multiple argument types and keep a result after a branch |
| Design a return contract | [Functions and results](Functions-and-Results.md) | Return a position or `null` from a search, write a procedure and discard a result |
| Decide when to stop a loop | [Control flow](Control-Flow.md) | Search a list, skip empty entries, traverse a string backwards and guard a read |
| Change or inspect a sequence | [Lists and Unicode text](Collections-and-Text.md) | Keep the original list, update a copy, split a record and count Unicode scalar values |
| Find a built-in function | [Standard library reference](Standard-Library.md) | Signatures, accepted argument types, conversion targets and boundary conditions |
| Combine library operations | [Standard library in practice](Library-Recipes.md) | Clean a delimited line, sum valid readings and choose exact or floating-point arithmetic |
| Understand a diagnostic | [Diagnostics](Diagnostics.md) | Reproduce a failure, identify the checking stage and repair the program |
| Format or inspect a file | [CLI and VS Code](CLI-Reference.md) | Inspect tokens, check types, run EphoFMT and configure the extension |

Each numbered file in [examples](../examples/) is a standalone program. From the repository root, try `epholang run examples/04_lists.epho`. In the chapters, each complete program can be saved in a separate `.epho` file. Preserve the literal tab at each indentation level. The diagnostics chapter labels intentional failures and gives the message to expect.

Read the [release notes](../RELEASE.md) for packages and release validation, the [changelog](../CHANGELOG.md) for changes since Alpha, and the [roadmap](../ROADMAP.md) for open design questions. The [Wiki](https://github.com/ItsMeQuote/epholang/wiki) carries the guides for online reading.
