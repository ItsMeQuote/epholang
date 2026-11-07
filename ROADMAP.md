# Epholang Roadmap

[**English**](ROADMAP.md) &bull; [Russian](ROADMAP.ru.md)

This document outlines the development of Epholang, including completed milestones and possible future improvements.

Epholang is an experimental programming language. Features, priorities, and release plans may change as the language evolves.

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

## Future Development

The following features are being considered for future versions. Their order and inclusion are not final.

### Type system

- [ ] Union types.
- [ ] Nested lists.
- [ ] User-defined types.
- [ ] Further improvements to type checking.

### Language features

- [ ] Error handling as values.
- [ ] User-defined modules and imports.
- [ ] Review numeric precision and conversion rules.

### Standard library

- [ ] Expand existing modules where useful.
- [ ] Consider additional standard modules.
- [ ] Improve consistency and documentation of built-in functions.

### Developer tools

- [ ] Expand GitHub Wiki documentation.
- [ ] Improve diagnostics and developer experience.
- [ ] Explore more advanced VS Code language support.
- [ ] Extend cross-platform testing.

### Compiler

- [ ] Research and design a native compiler.
- [ ] Explore compilation strategies and runtime requirements.
- [ ] Develop an initial compiler prototype.

## Notes

- No release dates are guaranteed.
- The roadmap is not a commitment to implement every listed feature.
- Planned features may be modified, postponed, or removed.
- Breaking changes may occur between experimental versions.
- Completed milestones describe implemented functionality, not a guarantee that every edge case is supported.

For detailed release history, see [CHANGELOG.md](CHANGELOG.md).

For language documentation, visit the [GitHub Wiki](https://github.com/ItsMeQuote/epholang/wiki).
