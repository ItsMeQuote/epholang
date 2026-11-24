# Epholang for VS Code

This extension supports Epholang 0.3 Beta. It provides TextMate highlighting, snippets, tab-based indentation and formatting through EphoFMT.

## Install

1. Download `epholang-language-0.3.0.vsix` and the matching CLI from the [0.3.0 release](https://github.com/ItsMeQuote/epholang/releases/tag/0.3.0).
2. Choose **Extensions → Install from VSIX** or run `code --install-extension epholang-language-0.3.0.vsix`.
3. Open an `.epho` file and verify that the language mode reads **Epholang**.

## Configure EphoFMT

Put the interpreter on PATH or set `epholang.executablePath` to its absolute path. Include `.exe` on Windows. Invoke **Format Document** to exercise the configured CLI. Epholang files use tab indentation and enable format-on-save by default. Use `epholang check file.epho` from a terminal for static errors.

The grammar highlights `-> T`, `-|`, explicit discard, list types, nested block comments and interpolation. The included snippets cover typed functions, procedures, imports, conditions and loops.

For installation and troubleshooting, see [CLI and VS Code](https://github.com/ItsMeQuote/epholang/wiki/CLI-Reference).
