# Epholang Roadmap

[**English**](ROADMAP.md) &bull; [Russian](ROADMAP.ru.md)

This document outlines the development of Epholang, including completed milestones and possible future improvements.

Epholang is an experimental programming language. Features, priorities, and release plans may change as the language evolves.

The current release is 0.3.0 Beta. See [After Beta](#after-beta) for open work.

## Version 0.1.0 — Released

The first public release of Epholang.

### Core language

- [x] Introduce statically typed variables.
- [x] Implement functions with explicit parameter and return types.
- [x] Add conditional statements and loops.
- [x] Introduce lists with 1-based indexing.
- [x] Implement string interpolation.
- [x] Support nested block comments.

### Tools

- [x] Implement the tree-walking interpreter.
- [x] Add static checking before execution.
- [x] Introduce the command-line interface.
- [x] Provide basic VS Code syntax highlighting.

## Version 0.2.0 ("Alpha") — Released

A major update to Epholang's syntax, type system, standard library, and development tools.

### Language changes

- [x] Replace `num` with `int` (`i64`) and `flt` (`f64`).
- [x] Require explicit conversions between numeric types.
- [x] Introduce the new `function Name(...) type { ... }` declaration syntax.
- [x] Remove default parameter values.
- [x] Rename `cycle` to `loop`.
- [x] Adopt PascalCase naming for functions as a convention.

### Standard library

- [x] Introduce the `strings` module.
- [x] Introduce the `math` module.
- [x] Introduce the `types` module.
- [x] Add `Input()` for console input.
- [x] Add `Includes()` for substring and list membership checks.
- [x] Add `math:RandomInterval()` with inclusive bounds.
- [x] Implement explicit type conversions.

### Improvements

- [x] Fix known parser and interpreter issues.
- [x] Improve error messages and runtime validation.
- [x] Add checks for integer overflow and numeric precision.
- [x] Expand automated unit and integration tests.
- [x] Update VS Code syntax highlighting and snippets.
- [x] Introduce automated checks using GitHub Actions.

## Version 0.3.0 ("Beta", β) — Released November 24, 2026

Beta changed how programs express block structure, return values and type alternatives. The migration guide pairs Alpha and Beta syntax. The changelog records the scope of the release. [Migration from 0.2](MIGRATION.md) · [Changelog](CHANGELOG.md)

| Area | Shipped in Beta | Where to try it |
| --- | --- | --- |
| Source syntax | Colon-and-tab blocks, type-first declarations, `else if` and `else`, multiline logical expressions and nested single-line interpolation | [Language reference](https://github.com/ItsMeQuote/epholang/wiki/Syntax-Overview) |
| Functions and checking | Explicit result types, procedures, mandatory use or discard of expression values, and checks for unused names and recursion | [Functions and results](https://github.com/ItsMeQuote/epholang/wiki/Functions) |
| Control flow | `break` and `skip` in loops | [Branches and loops](https://github.com/ItsMeQuote/epholang/wiki/Control-Flow) |
| Types and text | Unions, lists with union element types, one-based string indexing, declared and current type inspection, and conversion of a string into a list of scalar values | [Lists and Unicode text](https://github.com/ItsMeQuote/epholang/wiki/Collections-and-Text) |
| Standard library | ASCII case predicates and inclusive ascending or descending alphabet ranges | [Standard library](https://github.com/ItsMeQuote/epholang/wiki/Standard-Library) |
| Editor and distribution | EphoFMT in the CLI and VS Code, a VSIX and checksums, plus x86-64 and ARM64 CLI archives for Windows, macOS and Linux (glibc and musl) | [Release assets](https://github.com/ItsMeQuote/epholang/releases/tag/0.3.0) · [RC run](https://github.com/ItsMeQuote/epholang/actions/runs/45407953026) |

## After Beta

These are existing areas of interest. The project has not assigned them to a release or set a date. Each question below identifies a decision or an observable outcome that would help evaluate the work.

| Area | Starting point in 0.3 | Open work |
| --- | --- | --- |
| Types and checking | Unions can describe a variable or a list element. List elements are scalar values. | Explore nested lists and user-defined types. Define what declarations, indexing, assignment and diagnostics would mean before choosing syntax. |
| Error handling | Invalid conversions and out-of-bounds reads can stop execution. Some operations offer a predicate such as `IsConvertible`. | Explore representing errors as values. Decide how a caller would inspect a failure and how this fits the explicit-result rules. |
| Modules | Programs import the built-in `strings`, `math` and `types` modules by alias. | Explore user-defined modules and imports, including name visibility, loading order and cycles. |
| Numbers and library | Numeric types are distinct and precision checks can reject integer division or conversion. | Review precision rules against concrete examples. Use demonstrated needs to select additions to existing or new modules and document their boundary cases. |
| Tools and documentation | The CLI checks, runs and formats files. The extension supplies highlighting, snippets and formatting. The release uses a cross-platform build matrix. | Improve diagnostic guidance, deepen Wiki coverage, explore editor assistance and expand runtime test coverage across platforms. |
| Native compiler research | The released CLI executes programs with a tree-walking interpreter. | Compare compilation strategies and runtime requirements. An initial prototype could compare a small program's output and errors against the interpreter. |

For the behavior supported today, use the [Beta handbook on GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki/Handbook). Future experiments may change or be dropped as the language evolves.
