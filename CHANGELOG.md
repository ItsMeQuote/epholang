# Changelog

[**English**](CHANGELOG.md) &bull; [Russian](CHANGELOG.ru.md)

All notable changes to Epholang are recorded here.

## 0.3.0 ("Beta") — November 24, 2026

### Breaking changes

- Use colon-and-tab blocks and the branch keywords `else if` and `else`.
- Put types before variable and parameter names. Declare a function result using `->` and retain the explicit type on return statements.
- Consume each expression value or use `_ = expression`. A lone underscore is reserved for discard. Output now completes as a procedure.
- Replace TypeOf with types:CurrentTypeOf after importing the types module.

### Language and library

- Added procedures, union types, break and skip, multiline logical expressions, and nested one-line interpolation.
- User functions must be declared before use. Direct and indirect recursion are rejected. Declared names, including names with leading underscores, require references.
- Strings now support one-based Unicode scalar indexing. The strings module adds AlphabetRange and the ASCII case predicates.
- Added conversion from str to ls<str>, plus DeclaredTypeOf for declared types and CurrentTypeOf for actual types.

### Tools and distribution

- Added EphoFMT to the CLI and VS Code integration with indentation-aware highlighting and snippets.
- The Beta package includes Windows, macOS, glibc Linux and musl Linux builds for x86-64 and ARM64, a VSIX, and SHA256SUMS.
- See the [migration guide](MIGRATION.md) and [release notes](RELEASE.md) for examples and installation.

## 0.2.0 alpha

### Changed (breaking)

- Replaced `num` with `int` (`i64`) and `flt` (`f64`), with explicit conversions instead of implicit mixed numeric arithmetic.
- Replaced `name func := function(...) type { ... }` with `function Name(...) type { ... }`.
- Removed default function parameters.
- Renamed all built-in functions to PascalCase (`Output`, `Input`, `Length`, `Includes`, `Insert`, `Remove`, `TypeOf`). PascalCase is a convention, not a restriction on user-defined function names.
- Renamed the `cycle` keyword to **`loop`**, for both conditional loops and iteration over lists.
- List indices and `Length()` results now use `int`.

### Added

- Built-in modules `strings`, `math`, and `types`, with typed module calls using `alias:Function(...)`.
- `math:RandomInterval(minimum, maximum)` with inclusive `int` bounds.
- Explicit conversions with `types:Convert` and `types:IsConvertible`.
- `Input()` and `Includes()`.
- More CLI regression coverage and refreshed VS Code syntax highlighting and snippets.

### Notes

- The interpreter uses exact `int / int` conversion to `flt`: quotients that cannot be represented exactly raise a runtime error (e.g. `1 / 3`).
- Union types, nested lists, user-defined imports, and native compilation are not available.

## 0.1.0

First public release of Epholang.

### Added

- Variables and static type checking.
- Functions with explicitly declared parameter and return types.
- Default parameter values.
- Conditional statements (`if`, `otherwise`).
- Loops (`cycle`).
- Lists with fixed element types and 1-based indexing.
- String interpolation.
- Nested block comments.
- Built-in functions:
  - `output`
  - `length`
  - `insert`
  - `remove`
  - `typeOf`
- Command-line interface.
- Tree-walking interpreter.
- Static code checking before execution.
- VS Code syntax highlighting extension.

### Platform support

- Prebuilt Windows executable.
- Source code and build instructions using Rust.
