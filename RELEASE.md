# Epholang 0.3.0 "Beta" (β)

Released November 24, 2026. Beta makes indentation, types and function results explicit. This release adds the EphoFMT formatter and ships eight CLI packages plus a VS Code extension.

## What changed

- **Blocks use indentation.** End function, branch and loop headers with a colon. Indent each level with one tab. Branch keywords are `else if` and `else`. List literals still use braces.
- **Types precede names.** Write `int count := 3` and `function Square(int value) -> int:`. Return statements retain their explicit type, for example `return int value * value`.
- **Function results are checked.** A `-> T` function must return along every path. A `-|` procedure ends at the end of its body. `Output` is a procedure, while `-> none` returns an actual `null`. Consume every expression value or explicitly discard it with `_ = expression`.
- **Union types and type introspection.** Declare a value as `int | str` or a list as `ls<int | str>`. Import `types` for `DeclaredTypeOf` and `CurrentTypeOf`. The declared type of a variable remains the same when its current value changes.
- **Control flow and text.** Loops support `break` and `skip`. Boolean expressions can span lines using logical operators. Strings support one-based Unicode scalar indexing and nested interpolation. The strings module adds ASCII range and case helpers.
- **Formatting and editor support.** Run `epholang format program.epho` or use `format --check` in CI. The VS Code extension adds formatting through the CLI, syntax highlighting and snippets.

## Migrating from 0.2

Move declarations and parameter types before names, convert brace blocks to colon-and-tab blocks, and replace `otherwise` with `else`. Change uses of `TypeOf(value)` to `types:CurrentTypeOf(value)` after importing `types`. Review standalone calls individually. If a result is intentionally ignored, use `_ =`. Keep typed null returns where callers need a value, and change effect-only functions to `-|`.

```epholang
function Square(int value) -> int:
	return int value * value

function ShowSquare(int value) -|:
	Output("#{Square(value)}\n")

ShowSquare(7)
```

Run `check` first, then format and run the migrated source. See the [complete migration guide](https://github.com/ItsMeQuote/epholang/blob/main/MIGRATION.md), the [tutorial](https://github.com/ItsMeQuote/epholang/wiki/Hello-World) and the [language reference](https://github.com/ItsMeQuote/epholang/wiki/Syntax-Overview).

## Pick a download

All binaries are packaged as archives. Choose the architecture that matches your operating system.

| System | x86-64 | ARM64 |
| --- | --- | --- |
| Windows | `epholang-0.3.0-x86_64-pc-windows-msvc.zip` | `epholang-0.3.0-aarch64-pc-windows-msvc.zip` |
| macOS | `epholang-0.3.0-x86_64-apple-darwin.tar.gz` (Intel) | `epholang-0.3.0-aarch64-apple-darwin.tar.gz` (Apple Silicon) |
| Linux with glibc | `epholang-0.3.0-x86_64-unknown-linux-gnu.tar.gz` | `epholang-0.3.0-aarch64-unknown-linux-gnu.tar.gz` |
| Linux with static musl | `epholang-0.3.0-x86_64-unknown-linux-musl.tar.gz` | `epholang-0.3.0-aarch64-unknown-linux-musl.tar.gz` |

Use glibc for distributions such as Debian, Ubuntu and Fedora, and musl for Alpine. Compare the downloaded archive's SHA-256 digest with `SHA256SUMS`. Extract it and run `epholang check program.epho` from a shell. The Windows executable is `epholang.exe`. The [installation guide](https://github.com/ItsMeQuote/epholang/wiki/Installation) includes platform-specific commands.

Install `epholang-language-0.3.0.vsix` with **Extensions → Install from VSIX** or `code --install-extension epholang-language-0.3.0.vsix`. Configure `epholang.executablePath` if the CLI is outside PATH.

## What was verified

The [second release candidate](https://github.com/ItsMeQuote/epholang/actions/runs/45407953026) passed the release tests on all eight targets and ran the CLI from each extracted archive. Linux smoke tests covered Debian 12, Ubuntu 22.04, Fedora and Alpine. The workflow tested the extension against EphoFMT and installed the packaged VSIX. Its verified files were carried forward to the [final tag](https://github.com/ItsMeQuote/epholang/tree/0.3.0).

Beta remains a prerelease. Use the matching interpreter and extension, and include the source snippet, command, platform and diagnostic when [reporting an issue](https://github.com/ItsMeQuote/epholang/issues).
