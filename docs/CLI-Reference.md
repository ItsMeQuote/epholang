# Command line and VS Code

## Command reference

| Command | What it does |
| --- | --- |
| `epholang tokens program.epho` | Print lexer tokens, including indentation and source positions |
| `epholang parse program.epho` | Print the parsed syntax tree |
| `epholang check program.epho` | Parse and validate types, declarations and control flow |
| `epholang run program.epho` | Check and execute the program |
| `epholang format program.epho` | Apply EphoFMT in place |
| `epholang format --check program.epho` | Return success when the file already matches EphoFMT output |

The CLI reads a UTF-8 file. Quote paths with spaces. A failure exits with status 1. Static errors can be discovered before executing a program, while failures such as division by zero or an out-of-bounds index computed at runtime depend on the inputs. `run` checks the entire program before executing its first statement.

```sh
epholang check "my programs/report.epho"
epholang format --check "my programs/report.epho"
epholang run "my programs/report.epho"
```

## EphoFMT

EphoFMT parses the file before editing it. It checks syntax and can format a syntactically valid program with unresolved names or type errors. Use `check` for those diagnostics. EphoFMT normalizes indentation, LF line endings and statement separators, and inserts trailing commas in multiline lists. It preserves strings and comments. It reports invalid syntax instead of converting old brace blocks, so migrate those first.

## VS Code

Download the version-matched VSIX and interpreter from the same [release](https://github.com/ItsMeQuote/epholang/releases/tag/0.3.0). Install with `code --install-extension epholang-language-0.3.0.vsix` or choose **Extensions → Install from VSIX**. Open an `.epho` file and confirm that the language mode is **Epholang**.

The extension supplies syntax highlighting, snippets, and document formatting via the CLI. It enables format-on-save and configures tabs for Epholang files. If the configured CLI cannot be started, set `epholang.executablePath` to the full path, including `.exe` on Windows. In the command palette, use **Format Document**. Run `epholang check` from a terminal for complete language diagnostics.

For platform downloads and checksum verification, see [installation](Installation.md).

To reproduce and repair common static and runtime errors, follow the [diagnostics guide](Diagnostics.md).
